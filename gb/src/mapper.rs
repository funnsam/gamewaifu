pub enum Mapper<'a> {
    None {
        rom: &'a [u8],
        ram: Option<&'a mut [u8]>,
    },
}

impl<'a> Mapper<'a> {
    pub fn from_bin(bin: &'a [u8]) -> Self {
        if bin.len() < 0x150 {
            panic!("bin too smol");
        }

        let mut checksum = 0;
        for a in 0x134..=0x14c {
            checksum -= bin[a] + 1;
        }

        if checksum != bin[0x14d] {
            panic!("checksum error");
        }

        let rom_banks = 2_usize << bin[0x148];
        let ram_banks = match bin[0x149] {
            0x00 => 0,
            0x02 => 1,
            0x03 => 4,
            0x05 => 8,
            0x04 => 16,
            _ => panic!("illegal ram banks"),
        };

        match bin[0x147] {
            0x00 => { // rom only
                assert_eq!(rom_banks, 2);
                assert_eq!(ram_banks, 0);
                assert_eq!(bin.len(), 0x8000);

                Mapper::None {
                    rom: bin,
                    ram: None,
                }
            },
            m => panic!("unknown mapper {m:02x}"),
        }
    }
}

impl sm83::bus::Bus for Mapper<'_> {
    fn load(&mut self, a: u16) -> u8 {
        match self {
            Self::None { rom, ram: Some(ram) } => match a {
                0x0000..=0x7fff => rom[a as usize],
                0xa000..=0xbfff => ram[a as usize - 0xa000],
                _ => 0xff,
            },
            Self::None { rom, ram: None } => match a {
                0x0000..=0x7fff => rom[a as usize],
                _ => 0xff,
            },
        }
    }

    fn store(&mut self, a: u16, d: u8) {
        match self {
            Self::None { rom: _, ram: Some(ram) } => match a {
                0xa000..=0xbfff => ram[a as usize - 0xa000] = d,
                _ => {},
            },
            Self::None { rom: _, ram: None } => {},
        }
    }
}
