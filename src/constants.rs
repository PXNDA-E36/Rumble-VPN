use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use quinn::Runtime;

/// Size of the buffer used for bincode (de)serialization
pub const BINCODE_BUFFER_SIZE: usize = 128;

/// Grace interval to add to the auth_timeout variable used for timing out a connection
pub const AUTH_TIMEOUT_GRACE: u64 = 5;

/// Size of an `Ipv4Addr` address
pub const IPV4_ADDR_SIZE: usize = std::mem::size_of::<Ipv4Addr>();

/// Size of an `Ipv6Addr` address
pub const IPV6_ADDR_SIZE: usize = std::mem::size_of::<Ipv6Addr>();

/// Default MTU overhead for QUIC
pub const QUIC_MTU_OVERHEAD: u16 = 42;

/// Interval used by various cleanup tasks.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(1);

/// Supported TLS cipher suites for Rumble VPN
pub static RUMBLE_CIPHER_SUITES: &[rustls::SupportedCipherSuite] = &[
    rustls::cipher_suite::TLS13_AES_256_GCM_SHA384,
    rustls::cipher_suite::TLS13_CHACHA20_POLY1305_SHA256,
];

/// Supported TLS protocol versions for Rumble VPN
pub static TLS_PROTOCOL_VERSIONS: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

/// Supported TLS ALPN protocols for Rumble VPN
pub static TLS_ALPN_PROTOCOLS: Lazy<Vec<Vec<u8>>> = Lazy::new(|| vec![b"rumble".to_vec()]);

/// Async runtime used by Quinn
pub static QUINN_RUNTIME: Lazy<Arc<dyn Runtime>> = Lazy::new(|| Arc::new(quinn::TokioRuntime));