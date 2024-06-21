use std::{ops::Deref, sync::Mutex};

pub trait Bus {
    fn load(&mut self, a: u16) -> u8;
    fn store(&mut self, a: u16, d: u8);
}

impl<B: Bus, D: Deref<Target = Mutex<B>>> Bus for D {
    fn load(&mut self, a: u16) -> u8 {
        self.deref().lock().unwrap().load(a)
    }

    fn store(&mut self, a: u16, d: u8) {
        self.deref().lock().unwrap().store(a, d)
    }
}
