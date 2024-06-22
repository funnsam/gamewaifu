use std::sync::atomic::*;

pub struct Ppu {
    pub(crate) vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    pub(crate) ly: u8,
    pub(crate) lyc: u8,
    pub(crate) bgp: u8,
    pub(crate) scroll: (u8, u8),
    pub(crate) window: (u8, u8),
    pub(crate) lcdc: u8,
    pub(crate) obp: [u8; 2],

    stat: u8,
    wly: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xa0],

            ly: 0,
            lyc: 0,
            bgp: 0x1b,
            scroll: (0, 0),
            window: (0, 0),
            lcdc: 0x80,
            obp: [0; 2],

            stat: 0,
            wly: 0,
        }
    }

    // TODO: emulate correct behavior

    // returns interrupt
    pub fn render_strip(&mut self, fb: &[AtomicU8]) -> Option<u8> {
        if self.lcdc & 0x80 == 0 {
            return None;
        }

        let y = self.ly;
        self.ly = (self.ly + 1) % 153;
        if y >= 144 {
            self.wly = 0;
            return Some(0);
        } else if y > 144 {
            return None;
        }

        let mut strip_bg = [0; 160];
        let mut strip_ob = [(0, false); 160];
        let mut buf = [0; 2];

        let mut plot_bg = |x: usize, c: u8| if x < 144 {
            strip_bg[x] = (self.bgp >> (c * 2)) & 3;
        };

        let mut plot_ob = |x: usize, c: u8, pr: bool, p: u8| if x < 144 && strip_ob[x].0 == 0 {
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
                //     (p >> (o * 2)) & 3
                // } else {
                //     (self.bgp >> (b * 2)) & 3
                // },
                Ordering::Relaxed,
            );
        }

        (self.ly == self.lyc && self.stat & 0x40 != 0).then_some(1)
    }

    pub(crate) fn get_stat(&self) -> u8 {
        self.stat | (((self.ly == self.lyc) as u8) << 2)
    }

    pub(crate) fn set_stat(&mut self, v: u8) {
        self.stat = v & 0x78;
    }
}
