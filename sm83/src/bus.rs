pub trait Bus {
    fn load(&mut self, a: u16) -> u8;
    fn store(&mut self, a: u16, d: u8);
    fn glitch_store(&mut self, _a: u16) {}

    fn external_step(&mut self, div: usize, int_mgr: &mut crate::cpu::InterruptManager);
}
