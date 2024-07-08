use std::{sync::{atomic::*, *}, thread};

use clap::Parser;

mod args;

#[cfg(feature = "raylib")]
fn main() {
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
                let audio = RaylibAudio::init_audio_device().unwrap();
                ::core::hint::black_box(|| unsafe { raylib::ffi::SetAudioStreamBufferSizeDefault(gb::apu::BUFFER_SIZE as i32 * 2); })();
                let mut stream = audio.new_audio_stream(gb::apu::SAMPLE_RATE as _, 16, 2);
                stream.play();

                let gb = gb::Gameboy::new(mapper, br, gb_fb, Box::new(|buf| stream.update(buf)), keys);

                run_emu(gb);
            });
        }

        (gb_fb, keys)
    }

    use raylib::{ffi::Vector2, prelude::*};

    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .resizable()
        .vsync()
        .build();

    let args = args::Args::parse();
    let (gb_fb, keys) = init(&args);

    let mut fb = vec![0; 160 * 144 * 4];
    let mut rl_fb = rl.load_render_texture(&thread, 160, 144).unwrap();

    let font = rl.load_font_ex(&thread, "Roboto-Regular.ttf", 18, None).unwrap();

    while !rl.window_should_close() {
        {
            let mut d = rl.begin_drawing(&thread);

            convert(&gb_fb, &mut fb);
            rl_fb.update_texture(&fb);

            d.clear_background(Color::from_hex("0b1920").unwrap());

            let scale = (d.get_screen_width() as f32 / 160.0).min(d.get_screen_height() as f32 / 144.0).floor();
            let x = (d.get_screen_width() as f32 - scale * 160.0) / 2.0;
            let y = (d.get_screen_height() as f32 - scale * 144.0) / 2.0;

            d.draw_texture_ex(&rl_fb, Vector2 { x, y }, 0.0, scale, Color::WHITE);

            let fps = d.get_fps();
            d.draw_text_ex(&font, &format!("Display FPS {fps}\nScale {scale}"), Vector2 { x: 0.0, y: 0.0 }, 18.0, 0.0, Color::WHITE);

            if args.waifu {
                d.draw_text(&format!("bruh you expected waifu??"), 0, 100, 18, Color::RED);
            }
        }

        let du = rl.is_key_down(KeyboardKey::KEY_W) as u8;
        let dd = rl.is_key_down(KeyboardKey::KEY_S) as u8;
        let dl = rl.is_key_down(KeyboardKey::KEY_A) as u8;
        let dr = rl.is_key_down(KeyboardKey::KEY_D) as u8;
        let sa = rl.is_key_down(KeyboardKey::KEY_O) as u8;
        let sb = rl.is_key_down(KeyboardKey::KEY_I) as u8;
        let sl = rl.is_key_down(KeyboardKey::KEY_V) as u8;
        let st = rl.is_key_down(KeyboardKey::KEY_B) as u8;

        keys.store(
            dr | (dl << 1) | (du << 2) | (dd << 3)
               | (sa << 4) | (sb << 5) | (sl << 6) | (st << 7),
            Ordering::Relaxed
        );

        if rl.is_key_pressed(KeyboardKey::KEY_T) {
            let color_map = PALETTE.iter().flat_map(|v| TryInto::<[u8; 3]>::try_into(&v.to_be_bytes()[..3]).unwrap()).collect::<Vec<u8>>();

            let mut image = std::fs::File::create(&format!("screenshot_{}.gif", std::time::UNIX_EPOCH.elapsed().unwrap().as_millis())).unwrap();
            let mut encoder = gif::Encoder::new(&mut image, 160, 144, &color_map).unwrap();
            let mut frame = gif::Frame::default();
            frame.width = 160;
            frame.height = 144;
            frame.buffer = std::borrow::Cow::Owned(gb_fb.iter().map(|v| v.load(Ordering::Relaxed)).collect());
            encoder.write_frame(&frame).unwrap();
        }

        BURST.store(rl.is_key_down(KeyboardKey::KEY_ENTER), Ordering::Relaxed);
    }

    const PALETTE: [u32; 8] = [
        0xf5faefff,
        0x86c270ff,
        0x2f6957ff,
        0x0b1920ff,
        0xf50000ff,
        0x860000ff,
        0x2f0000ff,
        0x0b0000ff,
    ];

    fn convert(gb_fb: &[AtomicU8], fb: &mut [u8]) {
        for (i, c) in gb_fb.iter().enumerate() {
            let c = PALETTE[c.load(Ordering::Relaxed) as usize];
            let c = c.to_be_bytes();
            let (_, r) = fb.split_at_mut(i * 4);
            let (l, _) = r.split_at_mut(4);
            l.copy_from_slice(&c);
        }
    }
}

#[cfg(feature = "console")]
fn main() {
    let args = args::Args::parse();
    let (gb_fb, keys) = init(&args);

    println!("\x1b[?25l\x1b[?1049h\x1b[2J");
    let _ = ctrlc::set_handler(|| {
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

        start = Instant::now();
    }
}

/* fn init(args: &args::Args, aud_callback: gb::apu::Callback) -> (Arc<[AtomicU8]>, Arc<AtomicU8>) {
    let rom = std::fs::read(&args.rom).unwrap();
    let br = args.boot_rom.as_ref().map(|b| std::fs::read(b).unwrap().into());

    let mut gb_fb = Vec::with_capacity(160 * 144);
    for _ in 0..160 * 144 { gb_fb.push(AtomicU8::new(0)); }
    let gb_fb: Arc<_> = gb_fb.into();

    let keys = Arc::new(AtomicU8::new(0x00));

    let mapper = gb::mapper::Mapper::from_bin(&rom);
    let gb = gb::Gameboy::new(mapper, br, Arc::clone(&gb_fb), aud_callback, Arc::clone(&keys));

    thread::spawn(move || {
        run_emu(gb);
    });

    (gb_fb, keys)
} */
