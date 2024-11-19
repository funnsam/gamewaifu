use std::sync::{Arc, Mutex};

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Ppu {
    #[derivative(Debug = "ignore")]
    front_buffer: Arc<Mutex<[u8; 160 * 144]>>,
    #[derivative(Debug = "ignore")]
    back_buffer: [u8; 160 * 144],

    pub(crate) vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    lyc: u8,
    pub(crate) bgp: u8,
    scroll: (u8, u8),
    window: (u8, u8),
    pub(crate) lcdc: u8,
    obp: [u8; 2],
    pub(crate) stat: u8,

    scanline_dot: usize,
    pub(crate) ly: u8,
    pub(crate) mode: Mode,

    m2_objs: [(u8, u8, u8, u8); 10],
    m2_objc: usize,

    fetcher: PixelFetcher,

    stat_lines: u8,
}

#[derive(Debug, Clone, Copy, Default)]
struct FifoPixel {
    color: u8,
    bg_priority: bool,
    palette: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Mode {
    OamScan = 2,
    DrawPixel = 3,
    HBlank = 0,
    VBlank = 1,
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
struct PixelFetcher {
    lx: u8,
    discard_counter: u8,
    bg_fifo: FifoQueue<FifoPixel, 8>,
    obj_fifo: FifoQueue<FifoPixel, 8>,
    state: FetcherState,
    state_counter: usize,
    x: u8,
    can_window: bool,
    in_window: bool,
    wlx: u8,
    wly: u8,

    sprite_mode: Option<(u8, u8, u8, u8)>,
    next_sprite_mode: Option<(u8, u8, u8, u8)>,
}

#[derive(Debug, Clone, Copy)]
enum FetcherState {
    GetTile,
    GetTileDataLo(u8, u8),
    GetTileDataHi(u8, u8, u8),
    // NOTE: GetTileDataHi contains the sleep
    Push(u8, u8),
}

impl Ppu {
    pub fn new(front_buffer: Arc<Mutex<[u8; 160 * 144]>>) -> Self {
        Self {
            front_buffer,
            back_buffer: [0; 160 * 144],

            vram: [0; 0x2000],
            oam: [0; 0xa0],

            lyc: 0,
            bgp: 0x1b,
            scroll: (1, 0),
            window: (0, 0),
            lcdc: 0,
            obp: [0; 2],
            stat: 0,

            scanline_dot: 0,
            ly: 0,
            mode: Mode::OamScan,

            m2_objs: [(0, 0, 0, 0); 10],
            m2_objc: 0,

            fetcher: PixelFetcher {
                lx: 0,
                discard_counter: 0,
                bg_fifo: FifoQueue::new(),
                obj_fifo: FifoQueue::new(),
                state: FetcherState::GetTile,
                state_counter: 0,
                x: 0,
                can_window: false,
                in_window: false,
                wlx: 0,
                wly: 0,

                sprite_mode: None,
                next_sprite_mode: None,
            },

            stat_lines: 0,
        }
    }

    pub fn step(&mut self, int_mgr: &mut sm83::cpu::InterruptManager) {
        if self.is_disabled() { return; }

        let prev_stat = core::mem::take(&mut self.stat_lines) != 0;

        self.request_stat(matches!(self.mode, Mode::HBlank), 0x08);
        self.request_stat(matches!(self.mode, Mode::VBlank), 0x30);
        self.request_stat(matches!(self.mode, Mode::OamScan), 0x20);
        self.request_stat(self.ly == self.lyc, 0x40);

        match self.mode {
            Mode::OamScan => {
                if self.scanline_dot == 0 {
                    self.m2_objc = 0;

                    if self.lcdc & 2 != 0 {
                        let long = self.lcdc & 4 != 0;
                        let height = if long { 16 } else { 8 };

                        for o in 0..40 {
                            let obj = &self.oam[o * 4..o * 4 + 4];
                            let oy = obj[0];

                            if (oy..oy + height).contains(&(self.ly + 16)) {
                                self.m2_objs[self.m2_objc] = TryInto::<[u8; 4]>::try_into(obj).unwrap().into();
                                self.m2_objs[self.m2_objc].2 &= 0xfe | (!long as u8);
                                self.m2_objc += 1;
                                if self.m2_objc >= 10 { break; }
                            }
                        }

                        let objs = &mut self.m2_objs[..self.m2_objc];
                        objs.sort_by_key(|o| o.1);
                    }
                }

                if self.scanline_dot >= 79 {
                    self.mode = Mode::DrawPixel;
                }
            },
            Mode::DrawPixel => {
                if self.scanline_dot == 80 {
                    self.fetcher.reset_scanline(self.lcdc, self.ly, self.window, self.scroll);
                }

                self.fetcher.fetch(
                    self.lcdc,
                    self.ly,
                    self.window,
                    self.scroll,
                    |t| Self::get_tiledata_addr(self.lcdc, t),
                    &self.vram,
                    &self.obp,
                );

                if self.fetcher.sprite_mode.is_none() && self.fetcher.next_sprite_mode.is_none() {
                    if let Some(bg) = self.fetcher.bg_fifo.pop() {
                        let ob = self.fetcher.obj_fifo.pop();

                        if self.fetcher.discard_counter == 0 {
                            self.back_buffer[self.ly as usize * 160 + self.fetcher.lx as usize] = match ob {
                                Some(c) if c.color != 0 && (!c.bg_priority || bg.color == 0) => (c.palette >> (c.color * 2)) & 3,
                                _ if self.lcdc & 1 != 0 => (self.bgp >> (bg.color * 2)) & 3,
                                _ => self.bgp & 3,
                            } | ((ob.is_some() as u8) << 2);// | ((matches!(self.fetcher.state, FetcherState::GetTile) as u8) << 3);

                            self.fetcher.lx += 1;
                            if self.fetcher.lx >= 160 {
                                // println!("{} {}", self.scanline_dot - 80, self.m2_objc);
                                self.mode = Mode::HBlank;

                                if self.fetcher.can_window { self.fetcher.wly += 1; }
                            }

                            for o in self.m2_objs[..self.m2_objc].iter() {
                                if self.fetcher.lx + 8 == o.1 {
                                    self.fetcher.next_sprite_mode = Some(o.clone());
                                    // if !matches!(self.fetcher.state, FetcherState::GetTile) {
                                    //     self.fetcher.x -= 1;
                                    //     self.fetcher.state = FetcherState::GetTile;
                                    // }
                                    break;
                                }
                            }
                        } else {
                            self.fetcher.discard_counter -= 1;
                        }
                    }
                // } else {
                //     self.back_buffer[self.ly as usize * 160 + self.fetcher.lx as usize - 1] |= 0b1100;
                }
            },
            Mode::HBlank => {
            },
            Mode::VBlank => {
                if self.scanline_dot == 0 && self.ly == 144 {
                    let mut fb = self.front_buffer.lock().unwrap();
                    fb.copy_from_slice(&self.back_buffer);

                    int_mgr.interrupt(0);
                    self.fetcher.wly = 0;
                }
            },
        }

        self.scanline_dot = (self.scanline_dot + 1) % 456;
        if self.scanline_dot == 0 {
            self.ly = (self.ly + 1) % 154;
            self.mode = if self.ly < 144 { Mode::OamScan } else { Mode::VBlank };
        }

        if !prev_stat && self.stat_lines != 0 { int_mgr.interrupt(1); }
    }

    fn get_tiledata_addr(lcdc: u8, tile: u8) -> usize {
        if lcdc & 0x10 == 0 {
            (0x1000 + tile as i8 as isize * 16) as usize
        } else {
            tile as usize * 16
        }
    }

    fn request_stat(&mut self, cond: bool, bit: u8) {
        self.stat_lines |= (cond as u8 * bit) & self.stat;
    }

    fn is_disabled(&self) -> bool { self.lcdc & 0x80 == 0 }

    pub(crate) fn load(&self, addr: u16) -> u8 {
        match (addr, self.mode) {
            // TODO: get good timings to not glich games
            (0x8000..=0x9fff, _) => self.vram[addr as usize - 0x8000],
            (0xfe00..=0xfe9f, _) => self.oam[addr as usize - 0xfe00],
            (0xff40, _) => self.lcdc,
            (0xff41, _) => self.stat | (((self.ly == self.lyc) as u8) << 2) | self.mode as u8,
            (0xff42, _) => self.scroll.1,
            (0xff43, _) => self.scroll.0,
            (0xff44, _) => self.ly,
            (0xff45, _) => self.lyc,
            (0xff47, _) => self.bgp,
            (0xff48..=0xff49, _) => self.obp[addr as usize - 0xff48],
            (0xff4a, _) => self.window.1,
            (0xff4b, _) => self.window.0,
            _ => 0xff,
        }
    }

    pub(crate) fn store(&mut self, addr: u16, data: u8) {
        match (addr, self.mode) {
            // TODO: get good timings to not glich games
            (0x8000..=0x9fff, _) => self.vram[addr as usize - 0x8000] = data,
            (0xfe00..=0xfe9f, _) => self.oam[addr as usize - 0xfe00] = data,
            (0xff40, _) => {
                if data & 0x80 == 0 {
                    self.scanline_dot = 0;
                    self.ly = 0;
                    self.mode = Mode::HBlank;
                    self.fetcher.lx = 0;
                }
                self.lcdc = data;
            },
            (0xff41, _) => self.stat = data & 0x78,
            (0xff42, _) => self.scroll.1 = data,
            (0xff43, _) => self.scroll.0 = data,
            (0xff45, _) => self.lyc = data,
            (0xff47, _) => self.bgp = data,
            (0xff48..=0xff49, _) => self.obp[addr as usize - 0xff48] = data,
            (0xff4a, _) => self.window.1 = data,
            (0xff4b, _) => self.window.0 = data,
            _ => eprintln!("ppu write fail {addr:04x} {data:02x} {:?}", self.mode),
        }
    }
}

impl PixelFetcher {
    fn reset_scanline(&mut self, lcdc: u8, ly: u8, window: (u8, u8), scroll: (u8, u8)) {
        self.bg_fifo.clear();
        self.obj_fifo.clear();
        self.lx = 0;
        self.wlx = 0;
        self.x = 255;
        self.can_window = lcdc & 0x20 != 0 && window.0 <= 166 && window.1 <= ly;
        self.in_window = false;
        self.discard_counter = scroll.0 % 8 + 8;
        self.state = FetcherState::GetTile;
        self.sprite_mode = None;
        self.next_sprite_mode = None;
    }

    fn fetch<F: Fn(u8) -> usize>(
        &mut self,
        lcdc: u8,
        ly: u8,
        window: (u8, u8),
        scroll: (u8, u8),
        tiledata: F,
        vram: &[u8],
        obp: &[u8; 2],
    ) {
        if self.state_counter != 0 {
            self.state_counter -= 1;
            return;
        }

        // if self.can_window && !self.in_window && self.lx + 7 == window.0 {
        //     self.bg_fifo.clear();
        //     self.state = FetcherState::GetTile;
        //     self.in_window = true;
        // }

        match &mut self.state {
            FetcherState::GetTile => {
                self.state_counter = 1;
                self.sprite_mode = core::mem::take(&mut self.next_sprite_mode);

                let tilemap = if (lcdc & 8 != 0 && !self.in_window) || (lcdc & 0x40 != 0 && self.in_window) { 0x1c00 } else { 0x1800 };

                let (tile, i) = if let Some(ob) = self.sprite_mode {
                    (ob.2 + (ob.0 <= ly + 8) as u8, (ob.0 + ly) % 8)
                } else {
                    let (x, y, i) = if !self.in_window {
                        let y = scroll.1 + ly;
                        self.x += 1;
                        (scroll.0 / 8 + self.x - 1, y / 8, y % 8)
                    } else {
                        self.wlx += 1;
                        (self.wlx - 1, self.wly / 8, self.wly % 8)
                    };

                    (vram[tilemap + (x % 32) as usize + y as usize * 32], i)
                };
                self.state = FetcherState::GetTileDataLo(tile, i);
            },
            FetcherState::GetTileDataLo(tile, y) => {
                self.state_counter = 1;

                let tile = *tile;
                let y = *y;
                let tiledata = if self.sprite_mode.is_none() { tiledata(tile) } else { tile as usize * 16 } + y as usize * 2;
                let lo = vram[tiledata];
                self.state = FetcherState::GetTileDataHi(tile, y, lo);
            },
            FetcherState::GetTileDataHi(tile, y, lo) => {
                self.state_counter = 1;

                let tile = *tile;
                let y = *y;
                let lo = *lo;
                let tiledata = if self.sprite_mode.is_none() { tiledata(tile) } else { tile as usize * 16 } + y as usize * 2 + 1;
                let hi = vram[tiledata];
                self.state = FetcherState::Push(lo, hi);
            },
            FetcherState::Push(lo, hi) => {
                if self.sprite_mode.is_none() {
                    if self.bg_fifo.is_empty() {
                        for _ in 0..8 {
                            let px = FifoPixel {
                                color: ((*hi & 0x80) >> 6) | (*lo >> 7),
                                bg_priority: false,
                                palette: 0,
                            };

                            self.bg_fifo.push(px);

                            *lo <<= 1;
                            *hi <<= 1;
                        }

                        self.state = FetcherState::GetTile;
                    }
                } else {
                    let bg_priority = self.sprite_mode.unwrap().3 & 0x80 != 0;
                    let palette = obp[((self.sprite_mode.unwrap().3 >> 4) & 1) as usize];
                    let pre_head = self.obj_fifo.len();

                    for i in 0..pre_head {
                        let px = FifoPixel {
                            color: ((*hi & 0x80) >> 6) | (*lo >> 7),
                            bg_priority,
                            palette,
                        };

                        *lo <<= 1;
                        *hi <<= 1;

                        if px.color != 0 {
                            *self.obj_fifo.get_mut_after_pop_head(i).unwrap() = px;
                        }
                    }

                    for _ in pre_head..8 {
                        let px = FifoPixel {
                            color: ((*hi & 0x80) >> 6) | (*lo >> 7),
                            bg_priority,
                            palette,
                        };

                        self.obj_fifo.push(px);

                        *lo <<= 1;
                        *hi <<= 1;
                    }

                    self.state = FetcherState::GetTile;
                }
            },
        }
    }
}

#[derive(Debug)]
pub struct FifoQueue<T, const CAP: usize> {
    queue: [T; CAP],
    push_head: usize,
    pop_head: usize,
    length: usize,
}

impl<T, const CAP: usize> FifoQueue<T, CAP> {
    pub fn push(&mut self, item: T) {
        if self.len() >= CAP {
            panic!("too much elements");
        }

        self.queue[self.push_head] = item;
        self.push_head = (self.push_head + 1) % CAP;
        self.length += 1;
    }

    pub fn len(&self) -> usize { self.length }
    pub fn is_empty(&self) -> bool { self.length == 0 }

    pub fn clear(&mut self) {
        self.push_head = self.pop_head;
        self.length = 0;
    }

    pub fn get_after_pop_head(&self, rel: usize) -> Option<&T> {
        if self.len() <= rel {
            return None;
        }

        Some(&self.queue[(self.pop_head + rel) % CAP])
    }

    pub fn get_mut_after_pop_head(&mut self, rel: usize) -> Option<&mut T> {
        if self.len() <= rel {
            return None;
        }

        Some(&mut self.queue[(self.pop_head + rel) % CAP])
    }
}

impl<T: Copy + Default, const CAP: usize> FifoQueue<T, CAP> {
    pub fn new() -> Self {
        Self {
            queue: [T::default(); CAP],
            push_head: 0,
            pop_head: 0,
            length: 0,
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        (!self.is_empty()).then(|| {
            let v = core::mem::take(&mut self.queue[self.pop_head]);
            self.pop_head = (self.pop_head + 1) % CAP;
            self.length -= 1;
            v
        })
    }
}

#[test]
fn test_fifo_queue() {
    let mut q: FifoQueue<u8, 4> = FifoQueue::new();
    assert_eq!(q.len(), 0);
    q.push(1);
    assert_eq!(q.len(), 1);
    q.push(2);
    assert_eq!(q.len(), 2);
    q.push(3);
    assert_eq!(q.len(), 3);
    q.push(4);
    assert_eq!(q.len(), 4);

    assert_eq!(q.pop(), Some(1));
    assert_eq!(q.len(), 3);
    assert_eq!(q.pop(), Some(2));
    assert_eq!(q.len(), 2);
    assert_eq!(q.pop(), Some(3));
    assert_eq!(q.len(), 1);
    assert_eq!(q.pop(), Some(4));
    assert_eq!(q.len(), 0);
    assert!(q.is_empty());
    assert_eq!(q.pop(), None);

    q.push(1);
    q.push(1);
    q.push(1);
    assert_eq!(q.len(), 3);
    q.clear();
    assert!(q.is_empty());
}
