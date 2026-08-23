# Allow LAN / Advertise

Default bind is **loopback** (`Allow LAN` off). Tuner HTTP is on ports
**8080–8083**. There is **no client password**.

| Control | Effect |
|---------|--------|
| **Allow LAN** | Bind `0.0.0.0` so other machines on the LAN can reach the tuner. Trusted LAN only. |
| **Advertise tuners** | HDHomeRun UDP **65001** + SSDP. Turn on Allow LAN if Plex/Jellyfin/Emby is another PC. |

**IPTV remux:** keep **Remux IPTV playlist through Studio** **on** when Allow LAN
is on. Remux off puts **provider stream URLs** in the M3U that LAN clients
download. Remux on keeps playlist URLs on Studio (`/auto/vN`).

Plex / Jellyfin / Emby always use the local HDHomeRun proxy URLs, not provider
URLs.