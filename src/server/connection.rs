use crate::auth::server::{AuthServer, AuthState};
use crate::auth::user::UserDatabase;
use crate::config::ConnectionConfig;
use crate::utils::tasks::join_or_abort_task;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use delegate::delegate;
use ipnet::IpNet;

use quinn::Connection;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error};

/// Represents a Rumble connection with authentication and IO.
pub struct RumbleConnection {
    connection: Arc<Connection>,
    auth_server: Arc<RwLock<AuthServer>>,
    tun_queue: Arc<UnboundedSender<Bytes>>,
    tasks: Vec<JoinHandle<Result<()>>>,
}

impl RumbleConnection {
    /// Creates a new instance of the Rumble connection.
    ///
    /// Arguments
    /// `connection` - the underlying QUIC connection
    /// `tun_queue` - the queue to send data to the TUN interface
    /// `user_database` - the user database
    /// `auth_timeout` - the authentication timeout
    /// `client_address` - the assigned client address
    pub async fn new(
        connection: Connection,
        connection_config: &ConnectionConfig,
        tun_queue: Arc<UnboundedSender<Bytes>>,
        user_database: Arc<UserDatabase>,
        client_address: IpNet,
    ) -> Result<Self> {
        let connection = Arc::new(connection);
        let auth_server = AuthServer::new(
            user_database,
            connection.clone(),
            client_address,
            connection_config.timeout,
        )
        .await?;

        Ok(Self {
            connection,
            auth_server: Arc::new(RwLock::new(auth_server)),
            tun_queue,
            tasks: Vec::new(),
        })
    }

    /// Starts the tasks for this instance of Rumble connection.
    pub async fn start(&mut self) -> Result<()> {
        if self.is_ok() {
            return Err(anyhow!(
                "This instance of Rumble VPN connection is already running"
            ));
        }

        self.tasks.push(tokio::spawn(Self::process_incoming_data(
            self.connection.clone(),
            self.tun_queue.clone(),
            self.auth_server.clone(),
        )));

        Ok(())
    }

    /// Stops the tasks for this instance of Rumble connection.
    pub async fn stop(&mut self) -> Result<()> {
        let timeout = Duration::from_secs(1);

        while let Some(task) = self.tasks.pop() {
            if let Some(Err(e)) = join_or_abort_task(task, timeout).await {
                error!("An error occurred in the Rumble connection: {e}")
            }
        }

        Ok(())
    }

    /// Checks if the Rumble connection exists
    ///
    /// Returns
    /// `true` if all connection tasks are running
    pub fn is_ok(&self) -> bool {
        !self.tasks.is_empty() && self.tasks.iter().all(|task| !task.is_finished())
    }

    /// Sends an unreliable datagram to the client.
    ///
    /// Arguments
    /// `data` - the data to be sent
    pub async fn send_datagram(&self, data: Bytes) -> Result<()> {
        match self.auth_server.read().await.get_state().await {
            AuthState::Authenticated(_) => (),
            _ => {
                return Err(anyhow!(
                    "Attempted to send datagram to unauthenticated client {:?}",
                    self.connection.remote_address(),
                ))
            }
        }

        self.connection.send_datagram(data)?;

        Ok(())
    }

    delegate! {
        to self.connection {
            pub fn max_datagram_size(&self) -> Option<usize>;
            pub fn remote_address(&self) -> SocketAddr;
        }
    }

    /// Processes incoming data and sends it to TUN queue
    ///
    /// Arguments
    /// `connection` - a reference to the underlying QUIC connection
    /// `tun_queue` - a sender of an unbounded queue used by the tunnel worker to receive data
    /// `auth_server` - a reference to the authentication server
    async fn process_incoming_data(
        connection: Arc<Connection>,
        tun_queue: Arc<UnboundedSender<Bytes>>,
        auth_server: Arc<RwLock<AuthServer>>,
    ) -> Result<()> {
        Self::handle_authentication(&auth_server).await?;

        loop {
            match auth_server.read().await.get_state().await {
                AuthState::Authenticated(_) => (),
                _ => {
                    return Err(anyhow!(
                        "Connection {:?} not authenticated, dropping incoming data",
                        connection.remote_address(),
                    ))
                }
            }

            let data = connection.read_datagram().await?;
            debug!(
                "Received {} bytes from {:?}",
                data.len(),
                connection.remote_address()
            );

            tun_queue.send(data)?;
        }
    }

    async fn handle_authentication(auth_server: &Arc<RwLock<AuthServer>>) -> Result<()> {
        let mut auth_server = auth_server.write().await;
        auth_server.handle_authentication().await
    }
}