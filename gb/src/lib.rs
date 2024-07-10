use std::sync::{atomic::*, *};
use sm83::bus::Bus;

pub mod apu;
pub mod bus;
pub mod mapper;
pub mod ppu;

pub const CLOCK_HZ: usize = 4194304;

pub struct Gameboy<'a> {
    cpu: sm83::Sm83<bus::Bus<'a>>,

    timer_prev: bool,
    timer_reload: bool,

    keys: Arc<AtomicU8>,
}

impl<'a> Gameboy<'a> {
    pub fn new(
        mapper: mapper::Mapper,
        boot_rom: Option<Box<[u8]>>,
        framebuffer: Arc<[AtomicU8]>,
        aud_callback: apu::Callback<'a>,
        keys: Arc<AtomicU8>,
    ) -> Self {
        let have_br = boot_rom.is_some();
        let ppu = ppu::Ppu::new(framebuffer);
        let apu = apu::Apu::new(aud_callback);
        let bus = bus::Bus::new(ppu, apu, mapper, boot_rom);
        let mut cpu = sm83::Sm83::new(bus);

        if !have_br {
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
        }

        Self {
            cpu,

            timer_prev: false,
            timer_reload: false,

            keys,
        }
    }

    pub fn step(&mut self) {
        let tima = self.cpu.bus.tima;

        self.cpu.step();
        self.cpu.bus.ppu.step().map(|i| if i != 2 {
            self.cpu.interrupt(i);
        } else {
            self.cpu.interrupt(0);
            self.cpu.interrupt(1);
        });
        self.cpu.bus.apu.step(self.cpu.div & 0x2000 != 0);

        // oam dma
        let dma = self.cpu.bus.oam_dma_at;
        if dma.1 <= 0x9f {
            let v = self.cpu.bus.load(((dma.0 as u16) << 8) | dma.1 as u16);
            self.cpu.bus.ppu.oam[dma.1 as usize] = v;
            self.cpu.bus.oam_dma_at.1 += 1;
        }

        // timer update
        if self.cpu.div & 3 == 0 {
            if core::mem::replace(&mut self.timer_reload, false) {
                self.cpu.bus.tima = self.cpu.bus.tma;
                self.cpu.interrupt(2);
            }

            let bit = (self.cpu.div >> match self.cpu.bus.tac & 3 {
                1 => 3,
                2 => 5,
                3 => 7,
                0 => 9,
                _ => unreachable!(),
            }) as u8 & (self.cpu.bus.tac >> 2) != 0;
            let prev = core::mem::replace(&mut self.timer_prev, bit);

            if prev && !bit {
                self.cpu.bus.tima = tima + 1;
                self.timer_reload = self.cpu.bus.tima == 0;
            }
        }

        self.cpu.bus.keys = self.keys.load(Ordering::Relaxed);
    }

    pub fn set_sram(&mut self, sram: &[u8]) { self.cpu.bus.mapper.set_sram(sram) }
    pub fn get_sram(&self) -> Option<&[u8]> { self.cpu.bus.mapper.get_sram() }
}
