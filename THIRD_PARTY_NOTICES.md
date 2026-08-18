# Third-party notices

epg.monster studio bundles or may launch the following programs. Their licenses apply to those binaries, not only to this app.

## ffmpeg / ffprobe / ffplay

- **Project:** [FFmpeg](https://ffmpeg.org/)
- **Windows build used here:** [Gyan.dev essentials](https://www.gyan.dev/ffmpeg/builds/)
- **License:** GNU GPL v2 or later (essentials builds include GPL components). Some FFmpeg builds are LGPL; the essentials package we ship is treated as **GPL**.
- **Source:** https://ffmpeg.org/download.html
- Shipped under `tools/ffmpeg/` next to the app (and inside the installer).

## mpv

- **Project:** [mpv](https://mpv.io/)
- **License:** GNU GPL v2 or later
- **Source:** https://github.com/mpv-player/mpv
- Shipped under `tools/mpv/` next to the app (and inside the installer).

## VLC (optional, not bundled)

- **Project:** [VideoLAN VLC](https://www.videolan.org/vlc/)
- **License:** GNU GPL v2 or later
- The installer does **not** include VLC. If installed on the machine, Settings can point at `vlc.exe`.

## .NET / Windows App SDK

The published folder is a self-contained .NET 10 + Windows App SDK runtime. Those components are redistributed under Microsoft’s terms for the .NET runtime and Windows App SDK.

---

The studio application itself is GNU GPL v3.0. See [LICENSE](LICENSE).
