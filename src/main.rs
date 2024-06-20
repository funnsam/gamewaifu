fn main() {
    let path = std::env::args().nth(1).unwrap();
    let mut bin = std::fs::read(&path).unwrap();
    let mut bus = sm83::bus::Tester::new(&mut bin);
    let mut cpu = sm83::Sm83::new(&mut bus);

    loop { cpu.step(); }
}
