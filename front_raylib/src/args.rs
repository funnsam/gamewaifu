use clap::Parser;

#[derive(Parser)]
pub struct Args {
    pub rom: String,

    #[arg(short, long)]
    pub boot_rom: Option<String>,

    #[arg(long, hide = true)]
    pub waifu: bool,

    #[arg(long)]
    pub run_for: Option<usize>,
}
