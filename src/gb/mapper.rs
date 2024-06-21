pub struct Mapper {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
}

impl sm83::bus::Bus for Mapper {
    fn load(&mut self, a: u16) -> u8 {
        match a {
            0x0000..=0x7fff => self.rom[a as usize],
            0xa000..=0xbfff => self.ram[a as usize - 0xa000],
            _ => unimplemented!(),
        }
    }

    fn store(&mut self, a: u16, d: u8) {
        match a {
            0x0000..=0x7fff => {},
            0xa000..=0xbfff => self.ram[a as usize - 0xa000] = d,
            _ => unimplemented!(),
        }
    }
}
