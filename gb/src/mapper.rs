pub enum Mapper {
    None {
        rom: Vec<u8>,
        ram: Vec<u8>,
    },
    Mbc1 {
        rom: Vec<u8>,
        ram: Vec<u8>,

        rom_mask: usize,

        ram_en: bool,
        rom_bk: u8,
        ram_bk: u8,
        mode: bool,
        rom_ext: bool,
    },
    Mbc3 {
        rom: Vec<u8>,
        ram: Vec<u8>,

        rom_mask: usize,

        ram_en: bool,
        rom_bk: u8,
        ram_bk: u8,
    },
    Mbc5 {
        rom: Vec<u8>,
        ram: Vec<u8>,

        rom_mask: usize,

        ram_en: bool,
        rom_bk: u16,
        ram_bk: u8,
    },
}

impl Mapper {
    pub fn from_bin(bin: &[u8]) -> Self {
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
                    rom: bin.to_vec(),
                    ram: Vec::new(),
                }
            },
            // TODO: more asserts
            0x01 | 0x02 | 0x03 => { // mbc1
                Mapper::Mbc1 {
                    rom: bin.to_vec(),
                    ram: vec![0xff; ram_banks * 8192],

                    rom_mask: (rom_banks << 14) - 1,

                    ram_en: false,
                    rom_bk: 0,
                    ram_bk: 0,
                    mode: false,
                    rom_ext: false, // TODO: fat ass rom
                }
            },
            0x11 | 0x12 | 0x13 => { // mbc3
                Mapper::Mbc3 {
                    rom: bin.to_vec(),
                    ram: vec![0xff; ram_banks * 8192],

                    rom_mask: (rom_banks << 14) - 1,

                    ram_en: false,
                    rom_bk: 1,
                    ram_bk: 0,
                }
            },
            0x19 | 0x1a | 0x1b | 0x1c | 0x1d | 0x1e => { // mbc5
                Mapper::Mbc5 {
                    rom: bin.to_vec(),
                    ram: vec![0xff; ram_banks * 8192],

                    rom_mask: (rom_banks << 14) - 1,

                    ram_en: false,
                    rom_bk: 1,
                    ram_bk: 0,
                }
            },
            m => panic!("unknown mapper {m:02x}"),
        }
    }

    pub fn set_sram(&mut self, sram: &[u8]) {
        match self {
            Self::None { .. } => {},
            Self::Mbc1 { ram, .. }
                | Self::Mbc3 { ram, .. }
                | Self::Mbc5 { ram, .. }
            => ram.copy_from_slice(sram),
        }
    }

    pub fn get_sram(&self) -> Option<&[u8]> {
        match self {
            Self::None { .. } => None,
            Self::Mbc1 { ram, .. }
                | Self::Mbc3 { ram, .. }
                | Self::Mbc5 { ram, .. }
            => Some(&ram),
        }
    }

    pub(crate) fn load(&mut self, a: u16) -> u8 {
        match self {
            Self::None { rom, ram } => match a {
                0x0000..=0x7fff => rom.get(a as usize).copied().unwrap_or(0xff),
                0xa000..=0xbfff => ram.get(a as usize - 0xa000).copied().unwrap_or(0xff),
                _ => 0xff,
            },
            Self::Mbc1 { rom, ram, rom_mask, rom_bk, ram_en, ram_bk, mode, .. } => match a {
                0x0000..=0x3fff => rom.get(a as usize).copied().unwrap_or(0xff),
                0x4000..=0x7fff => rom.get(((a as usize & 0x3fff) | ((*rom_bk as usize).max(1) << 14)) & *rom_mask).copied().unwrap_or(0xff),
                0xa000..=0xbfff => if *ram_en {
                    ram.get((a as usize & 0x1fff) | (mbc1_get_ram_bank(*ram_bk as usize, *mode) << 13)).copied().unwrap_or(0xff)
                } else {
                    0xff
                },
                _ => 0xff,
            },
            Self::Mbc3 { rom, ram, rom_mask, ram_en, rom_bk, ram_bk } => match a {
                0x0000..=0x3fff => rom.get(a as usize).copied().unwrap_or(0xff),
                0x4000..=0x7fff => rom.get(((a as usize & 0x3fff) | ((*rom_bk as usize) << 14)) & *rom_mask).copied().unwrap_or(0xff),
                0xa000..=0xbfff => if *ram_en {
                    ram.get((a as usize & 0x1fff) | ((*ram_bk as usize) << 13)).copied().unwrap_or(0xff)
                } else {
                    0xff
                },
                _ => 0xff,
            },
            Self::Mbc5 { rom, ram, rom_mask, ram_en, rom_bk, ram_bk } => match a {
                0x0000..=0x3fff => rom.get(a as usize).copied().unwrap_or(0xff),
                0x4000..=0x7fff => rom.get(((a as usize & 0x3fff) | ((*rom_bk as usize) << 14)) & *rom_mask).copied().unwrap_or(0xff),
                0xa000..=0xbfff => if *ram_en {
                    ram.get((a as usize & 0x1fff) | ((*ram_bk as usize) << 13)).copied().unwrap_or(0xff)
                } else {
                    0xff
                },
                _ => 0xff,
            },
        }
    }

    pub(crate) fn store(&mut self, a: u16, d: u8) {
        match self {
            Self::None { rom: _, ram } => match a {
                0xa000..=0xbfff => ram.get_mut(a as usize - 0xa000).map(|r| *r = d).unwrap_or(()),
                _ => {},
            },
            Self::Mbc1 { ram, ram_en, rom_bk, ram_bk, mode, .. } => match a {
                0x0000..=0x1fff => *ram_en = d == 0xa,
                0x2000..=0x3fff => *rom_bk = d & 0x1f,
                0x4000..=0x5fff => *ram_bk = d & 3,
                0x6000..=0x7fff => *mode = d & 1 != 0,
                0xa000..=0xbfff => if *ram_en {
                    ram.get_mut((a as usize & 0x1fff) | (mbc1_get_ram_bank(*ram_bk as usize, *mode) << 13)).map(|r| *r = d);
                },
                _ => {},
            },
            Self::Mbc3 { ram, ram_en, rom_bk, ram_bk, .. } => match a {
                0x0000..=0x1fff => *ram_en = d == 0xa,
                0x2000..=0x23ff => *rom_bk = d.max(1),
                0x4000..=0x5fff => if d < 4 { *ram_bk = d; },
                0xa000..=0xbfff => if *ram_en {
                    ram.get_mut((a as usize & 0x1fff) | ((*ram_bk as usize) << 13)).map(|r| *r = d);
                },
                _ => {},
            },
            Self::Mbc5 { ram, ram_en, rom_bk, ram_bk, .. } => match a {
                0x0000..=0x1fff => *ram_en = d & 0xf == 0xa,
                0x2000..=0x2fff => {
                    *rom_bk &= !0xff;
                    *rom_bk |= d as u16;
                },
                0x3000..=0x3fff => {
                    *rom_bk &= !0x100;
                    *rom_bk |= (d as u16 & 1) << 8;
                },
                0x4000..=0x5fff => if d < 0x10 { *ram_bk = d; },
                0xa000..=0xbfff => if *ram_en {
                    ram.get_mut((a as usize & 0x1fff) | ((*ram_bk as usize) << 13)).map(|r| *r = d);
                },
                _ => {},
            },
        }
    }
}

fn mbc1_get_ram_bank(rbk: usize, mode: bool) -> usize {
    if mode { rbk } else { 0 }
}
