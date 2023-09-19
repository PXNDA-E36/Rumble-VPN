use anyhow::Result;
use clap::Parser;
use rumble::client::RumbleClient;
use rumble::config::{ClientConfig, FromPath};
use rumble::utils::cli::Args;
use rumble::utils::tracing::enable_tracing;
use tracing::error;

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    match run_client(args).await {
        Ok(_) => {}
        Err(e) => error!("A critical error occurred: {e}"),
    }
}

///Runs the Rumble client.
async fn run_client(args: Args) -> Result<()> {
    let config = ClientConfig::from_path(&args.config_path, &args.env_prefix)?;
    enable_tracing(&config.log.level);

    let client = RumbleClient::new(config);
    client.run().await
}