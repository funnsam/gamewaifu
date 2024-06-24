use std::{sync::{atomic::*, *}, thread};

#[cfg(feature = "raylib")]
fn main() {
    use raylib::{ffi::Vector2, prelude::*};

    let rom = std::env::args().nth(1).unwrap();
    let rom = std::fs::read(rom).unwrap();

    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .resizable()
        .vsync()
        .build();

    let mut gb_fb = Vec::with_capacity(160 * 144);
    for _ in 0..160 * 144 { gb_fb.push(AtomicU8::new(0)); }
    let gb_fb: Arc<_> = gb_fb.into();

    let mut fb = vec![0; 160 * 144 * 4];
    let mut rl_fb = rl.load_render_texture(&thread, 160, 144).unwrap();

    let mapper = gb::mapper::Mapper::from_bin(&rom);
    let keys = Arc::new(AtomicU8::new(0xff));
    let gb = gb::Gameboy::new(mapper, Arc::clone(&keys));

    {
        let gb_fb = Arc::clone(&gb_fb);
        thread::spawn(move || {
            run_emu(gb, gb_fb);
        });
    }

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
            d.draw_text_ex(&font, &format!("Display FPS {fps}   Scale {scale}"), Vector2 { x: 0.0, y: 0.0 }, 18.0, 0.0, Color::WHITE);
        }

        let du = !rl.is_key_down(KeyboardKey::KEY_W) as u8;
        let dd = !rl.is_key_down(KeyboardKey::KEY_S) as u8;
        let dl = !rl.is_key_down(KeyboardKey::KEY_A) as u8;
        let dr = !rl.is_key_down(KeyboardKey::KEY_D) as u8;
        let sa = !rl.is_key_down(KeyboardKey::KEY_I) as u8;
        let sb = !rl.is_key_down(KeyboardKey::KEY_O) as u8;
        let sl = !rl.is_key_down(KeyboardKey::KEY_V) as u8;
        let st = !rl.is_key_down(KeyboardKey::KEY_B) as u8;

        keys.store(
            dr | (dl << 1) | (du << 2) | (dd << 3)
               | (sa << 4) | (sb << 5) | (sl << 6) | (st << 7),
            Ordering::Relaxed
        );

        BURST.store(rl.is_key_down(KeyboardKey::KEY_ENTER), Ordering::Relaxed);
    }

    const PALETTE: [u32; 4] = [
        0xf5faefff,
        0x86c270ff,
        0x2f6957ff,
        0x0b1920ff,
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

#[cfg(not(feature = "raylib"))]
fn main() {
    let rom = std::env::args().nth(1).unwrap();
    let rom = std::fs::read(rom).unwrap();

    let mut gb_fb = Vec::with_capacity(160 * 144);
    for _ in 0..160 * 144 { gb_fb.push(AtomicU8::new(0)); }
    let gb_fb: Arc<_> = gb_fb.into();

    let rom = rom;
    let mapper = gb::mapper::Mapper::from_bin(&rom);
    let gb = gb::Gameboy::new(mapper, Arc::new(AtomicU8::new(0xff)));

    {
        let gb_fb = Arc::clone(&gb_fb);
        thread::spawn(move || {
            run_emu(gb, gb_fb);
        });
    }

    println!("\x1b[?25l\x1b[?1049h\x1b[2J\x1b[97m");

    let mut prev_pf = "";

    let tmx: usize = std::env::args().nth(2).and_then(|a| a.parse().ok()).unwrap_or(2);
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
                        l += gb_fb[(y + sy) * 160 + x + sx].load(Ordering::Relaxed) as usize;
                    }
                }

                let mut u = 0;
                for sy in 0..tmyh {
                    for sx in 0..tmx {
                        u += gb_fb[(y + sy + tmyh) * 160 + x + sx].load(Ordering::Relaxed) as usize;
                    }
                }

                let u = (u + (tmx * tmyh / 2)) / (tmx * tmyh);
                let l = (l + (tmx * tmyh / 2)) / (tmx * tmyh);

                let c = CHARS[l][u];
                let p = PREFIXES[l][u];

                if p != prev_pf {
                    print!("{p}");
                    prev_pf = p;
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

    const PREFIXES: [[&str; 4]; 4] = [
        // ↓ l   -> u
        ["\x1b[107m", "\x1b[97;47m", "\x1b[97;100m", "\x1b[97;40m"],
        ["\x1b[97;47m", "\x1b[47m", "\x1b[37;100m", "\x1b[37;40m"],
        ["\x1b[97;100m", "\x1b[37;100m", "\x1b[100m", "\x1b[40;90m"],
        ["\x1b[97;40m", "\x1b[37;40m", "\x1b[90;40m", "\x1b[40m"],
    ];
}

static BURST: AtomicBool = AtomicBool::new(false);

fn run_emu(mut gb: gb::Gameboy, gb_fb: Arc<[AtomicU8]>) {
    use std::time::*;

    let mut start = Instant::now();
    let mut dur = Duration::new(0, 0);

    loop {
        gb.step(&gb_fb);

        if !BURST.load(Ordering::Relaxed) {
            dur += Duration::from_secs_f64(1.0 / 4194304.0);
            dur = dur.saturating_sub(start.elapsed());

            if dur.as_millis() > 10 {
                thread::sleep(dur);
                dur = Duration::new(0, 0);
            }
        }

        start = Instant::now();
    }
}
