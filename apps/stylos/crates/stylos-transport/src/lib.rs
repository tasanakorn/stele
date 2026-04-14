//! Locator builders, port-walk, TLS path bundle.

use std::net::{TcpListener, UdpSocket};
use std::path::PathBuf;
use stylos_common::{Result, StylosError};
use stylos_config::ZenohSection;

pub fn listen_endpoints(port: u16, quic_enabled: bool) -> Vec<String> {
    if quic_enabled {
        vec![format!("quic/0.0.0.0:{port}"), format!("tcp/0.0.0.0:{port}")]
    } else {
        vec![format!("tcp/0.0.0.0:{port}")]
    }
}

pub fn walk_available_port(start: u16, cap: u16) -> Result<u16> {
    for p in start..start.saturating_add(cap) {
        let tcp_ok = TcpListener::bind(("0.0.0.0", p)).is_ok();
        let udp_ok = UdpSocket::bind(("0.0.0.0", p)).is_ok();
        if tcp_ok && udp_ok { return Ok(p); }
    }
    Err(StylosError::Transport(format!(
        "no free port in [{start}, {}) for TCP+UDP dual bind", start.saturating_add(cap)
    )))
}

#[derive(Debug, Clone, Default)]
pub struct TlsPaths {
    pub root_ca: Option<PathBuf>,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

impl TlsPaths {
    pub fn from_config(z: &ZenohSection) -> Option<Self> {
        let tls = z.transport.as_ref()?.link.as_ref()?.tls.as_ref()?;
        let paths = Self {
            root_ca: tls.root_ca_certificate.as_ref().map(PathBuf::from),
            cert:    tls.listen_certificate.as_ref().map(PathBuf::from),
            key:     tls.listen_private_key.as_ref().map(PathBuf::from),
        };
        if paths.root_ca.is_none() && paths.cert.is_none() && paths.key.is_none() {
            None
        } else {
            Some(paths)
        }
    }
}
