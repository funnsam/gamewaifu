pub struct Ppu {
    pub vram: [u8; 0x2000],
    pub oam: [u8; 0xa0],
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x2000],
            oam: [0; 0xa0],
        }
    }

    pub fn render_strip(&mut self, fb: &mut [u8], _y: usize) {
        for y in 0..18 {
            for x in 0..20 {
                let offset = 0x1800 + x + y * 32;
                let tile = self.vram[offset] as usize;

                for (j, r) in self.vram[(0x1000 + tile * 16)..].chunks(2).take(8).enumerate() {
                    for k in 0..8 {
                        let y = y * 8 + j;
                        let x = x * 8;
                        let kb = 7 - k;
                        fb[y * 160 + k + x] = (((r[1] >> kb) & 1) << 1) | ((r[0] >> kb) & 1);
                    }
                }
            }
        }

        // for (i, t) in self.vram[0x1000..].chunks(16).take(256).enumerate() {
        //     let x = (i & 0xf) * 8;
        //     let y = i / 16 * 8;

        //     for (j, r) in t.chunks(2).enumerate() {
        //         for k in 0..8 {
        //             let y = y + j;
        //             fb[y * 160 + k + x] = (((r[1] >> k) & 1) << 1) | ((r[0] >> k) & 1);
        //         }
        //     }
        // }
    }
}
