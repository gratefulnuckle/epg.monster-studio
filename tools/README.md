# Bundled tools

v3 testers run `studio.ps1` / `studio.sh` in the repo root (`INSTALL.md`). `--install` checks
for ffmpeg/ffprobe (and Node, Rust, GCC/WebKit) and prompts the OS package
manager: Scoop/winget on Windows, apt-get/dnf/pacman (sudo) on Linux, Homebrew
on macOS. Splash only reports whether they are present — it does not download
zips. Linux `.deb` depends on distro `ffmpeg`.

**mpv and VLC are optional** for Playlist Editor **Play**. **G-houl** (IPTV Player)
needs at least one of: GStreamer + `tools/ghoul/ghoul-gst`, `mpv.exe`, or
`libmpv-2.dll`. `--install` offers those the same way.

```
tools/ffmpeg/ffmpeg.exe
tools/ffmpeg/ffprobe.exe
tools/mpv/mpv.exe            (or system/scoop mpv)
tools/mpv/libmpv-2.dll
tools/gstreamer/             (MinGW runtime prefix)
tools/ghoul/ghoul-gst.exe
```

Linux and macOS do **not** download those Windows zips. Splash and Settings treat
system `ffmpeg` / `ffprobe` / `mpv` on PATH (or `/usr/bin`) as present.

```text
# Debian / Ubuntu / XFCE
sudo apt install ffmpeg mpv

# Fedora
sudo dnf install ffmpeg mpv

# macOS
brew install ffmpeg mpv
```

Binaries under `tools/` are gitignored.

**Detect bundled tools** in Settings fills mpv/ffmpeg/ffprobe from `tools/` next
to the app, then common system locations.

G-houl engines: `--install` stages libmpv from zhongfly mpv-dev, a GStreamer
prefix (`scripts/fetch-gstreamer-prefix.ps1`), and builds `ghoul-gst` when
pkg-config files are present. `--uninstall` prompts before removing those.
