use std::sync::atomic::*;

pub struct Ppu {
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
}

impl Ppu {
    pub fn new() -> Self {
        Self {
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
        }
    }

    // returns interrupt
    pub fn step(&mut self, fb: &[AtomicU8]) -> Option<u8> {
        if self.is_disabled() {
            fb[0].store(4, Ordering::Relaxed);
            return None;
        }

        let hsync = self.hsync;
        self.hsync = (self.hsync + 1) % 456;

        match (hsync, self.ly) {
            (0, _) => {},
            (1, ..144) => return self.gen_stat(true, 0x20),
            (252, ..144) => return self.gen_stat(true, 0x08),
            (1, 144) => return self.gen_stat(true, 0x10),
            _ => return None,
        }

        let y = self.ly;
        self.ly = (self.ly + 1) % 154;
        if y == 144 {
            self.wly = 0;
            return Some(0);
        } else if y >= 144 {
            return None;
        }

        let mut strip_bg = [0; 160];
        let mut strip_ob = [(0, false); 160];
        let mut buf = [0; 2];

        let mut plot_bg = |x: usize, c: u8| if x < 160 {
            strip_bg[x] = (self.bgp >> (c * 2)) & 3;
        };

        let mut plot_ob = |x: usize, c: u8, pr: bool, p: u8| if x < 160 && strip_ob[x].0 == 0 {
            strip_ob[x] = ((p >> (c * 2)) & 3, pr);
        };

        if self.lcdc & 1 != 0 { // bg & window enable
            let ty = (y + self.scroll.1) as usize / 8;
            let iy = (y + self.scroll.1) as usize % 8;

            let bg_tilemap_base = if self.lcdc & 8 == 0 { 0x1800 } else { 0x1c00 };
            let tiledata_base = |lcdc: u8, tile: u8, iy: usize| {
                if lcdc & 0x10 == 0 {
                    (0x1000 + tile as i8 as isize * 16) as usize + iy * 2
                } else {
                    tile as usize * 16 + iy * 2
                }
            };

            for sx in 0..21 {
                let tx = (sx + self.scroll.0 / 8) as usize % 32;
                let xo = (self.scroll.0 % 8) as u8;

                let offset = bg_tilemap_base + tx + ty * 32;
                let tile = self.vram[offset];

                let (_, r) = self.vram.split_at(tiledata_base(self.lcdc, tile, iy));
                let (r, _) = r.split_at(2);
                buf.copy_from_slice(r);

                for k in 0..8 {
                    let x = sx * 8;
                    let kb = 7 - k;
                    let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);
                    plot_bg((x + k - xo) as _, c);
                }
            }

            if self.lcdc & 0x20 != 0 && self.window.0 <= 166 && self.window.1 <= 143 && self.window.1 <= y { // window enable
                let wd_tilemap_base = if self.lcdc & 0x40 == 0 { 0x1800 } else { 0x1c00 };

                let ty = self.wly as usize / 8;
                let iy = self.wly as usize % 8;

                for tx in 0..21 {
                    let offset = wd_tilemap_base + tx + ty * 32;
                    let tile = self.vram[offset];

                    let (_, r) = self.vram.split_at(tiledata_base(self.lcdc, tile, iy));
                    let (r, _) = r.split_at(2);
                    buf.copy_from_slice(r);

                    for k in 0..8 {
                        let x = tx * 8 + self.window.0 as usize - 7;
                        let kb = 7 - k;
                        let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);
                        plot_bg((x + k) as _, c);
                    }
                }

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
                        plot_ob((x + k) as _, c, o.3 & 0x80 != 0, p);
                    }
                }
            }
        }

        for (x, (b, (o, pr))) in strip_bg.into_iter().zip(strip_ob).enumerate() {
            fb[y as usize * 160 + x].store(
                if o == 0 || (pr && b != 0) { b } else { o },
                Ordering::Relaxed,
            );
        }

        self.gen_stat(self.ly == self.lyc, 0x40)
    }

    fn gen_stat(&self, cond: bool, bit: u8) -> Option<u8> {
        if cond { println!("{bit:02x}"); }
        (cond && self.stat & bit != 0).then_some(1)
    }

    fn get_mode(&self) -> usize {
        if self.is_disabled() { return 0; }

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
            (0x8000..=0x9fff, 0..=2) => self.vram[addr as usize - 0x8000] = data,
            (0xfe00..=0xfe9f, 0..=1) => self.oam[addr as usize - 0xfe00] = data,
            (0xff40, _) => {
                if data & 0x80 == 0 {
                    self.hsync = 0;
                    self.ly = 0;
                }
                println!("yuh {data:02x}");
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
            _ => println!("nuh uh {addr:04x} {data:02x} {}", self.get_mode()),
        }
    }
}
