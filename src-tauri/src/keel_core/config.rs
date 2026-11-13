use std::net::{Ipv4Addr, SocketAddr};

pub const DEFAULT_KEEL_MCP_PORT: u16 = 19527;
pub const DEFAULT_KEEL_MCP_PATH: &str = "/mcp";
pub const KEEL_MCP_PORT_ENV: &str = "KEELDESK_MCP_PORT";

#[derive(Debug, Clone)]
pub struct KeelConfig {
    pub bind_ip: Ipv4Addr,
    pub port: u16,
    pub mcp_path: String,
    pub server_name: String,
    pub server_version: String,
    pub allowed_hosts: Vec<String>,
    pub allowed_origins: Vec<String>,
}

impl Default for KeelConfig {
    fn default() -> Self {
        let port = std::env::var(KEEL_MCP_PORT_ENV)
            .ok()
            .and_then(|val| val.trim().parse::<u16>().ok())
            .filter(|&p| p > 0)
            .unwrap_or(DEFAULT_KEEL_MCP_PORT);

        Self {
            bind_ip: Ipv4Addr::LOCALHOST,
            port,
            mcp_path: DEFAULT_KEEL_MCP_PATH.to_string(),
            server_name: "keeldesk".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            allowed_hosts: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "::1".to_string(),
            ],
            allowed_origins: vec![
                "http://localhost:*".to_string(),
                "http://127.0.0.1:*".to_string(),
                "http://[::1]:*".to_string(),
                "https://localhost:*".to_string(),
                "https://127.0.0.1:*".to_string(),
                "tauri://localhost".to_string(),
                "http://tauri.localhost".to_string(),
                "https://tauri.localhost".to_string(),
            ],
        }
    }
}

impl KeelConfig {
    #[cfg(test)]
    pub fn ephemeral_for_test() -> Self {
        Self {
            port: 0,
            ..Self::default()
        }
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from((self.bind_ip, self.port))
    }

    pub fn format_endpoint(&self, bound_port: u16) -> String {
        let normalized_path = if self.mcp_path.starts_with('/') {
            self.mcp_path.as_str()
        } else {
            DEFAULT_KEEL_MCP_PATH
        };
        format!("http://{}:{}{}", self.bind_ip, bound_port, normalized_path)
    }
}
