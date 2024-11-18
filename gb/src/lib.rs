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
    ) -> Self {
        let have_br = boot_rom.is_some();
        let ppu = ppu::Ppu::new(framebuffer);
        let apu = apu::Apu::new(aud_callback);
        let bus = bus::Bus::new(ppu, apu, mapper, boot_rom, keys);
        let mut cpu = sm83::Sm83::new(bus);

        if !have_br {
            let checksum = cpu.bus.mapper.load(0x014d);

            cpu.set_state(&sm83::cpu::State {
                a: 0x01,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xd8,
                f: 0x80 | if checksum != 0 { 0x30 } else { 0x00 },
                h: 0x01,
                l: 0x4d,

                pc: 0x0100,
                sp: 0xfffe,
                ir: 0,
            });

            cpu.div = 0xabff;
            cpu.bus.tac = 0xf8;
            cpu.ints.pending = 0xe1;

            cpu.bus.apu.seq_timer = 2;
            cpu.bus.apu.last_div_edge = true;
            cpu.bus.apu.enable = true;
            cpu.bus.apu.volume = (7, 7);
            cpu.bus.apu.ch1 = apu::Channel1 {
                active: true,
                triggered: false,
                hard_pan: (
                    true,
                    true,
                ),
                sweep_pace: 0,
                sweep_dir: false,
                sweep_step: 0,
                sweep_enabled: false,
                sweep_timer: 8,
                duty: 2,
                period: 1985,
                internal_period: 1985,
                envelope: apu::Envelope {
                    init_vol: 15,
                    env_dir: false,
                    pace: 3,
                    volume: 0,
                    pace_timer: 3,
                },
                length_timer: 64,
                length_en: false,
                freq_timer: 184,
                duty_pos: 7,
            };

            cpu.bus.ppu.lcdc = 0x91;
            cpu.bus.ppu.ly = 153;
            cpu.bus.ppu.bgp = 0xfc;
            // cpu.bus.ppu.hsync = 132;
        }

        Self {
            cpu,
        }
    }

    pub fn step(&mut self) { self.cpu.step(); }

    pub fn set_sram(&mut self, sram: &[u8]) { self.cpu.bus.mapper.set_sram(sram) }
    pub fn get_sram(&self) -> Option<&[u8]> { self.cpu.bus.mapper.get_sram() }
}
