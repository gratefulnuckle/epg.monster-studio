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
