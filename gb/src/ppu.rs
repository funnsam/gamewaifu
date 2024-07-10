use std::sync::{atomic::*, Arc};

pub struct Ppu {
    framebuffer: Arc<[AtomicU8]>,

    vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    ly: u8,
    lyc: u8,
    bgp: u8,
    scroll: (u8, u8),
    window: (u8, u8),
    lcdc: u8,
    obp: [u8; 2],

    stat: u8,
    wly: u8,

    hsync: usize,
    stat_request: u8,
}

impl Ppu {
    pub fn new(framebuffer: Arc<[AtomicU8]>) -> Self {
        Self {
            framebuffer,

            vram: [0; 0x2000],
            oam: [0; 0xa0],

            ly: 153,
            lyc: 0,
            bgp: 0x1b,
            scroll: (0, 0),
            window: (0, 0),
            lcdc: 0,
            obp: [0; 2],

            stat: 0,
            wly: 0,

            hsync: 0,
            stat_request: 0,
        }
    }

    // returns interrupt
    pub fn step(&mut self) -> Option<u8> {
        if self.is_disabled() {
            return None;
        }

        let hsync = self.hsync;
        self.hsync = (self.hsync + 1) % 456;

        let mode = self.get_mode();
        let prev_req = self.stat_request;

        self.stat_request = 0;
        self.stat_request(mode == 0, 0x08);
        self.stat_request(mode == 1, 0x10);
        self.stat_request(mode == 2, 0x20);

        if hsync != 455 {
            return self.check_stat(prev_req);
        }

        let y = self.ly;
        self.ly = (self.ly + 1) % 154;

        self.stat_request(self.ly == self.lyc || (self.ly == 153 && self.lyc == 0 && self.hsync >= 4), 0x40);

        if y > 144 {
            return self.check_stat(prev_req);
        } else if y == 144 {
            self.wly = 0;
            return Some(self.check_stat(prev_req).unwrap_or(0) << 1);
        }

        let mut strip_bg = [0; 160];
        let mut strip_ob = [(0, 0, false); 160];
        let mut buf = [0; 2];

        if self.lcdc & 1 != 0 {
            self.render_bg(y, &mut strip_bg, &mut buf);

            if self.lcdc & 0x20 != 0 && self.window.0 <= 166 && self.window.1 <= y {
                self.render_window(&mut strip_bg, &mut buf);
                self.wly += 1;
            }
        }

        if self.lcdc & 2 != 0 {
            let long = self.lcdc & 4 != 0;
            let height = if long { 16 } else { 8 };
            let t_mask = !(long as u8);

            let mut objs = [(0, 0, 0, 0); 10];
            let mut objc = 0;

            for o in 0..40 {
                let obj = &self.oam[o * 4..o * 4 + 4];
                let oy = obj[0] - 16;

                if (oy..oy + height).contains(&y) {
                    objs[objc] = TryInto::<[u8; 4]>::try_into(obj).unwrap().try_into().unwrap();
                    objc += 1;
                    if objc >= 10 { break; }
                }
            }

            let objs = &mut objs[..objc];
            objs.sort_by_key(|o| o.1);

            for o in objs.iter() {
                let x = o.1 - 8;
                let iy = if o.3 & 0x40 != 0 { height - y + o.0 - 17 } else { y - o.0 + 16 };

                let p = self.obp[(o.3 >> 4) as usize & 1];

                let (_, r) = self.vram.split_at((o.2 & t_mask) as usize * 16 + iy as usize * 2);
                let (r, _) = r.split_at(2);
                buf.copy_from_slice(r);

                for k in 0..8 {
                    let kb = if o.3 & 0x20 != 0 { k } else { 7 - k };
                    let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);

                    if c != 0 {
                        let x = (x + k) as usize;

                        if x < 160 && strip_ob[x].0 == 0 {
                            strip_ob[x] = (c, p, o.3 & 0x80 != 0);
                        };
                    }
                }
            }
        }

        for (x, (b, (o, p, pr))) in strip_bg.into_iter().zip(strip_ob).enumerate() {
            self.framebuffer[y as usize * 160 + x].store(
                if o == 0 || (pr && b != 0) {
                    (self.bgp >> (b * 2)) & 3
                } else {
                    (p >> (o * 2)) & 3
                },
                Ordering::Relaxed,
            );
        }

        self.check_stat(prev_req)
    }

    fn tiledata_base(&self, tile: u8, iy: usize) -> usize {
        if self.lcdc & 0x10 == 0 {
            (0x1000 + tile as i8 as isize * 16) as usize + iy * 2
        } else {
            tile as usize * 16 + iy * 2
        }
    }

    fn render_bg(&self, y: u8, strip_bg: &mut [u8; 160], buf: &mut [u8; 2]) {
        let ty = (y + self.scroll.1) as usize / 8;
        let iy = (y + self.scroll.1) as usize % 8;

        let tilemap_base = if self.lcdc & 8 == 0 { 0x1800 } else { 0x1c00 } + ty * 32;

        'a: for sx in 0..21 {
            let tx = (sx + self.scroll.0 / 8) as usize % 32;
            let xo = (self.scroll.0 % 8) as u8;

            let offset = tilemap_base + tx;
            let tile = self.vram[offset];

            let (_, r) = self.vram.split_at(self.tiledata_base(tile, iy));
            let (r, _) = r.split_at(2);
            buf.copy_from_slice(r);

            for k in 0..8 {
                let x = sx * 8;
                let kb = 7 - k;
                let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);

                let x = x as isize + k as isize - xo as isize;
                if 0 <= x && x < 160 {
                    strip_bg[x as usize] = c;
                } else if 0 <= x {
                    break 'a;
                }
            }
        }
    }

    fn render_window(&self, strip_bg: &mut [u8; 160], buf: &mut [u8; 2]) {
        let ty = self.wly as usize / 8;
        let iy = self.wly as usize % 8;

        let tilemap_base = if self.lcdc & 0x40 == 0 { 0x1800 } else { 0x1c00 } + ty * 32;

        'a: for tx in 0..21 {
            let offset = tilemap_base + tx;
            let tile = self.vram[offset];

            let (_, r) = self.vram.split_at(self.tiledata_base(tile, iy));
            let (r, _) = r.split_at(2);
            buf.copy_from_slice(r);

            for k in 0..8 {
                let x = tx * 8 + self.window.0 as usize - 7;
                let kb = 7 - k;
                let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);

                let x = x as isize + k as isize;
                if 0 <= x && x < 160 {
                    strip_bg[x as usize] = c;
                } else if 0 <= x {
                    break 'a;
                }
            }
        }
    }

    #[inline]
    fn stat_request(&mut self, cond: bool, bit: u8) {
        self.stat_request |= cond as u8 * bit;
    }

    fn check_stat(&self, prev: u8) -> Option<u8> {
        (!prev & self.stat_request & self.stat & 0x78 != 0).then_some(1)
    }

    fn get_mode(&self) -> usize {
        match (self.hsync, self.ly) {
            (0..80, ..144) => 2,
            (80..252, ..144) => 3,
            (252.., ..144) => 0,
            _ => 1,
        }
    }

    fn get_stat(&self) -> u8 {
        self.stat | (((self.ly == self.lyc) as u8) << 2) | self.get_mode() as u8
    }

    fn set_stat(&mut self, v: u8) {
        self.stat = v & 0x78;
    }

    fn is_disabled(&self) -> bool { self.lcdc & 0x80 == 0 }

    pub(crate) fn load(&self, addr: u16) -> u8 {
        match (addr, self.get_mode()) {
            (0x8000..=0x9fff, 0..=2) => self.vram[addr as usize - 0x8000],
            (0xfe00..=0xfe9f, 0..=1) => self.oam[addr as usize - 0xfe00],
            (0xff40, _) => self.lcdc,
            (0xff41, _) => self.get_stat(),
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
        match (addr, self.get_mode()) {
            // TODO: get good timings to not glich games
            (0x8000..=0x9fff, _) => self.vram[addr as usize - 0x8000] = data,
            (0xfe00..=0xfe9f, _) => self.oam[addr as usize - 0xfe00] = data,
            (0xff40, _) => {
                if data & 0x80 == 0 {
                    self.hsync = 455;
                    self.ly = 0;
                    self.framebuffer.iter().for_each(|i| { i.fetch_or(4, Ordering::Relaxed); });
                } else {
                    self.framebuffer.iter().for_each(|i| { i.fetch_and(!4, Ordering::Relaxed); });
                }
                self.lcdc = data;
            },
            (0xff41, _) => self.set_stat(data),
            (0xff42, _) => self.scroll.1 = data,
            (0xff43, _) => self.scroll.0 = data,
            (0xff45, _) => self.lyc = data,
            (0xff47, _) => self.bgp = data,
            (0xff48..=0xff49, _) => self.obp[addr as usize - 0xff48] = data,
            (0xff4a, _) => self.window.1 = data,
            (0xff4b, _) => self.window.0 = data,
            _ => eprintln!("ppu write fail {addr:04x} {data:02x} {}", self.get_mode()),
        }
    }
}
