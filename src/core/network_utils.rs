// core/network_utils.rs
// Single source of truth for macOS IP and MAC detection.

use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use crate::core::file_logger::FileLogger;

pub fn get_local_ip() -> String {
    let interfaces = match NetworkInterface::show() {
        Ok(list) => list,
        Err(e) => {
            FileLogger::error(&format!("get_local_ip: NetworkInterface::show() failed: {}", e));
            return "0.0.0.0".to_string();
        }
    };

    // Pass 1 — standard LAN (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
    for iface in &interfaces {
        for addr in &iface.addr {
            if let Addr::V4(v4) = addr {
                let o = v4.ip.octets();
                if o[0] == 192 && o[1] == 168 {
                    return v4.ip.to_string();
                }
            }
        }
    }

    // Pass 2 — any non-loopback IPv4 (e.g. en0 Wi-Fi or en1 Ethernet)
    for iface in &interfaces {
        let lower = iface.name.to_lowercase();
        if lower.contains("loopback") || lower.contains("bridge") || lower.contains("utun") {
            continue;
        }

        for addr in &iface.addr {
            if let Addr::V4(v4) = addr {
                let ip = v4.ip;
                if !ip.is_loopback() && !ip.is_link_local() {
                    return ip.to_string();
                }
            }
        }
    }

    "127.0.0.1".to_string()
}

pub fn get_mac_address() -> String {
    if let Ok(Some(mac)) = mac_address::get_mac_address() {
        return mac.to_string();
    }
    "00:00:00:00:00:00".to_string()
}
