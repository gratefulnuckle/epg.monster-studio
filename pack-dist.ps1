# After a local build, dist\ holds the portable studio exe.
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot
$dist = Join-Path $root "dist"
if (-not (Test-Path (Join-Path $dist "epg-monster-studio.exe"))) {
    Write-Error "No dist\epg-monster-studio.exe. Build the app first."
}
Write-Host "Portable folder: $dist"
