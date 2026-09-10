# Download libmpv-2.dll into tools/mpv for the G-houl embed engine.
# Uses the latest zhongfly mpv-dev Windows build (same lineage as scoop mpv-git).
param(
    [string]$DestDir = ""
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not $DestDir) { $DestDir = Join-Path $Root "tools\mpv" }
$Dll = Join-Path $DestDir "libmpv-2.dll"
if (Test-Path $Dll) {
    Write-Host "libmpv already at $Dll"
    exit 0
}

$headers = @{ "User-Agent" = "epg.monster-studio" }
$rel = Invoke-RestMethod -Uri "https://api.github.com/repos/zhongfly/mpv-winbuild/releases/latest" -Headers $headers
$asset = $rel.assets | Where-Object { $_.name -match '^mpv-dev-x86_64-.*\.7z$' } | Select-Object -First 1
if (-not $asset) {
    throw "No mpv-dev-x86_64 *.7z on the latest zhongfly/mpv-winbuild release."
}
$cache = Join-Path $env:LOCALAPPDATA "epg.monster-studio\tool-cache"
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$archive = Join-Path $cache $asset.name
if (-not (Test-Path $archive) -or ((Get-Item $archive).Length -lt 10000)) {
    Write-Host "Downloading $($asset.name)…"
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $archive -UseBasicParsing -Headers $headers
}

$seven = Get-Command 7z -ErrorAction SilentlyContinue
if (-not $seven) {
    throw "Need 7z on PATH to unpack libmpv (scoop install 7zip)."
}
$tmp = Join-Path $cache ("libmpv-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    & 7z e -y "-o$tmp" $archive "libmpv-2.dll" | Out-Null
    $found = Get-ChildItem $tmp -Filter "libmpv-2.dll" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $found) {
        throw "Archive did not contain libmpv-2.dll."
    }
    New-Item -ItemType Directory -Force -Path $DestDir | Out-Null
    Copy-Item -Force $found.FullName $Dll
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
if (-not (Test-Path $Dll)) { throw "Failed to stage $Dll" }
Write-Host "libmpv ready at $Dll"
