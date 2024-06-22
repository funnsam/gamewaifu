use std::sync::atomic::*;

pub struct Ppu {
    pub(crate) vram: [u8; 0x2000],
    pub(crate) oam: [u8; 0xa0],

    pub(crate) ly: u8,
    pub(crate) lyc: u8,
    pub(crate) bgp: u8,
    pub(crate) scroll: (u8, u8),
    pub(crate) lcdc: u8,

    stat: u8,
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
            lcdc: 0x80,

            stat: 0,
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
            return Some(0);
        } else if y > 144 {
            return None;
        }

        if self.lcdc & 1 != 0 {
            let mut buf = [0; 2];

            let ty = (y + self.scroll.1) as usize / 8;
            let iy = (y + self.scroll.1) as usize % 8;

            for tx in 0..20 {
                // TODO: alternative base
                let offset = 0x1800 + tx + ty * 32;
                let tile = self.vram[offset] as usize;

                // TODO: alternative base
                let (_, r) = self.vram.split_at(0x1000 + tile * 16 + iy * 2);
                let (r, _) = r.split_at(2);
                buf.copy_from_slice(r);

                for k in 0..8 {
                    let x = tx * 8;
                    let kb = 7 - k;
                    let c = (((buf[1] >> kb) & 1) << 1) | ((buf[0] >> kb) & 1);
                    self.plot(fb, x + k, y as _, c, self.bgp);
                }
            }
        } else {
            for x in 0..160 {
                self.plot(fb, x, y as _, 0, self.bgp);
            }
        }

        // TODO: objects and windows

        (self.ly == self.lyc && self.stat & 0x40 != 0).then_some(1)
    }

    fn plot(&mut self, fb: &[AtomicU8], x: usize, y: usize, c: u8, p: u8) {
        fb[y * 160 + x].store((p >> (c * 2)) & 3, Ordering::Relaxed);
    }

    pub(crate) fn get_stat(&self) -> u8 {
        self.stat | (((self.ly == self.lyc) as u8) << 2)
    }

    pub(crate) fn set_stat(&mut self, v: u8) {
        self.stat = v & 0x78;
    }
}
