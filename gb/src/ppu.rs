use std::{collections::VecDeque, sync::{Arc, Mutex}};

pub struct Ppu {
    front_buffer: Arc<Mutex<[u8; 160 * 144]>>,
    back_buffer: [u8; 160 * 144],

    vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    lyc: u8,
    bgp: u8,
    scroll: (u8, u8),
    window: (u8, u8),
    lcdc: u8,
    obp: [u8; 2],
    stat: u8,

    scanline_dot: usize,
    ly: u8,
    mode: Mode,

    m2_objs: [(u8, u8, u8, u8); 10],
    m2_objc: usize,

    m3_lx: u8,
    m3_counter: usize,
    m3_bg_fifo: VecDeque<FifoPixel>,
    m3_obj_fifo: VecDeque<FifoPixel>,
    m3_fetcher_state: FetcherState,
    m3_fetcher_counter: usize,
    wlx: u8,
    wly: u8,

    stat_lines: u8,
}

struct FifoPixel {
    color: u8,
    bg_priority: bool,
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
            scroll: (0, 0),
            window: (0, 0),
            lcdc: 0,
            obp: [0; 2],
            stat: 0,

            scanline_dot: 0,
            ly: 0,
            mode: Mode::OamScan,

            m2_objs: [(0, 0, 0, 0); 10],
            m2_objc: 0,

            m3_lx: 0,
            m3_counter: 0,
            m3_bg_fifo: VecDeque::with_capacity(8),
            m3_obj_fifo: VecDeque::with_capacity(8),
            m3_fetcher_state: FetcherState::GetTile,
            m3_fetcher_counter: 0,
            wlx: 0,
            wly: 0,

            stat_lines: 0,
        }
    }

    pub fn step(&mut self, int_mgr: &mut sm83::cpu::InterruptManager) {
        if self.is_disabled() { return; }

        let prev_stat = core::mem::take(&mut self.stat_lines) != 0;

        self.request_stat(matches!(self.mode, Mode::HBlank), 0x08);
        self.request_stat(matches!(self.mode, Mode::VBlank), 0x10);
        self.request_stat(matches!(self.mode, Mode::OamScan), 0x20);
        self.request_stat(self.ly == self.lyc, 0x40);

        match self.mode {
            Mode::OamScan => {
                if self.scanline_dot == 0 {
                    let long = self.lcdc & 4 != 0;
                    let height = if long { 16 } else { 8 };

                    for o in 0..40 {
                        let obj = &self.oam[o * 4..o * 4 + 4];
                        let oy = obj[0] as isize - 16;

                        if (oy..oy + height as isize).contains(&(self.ly as isize)) {
                            self.m2_objs[self.m2_objc] = TryInto::<[u8; 4]>::try_into(obj).unwrap().into();
                            self.m2_objc += 1;
                            if self.m2_objc >= 10 { break; }
                        }
                    }

                    let objs = &mut self.m2_objs[..self.m2_objc];
                    objs.sort_by_key(|o| o.1);
                }

                if self.scanline_dot >= 80 {
                    self.mode = Mode::DrawPixel;
                    self.m3_lx = 0;
                }
            },
            Mode::DrawPixel => {
                self.m3_fetch_bg();

                if !self.m3_bg_fifo.is_empty() {
                    self.back_buffer[self.ly as usize * 160 + self.m3_lx as usize] = self.m3_bg_fifo.pop_front().unwrap().color;

                    self.m3_lx += 1;
                    if self.m3_lx >= 160 { self.mode = Mode::HBlank; }
                }
            },
            Mode::HBlank => {},
            Mode::VBlank => {
                if self.scanline_dot == 0 && self.ly == 144 {
                    let mut fb = self.front_buffer.lock().unwrap();
                    fb.copy_from_slice(&self.back_buffer);

                    int_mgr.interrupt(0);
                }
            },
        }

        self.scanline_dot = (self.scanline_dot + 1) % 456;
        if self.scanline_dot == 0 {
            self.ly += 1;
            self.mode = if self.ly < 144 { Mode::OamScan } else { Mode::VBlank };
        }

        if !prev_stat && self.stat_lines != 0 { int_mgr.interrupt(1); }
    }

    fn m3_fetch_bg(&mut self) {
        if self.m3_fetcher_counter != 0 {
            self.m3_fetcher_counter -= 1;
            return;
        }

        match &mut self.m3_fetcher_state {
            FetcherState::GetTile => {
                self.m3_fetcher_counter = 1;

                let is_window = false;//self.m3_lx >= self.window.0;
                let tilemap = if (self.lcdc & 8 != 0 && !is_window) || (self.lcdc & 0x40 != 0 && is_window) { 0x1c00 } else { 0x1800 };
                let (x, y, i) = if !is_window {
                    ((self.scroll.0 + self.m3_lx) / 8, (self.scroll.1 + self.ly) / 8, (self.scroll.1 + self.ly) % 8)
                } else {
                    (self.wlx / 8, self.wly / 8, self.wly % 8)
                };

                let tile = self.vram[tilemap + x as usize + y as usize * 32];
                self.m3_fetcher_state = FetcherState::GetTileDataLo(tile, i);
            },
            FetcherState::GetTileDataLo(tile, y) => {
                self.m3_fetcher_counter = 1;

                let tile = *tile;
                let y = *y;
                let tiledata = self.get_tiledata_addr(tile) + y as usize * 2;
                let lo = self.vram[tiledata];
                self.m3_fetcher_state = FetcherState::GetTileDataHi(tile, y, lo);
            },
            FetcherState::GetTileDataHi(tile, y, lo) => {
                self.m3_fetcher_counter = 1;

                let tile = *tile;
                let y = *y;
                let lo = *lo;
                let tiledata = self.get_tiledata_addr(tile) + y as usize * 2 + 1;
                let hi = self.vram[tiledata];
                self.m3_fetcher_state = FetcherState::Push(lo, hi, 8);
            },
            FetcherState::Push(lo, hi, count) => {
                if self.m3_bg_fifo.is_empty() {
                    let px = FifoPixel {
                        color: ((*hi & 0x80) >> 6) | (*lo >> 7),
                        bg_priority: false,
                    };
                    self.m3_bg_fifo.push_back(px);

                    *lo <<= 1;
                    *hi <<= 1;

                    *count -= 1;
                    if *count == 0 { self.m3_fetcher_state = FetcherState::Sleep; }
                }
            },
            FetcherState::Sleep => {
                self.m3_fetcher_counter = 1;
                self.m3_fetcher_state = FetcherState::GetTile;
            },
        }
    }

    fn get_tiledata_addr(&self, tile: u8) -> usize {
        if self.lcdc & 0x10 == 0 {
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
                    self.m3_lx = 0;
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

#[derive(Debug, Clone, Copy)]
enum Mode {
    OamScan = 2,
    DrawPixel = 3,
    HBlank = 0,
    VBlank = 1,
}

#[derive(Debug, Clone, Copy)]
enum FetcherState {
    GetTile,
    GetTileDataLo(u8, u8),
    GetTileDataHi(u8, u8, u8),
    Push(u8, u8, usize),
    Sleep,
}
