use core::sync::atomic::*;
use sm83::bus::Bus;

pub mod bus;
pub mod mapper;
pub mod ppu;

pub struct Gameboy<'a> {
    pub cpu: sm83::Sm83<bus::Bus<'a>>,

    hsync: usize,
    timer_prev: bool,
}

impl<'a> Gameboy<'a> {
    pub fn new(mapper: mapper::Mapper<'a>) -> Self {
        let ppu = ppu::Ppu::new();
        let bus = bus::Bus::new(ppu, mapper);
        let mut cpu = sm83::Sm83::new(bus);

        cpu.set_state(&sm83::cpu::State {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xd8,
            f: 0x80,
            h: 0x01,
            l: 0xd4,

            pc: 0x0100,
            sp: 0xfffe,
            ir: 0,
        });

        Self {
            cpu,

            hsync: 0,
            timer_prev: false,
        }
    }

    pub fn step(&mut self, gb_fb: &[AtomicU8]) {
        self.cpu.step();

        if self.hsync >= 456 {
            self.cpu.bus.ppu.render_strip(gb_fb).map(|i| self.cpu.interrupt(i));
            self.hsync = 0;
        }

        self.hsync += 1;

        // oam dma
        let dma = self.cpu.bus.oam_dma_at;
        if dma.1 <= 0x9f {
            let v = self.cpu.bus.load(((dma.0 as u16) << 8) | dma.1 as u16);
            self.cpu.bus.store(0xfe00 | dma.1 as u16, v);
            self.cpu.bus.oam_dma_at.1 += 1;
        }

        // timer update
        let bit = (self.cpu.div >> match self.cpu.bus.tac & 3 {
            1 => 3,
            2 => 5,
            3 => 7,
            0 => 9,
            _ => unreachable!(),
        }) as u8 & (self.cpu.bus.tac >> 2) != 0;
        let prev = core::mem::replace(&mut self.timer_prev, bit);

        if prev && !bit {
            self.cpu.bus.tima += 1;

            if self.cpu.bus.tima == 0 {
                self.cpu.bus.tima = self.cpu.bus.tma;
                self.cpu.interrupt(2);
            }
        }
    }
}
