# Toolchain (this machine)

All project files and build artifacts live on **S:**. C: is too small.

| What | Path |
|------|------|
| Remake | `S:\Projects\epg.monster-studio-tauri` |
| C# oracle | `S:\Projects\epg.monster-studio` |
| Old path | `C:\Users\jonat\Projects` → junction to `S:\Projects` |
| WinLibs GCC | `S:\toolchains\winlibs\mingw64\bin` |
| Rust (GNU) | `S:\toolchains\rustup` (`RUSTUP_HOME`) |
| Cargo home | `S:\toolchains\cargo` (`CARGO_HOME`) |
| Cargo target | `S:\toolchains\cargo-target\epg-monster-studio` |
| npm cache | `S:\toolchains\npm-cache` |
| TEMP | `S:\Temp` |

```powershell
$env:RUSTUP_HOME = "S:\toolchains\rustup"
$env:CARGO_HOME = "S:\toolchains\cargo"
$env:TEMP = "S:\Temp"; $env:TMP = "S:\Temp"
$env:Path = "S:\toolchains\winlibs\mingw64\bin;S:\toolchains\rustup\toolchains\stable-x86_64-pc-windows-gnu\bin;" + $env:Path
cd S:\Projects\epg.monster-studio-tauri
cargo test --manifest-path src-tauri\Cargo.toml -p studio-core -p studio-tuner
```
