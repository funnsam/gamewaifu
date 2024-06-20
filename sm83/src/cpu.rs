use crate::*;

pub struct Sm83<'a> {
    bus: &'a mut dyn bus::Bus,

    regs: [u8; 8], // ordering according to constants to speedup lookup time
    sp: u16,
    pc: u16,

    cycles: usize,
}

const B: u8 = 0;
const C: u8 = 1;
const D: u8 = 2;
const E: u8 = 3;
const H: u8 = 4;
const L: u8 = 5;
const I: u8 = 6;
const A: u8 = 7;
const F: u8 = 6;

const BC: u8 = 0;
const DE: u8 = 1;
const HL: u8 = 2;
const SP: u8 = 3;
const AF: u8 = 3;
const HLI: u8 = 2;
const HLD: u8 = 3;

macro_rules! setfr {
    ($s: tt c $v: expr $(, $($t: tt)*)?) => {
        $s.regs[F as usize] &= !0x10;
        $s.regs[F as usize] |= ($v as u8) << 4;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt h add_u8 $a: tt $b: tt $(, $($t: tt)*)?) => {
        let (_, h) = ($a << 4).overflowing_add($b << 4);
        setfr!($s h h);
        $(setfr!($s $($t)*);)?
    };
    ($s: tt h sub_u8 $a: tt $b: tt $(, $($t: tt)*)?) => {
        let (_, h) = ($a << 4).overflowing_sub($b << 4);
        setfr!($s h h);
        $(setfr!($s $($t)*);)?
    };
    ($s: tt h $v: expr $(, $($t: tt)*)?) => {
        $s.regs[F as usize] &= !0x20;
        $s.regs[F as usize] |= ($v as u8) << 5;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt n $v: expr $(, $($t: tt)*)?) => {
        $s.regs[F as usize] &= !0x40;
        $s.regs[F as usize] |= ($v as u8) << 6;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt z $v: expr $(, $($t: tt)*)?) => {
        $s.regs[F as usize] &= !0x80;
        $s.regs[F as usize] |= ($v as u8) << 7;
        $(setfr!($s $($t)*);)?
    };
}


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
        macro_rules! setf {
            ($($t: tt)*) => {
                setfr!(self $($t)*);
            };
        }

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
            (0, 0, 0, _, _) => { // nop
            },
            (0, 1, 0, _, _) => { // ld [i16], sp
                let a = self.fetch_u16();
                self.store_bus_u16(a, self.sp);
            },
            (0, 3, 0, _, _) => { // jr
                self.pc += self.fetch_u8() as i8 as u16;
            },
            (0, 4..8, 0, _, _) => { // jr c, d
                let d = self.fetch_u8() as i8 as u16;
                if self.cond_check(y - 4) { self.pc += d; }
            },
            (0, _, 1, _, 0) => { // ld r16, i16
                let d = self.fetch_u16();
                self.store_reg_r16(p, d);
            },
            (0, _, 1, _, 1) => { // add hl, r16
                let a = self.load_reg_r16(HL);
                let b = self.load_reg_r16(p);
                let (v, c) = a.overflowing_add(b);
                let (_, h) = (a << 4).overflowing_add(b << 4);
                self.store_reg_r16(HL, v);
                setf!(c c, h h, n 0);
            },
            (0, _, 3, _, 0) => { // inc r16
                let d = self.load_reg_r16(p);
                self.store_reg_r16(p, d + 1);
            },
            (0, _, 3, _, 1) => { // dec r16
                let d = self.load_reg_r16(p);
                self.store_reg_r16(p, d - 1);
            },
            (0, _, 4, _, _) => { // inc r8
                let d = self.load_reg_r8(y);
                self.store_reg_r8(y, d + 1);
                setf!(h add_u8 d 1, n 0, z d == 0);
            },
            (0, _, 5, _, _) => { // dec r8
                let d = self.load_reg_r8(y);
                self.store_reg_r8(y, d - 1);
                setf!(h sub_u8 d 1, n 0, z d == 0);
            },
            (0, _, 6, _, _) => { // ld r8 i8
                let v = self.fetch_u8();
                self.store_reg_r8(y as _, v);
            },
            // TODO: z = 7
            (1, 6, 6, _, _) => { // halt
                panic!("halt");
            },
            (1, _, _, _, _) => { // ld r8, r8
                let d = self.load_reg_r8(z);
                self.store_reg_r8(y, d);
            },
            (2, _, _, _, _) => { // alu(y) r8
                let b = self.load_reg_r8(z);
                self.alu_op(y, b);
            },
            (3, _, 1, _, 0) => {
                let v = self.pop();
                self.store_reg_r16stk(p, v);
            },
            (3, 0, 3, _, _) => { // jp i16
                self.pc = self.fetch_u16();
            },
            (3, 5, 2, _, _) => { // ld [i16], a
                let a = self.fetch_u16();
                let d = self.load_reg_r8(A);
                self.store_bus_u8(a, d);
                println!("{a:04x} {d:04x}");
            },
            (3, _, 5, _, 0) => { // push r16
                let v = self.load_reg_r16stk(p);
                self.push(v);
            },
            (3, _, 6, _, _) => { // alu(y) i8
                let b = self.fetch_u8();
                self.alu_op(y, b);
            },
            _ => todo!("{x} {y} {z}"),
        }
    }

    fn push(&mut self, v: u16) {
        self.sp -= 2;
        self.store_bus_u16(self.sp, v);
    }

    fn pop(&mut self) -> u16 {
        let v = self.load_bus_u16(self.sp);
        self.sp += 2;
        v
    }

    fn cond_check(&self, n: u8) -> bool {
        let inv = n & 1 == 0;
        inv ^ (if n >> 1 == 0 {
            self.regs[F as usize] & 0x80 != 0
        } else {
            self.regs[F as usize] & 0x10 != 0
        })
    }

    fn alu_op(&mut self, op: u8, b: u8) {
        macro_rules! setf {
            ($($t: tt)*) => {
                setfr!(self $($t)*);
            };
        }

        let a = self.load_reg_r8(A);

        match op {
            0 => { // add
                let (v, c) = a.overflowing_add(b);
                self.store_reg_r8(A, v);
                setf!(h add_u8 a b, n 0, z v == 0, c c);
            },
            1 => { // adc
                let c = self.regs[F as usize] & 0x10 != 0;
                let (v, c) = a.carrying_add(b, c);
                self.store_reg_r8(A, v);
                setf!(h add_u8 a b, n 0, z v == 0, c c);
            },
            2 => { // sub
                let (v, c) = a.overflowing_sub(b);
                self.store_reg_r8(A, v);
                setf!(h sub_u8 a b, n 1, z v == 0, c c);
            },
            3 => { // sbc
                let c = self.regs[F as usize] & 0x10 != 0;
                let (v, c) = a.borrowing_sub(b, c);
                self.store_reg_r8(A, v);
                setf!(h sub_u8 a b, n 1, z v == 0, c c);
            },
            4 => { // and
                let v = a & b;
                self.store_reg_r8(A, v);
                setf!(c 0, h 1, n 0, z v == 0);
            },
            5 => { // xor
                let v = a ^ b;
                self.store_reg_r8(A, v);
                setf!(c 0, h 0, n 0, z v == 0);
            },
            6 => { // or
                let v = a | b;
                self.store_reg_r8(A, v);
                setf!(c 0, h 0, n 0, z v == 0);
            },
            7 => { // cp
                let (v, c) = a.overflowing_sub(b);
                setf!(h sub_u8 a b, n 1, z v == 0, c c);
            },
            _ => panic!(),
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

    fn load_reg_r8(&mut self, r: u8) -> u8 {
        match r {
            I => {
                let a = self.load_reg_r16(HL);
                self.load_bus_u8(a)
            },
            _ => self.regs[r as usize],
        }
    }

    fn load_reg_r16(&mut self, r: u8) -> u16 {
        match r {
            BC => ((self.load_reg_r8(B) as u16) << 8) | (self.load_reg_r8(C) as u16),
            DE => ((self.load_reg_r8(D) as u16) << 8) | (self.load_reg_r8(E) as u16),
            HL => ((self.load_reg_r8(H) as u16) << 8) | (self.load_reg_r8(L) as u16),
            SP => self.sp,
            _ => panic!(),
        }
    }

    fn load_reg_r16stk(&mut self, r: u8) -> u16 {
        match r {
            BC => ((self.load_reg_r8(B) as u16) << 8) | (self.load_reg_r8(C) as u16),
            DE => ((self.load_reg_r8(D) as u16) << 8) | (self.load_reg_r8(E) as u16),
            HL => ((self.load_reg_r8(H) as u16) << 8) | (self.load_reg_r8(L) as u16),
            AF => ((self.load_reg_r8(A) as u16) << 8) | (self.load_reg_r8(F) as u16),
            _ => panic!(),
        }
    }

    fn load_reg_r16mem(&mut self, r: u8) -> u16 {
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

    fn store_reg_r8(&mut self, r: u8, d: u8) {
        match r {
            I => {
                let a = self.load_reg_r16(HL);
                self.store_bus_u8(a, d)
            },
            _ => self.regs[r as usize] = d,
        }
    }

    fn store_reg_r16(&mut self, r: u8, d: u16) {
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

    fn store_reg_r16stk(&mut self, r: u8, d: u16) {
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
            AF => {
                self.store_reg_r8(H, (d >> 8) as u8);
                self.regs[F as usize] = d as u8;
            },
            _ => panic!(),
        }
    }
}
