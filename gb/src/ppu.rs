use std::sync::{Arc, Mutex};

use crate::Model;

pub struct Ppu {
    front_buffer: Arc<Mutex<[u8; 160 * 144]>>,
    back_buffer: [u8; 160 * 144],

    vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    vram_bank: u8,

    ly: u8,
    lyc: u8,
    scroll: (u8, u8),
    window: (u8, u8),

    bgp: u8,
    obp: [u8; 2],

    cgb_bg_palette: [u16; 32],
    cgb_obj_palette: [u16; 32],
    bcps: u8,
    ocps: u8,

    lcdc: u8,
    stat: u8,

    wly: u8,
    hsync: usize,
    stat_request: u8,

    mode_3_penalty: usize,

    model: Model,
}

impl Ppu {
    pub fn new(front_buffer: Arc<Mutex<[u8; 160 * 144]>>, model: Model) -> Self {
        Self {
            front_buffer,
            back_buffer: [0; 160 * 144],

            vram: [0; 0x2000],
            oam: [0; 0xa0],

            vram_bank: 0,

            ly: 153,
            lyc: 0,
            scroll: (0, 0),
            window: (0, 0),

            bgp: 0x1b,
            obp: [0; 2],

            cgb_bg_palette: [0; 32],
            cgb_obj_palette: [0; 32],
            bcps: 0,
            ocps: 0,

            lcdc: 0,
            stat: 0,

            wly: 0,
            hsync: 0,
            stat_request: 0,

            mode_3_penalty: 0,

            model,
        }
    }

    // returns interrupt
    pub fn step(&mut self, int_mgr: &mut sm83::cpu::InterruptManager) {
        if self.is_disabled() { return; }

        let mode = self.get_mode();
        let prev_req = core::mem::take(&mut self.stat_request) != 0;

        let hsync = self.hsync;
        self.hsync = (self.hsync + 1) % 456;

        let y = self.ly;
        if hsync == 455 {
            self.mode_3_penalty = 0;
            self.ly = (self.ly + 1) % 154;
        }

        self.stat_request(mode == 0, 0x08);
        self.stat_request(mode == 1, 0x10);
        self.stat_request(mode == 2, 0x20);
        self.stat_request(self.ly == self.lyc, 0x40);

        if hsync == 455 && y == 144 {
            self.wly = 0;
            self.check_stat(prev_req, int_mgr);
            int_mgr.interrupt(0);
            return;
        } else if hsync != 80 || y >= 144 {
            self.check_stat(prev_req, int_mgr);
            return;
        }

        let mut strip_bg = [0; 160];
        let mut strip_ob = [(0, 0, false); 160];
        let mut buf = [0; 2];

        self.mode_3_penalty = self.scroll.0 as usize % 8;

        if self.lcdc & 1 != 0 {
            self.render_bg(y, &mut strip_bg, &mut buf);

            if self.lcdc & 0x20 != 0 && self.window.0 <= 166 && self.window.1 <= y {
                self.mode_3_penalty += self.render_window(&mut strip_bg, &mut buf) as usize * 6;
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
                let oy = obj[0] as isize - 16;

                if (oy..oy + height as isize).contains(&(y as isize)) {
                    objs[objc] = TryInto::<[u8; 4]>::try_into(obj).unwrap().into();
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
                buf.copy_from_slice(&r[..2]);

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

                self.mode_3_penalty += 6; // TODO: more accurate penalty algo
            }
        }

        for (x, (b, (o, p, pr))) in strip_bg.into_iter().zip(strip_ob).enumerate() {
            self.back_buffer[y as usize * 160 + x] = if o == 0 || (pr && b != 0) {
                (self.bgp >> (b * 2)) & 3
            } else {
                (p >> (o * 2)) & 3
            };
        }

        if y == 143 {
            let mut fb = self.front_buffer.lock().unwrap();
            fb.copy_from_slice(&self.back_buffer);
        }

        self.check_stat(prev_req, int_mgr)
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
            let xo = self.scroll.0 % 8;

            let offset = tilemap_base + tx;
            let tile = self.vram[offset];

            let (_, r) = self.vram.split_at(self.tiledata_base(tile, iy));
            buf.copy_from_slice(&r[..2]);

            for k in 0..8 {
                let x = sx * 8;
                let kb = 7 - k;
                let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);

                let x = x as isize + k as isize - xo as isize;
                if (0..160).contains(&x) {
                    strip_bg[x as usize] = c;
                } else if 0 <= x {
                    break 'a;
                }
            }
        }
    }

    fn render_window(&self, strip_bg: &mut [u8; 160], buf: &mut [u8; 2]) -> bool {
        let ty = self.wly as usize / 8;
        let iy = self.wly as usize % 8;

        let tilemap_base = if self.lcdc & 0x40 == 0 { 0x1800 } else { 0x1c00 } + ty * 32;
        let mut any = false;

        'a: for tx in 0..21 {
            let offset = tilemap_base + tx;
            let tile = self.vram[offset];

            let (_, r) = self.vram.split_at(self.tiledata_base(tile, iy));
            buf.copy_from_slice(&r[..2]);

            for k in 0..8 {
                let x = tx * 8 + self.window.0 as usize - 7;
                let kb = 7 - k;
                let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);

                let x = x as isize + k as isize;
                if (0..160).contains(&x) {
                    strip_bg[x as usize] = c;
                    any = true;
                } else if 0 <= x {
                    break 'a;
                }
            }
        }

        any
    }

    #[inline]
    fn stat_request(&mut self, cond: bool, bit: u8) {
        self.stat_request |= cond as u8 * bit;
        self.stat_request &= self.stat & 0x78;
    }

    fn check_stat(&self, prev: bool, int_mgr: &mut sm83::cpu::InterruptManager) {
        if !prev && self.stat_request != 0 { int_mgr.interrupt(1); }
    }

    fn get_mode(&self) -> usize {
        match (self.hsync, self.ly) {
            (0..80, ..144) => 2,
            (80.., ..144) if (..172 + self.mode_3_penalty).contains(&(self.hsync - 80)) => 3,
            (_, ..144) => 0,
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

    fn vram_offset(&self) -> usize {
        self.vram_bank as usize * 0x2000
    }

    pub(crate) fn load(&self, addr: u16, dmg: bool) -> u8 {
        match (addr, self.get_mode()) {
            // TODO: get good timings to not glich games
            (0x8000..=0x9fff, _) => self.vram[addr as usize - 0x8000],
            (0xfe00..=0xfe9f, _) => self.oam[addr as usize - 0xfe00],
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

            (0xff4f, _) if !dmg => self.vram_bank | 0xfe,

            (0xff68, _) if !dmg => self.bcps,
            (0xff6a, _) if !dmg => self.ocps,
            _ => 0xff,
        }
    }

    pub(crate) fn store(&mut self, addr: u16, data: u8, dmg: bool) {
        match (addr, self.get_mode()) {
            // TODO: get good timings to not glich games
            (0x8000..=0x9fff, _) => self.vram[addr as usize - 0x8000] = data,
            (0xfe00..=0xfe9f, _) => self.oam[addr as usize - 0xfe00] = data,
            (0xff40, _) => {
                if data & 0x80 == 0 {
                    self.hsync = 0;
                    self.ly = 0;
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

            (0xff4f, _) if !dmg => self.vram_bank = data & 1,

            (0xff68, _) if !dmg => self.bcps = data & 0xbf,
            (0xff69, _) if !dmg => {
                let idx = ((self.bcps & 0x3f) >> 1) as usize;
                if self.bcps & 1 == 0 { // lo
                    self.cgb_bg_palette[idx] &= 0xff00;
                    self.cgb_obj_palette[idx] |= data as u16;
                } else { // hi
                    self.cgb_bg_palette[idx] &= 0x00ff;
                    self.cgb_bg_palette[idx] |= (data as u16) << 8;
                }

                if self.bcps & 0x80 != 0 {
                    self.bcps = (self.bcps + 1) & 0xbf;
                }
            },
            (0xff6a, _) if !dmg => self.ocps = data & 0xbf,
            (0xff6b, _) if !dmg => {
                let idx = ((self.ocps & 0x3f) >> 1) as usize;
                if self.ocps & 1 == 0 { // lo
                    self.cgb_obj_palette[idx] &= 0xff00;
                    self.cgb_obj_palette[idx] |= data as u16;
                } else { // hi
                    self.cgb_obj_palette[idx] &= 0x00ff;
                    self.cgb_obj_palette[idx] |= (data as u16) << 8;
                }

                if self.ocps & 0x80 != 0 {
                    self.ocps = (self.ocps + 1) & 0xbf;
                }
            },
            _ => eprintln!("ppu write fail {addr:04x} {data:02x} {}", self.get_mode()),
        }
    }
}
