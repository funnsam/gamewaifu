pub struct Bus {
    pub ppu: ppu::Ppu,
    pub mapper: super::mapper::Mapper,
    pub wram: [u8; 0x2000],
    pub hram: [u8; 0x7f],
}

impl sm83::bus::Bus for Bus {
    fn load(&mut self, a: u16) -> u8 {
        match a {
            0x0000..=0x7fff => self.mapper.load(a),
            0x8000..=0x9fff => self.ppu.vram[a as usize - 0x8000],
            0xa000..=0xbfff => self.mapper.load(a),
            0xc000..=0xdfff => self.wram[a as usize - 0xc000],
            0xe000..=0xfdff => self.wram[a as usize - 0xe000],
            0xfe00..=0xfe9f => self.ppu.oam[a as usize - 0xfe00],
            0xfea0..=0xfeff => unimplemented!(),
            0xff00..=0xff7f => { println!("io {a:04x}"); 144 },
            0xff80..=0xfffe => self.hram[a as usize - 0xff80],
            0xffff => unreachable!(),
        }
    }

    fn store(&mut self, a: u16, d: u8) {
        match a {
            0x0000..=0x7fff => self.mapper.store(a, d),
            0x8000..=0x9fff => self.ppu.vram[a as usize - 0x8000] = d,
            0xa000..=0xbfff => self.mapper.store(a, d),
            0xc000..=0xdfff => self.wram[a as usize - 0xc000] = d,
            0xe000..=0xfdff => self.wram[a as usize - 0xe000] = d,
            0xfe00..=0xfe9f => self.ppu.oam[a as usize - 0xfe00] = d,
            0xfea0..=0xfeff => unimplemented!(),
            0xff00..=0xff7f => println!("io {a:04x}"),
            0xff80..=0xfffe => self.hram[a as usize - 0xff80] = d,
            0xffff => unreachable!(),
        }
    }
}
