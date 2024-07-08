pub(crate) struct Bus {
    pub(crate) ppu: crate::ppu::Ppu,
    pub(crate) apu: crate::apu::Apu,

    pub(crate) mapper: crate::mapper::Mapper,
    pub(crate) wram: [u8; 0x2000],
    pub(crate) hram: [u8; 0x7f],

    pub(crate) oam_dma_at: (u8, u8),

    pub(crate) tima: u8,
    pub(crate) tma: u8,
    pub(crate) tac: u8,

    pub keys: u8,
    key_sel: u8,

    boot_rom: Option<Box<[u8]>>,
}

impl Bus {
    pub(crate) fn new(
        ppu: crate::ppu::Ppu,
        apu: crate::apu::Apu,
        mapper: crate::mapper::Mapper,
        boot_rom: Option<Box<[u8]>>
    ) -> Self {
        Self {
            ppu,
            apu,

            mapper,

            wram: [0; 0x2000],
            hram: [0; 0x7f],

            oam_dma_at: (0, 0xff),
            tima: 0,
            tma: 0,
            tac: 0,

            keys: 0xff,
            key_sel: 0,

            boot_rom,
        }
    }
}

impl sm83::bus::Bus for Bus {
    fn load(&mut self, a: u16) -> u8 {
        if let (true, Some(br)) = (a <= 0x00ff, &self.boot_rom) {
            return br[a as usize];
        }

        match a {
            0x0000..=0x7fff => self.mapper.load(a),
            0xa000..=0xbfff => self.mapper.load(a),
            0xc000..=0xdfff => self.wram[a as usize - 0xc000],
            0xe000..=0xfdff => self.wram[a as usize - 0xe000],
            0xfea0..=0xfeff => 0xff,
            0xff00 => {
                let dp = if self.key_sel & 0x10 == 0 { self.keys & 0xf } else { 0 };
                let sl = if self.key_sel & 0x20 == 0 { self.keys >> 4 } else { 0 };
                (0xf | self.key_sel) & !dp & !sl
            },
            0xff05 => self.tima,
            0xff06 => self.tma,
            0xff07 => self.tac,
            0xff46 => self.oam_dma_at.0,
            0x8000..=0x9fff | 0xfe00..=0xfe9f | 0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.load(a),
            0xff10..=0xff3f => self.apu.load(a),
            0xff00..=0xff7f => { eprintln!("io {a:04x}"); 0 },
            0xff80..=0xfffe => self.hram[a as usize - 0xff80],
            0xffff => unreachable!(),
        }
    }

    fn store(&mut self, a: u16, d: u8) {
        if self.boot_rom.is_some() && a == 0xff50 {
            self.boot_rom = None;
            return;
        }

        match a {
            0x0000..=0x7fff => self.mapper.store(a, d),
            0xa000..=0xbfff => self.mapper.store(a, d),
            0xc000..=0xdfff => self.wram[a as usize - 0xc000] = d,
            0xe000..=0xfdff => self.wram[a as usize - 0xe000] = d,
            0xfea0..=0xfeff => {},
            0xff00 => self.key_sel = d & 0x30,
            0xff05 => self.tima = d,
            0xff06 => self.tma = d,
            0xff07 => self.tac = d & 7,
            0xff46 => self.oam_dma_at = (d, 0),
            0x8000..=0x9fff | 0xfe00..=0xfe9f | 0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.store(a, d),
            0xff10..=0xff3f => self.apu.store(a, d),
            0xff00..=0xff7f => eprintln!("io {a:04x} {d:02x}"),
            0xff80..=0xfffe => self.hram[a as usize - 0xff80] = d,
            0xffff => unreachable!(),
        }
    }
}
