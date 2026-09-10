# Build the GStreamer sidecar and stage it to {app}/tools/ghoul/.
# Requires a GStreamer prefix with pkg-config (tools/gstreamer or S:\toolchains\gstreamer).
param(
    [string]$AppRoot = "",
    [string]$GstRoot = ""
)

$ErrorActionPreference = "Stop"
$Repo = Split-Path -Parent $PSScriptRoot
if (-not $AppRoot) { $AppRoot = $Repo }

function Find-GstRoot {
    param([string]$Hint)
    $cands = @()
    if ($Hint) { $cands += $Hint }
    $cands += @(
        (Join-Path $AppRoot "tools\gstreamer"),
        (Join-Path $Repo "tools\gstreamer"),
        "S:\toolchains\gstreamer"
    )
    foreach ($p in $cands) {
        if ($p -and (Test-Path (Join-Path $p "bin\gst-launch-1.0.exe"))) { return $p }
        if ($p -and (Test-Path (Join-Path $p "lib\pkgconfig\gstreamer-1.0.pc"))) { return $p }
    }
    return $null
}

$GstRoot = Find-GstRoot $GstRoot
if (-not $GstRoot) {
    throw "No GStreamer prefix (need bin\gst-launch-1.0.exe). Run --install or ghoul/scripts/fetch-gstreamer-prefix.ps1."
}

$pc = Join-Path $GstRoot "lib\pkgconfig"
$bin = Join-Path $GstRoot "bin"
if (-not (Test-Path (Join-Path $pc "gstreamer-1.0.pc"))) {
    throw "GStreamer prefix at $GstRoot has no lib\pkgconfig\gstreamer-1.0.pc (need the devel/runtime prefix to compile ghoul-gst)."
}

$env:PKG_CONFIG_PATH = $pc
if ($env:Path -notlike "*$bin*") {
    $env:Path = "$bin;" + $env:Path
}
$env:GST_PLUGIN_PATH = Join-Path $GstRoot "lib\gstreamer-1.0"
$env:GST_PLUGIN_SYSTEM_PATH = $env:GST_PLUGIN_PATH

$Manifest = Join-Path $Repo "src-tauri\crates\ghoul-gst\Cargo.toml"
$DestDir = Join-Path $AppRoot "tools\ghoul"
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

$gnu = "x86_64-pc-windows-gnu"
cargo build --release --target $gnu --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

function Find-BuiltExe {
    $leaf = "ghoul-gst.exe"
    $cands = @()
    try {
        $json = cargo metadata --format-version 1 --no-deps --offline --manifest-path $Manifest 2>$null
        if ($json) {
            $meta = $json | ConvertFrom-Json
            if ($meta.target_directory) {
                $td = [string]$meta.target_directory
                $cands += (Join-Path $td (Join-Path $gnu (Join-Path "release" $leaf)))
                $cands += (Join-Path $td (Join-Path "release" $leaf))
            }
        }
    } catch { }
    if ($env:CARGO_TARGET_DIR) {
        $cands += (Join-Path $env:CARGO_TARGET_DIR (Join-Path $gnu (Join-Path "release" $leaf)))
        $cands += (Join-Path $env:CARGO_TARGET_DIR (Join-Path "release" $leaf))
    }
    $cands += (Join-Path $Repo "src-tauri\target\$gnu\release\$leaf")
    $cands += (Join-Path $Repo "src-tauri\target\release\$leaf")
    $cands += (Join-Path "S:\toolchains\cargo-target\epg-monster-studio" "$gnu\release\$leaf")
    $cands += (Join-Path "S:\toolchains\cargo-target\epg-monster-studio" "release\$leaf")
    foreach ($c in $cands) {
        if ($c -and (Test-Path $c)) { return $c }
    }
    return $null
}

$Exe = Find-BuiltExe
if (-not $Exe) { throw "ghoul-gst.exe not found after cargo build." }
Copy-Item $Exe (Join-Path $DestDir "ghoul-gst.exe") -Force
Write-Host "staged $(Join-Path $DestDir 'ghoul-gst.exe')"
