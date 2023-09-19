use anyhow::Result;
use clap::Parser;
use rumble::config::{FromPath, ServerConfig};
use rumble::server::RumbleServer;
use rumble::utils::cli::Args;
use rumble::utils::tracing::enable_tracing;
use tracing::error;

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    match run_server(args).await {
        Ok(_) => {}
        Err(e) => error!("A critical error occurred: {e}"),
    }
}

///Runs the Rumble server.
async fn run_server(args: Args) -> Result<()> {
    let config = ServerConfig::from_path(&args.config_path, &args.env_prefix)?;
    enable_tracing(&config.log.level);

    let server = RumbleServer::new(config).await?;
    server.run().await
}