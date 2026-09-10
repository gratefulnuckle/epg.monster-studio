# One-time fetch of the official MinGW runtime into tools/gstreamer (gitignored).
param(
    [string]$Prefix = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not $Prefix) { $Prefix = Join-Path $root "tools\gstreamer" }
$Launch = Join-Path $Prefix "bin\gst-launch-1.0.exe"
if (Test-Path $Launch) {
    Write-Host "GStreamer prefix already at $Prefix"
    exit 0
}
$alt = "S:\toolchains\gstreamer"
if (Test-Path (Join-Path $alt "bin\gst-launch-1.0.exe")) {
    Write-Host "Using existing GStreamer prefix at $alt"
    $parent = Split-Path $Prefix
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
    if (Test-Path $Prefix) { cmd /c "rmdir `"$Prefix`"" | Out-Null }
    cmd /c "mklink /J `"$Prefix`" `"$alt`"" | Out-Null
    if (Test-Path $Launch) { exit 0 }
}
$Url = "https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/mingw/gstreamer-1.0-mingw-x86_64-1.28.6.exe"
$Sha = "F2CFF66983EF8A5361143487670E78F27E2387322D20BC94F7396F7D102E9B4C"
$cache = Join-Path $env:LOCALAPPDATA "epg.monster-studio\tool-cache"
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$setup = Join-Path $cache "gstreamer-1.28.6-mingw-x86_64.exe"
if (-not (Test-Path $setup) -or ((Get-FileHash $setup -Algorithm SHA256).Hash -ne $Sha)) {
    Write-Host "Downloading official GStreamer setup…"
    Invoke-WebRequest -Uri $Url -OutFile $setup -UseBasicParsing
}
$hash = (Get-FileHash $setup -Algorithm SHA256).Hash
if ($hash -ne $Sha) {
    Remove-Item $setup -Force
    throw "SHA-256 mismatch for GStreamer. Expected $Sha, got $hash."
}
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
Write-Host "Installing GStreamer prefix to $Prefix"
$p = Start-Process -FilePath $setup -ArgumentList @(
    "/VERYSILENT", "/NORESTART", "/SUPPRESSMSGBOXES", "/CURRENTUSER",
    "/TYPE=complete", "/DIR=$Prefix"
) -Wait -PassThru
if ($p.ExitCode -ne 0) {
    throw "GStreamer installer exited $($p.ExitCode)."
}
if (-not (Test-Path $Launch)) {
    throw "GStreamer installer finished but gst-launch-1.0.exe was not in $Prefix\bin."
}
Write-Host "GStreamer prefix ready."
