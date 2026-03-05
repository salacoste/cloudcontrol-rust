use std::net::UdpSocket;
use std::process::Command;

/// Get the host machine's IP address by opening a UDP socket to 8.8.8.8.
pub fn get_host_ip() -> String {
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => {
            // Connect to external address (doesn't actually send data)
            match socket.connect("8.8.8.8:80") {
                Ok(_) => {
                    if let Ok(addr) = socket.local_addr() {
                        if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                            return ipv4.to_string();
                        }
                    }
                }
                Err(_) => {}
            }
        }
        Err(e) => {
            tracing::warn!("[host_ip] Failed to bind socket: {}", e);
        }
    }
    "127.0.0.1".to_string()
}

/// Get local subnets by examining network interfaces.
/// Returns a list of subnet prefixes (e.g., ["192.168.1", "10.0.0"])
pub fn get_local_subnets() -> Vec<String> {
    let mut subnets = Vec::new();

    // Use get_host_ip approach and extract subnet
    let host_ip = get_host_ip();
    if let Some(subnet) = extract_subnet(&host_ip) {
        subnets.push(subnet);
    }

    // Try to get additional subnets from common interface patterns
    // On macOS/Linux, check for additional interfaces
    if let Ok(output) = Command::new("ifconfig").output() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            // Look for inet lines like: inet 192.168.1.100 netmask 0xffffff00
            if line.contains("inet ") && !line.contains("127.0.0.1") {
                if let Some(ip) = extract_ip_from_ifconfig_line(line) {
                    if let Some(subnet) = extract_subnet(&ip) {
                        if !subnets.contains(&subnet) {
                            subnets.push(subnet);
                        }
                    }
                }
            }
        }
    }

    // Fallback: try common subnets if none found
    if subnets.is_empty() {
        subnets = vec![
            "192.168.1".to_string(),
            "192.168.0".to_string(),
            "10.0.0".to_string(),
        ];
    }

    subnets
}

/// Extract IP address from ifconfig output line.
fn extract_ip_from_ifconfig_line(line: &str) -> Option<String> {
    // Parse lines like: inet 192.168.1.100 netmask 0xffffff00
    // or: inet 192.168.1.100 netmask 255.255.255.0
    for part in line.split_whitespace() {
        if part.contains('.') && part.parse::<std::net::Ipv4Addr>().is_ok() {
            return Some(part.to_string());
        }
    }
    None
}

/// Extract subnet prefix from IP address (e.g., "192.168.1.100" -> "192.168.1")
fn extract_subnet(ip: &str) -> Option<String> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() == 4 {
        Some(format!("{}.{}.{}", parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

/// Get the primary host subnet (most likely to contain devices).
pub fn get_primary_subnet() -> String {
    let subnets = get_local_subnets();
    subnets.first().cloned().unwrap_or_else(|| "192.168.1".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_host_ip_not_empty() {
        let ip = get_host_ip();
        assert!(!ip.is_empty(), "Host IP should not be empty");
    }

    #[test]
    fn test_get_host_ip_returns_valid_ip() {
        let ip = get_host_ip();
        let parts: Vec<&str> = ip.split('.').collect();
        assert_eq!(parts.len(), 4, "IP should have 4 octets: {}", ip);
        for part in parts {
            let num: u8 = part.parse().expect("Each octet should be a valid u8");
            assert!(num <= 255);
        }
    }

    #[test]
    fn test_extract_subnet() {
        assert_eq!(
            extract_subnet("192.168.1.100"),
            Some("192.168.1".to_string())
        );
        assert_eq!(extract_subnet("10.0.0.1"), Some("10.0.0".to_string()));
        assert_eq!(extract_subnet("invalid"), None);
        assert_eq!(extract_subnet("192.168.1"), None);
    }

    #[test]
    fn test_get_local_subnets_not_empty() {
        let subnets = get_local_subnets();
        assert!(!subnets.is_empty(), "Should return at least one subnet");
    }

    #[test]
    fn test_get_primary_subnet_valid() {
        let subnet = get_primary_subnet();
        let parts: Vec<&str> = subnet.split('.').collect();
        assert_eq!(parts.len(), 3, "Subnet should have 3 parts");
    }
}
