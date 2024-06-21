use crate::*;
use core::fmt;
use core::ops::*;

pub struct Sm83<'a> {
    bus: &'a mut dyn bus::Bus,

    regs: [u8; 8], // ordering according to constants to speedup lookup time
    sp: u16,
    pc: u16,
    ir: u8,
    ime: bool,

    cycles: usize,
    after_ei: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct State {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
    pub pc: u16,
    pub sp: u16,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "a: {:02x}, b: {:02x}, c: {:02x}, d: {:02x}", self.a, self.b, self.c, self.d)?;
        writeln!(f, "e: {:02x}, f: {:02x}, h: {:02x}, l: {:02x}", self.e, self.f, self.h, self.l)?;
        write!(f, "pc: {:04x}, sp: {:04x}", self.pc, self.sp)
    }
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

const FZ: u8 = 0x80;
const FN: u8 = 0x40;
const FH: u8 = 0x20;
const FC: u8 = 0x10;

fn hc_u8<F: Fn(u8, u8) -> u8>(a: u8, b: u8, op: F) -> bool {
    op(a & 0xf, b & 0xf) > 0xf
}

fn hc_u16<F: Fn(u16, u16) -> u16>(a: u16, b: u16, op: F) -> bool {
    op(a & 0xfff, b & 0xfff) > 0xfff
}

fn hc3_u8<F: Fn(u8, u8) -> u8>(a: u8, b: u8, c: u8, op: F) -> bool {
    op(op(a & 0xf, b & 0xf), c & 0xf) > 0xf
}

macro_rules! setfr {
    ($s: tt c $v: expr $(, $($t: tt)*)?) => {
        let v = $v;
        $s.regs[F as usize] &= !FC;
        $s.regs[F as usize] |= (v as u8) << 4;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt h $v: expr $(, $($t: tt)*)?) => {
        let v = $v;
        $s.regs[F as usize] &= !FH;
        $s.regs[F as usize] |= (v as u8) << 5;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt n $v: expr $(, $($t: tt)*)?) => {
        let v = $v;
        $s.regs[F as usize] &= !FN;
        $s.regs[F as usize] |= (v as u8) << 6;
        $(setfr!($s $($t)*);)?
    };
    ($s: tt z $v: expr $(, $($t: tt)*)?) => {
        let v = $v;
        $s.regs[F as usize] &= !FZ;
        $s.regs[F as usize] |= (v as u8) << 7;
        $(setfr!($s $($t)*);)?
    };
}


impl<'a> Sm83<'a> {
    pub fn new(bus: &'a mut dyn bus::Bus) -> Self {
        Self {
            bus,

            ir: 0,

            regs: [0; 8],
            sp: 0,
            pc: 0,
            ime: false,

            cycles: 0,
            after_ei: false,
        }
    }

    pub fn set_state(&mut self, s: &State) {
        self.regs[A as usize] = s.a;
        self.regs[B as usize] = s.b;
        self.regs[C as usize] = s.c;
        self.regs[D as usize] = s.d;
        self.regs[E as usize] = s.e;
        self.regs[F as usize] = s.f;
        self.regs[H as usize] = s.h;
        self.regs[L as usize] = s.l;

        self.pc = s.pc;
        self.sp = s.sp;
    }

    pub fn get_state(&self) -> State {
        State {
            a: self.regs[A as usize],
            b: self.regs[B as usize],
            c: self.regs[C as usize],
            d: self.regs[D as usize],
            e: self.regs[E as usize],
            f: self.regs[F as usize],
            h: self.regs[H as usize],
            l: self.regs[L as usize],

            pc: self.pc,
            sp: self.sp,
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

        if self.after_ei {
            self.ime = true;
            self.after_ei = false;
        }

        let inst = self.ir;
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
            (0, 2, 0, _, _) => { // stop
                // TODO: do the accurate thing
                // https://gist.github.com/SonoSooS/c0055300670d678b5ae8433e20bea595#nop-and-stop
                self.fetch_u8();
            },
            (0, 3, 0, _, _) => { // jr s8
                self.pc += self.fetch_u8() as i8 as u16;
            },
            (0, 4..=7, 0, _, _) => { // jr cc, d
                let d = self.fetch_u8() as i8 as u16;
                if self.cond_check(y - 4) { self.pc += d; }
            },
            (0, _, 1, _, 0) => { // ld r16, i16
                let d = self.fetch_u16();
                self.store_reg_r16(p, d);
            },
            (0, _, 1, _, 1) => { // add hl, r16
                self.incr_cycles(1);
                let a = self.load_reg_r16(HL);
                let b = self.load_reg_r16(p);
                let (v, c) = a.overflowing_add(b);
                self.store_reg_r16(HL, v);
                setf!(c c, h hc_u16(a, b, u16::add), n 0);
            },
            (0, _, 2, _, 0) => { // ld [m16], a
                let a = self.load_reg_r16mem(p);
                let d = self.load_reg_r8(A);
                self.store_bus_u8(a, d);
            },
            (0, _, 2, _, 1) => { // ld a, [m16]
                let a = self.load_reg_r16mem(p);
                let v = self.load_bus_u8(a);
                self.store_reg_r8(A, v);
            },
            (0, _, 3, _, 0) => { // inc r16
                self.incr_cycles(1);
                let d = self.load_reg_r16(p);
                self.store_reg_r16(p, d + 1);
            },
            (0, _, 3, _, 1) => { // dec r16
                self.incr_cycles(1);
                let d = self.load_reg_r16(p);
                self.store_reg_r16(p, d - 1);
            },
            (0, _, 4, _, _) => { // inc r8
                let d = self.load_reg_r8(y);
                self.store_reg_r8(y, d + 1);
                setf!(h hc_u8(d, 1, u8::add), n 0, z d == 0xff);
            },
            (0, _, 5, _, _) => { // dec r8
                let d = self.load_reg_r8(y);
                self.store_reg_r8(y, d - 1);
                setf!(h hc_u8(d, 1, u8::sub), n 1, z d == 0x01);
            },
            (0, _, 6, _, _) => { // ld r8 i8
                let v = self.fetch_u8();
                self.store_reg_r8(y, v);
            },
            (0, 0..=3, 7, _, _) => { // rot[0..=3] a
                let a = self.load_reg_r8(A);
                let a = self.shift(y, a, false);
                self.store_reg_r8(A, a);
            },
            (0, 4, 7, _, _) => { // daa
                let mut a = self.load_reg_r8(A);
                let n = self.get_flag(FN);
                if self.get_flag(FC) || (!n && (a & 0xff) > 0x99) { a -= 0x60 * (n as u8 * 2 - 1); setf!(c 1); }
                if self.get_flag(FH) || (!n && (a & 0x0f) > 0x09) { a -= 0x06 * (n as u8 * 2 - 1); }
                self.store_reg_r8(A, a);

                setf!(z a == 0, h 0);
            },
            (0, 5, 7, _, _) => { // cpl
                let a = self.load_reg_r8(A);
                self.store_reg_r8(A, !a);
                setf!(n 1, h 1);
            },
            (0, 6, 7, _, _) => { // scf
                setf!(n 0, h 0, c 1);
            },
            (0, 7, 7, _, _) => { // ccf
                setf!(n 0, h 0, c !self.get_flag(FC));
            },
            (1, 6, 6, _, _) => { // halt
                self.ir = self.load_bus_u8(self.pc);
                self.check_interrupts();
                return; // some funny quirk
            },
            (1, _, _, _, _) => { // ld r8, r8
                let d = self.load_reg_r8(z);
                self.store_reg_r8(y, d);
            },
            (2, _, _, _, _) => { // alu(y) r8
                let b = self.load_reg_r8(z);
                self.alu_op(y, b);
            },
            (3, 0..=3, 0, _, _) => { // ret cc
                self.incr_cycles(1);
                if self.cond_check(y) {
                    let r = self.pop();
                    self.pc = r;
                }
            },
            (3, 4, 0, _, _) => { // ldh i8, a
                let a = 0xff00 | self.fetch_u8() as u16;
                let d = self.load_reg_r8(A);
                self.store_bus_u8(a, d);
            },
            (3, 5, 0, _, _) => { // add sp, s8
                self.incr_cycles(2); // goddamn sharp
                let e = self.fetch_u8();
                let sp = self.sp;
                self.sp += e as i8 as u16;

                let (_, c) = (sp as u8).overflowing_add(e as u8);
                setf!(z 0, n 0, c c, h hc_u8(sp as _, e, u8::add));
            },
            (3, 6, 0, _, _) => { // ldh a, i8
                let a = 0xff00 | self.fetch_u8() as u16;
                let d = self.load_bus_u8(a);
                self.store_reg_r8(A, d);
            },
            (3, 7, 0, _, _) => { // ld hl, sp + s8
                self.incr_cycles(2); // goddamn sharp
                let e = self.fetch_u8();
                self.store_reg_r16(HL, self.sp + e as i8 as u16);

                let (_, c) = (self.sp as u8).overflowing_add(e as u8);
                let sp = self.sp;
                setf!(z 0, n 0, c c, h hc_u8(sp as _, e, u8::add));
            },
            (3, _, 1, _, 0) => { // pop r16
                let v = self.pop();
                self.store_reg_r16stk(p, v);
            },
            (3, _, 1, 0, 1) => { // ret
                let r = self.pop();
                self.pc = r;
            },
            (3, _, 1, 1, 1) => { // reti
                self.ime = true;
                let r = self.pop();
                self.pc = r;
            },
            (3, _, 1, 2, 1) => { // jp hl
                let p = self.load_reg_r16(HL);
                self.pc = p;
            },
            (3, _, 1, 3, 1) => { // ld sp, hl
                let p = self.load_reg_r16(HL);
                self.sp = p;
            },
            (3, 0..=3, 2, _, _) => { // jp cc, i16
                let n = self.fetch_u16();
                if self.cond_check(y) {
                    self.pc = n;
                }
            },
            (3, 4, 2, _, _) => { // ldh c, a
                let c = self.load_reg_r8(C);
                let a = 0xff00 | c as u16;
                let d = self.load_reg_r8(A);
                self.store_bus_u8(a, d);
            },
            (3, 5, 2, _, _) => { // ld [i16], a
                let a = self.fetch_u16();
                let d = self.load_reg_r8(A);
                self.store_bus_u8(a, d);
            },
            (3, 6, 2, _, _) => { // ldh a, c
                let c = self.load_reg_r8(C);
                let a = 0xff00 | c as u16;
                let d = self.load_bus_u8(a);
                self.store_reg_r8(A, d);
            },
            (3, 7, 2, _, _) => { // ld a, [i16]
                let a = self.fetch_u16();
                let d = self.load_bus_u8(a);
                self.store_reg_r8(A, d);
            },
            (3, 0, 3, _, _) => { // jp i16
                self.pc = self.fetch_u16();
            },
            (3, 1, 3, _, _) => { // cb prefix
                self.execute_cb();
            },
            (3, _, 3, 3, 0) => { // di
                self.ime = false;
            },
            (3, _, 3, 3, 1) => { // ei
                self.after_ei = true;
            },
            (3, 0..=3, 4, _, _) => { // call cc, i16
                let wz = self.fetch_u16();
                if self.cond_check(y) {
                    self.call(wz);
                }
            },
            (3, _, 5, _, 0) => { // push r16
                let v = self.load_reg_r16stk(p);
                self.push(v);
            },
            (3, _, 5, 0, 1) => { // call i16
                let wz = self.fetch_u16();
                self.call(wz);
            },
            (3, _, 6, _, _) => { // alu(y) i8
                let b = self.fetch_u8();
                self.alu_op(y, b);
            },
            (3, _, 7, _, _) => { // rst
                self.call(y as u16 * 8);
            },
            _ => {
                println!("inv {x} {y} {z}");
                return; // inv opc never fetch
            },
        }

        self.ir = self.fetch_u8();
        self.check_interrupts();
    }

    fn execute_cb(&mut self) {
        let inst = self.fetch_u8();
        let x = inst >> 6;
        let y = (inst >> 3) & 7;
        let z = inst & 7;

        let v = self.load_reg_r8(z);

        let v = match x {
            0 => self.shift(y, v, true),
            1 => {
                setfr!(self z v & (1 << y) == 0, n 0, h 1);
                return;
            },
            2 => v & !(1 << y),
            3 => v | (1 << y),
            _ => unreachable!(),
        };

        self.store_reg_r8(z, v);
    }

    fn shift(&mut self, m: u8, v: u8, a: bool) -> u8 {
        match m {
            0 => {
                let n = v.rotate_left(1);
                setfr!(self z a && n == 0, n 0, h 0, c v & 0x80 != 0);
                n
            },
            1 => {
                let n = v.rotate_right(1);
                setfr!(self z a && n == 0, n 0, h 0, c v & 1 != 0);
                n
            },
            2 => {
                let n = (v << 1) | self.get_flag(FC) as u8;
                setfr!(self z a && n == 0, n 0, h 0, c v & 0x80 != 0);
                n
            },
            3 => {
                let n = (v >> 1) | ((self.get_flag(FC) as u8) << 7);
                setfr!(self z a && n == 0, n 0, h 0, c v & 1 != 0);
                n
            },
            4 => {
                let n = v << 1;
                setfr!(self z a && n == 0, n 0, h 0, c v & 0x80 != 0);
                n
            },
            5 => {
                let n = ((v as i8) >> 1) as u8;
                setfr!(self z a && n == 0, n 0, h 0, c v & 1 != 0);
                n
            },
            6 => {
                let n = (v << 4) | (v >> 4);
                setfr!(self z a && n == 0, n 0, h 0, c 0);
                n
            },
            7 => {
                let n = v >> 1;
                setfr!(self z a && n == 0, n 0, h 0, c v & 1 != 0);
                n
            },
            _ => panic!(),
        }
    }

    fn call(&mut self, a: u16) {
        let npc = core::mem::replace(&mut self.pc, a);
        self.push(npc);
    }

    fn check_interrupts(&mut self) {
    }

    fn incr_cycles(&mut self, t: usize) {
        self.cycles += 4 * t;
    }

    fn push(&mut self, v: u16) {
        self.sp -= 2;
        self.incr_cycles(1);
        self.store_bus_u16_rev(self.sp, v);
    }

    fn pop(&mut self) -> u16 {
        self.incr_cycles(1);
        let v = self.load_bus_u16(self.sp);
        self.sp += 2;
        v
    }

    fn cond_check(&self, n: u8) -> bool {
        let inv = n & 1 == 0;
        inv ^ self.get_flag(if n >> 1 == 0 {
            FZ
        } else {
            FC
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
                setf!(h hc_u8(a, b, u8::add), n 0, z v == 0, c c);
            },
            1 => { // adc
                let c = self.get_flag(FC);
                let (v, d) = a.carrying_add(b, c);
                self.store_reg_r8(A, v);
                setf!(h hc3_u8(a, b, c as u8, u8::add), n 0, z v == 0, c d);
            },
            2 => { // sub
                let (v, c) = a.overflowing_sub(b);
                self.store_reg_r8(A, v);
                setf!(h hc_u8(a, b, u8::sub), n 1, z v == 0, c c);
            },
            3 => { // sbc
                let c = self.get_flag(FC);
                let (v, d) = a.borrowing_sub(b, c);
                self.store_reg_r8(A, v);
                setf!(h hc3_u8(a, b, c as u8, u8::sub), n 1, z v == 0, c d);
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
                setf!(h hc_u8(a, b, u8::sub), n 1, z v == 0, c c);
            },
            _ => panic!(),
        }
    }

    fn get_flag(&self, f: u8) -> bool {
        self.regs[F as usize] & f != 0
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
        self.incr_cycles(1);
        self.bus.load(a)
    }

    fn load_bus_u16(&mut self, a: u16) -> u16 {
        let l = self.load_bus_u8(a);
        let h = self.load_bus_u8(a + 1);
        ((h as u16) << 8) | l as u16
    }

    fn store_bus_u8(&mut self, a: u16, d: u8) {
        self.incr_cycles(1);
        self.bus.store(a, d)
    }

    fn store_bus_u16(&mut self, a: u16, d: u16) {
        self.store_bus_u8(a, d as u8);
        self.store_bus_u8(a + 1, (d >> 8) as u8);
    }

    fn store_bus_u16_rev(&mut self, a: u16, d: u16) {
        self.store_bus_u8(a + 1, (d >> 8) as u8);
        self.store_bus_u8(a, d as u8);
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
            AF => ((self.load_reg_r8(A) as u16) << 8) | (self.regs[F as usize] as u16),
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
                self.store_reg_r8(A, (d >> 8) as u8);
                self.regs[F as usize] = d as u8 & 0xf0;
            },
            _ => panic!(),
        }
    }
}
