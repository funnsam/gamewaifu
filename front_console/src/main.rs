use std::{sync::{atomic::*, *}, thread};

use clap::Parser;

mod args;

fn main() {
    let args = args::Args::parse();
    let (gb_fb, keys) = init(&args);

    println!("\x1b[?25l\x1b[?1049h\x1b[2J");
    let _ = ctrlc::set_handler(|| {
        STOP.store(true, Ordering::Relaxed);
        std::thread::sleep_ms(1000);
        println!("\x1b[0m\x1b[?25h\x1b[?1049l");
        std::process::exit(0);
    });

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

            println!();
        }

        if STOP.load(Ordering::Relaxed) { loop {} }
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

    let mut start = Instant::now();
    let mut dur = Duration::new(0, 0);
    let t_cycle = Duration::from_secs_f64(1.0 / gb::CLOCK_HZ as f64);

    loop {
        gb.step();

        if !BURST.load(Ordering::Relaxed) {
            dur += t_cycle;
            dur = dur.saturating_sub(start.elapsed());

            if dur.as_millis() > 3 {
                thread::sleep(dur);
                dur = Duration::new(0, 0);
            }
        }

        if STOP.load(Ordering::Relaxed) {
            break;
        }

        start = Instant::now();
    }
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
            let mut wav = Vec::<u8>::new();
            wav.extend(b"RIFF");
            let file_size_idx = wav.len();
            wav.extend(0_u32.to_le_bytes());
            wav.extend(b"WAVE");
            wav.extend(b"fmt ");
            wav.extend(16_u32.to_le_bytes());
            wav.extend(1_u16.to_le_bytes());
            wav.extend(2_u16.to_le_bytes());
            wav.extend((gb::apu::SAMPLE_RATE as u32).to_le_bytes());
            wav.extend((gb::apu::SAMPLE_RATE as u32 * 16 * 2 / 8).to_le_bytes());
            wav.extend(4_u16.to_le_bytes());
            wav.extend(16_u16.to_le_bytes());
            wav.extend(b"data");
            let data_size_idx = wav.len();
            wav.extend(0_u32.to_le_bytes());

            let gb = gb::Gameboy::new(mapper, br, gb_fb, Box::new(|buf| {
                wav.extend(buf.iter().flat_map(|v| v.to_le_bytes()));
            }), keys);

            run_emu(gb);

            let wav_len = wav.len();
            wav[file_size_idx..file_size_idx + 4].copy_from_slice(&(wav_len as u32).to_le_bytes());
            wav[data_size_idx..data_size_idx + 4].copy_from_slice(&((wav_len - data_size_idx - 4) as u32).to_le_bytes());
            std::fs::write("audio.wav", wav).unwrap();
        });
    }

    (gb_fb, keys)
}

