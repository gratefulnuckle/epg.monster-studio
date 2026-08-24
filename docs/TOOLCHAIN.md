# Toolchain

v2 launches with `studio.ps1` / `studio.sh` in the repo root. Those scripts set
`EPG_MONSTER_HOME` to the repo so data stays in `{repo}/data`.

Needs **Rust** (stable) and **Node 22+**. Windows GNU builds also need **gcc**
(MinGW) on PATH. `--install` offers missing Node, Rust, ffmpeg/ffprobe, mpv, and
VLC: Scoop then winget on Windows; `apt-get` / `dnf` / `pacman` (sudo) on Linux;
Homebrew on macOS. `--uninstall` prompts for those same tools; `./data` is never
deleted.

```powershell
# Windows GNU
$env:Path = "<mingw64\bin>;<rustup\toolchains\stable-x86_64-pc-windows-gnu\bin>;" + $env:Path
$env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
.\studio.ps1 --install
.\studio.ps1 --shortcuts
.\studio.ps1
```

```bash
# Linux / macOS
chmod +x studio.sh
./studio.sh --install
./studio.sh --shortcuts
./studio.sh
```

Tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -p studio-core -p studio-tuner
```

On Windows GNU add `--target x86_64-pc-windows-gnu` and the same PATH as above.

## WebView2Loader.dll (Windows GNU)

Tauri on GNU Windows needs `WebView2Loader.dll` next to the exe. The copy in
`src-tauri/windows/WebView2Loader.dll` is vendored and listed in
`tauri.windows.conf.json` `bundle.resources` so `--install` / `cargo build` places
it beside `epg-monster-studio.exe`. Linux `.deb` / AppImage do not ship that DLL.

Update it only when bumping the WebView2 / Tauri Windows loader:

1. Take the x64 `WebView2Loader.dll` from the matching Evergreen Bootstrapper
   or the Tauri Windows GNU build output.
2. Replace `src-tauri/windows/WebView2Loader.dll`.
3. Record size + SHA-256 in the commit message (`Get-FileHash` on PowerShell).
4. Smoke-launch `.\studio.ps1 --start` so the window still opens.

Do not commit a different arch (ARM64 / x86).
