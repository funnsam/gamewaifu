pub const SAMPLE_RATE: usize = 48000;
pub const BUFFER_SIZE: usize = 1024;

const SQ_WAVE_WAVEFORM: [u8; 4] = [0x01, 0x03, 0x0f, 0xfc];

pub type Callback<'a> = Box<dyn FnMut(&[i16]) + 'a>;

pub struct Apu<'a> {
    buffer: [i16; BUFFER_SIZE],
    buffer_at: usize,
    callback: Callback<'a>,

    output_timer: usize,

    enable: bool,

    ch1: Channel1,
    ch2: Channel2,
    ch3: Channel3,
    ch4: Channel4,

    volume: (u8, u8),
}

#[derive(Default)]
struct Channel1 {
    pub active: bool,
    pub triggered: bool,
    pub hard_pan: (bool, bool),

    pub duty: u8,
    pub timer: u8,
    pub envelope: Envelope,
    pub period: u16,
    pub length_en: bool,

    freq_timer: u16,
    duty_pos: u8,
}

#[derive(Default)]
struct Channel2 {
    pub active: bool,
    pub hard_pan: (bool, bool),
}

#[derive(Default)]
struct Channel3 {
    pub active: bool,
    pub hard_pan: (bool, bool),
}

#[derive(Default)]
struct Channel4 {
    pub active: bool,
    pub hard_pan: (bool, bool),
}

#[derive(Default)]
struct Envelope {
    pub init_vol: u8,
    pub env_dir: bool, // false = decrease, true = increase
    pub pace: u8,
}

impl<'a> Apu<'a> {
    pub fn new(callback: Callback<'a>) -> Self {
        Self {
            buffer: [0; BUFFER_SIZE],
            buffer_at: 0,
            callback,

            output_timer: 0,

            enable: false,
            volume: (0, 0),

            ch1: Channel1::default(),
            ch2: Channel2::default(),
            ch3: Channel3::default(),
            ch4: Channel4::default(),
        }
    }

    pub fn step(&mut self) {
        if !self.enable {
            return self.write(|_| (0, 0));
        }

        self.ch1.step();

        self.write(|s| {
            let (l1, r1) = s.ch1.get_amp();
            let l1 = if s.ch1.hard_pan.0 { l1 } else { 0 };
            let r1 = if s.ch1.hard_pan.1 { r1 } else { 0 };

            (l1, r1)
        });
    }

    fn write(&mut self, cb: fn(&mut Self) -> (i16, i16)) {
        self.output_timer += 1;
        if self.output_timer % (crate::CLOCK_HZ / SAMPLE_RATE) == 0 {
            let (l, r) = cb(self);
            self.buffer[self.buffer_at + 0] = l * 0x3fff; // NOTE: remove these when have more
                                                          // sounds
            self.buffer[self.buffer_at + 1] = r * 0x3fff;
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
                    | ((self.ch1.active as u8) << 0)
            },
            0xff25 => { // NR51
                0
                    | ((self.ch4.hard_pan.0 as u8) << 7)
                    | ((self.ch4.hard_pan.1 as u8) << 3)
                    | ((self.ch3.hard_pan.0 as u8) << 6)
                    | ((self.ch3.hard_pan.1 as u8) << 2)
                    | ((self.ch2.hard_pan.0 as u8) << 5)
                    | ((self.ch2.hard_pan.1 as u8) << 1)
                    | ((self.ch1.hard_pan.0 as u8) << 4)
                    | ((self.ch1.hard_pan.1 as u8) << 0)
            },
            0xff24 => (self.volume.0 << 4) | self.volume.1, // NR50

            0xff10 => 0, // NR10
                         // TODO: sweep
            0xff11 => self.ch1.duty << 6, // NR11
            0xff12 => self.ch1.envelope.to_bits(),
            0xff13 => 0,
            0xff14 => ((self.ch1.triggered as u8) << 7) | ((self.ch1.length_en as u8) << 6), // NR14
            _ => 0xff,
        }
    }

    pub fn store(&mut self, addr: u16, data: u8) {
        match addr {
            0xff26 => self.enable = data & 0x80 != 0, // NR52
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
                self.volume.1 = (data >> 0) & 7;
            },
            0xff10 => {}, // NR10
                          // TODO: sweep
            0xff11 => { // NR11
                self.ch1.duty = data >> 6;
                self.ch1.timer = data & 0x3f;
            },
            0xff12 => self.ch1.envelope.update_from_bits(data), // NR12
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
            _ => {},
        }
    }
}

impl Envelope {
    pub fn to_bits(&self) -> u8 {
        (self.init_vol << 4) | ((self.env_dir as u8) << 3) | self.pace
    }

    pub fn update_from_bits(&mut self, data: u8) {
        self.init_vol = data >> 4;
        self.env_dir = data & 8 != 0;
        self.pace = data & 7;
    }
}

impl Channel1 {
    fn step(&mut self) {
        if core::mem::replace(&mut self.triggered, false) {
            self.active = true;
        }

        if self.active {
            self.freq_timer -= 1;
            if self.freq_timer == 0 {
                self.freq_timer = (2048 - self.period) * 4;
                self.duty_pos = (self.duty_pos + 1) % 8;
            }
        }
    }

    fn get_amp(&self) -> (i16, i16) {
        if !self.active { return (0, 0); }

        let amp = (SQ_WAVE_WAVEFORM[self.duty as usize] >> self.duty_pos) & 1;
        (amp as i16, amp as i16)
    }
}
