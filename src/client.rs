use crate::auth::client::AuthClient;

use crate::config::ClientConfig;
use crate::constants::QUINN_RUNTIME;
use crate::utils::socket::bind_socket;
use anyhow::{anyhow, Result};
use quinn::{Connection, Endpoint};

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};

use crate::utils::interface::{read_from_interface, set_up_interface, write_to_interface};
use std::sync::Arc;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::try_join;
use tracing::{debug, info, warn};
use tun::AsyncDevice;

/// Rumble client that connects to a server and relays packets between the server and a TUN interface
pub struct RumbleClient {
    client_config: ClientConfig,
}

impl RumbleClient {
    /// New instance of a Rumble client
    ///
    /// Arguments
    /// `client_config` - config for client
    pub fn new(client_config: ClientConfig) -> Self {
        Self { client_config }
    }

    /// Connects to the server and starts the workers
    pub async fn run(&self) -> Result<()> {
        let connection = self.connect_to_server().await?;
        let mut auth_client =
            AuthClient::new(&connection, &self.client_config.authentication).await?;

        let assigned_address = auth_client.authenticate().await?;

        info!("Received client address: {assigned_address}");

        let interface = set_up_interface(assigned_address, self.client_config.connection.mtu)?;

        self.relay_packets(
            connection,
            interface,
            self.client_config.connection.mtu as usize,
        )
        .await?;

        Ok(())
    }

    /// Connects to the Rumble server.
    ///
    /// Returns
    /// `Connection` - connection representing the connection to the server
    async fn connect_to_server(&self) -> Result<Connection> {
        let quinn_config = self.client_config.as_quinn_client_config()?;
        let endpoint = self.create_quinn_endpoint()?;

        let server_hostname = self
            .client_config
            .connection_string
            .split(':')
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "Could not parse hostname from connection string '{}'",
                    self.client_config.connection_string
                )
            })?;

        let server_addr = self
            .client_config
            .connection_string
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "Connection string '{}' is invalid",
                    self.client_config.connection_string
                )
            })?;

        info!("Connecting: {}", self.client_config.connection_string);

        let connection = endpoint
            .connect_with(quinn_config, server_addr, server_hostname)?
            .await?;

        info!(
            "Connection established: {}",
            self.client_config.connection_string
        );

        Ok(connection)
    }

    /// Creates a Quinn endpoint.
    ///
    /// Returns
    /// `Endpoint` - Quinn endpoint
    fn create_quinn_endpoint(&self) -> Result<Endpoint> {
        let bind_addr: SocketAddr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        debug!("QUIC socket local address: {:?}", bind_addr);

        let socket = bind_socket(
            bind_addr,
            self.client_config.connection.send_buffer_size as usize,
            self.client_config.connection.recv_buffer_size as usize,
        )?;

        let endpoint_config = self.client_config.connection.as_endpoint_config()?;
        let endpoint = Endpoint::new(endpoint_config, None, socket, QUINN_RUNTIME.clone())?;

        Ok(endpoint)
    }

    /// Relays packets between the TUN interface and the Rumble server
    ///
    /// Arguments
    /// `connection` - Quinn connection representing the connection to the server
    /// `interface` - TUN interface
    async fn relay_packets(
        &self,
        connection: Connection,
        interface: AsyncDevice,
        interface_mtu: usize,
    ) -> Result<()> {
        let connection = Arc::new(connection);
        let (read, write) = tokio::io::split(interface);

        let (outbound_task, inbound_task) = try_join!(
            tokio::spawn(Self::process_outbound_traffic(
                connection.clone(),
                read,
                interface_mtu
            )),
            tokio::spawn(Self::process_inbound_traffic(
                connection.clone(), 
                write,
            )),
        )?;

        inbound_task?;
        outbound_task?;

        Ok(())
    }

    /// Handles incoming packets from the TUN interface and relays them to the server
    ///
    /// Arguments
    /// `connection` - Quinn connection representing the connection to the server
    /// `read_interface` - read half of the TUN interface
    /// `interface_mtu` - MTU of the TUN interface
    async fn process_outbound_traffic(
        connection: Arc<Connection>,
        mut read_interface: ReadHalf<AsyncDevice>,
        interface_mtu: usize,
    ) -> Result<()> {
        debug!("Started outbound traffic task (interface -> QUIC tunnel)");

        loop {
            let quinn_mtu = connection
                .max_datagram_size()
                .ok_or_else(|| anyhow!("The Rumble server does not support datagram transfer"))?;

            let data = read_from_interface(&mut read_interface, interface_mtu).await?;

            if data.len() > quinn_mtu {
                warn!(
                    "Dropping packet of size {} due to maximum datagram size being {}",
                    data.len(),
                    quinn_mtu
                );
                continue;
            }

            debug!(
                "Sending {} bytes to {:?}",
                data.len(),
                connection.remote_address()
            );

            connection.send_datagram(data)?;
        }
    }

    /// Handles incoming packets from the Rumble server and relays them to the TUN interface.
    ///
    /// Arguments
    /// `connection` - Quinn connection representing the connection to the server
    /// `write_interface` - write half of the TUN interface
    async fn process_inbound_traffic(
        connection: Arc<Connection>,
        mut write_interface: WriteHalf<AsyncDevice>,
    ) -> Result<()> {
        debug!("Started inbound traffic task (QUIC tunnel -> interface)");

        loop {
            let data = connection.read_datagram().await?;

            debug!(
                "Received {} bytes from {:?}",
                data.len(),
                connection.remote_address()
            );

            write_to_interface(&mut write_interface, data).await?;
        }
    }
}