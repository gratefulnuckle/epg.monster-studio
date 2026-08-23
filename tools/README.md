# Bundled tools

v2 testers run `studio.ps1` / `studio.sh` in the repo root. `--install` checks
for ffmpeg/ffprobe (and Node, Rust, GCC/WebKit) and prompts the OS package
manager: Scoop/winget on Windows, apt-get/dnf/pacman (sudo) on Linux, Homebrew
on macOS. Splash only reports whether they are present — it does not download
zips. Linux `.deb` depends on distro `ffmpeg`.

**mpv and VLC are optional.** `--install` offers them the same way. Settings can
point at an existing player if you skip the prompt.

```
tools/ffmpeg/ffmpeg.exe
tools/ffmpeg/ffprobe.exe
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

G-houl / GStreamer is **v3** (`docs/V3.md`).
