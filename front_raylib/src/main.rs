use std::{sync::{atomic::*, *}, thread};

use clap::Parser;
use raylib::{ffi::Vector2, prelude::*};

mod args;
mod config;

const BURST_CYCLES: usize = gb::CLOCK_HZ / 120;

static BURST: AtomicBool = AtomicBool::new(false);
static SAVE: AtomicBool = AtomicBool::new(false);
static STEP_FOR_BURSTS: AtomicIsize = AtomicIsize::new(-1);

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .resizable()
        .vsync()
        .build();

    rl.set_exit_key(None);

    let args = args::Args::parse();
    if args.paused { STEP_FOR_BURSTS.store(0, Ordering::Relaxed); }

    let config = config::get_config();
    let key_u = key_from_i32(config.inputs.u).unwrap();
    let key_l = key_from_i32(config.inputs.l).unwrap();
    let key_d = key_from_i32(config.inputs.d).unwrap();
    let key_r = key_from_i32(config.inputs.r).unwrap();
    let key_a = key_from_i32(config.inputs.a).unwrap();
    let key_b = key_from_i32(config.inputs.b).unwrap();
    let key_select = key_from_i32(config.inputs.select).unwrap();
    let key_start = key_from_i32(config.inputs.start).unwrap();
    let key_save = key_from_i32(config.inputs.save).unwrap();
    let key_no_save = key_from_i32(config.inputs.no_save).unwrap();
    let key_screenshot = key_from_i32(config.inputs.screenshot).unwrap();
    let key_pause = key_from_i32(config.inputs.pause).unwrap();
    let key_step_frame = key_from_i32(config.inputs.step_frame).unwrap();
    let key_burst = key_from_i32(config.inputs.burst).unwrap();

    let (gb_fb, keys) = crate::init(&args);
    let mut fb = vec![0; 160 * 144 * 4];
    let mut rl_fb = rl.load_render_texture(&thread, 160, 144).unwrap();
    let font = rl.load_font_ex(&thread, "Roboto-Regular.ttf", 18, None).unwrap();

    while !rl.window_should_close() {
        {
            let mut d = rl.begin_drawing(&thread);

            convert(&gb_fb.lock().unwrap(), &mut fb);
            rl_fb.update_texture(&fb);

            d.clear_background(Color::from_hex("0b1920").unwrap());

            let scale = (d.get_screen_width() as f32 / 160.0).min(d.get_screen_height() as f32 / 144.0).floor();
            let x = (d.get_screen_width() as f32 - scale * 160.0) / 2.0;
            let y = (d.get_screen_height() as f32 - scale * 144.0) / 2.0;

            d.draw_texture_ex(&rl_fb, Vector2 { x, y }, 0.0, scale, Color::WHITE);

            let fps = d.get_fps();
            d.draw_text_ex(&font, &format!("Display FPS {fps}\nScale {scale}"), Vector2 { x: 0.0, y: 0.0 }, 18.0, 0.0, Color::WHITE);

            if args.waifu {
                d.draw_text("bruh you expected waifu??", 0, 100, 18, Color::RED);
            }

            let sfb = STEP_FOR_BURSTS.load(Ordering::Relaxed);
            if sfb != -1 {
                d.draw_text_ex(&font, &format!("{} cycles left", sfb as usize * BURST_CYCLES), Vector2 { x: 0.0, y: 100.0 }, 18.0, 0.0, Color::WHITE);
            }
        }

        let du = rl.is_key_down(key_u) as u8;
        let dd = rl.is_key_down(key_d) as u8;
        let dl = rl.is_key_down(key_l) as u8;
        let dr = rl.is_key_down(key_r) as u8;
        let sa = rl.is_key_down(key_a) as u8;
        let sb = rl.is_key_down(key_b) as u8;
        let sl = rl.is_key_down(key_select) as u8;
        let st = rl.is_key_down(key_start) as u8;

        keys.store(
            dr | (dl << 1) | (du << 2) | (dd << 3)
               | (sa << 4) | (sb << 5) | (sl << 6) | (st << 7),
            Ordering::Relaxed
        );

        if rl.is_key_pressed(key_screenshot) {
            let color_map = if !rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT) {
                PALETTE.iter().flat_map(|v| TryInto::<[u8; 3]>::try_into(&v.to_be_bytes()[..3]).unwrap()).collect::<Vec<u8>>()
            } else {
                vec![0xff, 0xff, 0xff, 0xaa, 0xaa, 0xaa, 0x55, 0x55, 0x55, 0x00, 0x00, 0x00].repeat(4)
            };

            let mut image = std::fs::File::create(&format!("screenshot_{}.gif", std::time::UNIX_EPOCH.elapsed().unwrap().as_millis())).unwrap();
            let mut encoder = gif::Encoder::new(&mut image, 160, 144, &color_map).unwrap();
            let mut frame = gif::Frame::default();
            frame.width = 160;
            frame.height = 144;
            frame.buffer = std::borrow::Cow::Owned((*gb_fb.lock().unwrap()).to_vec());
            encoder.write_frame(&frame).unwrap();
        }

        if rl.is_key_pressed(key_save) {
            SAVE.store(true, Ordering::Relaxed);
        }

        if rl.is_key_pressed(key_pause) {
            _ = STEP_FOR_BURSTS.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(if v == -1 { 0 } else { -1 }));
        }

        if rl.is_key_pressed(key_step_frame) {
            _ = STEP_FOR_BURSTS.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.max(0) + (gb::CLOCK_HZ / 60 / BURST_CYCLES) as isize));
        }

        BURST.store(rl.is_key_down(key_burst), Ordering::Relaxed);
    }

    const PALETTE: &[u32] = &[
        // normal colors
        0xf5faefff,
        0x86c270ff,
        0x2f6957ff,
        0x0b1920ff,
        // debug colors
        0xf50000ff,
        0x860000ff,
        0x2f0000ff,
        0x0b0000ff,

        0x00fa00ff,
        0x00c200ff,
        0x006900ff,
        0x001900ff,

        0xf5fa00ff,
        0x86c200ff,
        0x2f6900ff,
        0x0b1900ff,
    ];

    fn convert(gb_fb: &[u8], fb: &mut [u8]) {
        for (i, c) in gb_fb.iter().enumerate() {
            let c = PALETTE[*c as usize];
            let c = c.to_be_bytes();
            let (_, r) = fb.split_at_mut(i * 4);
            let (l, _) = r.split_at_mut(4);
            l.copy_from_slice(&c);
        }
    }

    SAVE.store(!rl.is_key_down(key_no_save), Ordering::Relaxed);
    while SAVE.load(Ordering::Relaxed) {}
}

fn run_emu(mut gb: gb::Gameboy, save_file: String) {
    use std::time::*;

    let mut dur = Duration::new(0, 0);

    loop {
        let start = Instant::now();
        for _ in 0..BURST_CYCLES { gb.step(); }

        if !BURST.load(Ordering::Relaxed) {
            dur += Duration::from_secs_f64(BURST_CYCLES as f64 / gb::CLOCK_HZ as f64);
            dur = dur.saturating_sub(start.elapsed());

            if Duration::from_secs_f64(BURST_CYCLES as f64 / gb::CLOCK_HZ as f64) < start.elapsed() {
                println!("! {:?}", start.elapsed().saturating_sub(Duration::from_secs_f64(BURST_CYCLES as f64 / gb::CLOCK_HZ as f64)));
            }

            if dur.as_millis() > 5 {
                thread::sleep(dur);
                dur = Duration::new(0, 0);
            }
        }

        if SAVE.load(Ordering::Acquire) {
            if let Some(sram) = gb.get_sram() {
                std::fs::write(&save_file, sram).unwrap();
            }

            SAVE.store(false, Ordering::Release);
            println!("Saved to {save_file}");
        }

        while STEP_FOR_BURSTS.load(Ordering::Relaxed) == 0 { ::core::hint::spin_loop(); }

        if STEP_FOR_BURSTS.load(Ordering::Relaxed) != -1 {
            STEP_FOR_BURSTS.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

fn init(args: &args::Args) -> (Arc<Mutex<[u8]>>, Arc<AtomicU8>) {
    let rom = std::fs::read(&args.rom).unwrap();
    let br = args.boot_rom.as_ref().map(|b| std::fs::read(b).unwrap().into());

    let gb_fb = Mutex::new([0; 160 * 144]).into();
    let keys = Arc::new(AtomicU8::new(0x00));

    let mapper = gb::mapper::Mapper::from_bin(&rom);

    {
        let gb_fb = Arc::clone(&gb_fb);
        let keys = Arc::clone(&keys);
        let save_file = args.save_file.clone().unwrap_or(args.rom.to_string() + ".sav");

        thread::spawn(move || {
            let (_stream, st_handle) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&st_handle).unwrap();

            let mut gb = gb::Gameboy::new(mapper, br, gb_fb, Box::new(|buf| {
                if sink.len() > 3 {
                    for _ in 0..sink.len() { sink.skip_one(); }
                }

                sink.append(rodio::buffer::SamplesBuffer::new(2, gb::apu::SAMPLE_RATE as u32, buf));
            }), keys);

            if let Ok(sav) = std::fs::read(&save_file) {
                gb.set_sram(&sav);
                println!("Restored save file from {save_file}");
            }

            run_emu(gb, save_file);
        });
    }

    (gb_fb, keys)
}

