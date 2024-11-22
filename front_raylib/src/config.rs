use serde::Deserialize;

pub fn get_config() -> Config {
    if let Ok(config) = std::fs::read_to_string("config.toml") {
        toml::from_str(&config).unwrap()
    } else {
        Config::default()
    }
}

#[derive(Deserialize, Default)]
pub struct Config {
    pub inputs: Inputs,
}

#[derive(Deserialize)]
/// For key IDs, refer to https://github.com/raysan5/raylib/blob/47f83aa58f7a20110b0dc0d031b377faa50dd31e/src/raylib.h#L577///
pub struct Inputs {
    pub u: i32,
    pub l: i32,
    pub d: i32,
    pub r: i32,

    pub a: i32,
    pub b: i32,

    pub select: i32,
    pub start: i32,

    pub save: i32,
    pub no_save: i32,
    pub screenshot: i32,
    pub pause: i32,
    pub step_frame: i32,
    pub burst: i32,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            //   W
            // A   D       O
            //   S       I
            //
            //      V  B
            u: 87,
            l: 65,
            d: 83,
            r: 68,

            a: 79,
            b: 73,

            select: 86,
            start: 66,

            save: 89,
            no_save: 259,
            screenshot: 84,
            pause: 80,
            step_frame: 91,
            burst: 257,
        }
    }
}
