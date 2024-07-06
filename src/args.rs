use clap::Parser;

#[derive(Parser)]
pub struct Args {
    pub rom: String,

    #[arg(short, long)]
    pub boot_rom: Option<String>,

    #[cfg(feature = "console")]
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    pub zoom: u32,

    #[cfg(feature = "raylib")]
    #[arg(long, hide = true)]
    pub waifu: bool,
}
