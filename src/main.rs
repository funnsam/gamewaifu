use std::{sync::{atomic::*, *}, thread};

#[cfg(feature = "raylib")]
fn main() {
    use raylib::{ffi::Vector2, prelude::*};

    let rom = std::env::args().nth(1).unwrap();
    let rom = std::fs::read(rom).unwrap();

    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .build();

    let mut gb_fb = Vec::with_capacity(160 * 144);
    for _ in 0..160 * 144 { gb_fb.push(AtomicU8::new(0)); }
    let gb_fb: Arc<_> = gb_fb.into();

    let mut fb = vec![0; 160 * 144 * 4];
    let mut rl_fb = rl.load_render_texture(&thread, 160, 144).unwrap();

    {
        let gb_fb = Arc::clone(&gb_fb);
        thread::spawn(move || {
            let rom = rom;
            let mapper = gb::mapper::Mapper::from_bin(&rom);
            let gb = gb::Gameboy::new(mapper);
            run_emu(gb, gb_fb);
        });
    }

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        convert(&gb_fb, &mut fb);
        rl_fb.update_texture(&fb);

        d.clear_background(Color::BLACK);
        d.draw_texture_ex(&rl_fb, Vector2 { x: 10.0, y: 10.0 }, 0.0, 3.0, Color::WHITE);
        // d.draw_fps(0, 0);
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

    {
        let gb_fb = Arc::clone(&gb_fb);
        thread::spawn(move || {
            let rom = rom;
            let mapper = gb::mapper::Mapper::from_bin(&rom);
            let gb = gb::Gameboy::new(mapper);
            run_emu(gb, gb_fb);
        });
    }

    eprintln!("\x1b[?25l\x1b[?1049h\x1b[2J\x1b[97m");
    thread::sleep_ms(100);

    loop {
        eprint!("\x1b[2K\x1b[H");
        const MX: usize = 2;
        const MY: usize = MX * 2;

        for my in 0..144 / MY {
            for mx in 0..160 / MX {
                let x = mx * MX;
                let y = my * MY;
                let mut v = 0;

                for sy in 0..MY {
                    for sx in 0..MX {
                        v += gb_fb[(y + sy) * 160 + x + sx].load(Ordering::Relaxed) as usize;
                    }
                }

                let c = CHARS[(v + (MX * MY / 2)) / (MX * MY)];
                eprint!("{}", c.to_string().repeat((MX * 2) / MY));
            }

            eprintln!();
        }
    }

    const CHARS: [char; 4] = ['█', '▓', '░', ' '];
}

fn run_emu(mut gb: gb::Gameboy, gb_fb: Arc<[AtomicU8]>) {
    use std::time::*;

    loop {
        let start = Instant::now();

        gb.step(&gb_fb);

        let dur = start.elapsed().saturating_sub(Duration::from_secs_f64(1e6 / 4.194304));
        thread::sleep(dur);
    }
}
