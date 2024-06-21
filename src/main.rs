use std::{sync::{atomic::*, Arc, Mutex}, thread::sleep};

use raylib::{ffi::Vector2, prelude::*};

mod gb;

fn main() {
    let rom = std::env::args().nth(1).unwrap();
    let rom = std::fs::read(rom).unwrap();

    let ppu = ppu::Ppu::new();
    let bus = Arc::new(Mutex::new(gb::bus::Bus {
        ppu,
        mapper: gb::mapper::Mapper {
            rom,
            ram: Vec::new(),
        },
        wram: [0; 0x2000],
        hram: [0; 0x7f],
    }));
    let cpu = sm83::Sm83::new(bus);

    let (mut rl, thread) = raylib::init()
        .size(640, 570)
        .title("Gamewaifu")
        .build();

    let mut gb_fb = vec![0; 160 * 144];
    let mut fb = vec![0; 160 * 144 * 4];
    let mut rl_fb = rl.load_render_texture(&thread, 160, 144).unwrap();

    GB_FB.store(gb_fb.as_mut_ptr(), Ordering::Relaxed);
    std::thread::spawn(|| run_emu(cpu));

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        convert(&gb_fb, &mut fb);
        rl_fb.update_texture(&fb);

        d.clear_background(Color::BLACK);
        d.draw_texture_ex(&rl_fb, Vector2 { x: 0.0, y: 0.0 }, 0.0, 4.0, Color::WHITE);
        d.draw_fps(0, 0);
    }
}

static GB_FB: AtomicPtr<u8> = AtomicPtr::new(::core::ptr::null_mut());

fn run_emu(mut cpu: sm83::Sm83<Arc<Mutex<gb::bus::Bus>>>) {
    use std::time::*;

    let gb_fb = unsafe {
        let gb_fb = GB_FB.load(Ordering::Relaxed);
        ::core::slice::from_raw_parts_mut(gb_fb, 160 * 144)
    };

    let mut hsync = 0; // count to 456
    let mut scanline = 0;

    loop {
        let start = Instant::now();

        cpu.step();

        if hsync >= 456 {
            cpu.bus.lock().unwrap().ppu.render_strip(gb_fb, 0);
            hsync = 0;
            scanline = (scanline + 1) % 153;
        }

        hsync += 1;

        let dur = start.elapsed().saturating_sub(Duration::from_secs_f64(1e6 / 4.194304));
        sleep(dur);
    }
}

const PALETTE: [u32; 4] = [
    0xf5faefff,
    0x86c270ff,
    0x2f6957ff,
    0x0b1920ff,
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
