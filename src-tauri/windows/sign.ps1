# Authenticode sign one file. No-ops when no cert is configured so `tauri build`
# still works on unsigned GNU machines. Tauri replaces %1 with the path.
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Path
)
$ErrorActionPreference = "Stop"
$Path = $Path.Trim().Trim('"')
if (-not (Test-Path -LiteralPath $Path)) {
    Write-Error "sign.ps1: file not found: $Path"
}

function Find-SignTool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $roots = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
        "${env:ProgramFiles}\Windows Kits\10\bin"
    )
    foreach ($root in $roots) {
        if (-not (Test-Path $root)) { continue }
        $hit = Get-ChildItem $root -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.DirectoryName -match '\\x64$' } |
            Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    return $null
}

function Import-PfxIfNeeded {
    $raw = $env:EPGM_PFX
    if (-not $raw) { $raw = $env:TAURI_WINDOWS_PFX }
    if (-not $raw) { $raw = $env:WINDOWS_CERTIFICATE }
    $pass = $env:EPGM_PFX_PASSWORD
    if (-not $pass) { $pass = $env:TAURI_WINDOWS_PFX_PASSWORD }
    if (-not $pass) { $pass = $env:WINDOWS_CERTIFICATE_PASSWORD }
    if (-not $raw) { return }
    $pfx = $raw
    if (-not (Test-Path -LiteralPath $raw)) {
        $dir = Join-Path $env:TEMP "epg-monster-sign"
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $pfx = Join-Path $dir "code-sign.pfx"
        $bytes = [Convert]::FromBase64String(($raw -replace '\s', ''))
        [IO.File]::WriteAllBytes($pfx, $bytes)
    }
    $secure = if ($pass) {
        ConvertTo-SecureString $pass -AsPlainText -Force
    } else {
        New-Object SecureString
    }
    try {
        Import-PfxCertificate -FilePath $pfx -CertStoreLocation Cert:\CurrentUser\My -Password $secure | Out-Null
    } catch {
        Write-Host "PFX import: $($_.Exception.Message)"
    }
}

function CodeSigningCerts {
    Get-ChildItem Cert:\CurrentUser\My -ErrorAction SilentlyContinue | Where-Object {
        $_.HasPrivateKey -and (
            $_.EnhancedKeyUsageList.FriendlyName -contains "Code Signing" -or
            $_.Subject -match "epg\.monster|Code Signing"
        )
    }
}

Import-PfxIfNeeded
$thumb = $env:EPGM_CERT_THUMBPRINT
if (-not $thumb) { $thumb = $env:TAURI_WINDOWS_CERTIFICATE_THUMBPRINT }

$tool = Find-SignTool
if (-not $tool) {
    Write-Host "Authenticode skipped (signtool.exe not found)."
    exit 0
}

$stamp = "http://timestamp.digicert.com"
$args = @("sign", "/fd", "sha256", "/td", "sha256", "/tr", $stamp, "/v")
if ($thumb) {
    $args += @("/sha1", ($thumb -replace '\s', ''))
} elseif (CodeSigningCerts) {
    $args += "/a"
} else {
    Write-Host "Authenticode skipped (set EPGM_PFX + EPGM_PFX_PASSWORD, or EPGM_CERT_THUMBPRINT)."
    exit 0
}
$args += $Path
& $tool @args
if ($LASTEXITCODE -ne 0) {
    Write-Error "signtool failed with exit $LASTEXITCODE for $Path"
}
Write-Host "Signed $Path"
