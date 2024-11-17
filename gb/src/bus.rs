use std::sync::{atomic::*, *};

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub(crate) struct Bus<'a> {
    pub(crate) ppu: crate::ppu::Ppu,
    pub(crate) apu: crate::apu::Apu<'a>,

    #[derivative(Debug = "ignore")]
    pub(crate) mapper: crate::mapper::Mapper,
    pub(crate) wram: [u8; 0x2000],
    pub(crate) hram: [u8; 0x7f],

    pub(crate) oam_dma_at: (u8, u8),

    pub(crate) tima: u8,
    pub(crate) tma: u8,
    pub(crate) tac: u8,

    timer_prev: bool,
    timer_reload: bool,

    keys: Arc<AtomicU8>,
    pub(crate) key_sel: u8,

    boot_rom: Option<Box<[u8]>>,
}

impl<'a> Bus<'a> {
    pub(crate) fn new(
        ppu: crate::ppu::Ppu,
        apu: crate::apu::Apu<'a>,
        mapper: crate::mapper::Mapper,
        boot_rom: Option<Box<[u8]>>,
        keys: Arc<AtomicU8>,
    ) -> Self {
        Self {
            ppu,
            apu,

            mapper,

            wram: [0; 0x2000],
            hram: [0; 0x7f],

            oam_dma_at: (0xff, 0xff),

            tima: 0,
            tma: 0,
            tac: 0,

            timer_prev: false,
            timer_reload: false,

            keys,
            key_sel: 0xc0,

            boot_rom,
        }
    }
}

impl sm83::bus::Bus for Bus<'_> {
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
                let keys = self.keys.load(Ordering::Relaxed);
                let dp = if self.key_sel & 0x10 == 0 { keys & 0xf } else { 0 };
                let sl = if self.key_sel & 0x20 == 0 { keys >> 4 } else { 0 };
                (0xf | self.key_sel) & !dp & !sl
            },
            0xff01 => 0x00,
            0xff02 => 0x7e,
            0xff05 => self.tima,
            0xff06 => self.tma,
            0xff07 => self.tac,
            0xff46 => self.oam_dma_at.0,
            0x8000..=0x9fff | 0xfe00..=0xfe9f | 0xff40..=0xff45 | 0xff47..=0xff4b => self.ppu.load(a),
            0xff10..=0xff3f => self.apu.load(a),
            0xff00..=0xff7f => 0xff,
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
            0xff00..=0xff7f => {},
            0xff80..=0xfffe => self.hram[a as usize - 0xff80] = d,
            0xffff => unreachable!(),
        }
    }

    fn external_step(&mut self, div: usize, int_mgr: &mut sm83::cpu::InterruptManager) {
        let tima = self.tima;

        self.ppu.step(int_mgr);
        self.apu.step(div & 0x1000 != 0);

        // oam dma
        let dma = self.oam_dma_at;
        if dma.1 <= 0x9f {
            let v = self.load(((dma.0 as u16) << 8) | dma.1 as u16);
            self.ppu.oam[dma.1 as usize] = v;
            self.oam_dma_at.1 += 1;
        }

        // timer update
        if div & 3 == 0 {
            if core::mem::replace(&mut self.timer_reload, false) {
                self.tima = self.tma;
                int_mgr.interrupt(2);
            }

            let bit = (div >> match self.tac & 3 {
                1 => 3,
                2 => 5,
                3 => 7,
                0 => 9,
                _ => unreachable!(),
            }) as u8 & (self.tac >> 2) != 0;
            let prev = core::mem::replace(&mut self.timer_prev, bit);

            if prev && !bit {
                self.tima = tima + 1;
                self.timer_reload = self.tima == 0;
            }
        }
    }
}
