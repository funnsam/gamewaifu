use core::sync::atomic::*;

static COUNT: AtomicUsize = AtomicUsize::new(0);

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let json = std::fs::read_to_string(&path).unwrap();
    let json = json::parse(&json).unwrap();

    let mut bus = Bus {
        ram: vec![0; 1 << 16],
        hist: Vec::new(),
    };

    let mut err = 0;

    'a: for t in json.members() {
        let name = t["name"].as_str().unwrap();
        let init = &t["initial"];
        let expt = &t["final"];
        let cycl = &t["cycles"];
        let cycl = cycl.members().filter(|i| !i.is_null()).collect::<Vec<_>>();

        bus.ram.fill(0);
        bus.hist.clear();
        COUNT.store(0, Ordering::Relaxed);

        for b in init["ram"].members() {
            bus.ram[b[0].as_usize().unwrap()] = b[1].as_u8().unwrap();
        }

        let mut cpu = sm83::Sm83::new(&mut bus);
        let mut state = get_state(init);
        state.pc -= 1;
        cpu.set_state(&state);

        while COUNT.load(Ordering::Relaxed) <= cycl.len() {
            cpu.step();
        }

        let cpu_s = cpu.get_state();
        let expt_s = get_state(expt);
        if cpu_s != expt_s {
            println!("state {name}\n{}\n{}", expt_s, cpu_s);
            err += 1;
            continue;
        }

        for b in expt["ram"].members() {
            if bus.ram[b[0].as_usize().unwrap()] != b[1].as_u8().unwrap() {
                println!("ram {name}");
                err += 1;
                continue 'a;
            }
        }

        for (e, h) in cycl.iter().zip(bus.hist.iter().skip(1)) {
            if e[0] != h.0 || e[1] != h.1 || (e[2] == "write") != h.2 {
                println!("cycles {name} {} {h:?}", e);
                err += 1;
                continue 'a;
            }
        }
    }

    std::process::exit(err);
}

fn get_state(o: &json::JsonValue) -> sm83::cpu::State {
    sm83::cpu::State {
        a: o["a"].as_u8().unwrap(),
        b: o["b"].as_u8().unwrap(),
        c: o["c"].as_u8().unwrap(),
        d: o["d"].as_u8().unwrap(),
        e: o["e"].as_u8().unwrap(),
        f: o["f"].as_u8().unwrap(),
        h: o["h"].as_u8().unwrap(),
        l: o["l"].as_u8().unwrap(),

        pc: o["pc"].as_u16().unwrap(),
        sp: o["sp"].as_u16().unwrap(),
    }
}

struct Bus {
    ram: Vec<u8>,
    hist: Vec<(u16, u8, bool)>,
}

impl sm83::bus::Bus for Bus {
    fn load(&mut self, a: u16) -> u8 {
        let v = self.ram[a as usize];
        self.hist.push((a, v, false));
        COUNT.fetch_add(1, Ordering::Relaxed);
        v
    }

    fn store(&mut self, a: u16, d: u8) {
        self.ram[a as usize] = d;
        self.hist.push((a, d, true));
        COUNT.fetch_add(1, Ordering::Relaxed);
    }
}
