use std::{sync::{atomic::*, *}, thread};

use clap::Parser;
use raylib::{ffi::Vector2, prelude::*};

mod args;

const BURST_CYCLES: usize = gb::CLOCK_HZ / 120;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .resizable()
        .vsync()
        .build();

    rl.set_exit_key(None);

    let args = args::Args::parse();
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
            frame.buffer = std::borrow::Cow::Owned((*gb_fb.lock().unwrap()).to_vec());
            encoder.write_frame(&frame).unwrap();
        } else if rl.is_key_pressed(KeyboardKey::KEY_Y) {
            SAVE.store(true, Ordering::Relaxed);
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

    fn convert(gb_fb: &[u8], fb: &mut [u8]) {
        for (i, c) in gb_fb.iter().enumerate() {
            let c = PALETTE[*c as usize];
            let c = c.to_be_bytes();
            let (_, r) = fb.split_at_mut(i * 4);
            let (l, _) = r.split_at_mut(4);
            l.copy_from_slice(&c);
        }
    }

    SAVE.store(!rl.is_key_down(KeyboardKey::KEY_BACKSPACE), Ordering::Relaxed);
    while SAVE.load(Ordering::Relaxed) {}
}

static BURST: AtomicBool = AtomicBool::new(false);
static SAVE: AtomicBool = AtomicBool::new(false);

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

            // let mut wav = Vec::<u8>::new();
            // wav.extend(b"RIFF");
            // let file_size_idx = wav.len();
            // wav.extend(0_u32.to_le_bytes());
            // wav.extend(b"WAVE");
            // wav.extend(b"fmt ");
            // wav.extend(16_u32.to_le_bytes());
            // wav.extend(1_u16.to_le_bytes());
            // wav.extend(2_u16.to_le_bytes());
            // wav.extend((gb::apu::SAMPLE_RATE as u32).to_le_bytes());
            // wav.extend((gb::apu::SAMPLE_RATE as u32 * 16 * 2 / 8).to_le_bytes());
            // wav.extend(4_u16.to_le_bytes());
            // wav.extend(16_u16.to_le_bytes());
            // wav.extend(b"data");
            // let data_size_idx = wav.len();
            // wav.extend(0_u32.to_le_bytes());

            let mut gb = gb::Gameboy::new(mapper, br, gb_fb, Box::new(|buf| {
                // if sink.len() > 3 {
                //     for _ in 0..sink.len() { sink.skip_one(); }
                // }

                sink.append(rodio::buffer::SamplesBuffer::new(2, gb::apu::SAMPLE_RATE as u32, buf));

                // wav.extend(buf.iter().flat_map(|v| v.to_le_bytes()));
            }), keys);

            if let Ok(sav) = std::fs::read(&save_file) {
                gb.set_sram(&sav);
                println!("Restored save file from {save_file}");
            }

            run_emu(gb, save_file);

            // let wav_len = wav.len();
            // wav[file_size_idx..file_size_idx + 4].copy_from_slice(&(wav_len as u32).to_le_bytes());
            // wav[data_size_idx..data_size_idx + 4].copy_from_slice(&((wav_len - data_size_idx - 4) as u32).to_le_bytes());
            // std::fs::write("audio.wav", wav).unwrap();
        });
    }

    (gb_fb, keys)
}

