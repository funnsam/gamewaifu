use core::sync::atomic::*;

pub mod bus;
pub mod mapper;
pub mod ppu;

pub struct Gameboy<'a> {
    pub cpu: sm83::Sm83<bus::Bus<'a>>,

    hsync: usize,
}

impl<'a> Gameboy<'a> {
    pub fn new(mapper: mapper::Mapper<'a>) -> Self {
        let ppu = ppu::Ppu::new();
        let bus = bus::Bus::new(ppu, mapper);

        Self {
            cpu: sm83::Sm83::new(bus),

            hsync: 0,
        }
    }

    pub fn step(&mut self, gb_fb: &[AtomicU8]) {
        self.cpu.step();

        if self.hsync >= 456 {
            self.cpu.bus.ppu.render_strip(gb_fb).map(|i| self.cpu.interrupt(i));
            self.hsync = 0;
        }

        self.hsync += 1;
    }
}
