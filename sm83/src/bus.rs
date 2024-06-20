pub trait Bus {
    fn load_u8(&self, a: u16) -> u8;
    fn load_u16(&self, a: u16) -> u16;
    fn store_u8(&mut self, a: u16, d: u8);
    fn store_u16(&mut self, a: u16, d: u16);
}
