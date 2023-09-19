use crate::server::tunnel::RumbleTunnel;
use crate::{config::ServerConfig, constants::CLEANUP_INTERVAL};
use anyhow::Result;
use dashmap::DashMap;
use tokio::time::sleep;
use tracing::{error, info};

pub mod address_pool;
pub mod connection;
pub mod tunnel;

/// Rumble server with multiple underlying tunnels.
pub struct RumbleServer {
    active_tunnels: DashMap<String, RumbleTunnel>,
}

impl RumbleServer {
    /// New instance of a server.
    ///
    /// Arguments
    /// `config` - the config for the server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let tunnels = DashMap::new();

        for (name, tunnel_config) in config.tunnels.iter() {
            let tunnel =
                RumbleTunnel::new(name.clone(), tunnel_config.clone(), &config.connection)?;

            tunnels.insert(name.clone(), tunnel);
        }

        Ok(Self {
            active_tunnels: tunnels,
        })
    }

    /// Starts the server and all tunnels
    pub async fn run(&self) -> Result<()> {
        for mut entry in self.active_tunnels.iter_mut() {
            let tunnel = entry.value_mut();

            tunnel.start().await?;
        }

        loop {
            for mut entry in self.active_tunnels.iter_mut() {
                let tunnel_name = entry.key().to_owned();
                let tunnel = entry.value_mut();

                if tunnel.is_ok() {
                    continue;
                }

                error!("Tunnel '{tunnel_name}' encountered an error, restarting...");

                tunnel.stop().await?;
                tunnel.start().await?;

                info!("Tunnel '{tunnel_name}' restarted successfully");
            }

            sleep(CLEANUP_INTERVAL).await;
        }
    }
}