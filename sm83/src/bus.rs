pub trait Bus {
    fn load(&mut self, a: u16) -> u8;
    fn store(&mut self, a: u16, d: u8);
}
