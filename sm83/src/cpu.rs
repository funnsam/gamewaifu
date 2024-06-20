use crate::*;

pub struct Sm83<'a> {
    bus: &'a mut dyn bus::Bus,

    regs: [u8; 8], // ordering according to constants to speedup lookup time
    sp: u16,
    pc: u16,

    cycles: usize,
}

const B: usize = 0;
const C: usize = 1;
const D: usize = 2;
const E: usize = 3;
const H: usize = 4;
const L: usize = 5;
const I: usize = 6;
const A: usize = 7;
const F: usize = 6;

const BC: usize = 0;
const DE: usize = 1;
const HL: usize = 2;
const SP: usize = 3;
const AF: usize = 3;
const HLI: usize = 2;
const HLD: usize = 3;

impl<'a> Sm83<'a> {
    pub fn new(bus: &'a mut dyn bus::Bus) -> Self {
        Self {
            bus,

            regs: [0; 8],
            sp: 0,
            pc: 0,

            cycles: 0,
        }
    }

    pub fn step(&mut self) {
        if self.cycles != 0 {
            self.cycles -= 1;
            return;
        }

        let inst = self.fetch_u8();
        let x = inst >> 6;
        let y = (inst >> 3) & 7;
        let z = inst & 7;
        let p = (inst >> 4) & 3;
        let q = (inst >> 3) & 1;

        match (x, y, z, p, q) {
            (0, 0, 0, _, _) => {}, // nop
            (0, 1, 0, _, _) => { // ld [i16], sp
                let a = self.fetch_u16();
                self.store_bus_u16(a, self.sp);
            },
            _ => todo!("{x} {y} {z}"),
        }
    }

    fn fetch_u8(&mut self) -> u8 {
        let v = self.load_bus_u8(self.pc);
        self.pc += 1;
        v
    }

    fn fetch_u16(&mut self) -> u16 {
        let v = self.load_bus_u16(self.pc);
        self.pc += 2;
        v
    }

    fn load_bus_u8(&mut self, a: u16) -> u8 {
        self.cycles += 4;
        self.bus.load_u8(a)
    }

    fn load_bus_u16(&mut self, a: u16) -> u16 {
        self.cycles += 4;
        self.bus.load_u16(a)
    }

    fn store_bus_u8(&mut self, a: u16, d: u8) {
        self.cycles += 4;
        self.bus.store_u8(a, d)
    }

    fn store_bus_u16(&mut self, a: u16, d: u16) {
        self.cycles += 4;
        self.bus.store_u16(a, d)
    }

    fn load_reg_r8(&mut self, r: usize) -> u8 {
        match r {
            I => {
                let a = self.load_reg_r16(HL);
                self.load_bus_u8(a)
            },
            _ => self.regs[r],
        }
    }

    fn load_reg_r16(&mut self, r: usize) -> u16 {
        match r {
            BC => ((self.load_reg_r8(B) as u16) << 8) | (self.load_reg_r8(C) as u16),
            DE => ((self.load_reg_r8(D) as u16) << 8) | (self.load_reg_r8(E) as u16),
            HL => ((self.load_reg_r8(H) as u16) << 8) | (self.load_reg_r8(L) as u16),
            SP => self.sp,
            _ => panic!(),
        }
    }

    fn load_reg_r16stk(&mut self, r: usize) -> u16 {
        match r {
            BC => ((self.load_reg_r8(B) as u16) << 8) | (self.load_reg_r8(C) as u16),
            DE => ((self.load_reg_r8(D) as u16) << 8) | (self.load_reg_r8(E) as u16),
            HL => ((self.load_reg_r8(H) as u16) << 8) | (self.load_reg_r8(L) as u16),
            AF => ((self.load_reg_r8(A) as u16) << 8) | (self.load_reg_r8(F) as u16),
            _ => panic!(),
        }
    }

    fn load_reg_r16mem(&mut self, r: usize) -> u16 {
        match r {
            BC => ((self.load_reg_r8(B) as u16) << 8) | (self.load_reg_r8(C) as u16),
            DE => ((self.load_reg_r8(D) as u16) << 8) | (self.load_reg_r8(E) as u16),
            HLI => {
                let hl = self.load_reg_r16(HL);
                self.store_reg_r16(HL, hl + 1);
                hl
            },
            HLD => {
                let hl = self.load_reg_r16(HL);
                self.store_reg_r16(HL, hl - 1);
                hl
            },
            _ => panic!(),
        }
    }

    fn store_reg_r8(&mut self, r: usize, d: u8) {
        match r {
            I => {
                let a = self.load_reg_r16(HL);
                self.store_bus_u8(a, d)
            },
            _ => self.regs[r] = d,
        }
    }

    fn store_reg_r16(&mut self, r: usize, d: u16) {
        match r {
            BC => {
                self.store_reg_r8(B, (d >> 8) as u8);
                self.store_reg_r8(C, d as u8);
            },
            DE => {
                self.store_reg_r8(D, (d >> 8) as u8);
                self.store_reg_r8(E, d as u8);
            },
            HL => {
                self.store_reg_r8(H, (d >> 8) as u8);
                self.store_reg_r8(L, d as u8);
            },
            SP => self.sp = d,
            _ => panic!(),
        }
    }
}
