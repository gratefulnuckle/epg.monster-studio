// SPDX-License-Identifier: GPL-3.0-or-later

pub fn text_for(
    kind: &str,
    host: &str,
    device_id: &str,
    port: i32,
    enabled: bool,
    running: bool,
    downspiral: bool,
) -> String {
    let state = if enabled {
        if running {
            "Studio is listening now."
        } else {
            "Enabled in Settings. Click Start on this card."
        }
    } else {
        "Off in Settings — check the box and click Save, then Start here."
    };
    let common = format!(
        "Studio status: {state}\nDevice ID: {device_id} (stable)\nPort: {port}\n\n\
Default ports (no Windows URL reservation): Plex 8080, Jellyfin 8081, Emby 8082, IPTV 8083.\n\
You need channels in Managed Output → Tuner lineup first, or the lineup is empty.\n\n\
Allow LAN only if the player is on another device. Studio listens with a normal TCP socket, so you should not need netsh or a URL ACL.\n\n"
    );
    match kind {
        "Plex" => common + &format!(
            "Plex (same PC unless Allow LAN)\n\
1. Settings → Live TV & DVR → Set Up Plex DVR.\n\
2. When it searches, choose “Don't have an antenna?” / enter the tuner URL manually if needed.\n\
3. Tuner / HDHomeRun address: {host}\n   Discover: {host}/discover.json\n\
4. After the device is added, set the EPG to XMLTV:\n   {host}/guide.xml\n\
5. Map channels. Plex shows the first XMLTV display-name (the channel name). It talks HDHomeRun, not M3U.\n\
6. Plex saves mappings in the request URL. A 2000-channel lineup will fail to save (HTTP 400). Keep the Plex tuner under ~400 channels; use the IPTV card (8083) for the full list.\n\n\
Do not paste the M3U into Plex. If Plex is on another machine, use that PC’s LAN IP instead of 127.0.0.1 and turn on Allow LAN (then Start again).\n"
        ),
        "Jellyfin" => {
            let mut s = common + &format!(
                "Jellyfin\n\
1. Dashboard → Live TV → Tuner Devices → Add.\n\
2. HDHomeRun: {host}\n   or M3U tuner: {host}/tuner.m3u\n\
3. Guide data: XMLTV → {host}/guide.xml\n\
4. Map guide to channels.\n\n\
TiviMate / IPTV players — prefer the IPTV card (port 8083). This Jellyfin card still serves:\n\
Playlist: {host}/playlist.m3u8\n\
EPG: {host}/guide.xml\n"
            );
            if downspiral {
                s.push_str(&format!(
                    "\nDownspiral is on. Jellyfin Live TV cannot switch channel lists without changing user profiles.\n\
Studio publishes one playlist + guide per Managed Output group:\n\n\
   Index: {host}/downspiral/index.json\n\
   Example: {host}/downspiral/sports.m3u8\n\
   Example EPG: {host}/downspiral/sports.xml\n\n\
Point a Downspiral (or similar) plugin at the index, or add each group M3U as its own tuner.\n\
The full HDHomeRun /tuner.m3u lineup is unchanged.\n"
                ));
            }
            s
        }
        "Emby" => common + &format!(
            "Emby\n\
1. Live TV → Tuner Devices → Add HDHomeRun.\n\
2. Tuner address: {host}\n   Discover: {host}/discover.json\n\
3. Guide: XMLTV → {host}/guide.xml\n\
4. Map channels.\n\n\
Emby uses HDHomeRun like Plex, not the M3U. If Emby is on another machine, use the LAN IP and Allow LAN.\n"
        ),
        _ => common + &format!(
            "IPTV players (TiviMate, IPTV Smarters, similar)\n\
1. Enable this IPTV card in Settings and click Start.\n\
2. Turn on Allow LAN if the player is another device (Fire Stick, phone, etc.).\n\
3. In the player, add a custom playlist:\n\n\
   Playlist: {host}/playlist.m3u8\n\
   EPG:      {host}/guide.xml\n\n\
The playlist already has url-tvg pointing at the guide. Logos are on both the M3U (tvg-logo) and the EPG (<icon>).\n\
Studio must stay running while the player uses these URLs. Streams are remuxed locally — provider URLs stay inside Studio.\n"
        ),
    }
}
