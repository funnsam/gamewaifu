pub struct Bus {
    pub ppu: crate::ppu::Ppu,
    pub mapper: crate::mapper::Mapper,
    pub wram: [u8; 0x2000],
    pub hram: [u8; 0x7f],

    pub(crate) oam_dma_at: (u8, u8),

    pub(crate) tima: u8,
    pub(crate) tma: u8,
    pub(crate) tac: u8,

    pub keys: u8,
    key_sel: u8,
}

impl Bus {
    pub fn new(ppu: crate::ppu::Ppu, mapper: crate::mapper::Mapper) -> Self {
        Self {
            ppu,
            mapper,

            wram: [0; 0x2000],
            hram: [0; 0x7f],

            oam_dma_at: (0, 0xff),
            tima: 1,
            tma: 0,
            tac: 0,

            keys: 0xff,
            key_sel: 0,
        }
    }
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
            0xfea0..=0xfeff => 0xff,
            0xff00 => {
                let dp = if self.key_sel & 0x10 == 0 { self.keys & 0xf } else { 0 };
                let sl = if self.key_sel & 0x20 == 0 { self.keys >> 4 } else { 0 };
                self.key_sel | dp | sl
            },
            0xff05 => self.tima,
            0xff06 => self.tma,
            0xff07 => self.tac,
            0xff40 => self.ppu.lcdc,
            0xff41 => self.ppu.get_stat(),
            0xff42 => self.ppu.scroll.1,
            0xff43 => self.ppu.scroll.0,
            0xff44 => self.ppu.ly,
            0xff45 => self.ppu.lyc,
            0xff46 => self.oam_dma_at.0,
            0xff47 => self.ppu.bgp,
            0xff48..=0xff49 => self.ppu.obp[a as usize - 0xff48],
            0xff4a => self.ppu.window.1,
            0xff4b => self.ppu.window.0,
            0xff00..=0xff7f => { eprintln!("io {a:04x}"); 144 },
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
            0xfea0..=0xfeff => {},
            0xff00 => self.key_sel = d & 0x30,
            0xff05 => self.tima = d,
            0xff06 => self.tma = d,
            0xff07 => self.tac = d & 7,
            0xff40 => self.ppu.lcdc = d,
            0xff41 => self.ppu.set_stat(d),
            0xff42 => self.ppu.scroll.1 = d,
            0xff43 => self.ppu.scroll.0 = d,
            0xff44 => {},
            0xff45 => self.ppu.lyc = d,
            0xff46 => self.oam_dma_at = (d, 0),
            0xff47 => self.ppu.bgp = d,
            0xff48..=0xff49 => self.ppu.obp[a as usize - 0xff48] = d,
            0xff4a => self.ppu.window.1 = d,
            0xff4b => self.ppu.window.0 = d,
            0xff00..=0xff7f => eprintln!("io {a:04x} {d:02x}"),
            0xff80..=0xfffe => self.hram[a as usize - 0xff80] = d,
            0xffff => unreachable!(),
        }
    }
}
