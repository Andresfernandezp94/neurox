//! SSRF guard for tools that fetch URLs from external sources.
//!
//! The `is_safe_target` function resolves a URL's host and rejects
//! any IP that is loopback, private, link-local, or otherwise
//! "internal" — so a tool that fetches a URL the LLM passed in
//! (or that the LLM tricked the upstream API into returning)
//! cannot probe the daemon's loopback services, the cloud metadata
//! endpoint (169.254.169.254), or any RFC1918 network the daemon
//! happens to be on.
//!
//! Originally added in `web_fetch` to fix a critical SSRF; extracted
//! here so the same check can be applied uniformly to every tool
//! that takes a URL — including the media tools (`generate_*`)
//! that download from URLs returned by an upstream API.

use std::net::IpAddr;

/// Maximum body size (in bytes) that download-style tools will accept
/// from a single response. 100 MiB is enough for any reasonable image,
/// audio, or video while still bounding memory. Override per-call if
/// needed.
pub const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

/// Resolve a URL and return `Ok(())` if the target is "safe" (public
/// internet) or an `Err` explaining why it was rejected.
pub fn is_safe_target(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid url: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("scheme not allowed: {}", parsed.scheme()));
    }
    let host = parsed.host_str().ok_or("no host")?;
    let host_owned = host.to_string();
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(
        &format!("{host_owned}:{port}"),
    )
    .map_err(|e| format!("dns: {e}"))?
    .collect();
    if addrs.is_empty() {
        return Err("no addresses resolved".into());
    }
    for addr in &addrs {
        if is_unsafe_ip(&addr.ip()) {
            return Err(format!(
                "refusing to fetch private/loopback address: {} (resolved from {})",
                addr.ip(),
                host
            ));
        }
    }
    Ok(())
}

/// Returns true if `ip` is loopback, private, link-local, or
/// otherwise "non-public" per the same criteria that RFC1918 +
/// RFC6890 use.
pub fn is_unsafe_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()        // 127.0.0.0/8
            || v4.is_private()      // 10/8, 172.16/12, 192.168/16
            || v4.is_link_local()    // 169.254/16 (cloud metadata!)
            || v4.is_unspecified()   // 0.0.0.0
            || v4.is_broadcast()     // 255.255.255.255
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()        // ::1
            || v6.is_unspecified()  // ::
            // Unique local fc00::/7
            || (v6.segments()[0] & 0xfe00) == 0xfc00
            // Link-local fe80::/10
            || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_target_blocks_loopback() {
        assert!(is_safe_target("http://127.0.0.1/").is_err());
        assert!(is_safe_target("http://127.0.0.1:7878/health").is_err());
        assert!(is_safe_target("http://localhost/foo").is_err());
    }

    #[test]
    fn safe_target_blocks_private() {
        assert!(is_safe_target("http://10.0.0.1/").is_err());
        assert!(is_safe_target("http://192.168.1.1/").is_err());
        assert!(is_safe_target("http://172.16.0.1/").is_err());
        assert!(is_safe_target("http://169.254.169.254/latest/").is_err());
    }

    #[test]
    fn safe_target_blocks_non_http() {
        assert!(is_safe_target("file:///etc/passwd").is_err());
        assert!(is_safe_target("javascript:alert(1)").is_err());
        assert!(is_safe_target("gopher://example.com").is_err());
    }

    #[test]
    fn safe_target_allows_public() {
        assert!(is_safe_target("https://example.com").is_ok());
        assert!(is_safe_target("https://httpbin.org/get").is_ok());
        assert!(is_safe_target("http://api.openai.com/v1/audio").is_ok());
    }

    #[test]
    fn safe_target_rejects_invalid_url() {
        assert!(is_safe_target("not-a-url").is_err());
    }

    #[test]
    fn unsafe_ip_categorization() {
        use std::net::IpAddr;
        assert!(is_unsafe_ip(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"::1".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"10.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(is_unsafe_ip(&"169.254.169.254".parse::<IpAddr>().unwrap()));
        assert!(!is_unsafe_ip(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_unsafe_ip(&"1.1.1.1".parse::<IpAddr>().unwrap()));
    }
}
