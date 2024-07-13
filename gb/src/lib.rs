use std::sync::{atomic::*, *};

pub mod apu;
pub mod bus;
pub mod mapper;
pub mod ppu;

pub const CLOCK_HZ: usize = 4194304;

pub struct Gameboy<'a> {
    cpu: sm83::Sm83<bus::Bus<'a>>,
}

impl<'a> Gameboy<'a> {
    pub fn new(
        mapper: mapper::Mapper,
        boot_rom: Option<Box<[u8]>>,
        framebuffer: Arc<Mutex<[u8; 160 * 144]>>,
        aud_callback: apu::Callback<'a>,
        keys: Arc<AtomicU8>,
        model: Model,
    ) -> Self {
        let have_br = boot_rom.is_some();
        let ppu = ppu::Ppu::new(framebuffer, model);
        let apu = apu::Apu::new(aud_callback, model);
        let bus = bus::Bus::new(ppu, apu, mapper, boot_rom, keys, model);
        let mut cpu = sm83::Sm83::new(bus, model.is_cgb());

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
        }
    }

    pub fn step(&mut self) { self.cpu.step(); }

    pub fn set_sram(&mut self, sram: &[u8]) { self.cpu.bus.mapper.set_sram(sram) }
    pub fn get_sram(&self) -> Option<&[u8]> { self.cpu.bus.mapper.get_sram() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Dmg0,
    DmgA,
    DmgB,
    DmgC,

    Cgb0,
    CgbA,
    CgbB,
    CgbC,
    CgbD,
    CgbE,
}

impl Model {
    pub fn is_dmg(&self) -> bool {
        matches!(self, Self::Dmg0 | Self::DmgA | Self::DmgB | Self::DmgC)
    }

    pub fn is_cgb(&self) -> bool {
        matches!(self, Self::Cgb0 | Self::CgbA | Self::CgbB | Self::CgbC | Self::CgbD | Self::CgbE)
    }
}
