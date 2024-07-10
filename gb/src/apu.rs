pub const SAMPLE_RATE: usize = 44100;
pub const FRAME_COUNT: usize = 1024;
pub const BUFFER_SIZE: usize = FRAME_COUNT * 2;

const SQ_WAVE_WAVEFORM: [u8; 4] = [0x01, 0x03, 0x0f, 0xfc];

pub type Callback<'a> = Box<dyn FnMut(&[i16]) + 'a>;

pub struct Apu<'a> {
    buffer: [i16; BUFFER_SIZE], // 12 fix point
    buffer_at: usize,
    callback: Callback<'a>,

    output_timer: usize,
    seq_timer: usize,
    last_div_edge: bool,

    enable: bool,

    ch1: Channel1,
    ch2: Channel2,
    ch3: Channel3,
    ch4: Channel4,

    volume: (u8, u8),
    vin_enabled: (bool, bool),
}

#[derive(Default)]
struct Channel1 {
    pub active: bool,
    pub triggered: bool,
    pub hard_pan: (bool, bool),

    pub sweep_pace: u8,
    pub sweep_dir: bool, // false: increase, true: decrease
    pub sweep_step: u8,
    pub sweep_enabled: bool,
    sweep_timer: u8,

    pub duty: u8,
    pub period: u16,
    internal_period: u16,

    pub envelope: Envelope,

    pub length_timer: u8,
    pub length_en: bool,

    freq_timer: u16,
    duty_pos: u8,
}

#[derive(Default)]
struct Channel2 {
    pub active: bool,
    pub triggered: bool,
    pub hard_pan: (bool, bool),

    pub duty: u8,
    pub period: u16,

    pub envelope: Envelope,

    pub length_timer: u8,
    pub length_en: bool,

    freq_timer: u16,
    duty_pos: u8,
}

#[derive(Default)]
struct Channel3 {
    pub active: bool,
    pub dac_enabled: bool,
    pub triggered: bool,
    pub hard_pan: (bool, bool),

    pub period: u16,

    pub out_level: u8,
    pub wave: [u8; 16],

    pub length_timer: u16,
    pub length_en: bool,

    freq_timer: u16,
    wave_pos: usize,
}

#[derive(Default)]
struct Channel4 {
    pub active: bool,
    pub triggered: bool,
    pub hard_pan: (bool, bool),

    pub clock_shift: u8,
    pub clock_div: u8,
    pub width: bool,

    pub envelope: Envelope,

    pub length_timer: u8,
    pub length_en: bool,

    freq_timer: u16,
    lfsr: u16,
}

#[derive(Default)]
struct Envelope {
    pub init_vol: u8,
    pub env_dir: bool, // false = decrease, true = increase
    pub pace: u8,

    pub volume: u8,
    pub pace_timer: u8,
}

impl<'a> Apu<'a> {
    pub fn new(callback: Callback<'a>) -> Self {
        Self {
            buffer: [0; BUFFER_SIZE],
            buffer_at: 0,
            callback,

            output_timer: 0,
            seq_timer: 0,
            last_div_edge: false,

            enable: false,

            ch1: Channel1::default(),
            ch2: Channel2::default(),
            ch3: Channel3::default(),
            ch4: Channel4::default(),

            volume: (0, 0),
            vin_enabled: (false, false),
        }
    }

    pub fn step(&mut self, div_edge: bool) {
        if !self.enable {
            return self.write(|_| (0, 0));
        }

        self.ch1.step();
        self.ch2.step();
        self.ch3.step();
        self.ch4.step();

        if core::mem::replace(&mut self.last_div_edge, div_edge) && !div_edge {
            self.seq_timer = (self.seq_timer + 1) % 8;
            let (len, env, sweep) = [
                (true, false, false),
                (false, false, false),
                (true, false, true),
                (false, false, false),
                (true, false, false),
                (false, false, false),
                (true, false, true),
                (false, true, false),
            ][self.seq_timer];

            if len {
                self.ch1.step_len();
                self.ch2.step_len();
                self.ch3.step_len();
                self.ch4.step_len();
            }

            if env {
                self.ch1.envelope.step();
                self.ch2.envelope.step();
                self.ch4.envelope.step();
            }

            if sweep {
                self.ch1.step_sweep();
            }
        }

        self.write(|s| {
            let c1 = s.ch1.get_amp();
            let l1 = if s.ch1.hard_pan.0 { c1 } else { 0 };
            let r1 = if s.ch1.hard_pan.1 { c1 } else { 0 };

            let c2 = s.ch2.get_amp();
            let l2 = if s.ch2.hard_pan.0 { c2 } else { 0 };
            let r2 = if s.ch2.hard_pan.1 { c2 } else { 0 };

            let c3 = s.ch3.get_amp();
            let l3 = if s.ch3.hard_pan.0 { c3 } else { 0 };
            let r3 = if s.ch3.hard_pan.1 { c3 } else { 0 };

            let c4 = s.ch4.get_amp();
            let l4 = if s.ch4.hard_pan.0 { c4 } else { 0 };
            let r4 = if s.ch4.hard_pan.1 { c4 } else { 0 };

            (l1 + l2 + l3 + l4, r1 + r2 + r3 + r4)
        });
    }

    fn write(&mut self, cb: fn(&mut Self) -> (i16, i16)) {
        self.output_timer += 1;
        if self.output_timer % (crate::CLOCK_HZ / SAMPLE_RATE) == 0 {
            let (l, r) = cb(self);
            self.buffer[self.buffer_at] = l;
            self.buffer[self.buffer_at + 1] = r;
            self.buffer_at += 2;

            if self.buffer_at >= BUFFER_SIZE {
                (self.callback)(&self.buffer);
                self.buffer_at = 0;
            }
        }
    }

    pub fn load(&mut self, addr: u16) -> u8 {
        match addr {
            0xff26 => { // NR52
                ((self.enable as u8) << 7)
                    | ((self.ch4.active as u8) << 3)
                    | ((self.ch3.active as u8) << 2)
                    | ((self.ch2.active as u8) << 1)
                    | (self.ch1.active as u8)
                    | 0x70
            },
            0xff25 => { // NR51
                ((self.ch4.hard_pan.0 as u8) << 7)
                    | ((self.ch4.hard_pan.1 as u8) << 3)
                    | ((self.ch3.hard_pan.0 as u8) << 6)
                    | ((self.ch3.hard_pan.1 as u8) << 2)
                    | ((self.ch2.hard_pan.0 as u8) << 5)
                    | ((self.ch2.hard_pan.1 as u8) << 1)
                    | ((self.ch1.hard_pan.0 as u8) << 4)
                    | (self.ch1.hard_pan.1 as u8)
            },
            0xff24 => { // NR50
                ((self.vin_enabled.0 as u8) << 7)
                    | ((self.vin_enabled.1 as u8) << 3)
                    | (self.volume.0 << 4)
                    | self.volume.1
            },

            0xff10 => { // NR10
                (self.ch1.sweep_pace << 4)
                    | ((self.ch1.sweep_dir as u8) << 3)
                    | self.ch1.sweep_step
                    | 0x80
            },
            0xff11 => (self.ch1.duty << 6) | 0x3f, // NR11
            0xff12 => self.ch1.envelope.to_bits(), // NR12
            0xff14 => ((self.ch1.length_en as u8) << 6) | 0xbf, // NR14

            0xff16 => (self.ch2.duty << 6) | 0x3f, // NR21
            0xff17 => self.ch2.envelope.to_bits(), // NR22
            0xff19 => ((self.ch2.length_en as u8) << 6) | 0xbf, // NR24

            0xff1a => ((self.ch3.dac_enabled as u8) << 7) | 0x7f, // NR30
            0xff1c => (self.ch3.out_level << 5) | 0x9f, // NR32
            0xff1e => ((self.ch3.length_en as u8) << 6) | 0xbf, // NR34
            0xff30..=0xff3f => self.ch3.wave[(addr - 0xff30) as usize],

            0xff21 => self.ch4.envelope.to_bits(), // NR42
            0xff22 => { // NR43
                (self.ch4.clock_shift << 4)
                    | ((self.ch4.width as u8) << 3)
                    | self.ch4.clock_div
            },
            0xff23 => ((self.ch4.length_en as u8) << 6) | 0xbf, // NR44

            _ => 0xff,
        }
    }

    pub fn store(&mut self, addr: u16, data: u8) {
        if !self.enable && !matches!(addr, 0xff26 | 0xff30..=0xff3f) { return; }

        match addr {
            0xff26 => { // NR52
                self.enable = data & 0x80 != 0;

                if !self.enable {
                    self.ch1 = Channel1::default();
                    self.ch2 = Channel2::default();
                    let wave = self.ch3.wave;
                    self.ch3 = Channel3::default();
                    self.ch3.wave = wave;
                    self.ch4 = Channel4::default();

                    self.volume = (0, 0);
                    self.vin_enabled = (false, false);
                }
            },
            0xff25 => { // NR51
                self.ch4.hard_pan.0 = data & 0x80 != 0;
                self.ch4.hard_pan.1 = data & 0x08 != 0;
                self.ch3.hard_pan.0 = data & 0x40 != 0;
                self.ch3.hard_pan.1 = data & 0x04 != 0;
                self.ch2.hard_pan.0 = data & 0x20 != 0;
                self.ch2.hard_pan.1 = data & 0x02 != 0;
                self.ch1.hard_pan.0 = data & 0x10 != 0;
                self.ch1.hard_pan.1 = data & 0x01 != 0;
            },
            0xff24 => { // NR50
                self.volume.0 = (data >> 4) & 7;
                self.volume.1 = data & 7;
                self.vin_enabled.0 = (data & 0x80) != 0;
                self.vin_enabled.1 = (data & 0x08) != 0;
            },

            0xff10 => { // NR10
                self.ch1.sweep_pace = (data & 0x70) >> 4;
                self.ch1.sweep_dir = data & 8 != 0;
                self.ch1.sweep_step = data & 7;
            },
            0xff11 => { // NR11
                self.ch1.duty = data >> 6;
                self.ch1.length_timer = 64 - (data & 0x3f);
                if self.ch1.length_timer == 0 { self.ch1.length_timer = 64; }
            },
            0xff12 => self.ch1.active &= self.ch1.envelope.update_from_bits(data), // NR12
            0xff13 => { // NR13
                self.ch1.period &= !0xff;
                self.ch1.period |= data as u16;
            },
            0xff14 => { // NR14
                self.ch1.period &= 0xff;
                self.ch1.period |= (data as u16 & 7) << 8;
                self.ch1.length_en = data & 0x40 != 0;
                self.ch1.triggered = data & 0x80 != 0;
            },

            0xff16 => { // NR21
                self.ch2.duty = data >> 6;
                self.ch2.length_timer = 64 - (data & 0x3f);
                if self.ch2.length_timer == 0 { self.ch2.length_timer = 64; }
            },
            0xff17 => self.ch2.active &= self.ch2.envelope.update_from_bits(data), // NR22
            0xff18 => { // NR23
                self.ch2.period &= !0xff;
                self.ch2.period |= data as u16;
            },
            0xff19 => { // NR24
                self.ch2.period &= 0xff;
                self.ch2.period |= (data as u16 & 7) << 8;
                self.ch2.length_en = data & 0x40 != 0;
                self.ch2.triggered = data & 0x80 != 0;
            },

            0xff1a => { // NR30
                self.ch3.dac_enabled = data & 0x80 != 0;
                self.ch3.active &= self.ch3.dac_enabled;
            },
            0xff1b => { // NR31
                self.ch3.length_timer = 256 - data as u16;
                if self.ch3.length_timer == 0 { self.ch3.length_timer = 256; }
            },
            0xff1c => self.ch3.out_level = (data & 0x60) >> 5, // NR32
            0xff1d => { // NR33
                self.ch3.period &= !0xff;
                self.ch3.period |= data as u16;
            },
            0xff1e => { // NR34
                self.ch3.period &= 0xff;
                self.ch3.period |= (data as u16 & 7) << 8;
                self.ch3.length_en = data & 0x40 != 0;
                self.ch3.triggered = data & 0x80 != 0;
            },
            0xff30..=0xff3f => self.ch3.wave[(addr - 0xff30) as usize] = data,

            0xff20 => { // NR41
                self.ch4.length_timer = 64 - (data & 0x3f);
                if self.ch4.length_timer == 0 { self.ch4.length_timer = 64; }
            },
            0xff21 => self.ch4.active &= self.ch4.envelope.update_from_bits(data), // NR42
            0xff22 => { // NR43
                self.ch4.clock_shift = data >> 4;
                self.ch4.width = data & 8 != 0;
                self.ch4.clock_div = data & 7;
            },
            0xff23 => { // NR44
                self.ch4.length_en = data & 0x40 != 0;
                self.ch4.triggered = data & 0x80 != 0;
            },

            _ => {},
        }
    }
}

impl Envelope {
    pub fn to_bits(&self) -> u8 {
        (self.init_vol << 4) | ((self.env_dir as u8) << 3) | self.pace
    }

    pub fn update_from_bits(&mut self, data: u8) -> bool {
        self.init_vol = data >> 4;
        self.env_dir = data & 8 != 0;
        self.pace = data & 7;

        self.volume = self.init_vol;

        data & 0xf8 != 0
    }

    fn step(&mut self) {
        if self.pace != 0 {
            if self.pace_timer > 0 {
                self.pace_timer -= 1;
            }

            if self.pace_timer != 0 { return; }

            self.pace_timer = self.pace;

            if self.volume < 0xf && self.env_dir {
                self.volume += 1;
            } else if self.volume > 0x0 && !self.env_dir {
                self.volume -= 1;
            }
        }
    }
}

impl Channel1 {
    fn step(&mut self) {
        if core::mem::replace(&mut self.triggered, false) && self.dac_enabled() {
            self.active = true;
            self.envelope.pace_timer = self.envelope.pace;
            self.envelope.volume = self.envelope.init_vol;
            if self.length_timer == 0 { self.length_timer = 64; }

            self.internal_period = self.period;
            self.sweep_enabled = self.sweep_pace != 0 || self.sweep_step != 0;
            if self.sweep_step != 0 { self.calculate_sweep_next_freq(); }
        }

        if self.active {
            self.freq_timer -= 1;
            if self.freq_timer == 0 {
                self.freq_timer = (2048 - self.internal_period) * 4;
                self.duty_pos = (self.duty_pos + 1) % 8;
            }
        }
    }

    fn step_len(&mut self) {
        if self.length_en {
            self.length_timer = self.length_timer.saturating_sub(1);
            self.active &= self.length_timer != 0;
        }
    }

    fn step_sweep(&mut self) {
        self.sweep_timer = self.sweep_timer.saturating_sub(1);
        if self.sweep_timer != 0 { return; }

        self.sweep_timer = if self.sweep_pace != 0 { self.sweep_pace } else { 8 };

        if self.sweep_enabled && self.sweep_pace != 0 {
            self.calculate_sweep_next_freq().map(|p| {
                if self.sweep_step != 0 {
                    self.period = p;
                    self.internal_period = p;
                    self.calculate_sweep_next_freq();
                }
            });
        }
    }

    fn calculate_sweep_next_freq(&mut self) -> Option<u16> {
        let mod_freq = self.internal_period >> self.sweep_step;
        let new = if !self.sweep_dir { self.internal_period + mod_freq } else { self.internal_period - mod_freq };

        if new <= 0x7ff {
            Some(new)
        } else {
            self.active = false;
            None
        }
    }

    fn get_amp(&self) -> i16 {
        if !self.active || !self.dac_enabled() { return 0; }

        let amp = ((SQ_WAVE_WAVEFORM[self.duty as usize] >> self.duty_pos) & 1) * self.envelope.volume;
        
        amp as i16 * 0x222 - 0x1000
    }

    fn dac_enabled(&self) -> bool { self.envelope.init_vol != 0 || self.envelope.env_dir }
}

impl Channel2 {
    fn step(&mut self) {
        if core::mem::replace(&mut self.triggered, false) && self.dac_enabled() {
            self.active = true;
            self.envelope.pace_timer = self.envelope.pace;
            self.envelope.volume = self.envelope.init_vol;
            if self.length_timer == 0 { self.length_timer = 64; }
        }

        if self.active {
            self.freq_timer -= 1;
            if self.freq_timer == 0 {
                self.freq_timer = (2048 - self.period) * 4;
                self.duty_pos = (self.duty_pos + 1) % 8;
            }
        }
    }

    fn step_len(&mut self) {
        if self.length_en {
            self.length_timer = self.length_timer.saturating_sub(1);
            self.active &= self.length_timer != 0;
        }
    }

    fn get_amp(&self) -> i16 {
        if !self.active || !self.dac_enabled() { return 0; }

        let amp = ((SQ_WAVE_WAVEFORM[self.duty as usize] >> self.duty_pos) & 1) * self.envelope.volume;
        
        amp as i16 * 0x222 - 0x1000
    }

    fn dac_enabled(&self) -> bool { self.envelope.init_vol != 0 || self.envelope.env_dir }
}

impl Channel3 {
    fn step(&mut self) {
        if core::mem::replace(&mut self.triggered, false) && self.dac_enabled {
            self.active = true;
            if self.length_timer == 0 { self.length_timer = 256; }
        }

        if self.active {
            self.freq_timer -= 1;
            if self.freq_timer == 0 {
                self.freq_timer = (2048 - self.period) * 4;
                self.wave_pos = (self.wave_pos + 1) % 32;
            }
        }
    }

    fn step_len(&mut self) {
        if self.length_en {
            self.length_timer = self.length_timer.saturating_sub(1);
            self.active &= self.length_timer != 0;
        }
    }

    fn get_amp(&self) -> i16 {
        if !self.active || !self.dac_enabled { return 0; }

        let wa = self.wave[self.wave_pos >> 1];
        let wa = if self.wave_pos & 1 == 0 { wa >> 4 } else { wa & 0xf };

        let amp = wa >> [4, 0, 1, 2][self.out_level as usize];
        
        amp as i16 * 0x222 - 0x1000
    }
}

impl Channel4 {
    fn step(&mut self) {
        if core::mem::replace(&mut self.triggered, false) && self.dac_enabled() {
            self.active = true;
            self.envelope.pace_timer = self.envelope.pace;
            self.envelope.volume = self.envelope.init_vol;
            if self.length_timer == 0 { self.length_timer = 64; }
            self.lfsr = 0x7fff;
        }

        if self.active {
            self.freq_timer -= 1;
            if self.freq_timer == 0 {
                self.freq_timer = [8, 16, 32, 48, 64, 80, 96, 112][self.clock_div as usize] << self.clock_shift;
                let xor = (self.lfsr & 1) ^ ((self.lfsr & 2) >> 1);
                self.lfsr = (self.lfsr >> 1) | (xor << 14);

                if self.width {
                    self.lfsr &= !(1 << 6);
                    self.lfsr |= xor << 6;
                }
            }
        }
    }

    fn step_len(&mut self) {
        if self.length_en {
            self.length_timer = self.length_timer.saturating_sub(1);
            self.active &= self.length_timer != 0;
        }
    }

    fn get_amp(&self) -> i16 {
        if !self.active || !self.dac_enabled() { return 0; }

        let amp = (self.lfsr as u8 & 1) * self.envelope.volume;
        
        amp as i16 * 0x222 - 0x1000
    }

    fn dac_enabled(&self) -> bool { self.envelope.init_vol != 0 || self.envelope.env_dir }
}
