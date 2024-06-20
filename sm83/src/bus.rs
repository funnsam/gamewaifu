pub trait Bus {
    fn load_u8(&self, a: u16) -> u8;
    fn load_u16(&self, a: u16) -> u16;
    fn store_u8(&mut self, a: u16, d: u8);
    fn store_u16(&mut self, a: u16, d: u16);
}

pub struct Tester<'a> {
    rom: &'a mut [u8],
}

impl<'a> Tester<'a> {
    pub fn new(rom: &'a mut [u8]) -> Self {
        Self {
            rom,
        }
    }
}

impl Bus for Tester<'_> {
    fn load_u8(&self, a: u16) -> u8 {
        self.rom[a as usize]
    }

    fn load_u16(&self, a: u16) -> u16 {
        u16::from_le_bytes(self.rom[a as usize..a as usize + 2].try_into().unwrap())
    }

    fn store_u8(&mut self, a: u16, d: u8) {
        if a == 0 {
            println!("{d}");
        }

        self.rom[a as usize] = d;
    }

    fn store_u16(&mut self, a: u16, d: u16) {
        self.rom[a as usize] = d as u8;
        self.rom[a as usize] = (d >> 8) as u8;
    }
}
