use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rumble")]
pub struct Args {
    #[arg(long)]
    pub config_path: PathBuf,
    #[arg(long, default_value = "RUMBLE_")]
    pub env_prefix: String,
}