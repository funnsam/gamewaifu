use std::{sync::{atomic::*, *}, thread};

use clap::Parser;
use termion::{input::TermRead, raw::IntoRawMode};

mod args;

const BURST_CYCLES: usize = gb::CLOCK_HZ / 60;

fn main() {
    let args = args::Args::parse();
    let (gb_fb, keys) = init(&args);

    let raw = termion::get_tty().unwrap().into_raw_mode().unwrap();
    raw.activate_raw_mode().unwrap();

    let mut in_keys = termion::async_stdin().keys();

    println!("\x1b[?25l\x1b[?1049h\x1b[2J");

    let mut prev_pf_a = "";
    let mut prev_pf_b = "";

    let tmx = args.zoom as usize;
    let tmy = tmx * 2;
    let tmyh = tmy / 2;

    loop {
        print!("\x1b[2K\x1b[H");

        for my in 0..144 / tmy {
            for mx in 0..160 / tmx {
                let x = mx * tmx;
                let y = my * tmy;

                let mut l = 0;
                for sy in 0..tmyh {
                    for sx in 0..tmx {
                        l += gb_fb[(y + sy) * 160 + x + sx].load(Ordering::Relaxed) as usize & 3;
                    }
                }

                let mut u = 0;
                for sy in 0..tmyh {
                    for sx in 0..tmx {
                        u += gb_fb[(y + sy + tmyh) * 160 + x + sx].load(Ordering::Relaxed) as usize & 3;
                    }
                }

                let u = (u + (tmx * tmyh / 2)) / (tmx * tmyh);
                let l = (l + (tmx * tmyh / 2)) / (tmx * tmyh);

                let c = CHARS[l][u];

                let pa = PREFIXES_A[l][u];
                let pb = PREFIXES_B[l][u];

                if pa != prev_pf_a && pb != prev_pf_b && pa != "" && pb != "" {
                    print!("\x1b[{pa};{pb}m");
                    prev_pf_a = pa;
                    prev_pf_b = pb;
                } else if pa != prev_pf_a && pa != "" {
                    print!("\x1b[{pa}m");
                    prev_pf_a = pa;
                } else if pb != prev_pf_b && pb != "" {
                    print!("\x1b[{pb}m");
                    prev_pf_b = pb;
                }

                print!("{c}");
            }

            println!("\r");
        }

        let mut du = 0;
        let mut dd = 0;
        let mut dl = 0;
        let mut dr = 0;
        let mut sa = 0;
        let mut sb = 0;
        let mut sl = 0;
        let mut st = 0;

        while let Some(k) = in_keys.next() {
            use termion::event::Key;

            match k {
                Ok(Key::Char('w')) => du = 1,
                Ok(Key::Char('s')) => dd = 1,
                Ok(Key::Char('a')) => dl = 1,
                Ok(Key::Char('d')) => dr = 1,
                Ok(Key::Char('o')) => sa = 1,
                Ok(Key::Char('i')) => sb = 1,
                Ok(Key::Char('v')) => sl = 1,
                Ok(Key::Char('b')) => st = 1,
                Ok(Key::Esc) => {
                    STOP.store(true, Ordering::Relaxed);
                    while STOP.load(Ordering::Relaxed) {}

                    println!("\x1b[0m\x1b[?25h\x1b[?1049l");
                    raw.suspend_raw_mode().unwrap();

                    std::process::exit(0);
                },
                _ => {},
            }
        }

        keys.store(
            dr | (dl << 1) | (du << 2) | (dd << 3)
               | (sa << 4) | (sb << 5) | (sl << 6) | (st << 7),
            Ordering::Relaxed
        );
    }

    const CHARS: [[char; 4]; 4] = [
        // ↓ l   -> u
        [' ', '▀', '▀', '▀'],
        ['▄', ' ', '▀', '▀'],
        ['▄', '▄', ' ', '▀'],
        ['▄', '▄', '▄', ' '],
    ];

    const PREFIXES_A: [[&str; 4]; 4] = [
        // ↓ l   -> u
        ["107", "47", "100", "49"],
        ["47", "47", "100", "49"],
        ["100", "100", "100", "49"],
        ["49", "49", "49", "49"],
    ];

    const PREFIXES_B: [[&str; 4]; 4] = [
        // ↓ l   -> u
        ["", "97", "97", "97"],
        ["97", "", "37", "37"],
        ["97", "37", "", "90"],
        ["97", "37", "90", ""],
    ];
}

static BURST: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);

fn run_emu(mut gb: gb::Gameboy) {
    use std::time::*;

    let mut dur = Duration::new(0, 0);

    while !STOP.load(Ordering::Relaxed) {
        let start = Instant::now();
        for _ in 0..BURST_CYCLES { gb.step(); }

        if !BURST.load(Ordering::Relaxed) {
            dur += Duration::from_secs_f64(BURST_CYCLES as f64 / gb::CLOCK_HZ as f64);
            dur = dur.saturating_sub(start.elapsed());

            if dur.as_millis() > 5 {
                thread::sleep(dur);
                dur = Duration::new(0, 0);
            }
        }
    }

    STOP.store(false, Ordering::Relaxed);
}

fn init(args: &args::Args) -> (Arc<[AtomicU8]>, Arc<AtomicU8>) {
    let rom = std::fs::read(&args.rom).unwrap();
    let br = args.boot_rom.as_ref().map(|b| std::fs::read(b).unwrap().into());

    let mut gb_fb = Vec::with_capacity(160 * 144);
    for _ in 0..160 * 144 { gb_fb.push(AtomicU8::new(0)); }
    let gb_fb: Arc<_> = gb_fb.into();

    let keys = Arc::new(AtomicU8::new(0x00));

    let mapper = gb::mapper::Mapper::from_bin(&rom);

    {
        let gb_fb = Arc::clone(&gb_fb);
        let keys = Arc::clone(&keys);

        thread::spawn(move || {
            #[cfg(feature = "audio")]
            let sink = {
                let (_stream, st_handle) = rodio::OutputStream::try_default().unwrap();
                rodio::Sink::try_new(&st_handle).unwrap()
            };

            let gb = gb::Gameboy::new(mapper, br, gb_fb, Box::new(|buf| {
                #[cfg(feature = "audio")] {
                    if sink.len() > 3 {
                        for _ in 0..sink.len() { sink.skip_one(); }
                    }

                    sink.append(rodio::buffer::SamplesBuffer::new(2, gb::apu::SAMPLE_RATE as u32, buf));
                }
            }), keys);

            run_emu(gb);
        });
    }

    (gb_fb, keys)
}

