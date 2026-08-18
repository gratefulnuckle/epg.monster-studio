// SPDX-License-Identifier: GPL-3.0-or-later

/// Default tuner ports — match C# TunerServerProfile.
pub const PLEX_PORT: u16 = 8080;
pub const JELLYFIN_PORT: u16 = 8081;
pub const EMBY_PORT: u16 = 8082;
pub const IPTV_PORT: u16 = 8083;

pub fn default_port(kind: &str) -> u16 {
    match kind {
        "Plex" => PLEX_PORT,
        "Jellyfin" => JELLYFIN_PORT,
        "Emby" => EMBY_PORT,
        "Iptv" => IPTV_PORT,
        _ => PLEX_PORT,
    }
}

pub fn is_legacy_reserved_port(port: u16) -> bool {
    matches!(port, 5004 | 5005 | 5006 | 5007)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports_match_csharp() {
        assert_eq!(default_port("Plex"), 8080);
        assert_eq!(default_port("Jellyfin"), 8081);
        assert_eq!(default_port("Emby"), 8082);
        assert_eq!(default_port("Iptv"), 8083);
    }

    #[test]
    fn legacy_hdhomerun_ports_are_reserved() {
        assert!(is_legacy_reserved_port(5004));
        assert!(!is_legacy_reserved_port(8080));
    }
}
