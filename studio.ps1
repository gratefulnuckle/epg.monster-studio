# v2 portable launcher (Windows). Same flags as studio.sh.
# Data and logs live next to the repo. NSIS / certs are v3.
# Use $args (not param()) so --start/--stop are not parsed as named parameters.
$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
$PidFile = Join-Path $Root ".studio-dev.pid"
$Launchable = Join-Path $Root "epg-monster-studio.exe"
$IconIco = Join-Path $Root "src-tauri\icons\mascot.ico"
$WebViewDll = Join-Path $Root "WebView2Loader.dll"
$ToolStatePath = Join-Path $Root ".studio-install.json"
$env:EPG_MONSTER_HOME = $Root
Set-Location $Root
$script:Esc = [char]27
try {
    chcp 65001 | Out-Null
    [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
} catch { }
try {
    if (-not ("NativeConsole" -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class NativeConsole {
  [DllImport("kernel32.dll")] public static extern IntPtr GetStdHandle(int n);
  [DllImport("kernel32.dll")] public static extern bool GetConsoleMode(IntPtr h, out uint m);
  [DllImport("kernel32.dll")] public static extern bool SetConsoleMode(IntPtr h, uint m);
}
"@
    }
    $stdOut = [NativeConsole]::GetStdHandle(-11)
    $mode = [uint32]0
    if ([NativeConsole]::GetConsoleMode($stdOut, [ref]$mode)) {
        [void][NativeConsole]::SetConsoleMode($stdOut, $mode -bor 4)
    }
} catch { }

function Get-DesktopLnk { Join-Path ([Environment]::GetFolderPath("Desktop")) "epg.monster studio.lnk" }
function Get-StartLnk { Join-Path ([Environment]::GetFolderPath("StartMenu")) "Programs\epg.monster studio.lnk" }

function Write-Rule {
    Write-Host ("  " + ("-" * 56)) -ForegroundColor DarkMagenta
}

function Write-Center([string]$Text, [int]$Width) {
    if ($Text.Length -ge $Width) { return $Text.Substring(0, $Width) }
    $pad = $Width - $Text.Length
    $left = [int][Math]::Floor($pad / 2)
    $right = $pad - $left
    return ((" " * $left) + $Text + (" " * $right))
}

$script:UiMode = "help"
$script:UiRows = [ordered]@{}
$script:UiNote = ""
$script:UiPainted = $false
$script:CompileCrates = New-Object System.Collections.Generic.List[string]
$script:CheckCrates = New-Object System.Collections.Generic.List[string]
$script:LaunchTree = New-Object System.Collections.Generic.List[string]
$script:CargoDone = $false

function Get-ConsoleHeight {
    try {
        $h = [Console]::WindowHeight
        if ($h -ge 24) { return $h }
    } catch { }
    return 32
}

function Get-ConsoleWidth {
    try {
        $w = [Console]::WindowWidth
        if ($w -ge 60) { return $w }
    } catch { }
    return 80
}

function Set-UiHome {
    try {
        [Console]::SetCursorPosition([Console]::WindowLeft, [Console]::WindowTop)
        return $true
    } catch {
        return $false
    }
}

$script:UseSplit = $false
$script:PromptText = ""
$script:PaintY = 0
$script:LogLines = New-Object System.Collections.Generic.List[string]

function Get-PaneLayout {
    $h = Get-ConsoleHeight
    if (-not $script:UseSplit) {
        return @{ Top = $h; Div = -1; Log = 0 }
    }
    $rowN = 0
    if ($script:UiRows) { $rowN = $script:UiRows.Count }
    $need = 4 + 3 + 2 + $rowN + 3
    $top = [int][Math]::Floor($h / 2)
    if ($need -gt $top) { $top = $need }
    if ($top -gt $h - 8) { $top = $h - 8 }
    $log = $h - $top - 2
    if ($log -lt 6) {
        $log = 6
        $top = [Math]::Max(12, $h - 8)
    }
    return @{ Top = $top; Div = $top; Log = $log }
}

function Write-ConsoleLine([int]$X, [int]$Y, [string]$Text, [string]$Color) {
    $w = Get-ConsoleWidth
    $winTop = 0
    $winH = $w
    try {
        $winTop = [Console]::WindowTop
        $winH = [Console]::WindowHeight
    } catch {
        $winH = 32
    }
    $lastY = $winTop + $winH - 2
    if ($Y -gt $lastY) { return }
    try { [Console]::SetCursorPosition($X, $Y) } catch { return }
    if ($null -eq $Text) { $Text = "" }
    $Text = $Text -replace "[\r\n]", " "
    $max = [Math]::Max(1, $w - $X - 1)
    if ($Text.Length -gt $max) { $Text = $Text.Substring(0, $max) }
    else { $Text = $Text.PadRight($max) }
    if (-not $Color) { $Color = "Gray" }
    $old = [Console]::ForegroundColor
    try {
        try { [Console]::ForegroundColor = [ConsoleColor]$Color } catch { }
        [Console]::Write($Text)
    } finally {
        try { [Console]::ForegroundColor = $old } catch { }
    }
}

function Out-UiLine([string]$Text, [string]$Color) {
    $layout = Get-PaneLayout
    $top = 0
    $left = 0
    try {
        $top = [Console]::WindowTop
        $left = [Console]::WindowLeft
    } catch { }
    $maxY = $top + $layout.Top - 3
    if ($script:UseSplit -and $script:PaintY -gt $maxY) { return }
    Write-ConsoleLine $left $script:PaintY $Text $Color
    $script:PaintY += 1
}

function Show-LogPane {
    if (-not $script:UseSplit) { return }
    $layout = Get-PaneLayout
    if ($layout.Log -lt 1) { return }
    $left = 0
    $winTop = 0
    try {
        $left = [Console]::WindowLeft
        $winTop = [Console]::WindowTop
    } catch { }
    $startY = $winTop + $layout.Div + 1
    $n = $layout.Log
    $from = 0
    if ($script:LogLines.Count -gt $n) { $from = $script:LogLines.Count - $n }
    for ($i = 0; $i -lt $n; $i++) {
        $text = ""
        $idx = $from + $i
        if ($idx -lt $script:LogLines.Count) { $text = [string]$script:LogLines[$idx] }
        Write-ConsoleLine $left ($startY + $i) $text "DarkGray"
    }
}

function Add-UiLog([string]$Text) {
    if ($null -eq $Text) { return }
    foreach ($line in ($Text -split "`r?`n")) {
        $line = $line.TrimEnd()
        if ($line.Length -eq 0) { continue }
        $script:LogLines.Add($line) | Out-Null
    }
    Show-LogPane
}

function Write-SectionTree([string]$Title, [System.Collections.IList]$Items, [int]$Keep) {
    if (-not $Items -or $Items.Count -eq 0) { return }
    $all = @($Items)
    Out-UiLine ("            $Title  ($($all.Count))") "DarkMagenta"
    $start = 0
    if ($Keep -gt 0 -and $all.Count -gt $Keep) {
        Out-UiLine ("            |-- ... $($all.Count - $Keep) earlier") "DarkGray"
        $start = $all.Count - $Keep
    }
    for ($i = $start; $i -lt $all.Count; $i++) {
        $branch = if ($i -eq ($all.Count - 1)) { "+-- " } else { "|-- " }
        $color = if ($i -eq ($all.Count - 1) -and -not $script:CargoDone) { "Cyan" } else { "DarkGray" }
        Out-UiLine ("            $branch$($all[$i])") $color
    }
}

$script:QuitHint = "CTRL+Q to QUIT"

function Write-Logo([string]$Mode) {
    $h = [string][char]0x2550
    $v = [string][char]0x2551
    $tl = [string][char]0x2554
    $tr = [string][char]0x2557
    $bl = [string][char]0x255A
    $br = [string][char]0x255D
    $title = "epg.monster studio"
    $sub = "2026 edition  -  v2.0.2  -  " + $Mode
    $inner = [Math]::Max($title.Length, $sub.Length) + 2
    $fill = $h * $inner
    Out-UiLine ("  " + $tl + $fill + $tr) "Magenta"
    Out-UiLine ("  " + $v + (Write-Center $title $inner) + $v) "Magenta"
    Out-UiLine ("  " + $v + (Write-Center $sub $inner) + $v) "Magenta"
    Out-UiLine ("  " + $bl + $fill + $br) "Magenta"
}

function Write-QuitHint {
    $cols = Get-ConsoleWidth
    $msg = $script:QuitHint
    $layout = Get-PaneLayout
    try {
        $left = [Console]::WindowLeft
        $top = [Console]::WindowTop
        $y = $top + $layout.Top - 1
        if ($y -lt $top) { $y = $top }
        $x = [Math]::Max(0, $cols - $msg.Length - 1)
        $pad = ""
        if ($x -gt 0) { $pad = " " * $x }
        Write-ConsoleLine $left $y ($pad + $msg) "DarkGray"
    } catch {
        Write-Host $msg -ForegroundColor DarkGray
    }
}

function Exit-StudioQuit {
    try { [Console]::CursorVisible = $true } catch { }
    Write-Host ""
    Write-Host "  quit" -ForegroundColor DarkGray
    exit 0
}

function Test-QuitKey($Key) {
    if (-not $Key) { return $false }
    return (($Key.Modifiers -band [ConsoleModifiers]::Control) -eq [ConsoleModifiers]::Control) -and ($Key.Key -eq [ConsoleKey]::Q)
}

function Read-PromptChar {
    $k = [Console]::ReadKey($true)
    if (Test-QuitKey $k) { Exit-StudioQuit }
    if ($k.Key -eq [ConsoleKey]::Enter) { return "" }
    return [string]$k.KeyChar
}

function Poll-QuitKey {
    try {
        while ([Console]::KeyAvailable) {
            $k = [Console]::ReadKey($true)
            if (Test-QuitKey $k) { Exit-StudioQuit }
        }
    } catch { }
}

function Write-Banner([string]$Mode) {
    $script:UiMode = $Mode
    $script:UiPainted = $false
    Show-UiScreen
}

function Write-StepLine([string]$Name, [string]$State, [string]$Detail, [string]$Kind) {
    $tag = switch ($Kind) {
        "ok" { "[ ok ]" }
        "skip" { "[ -- ]" }
        "warn" { "[ !! ]" }
        "fail" { "[ XX ]" }
        default { "[ .. ]" }
    }
    $color = switch ($Kind) {
        "ok" { "Green" }
        "skip" { "DarkGray" }
        "warn" { "Yellow" }
        "fail" { "Red" }
        default { "Cyan" }
    }
    $line = "  $tag  $($Name.PadRight(12)) $State"
    if ($Detail) { $line = $line + "  " + $Detail }
    Out-UiLine $line $color
}

function Write-Step([string]$Name, [string]$State, [string]$Detail, [string]$Kind) {
    Set-UiRow $Name $State $Detail $Kind
}

function Write-Phase([string]$Text) {
    $script:UiNote = $Text
    Show-UiScreen
}

function Show-UiScreen {
    try { [Console]::CursorVisible = $false } catch { }
    if (-not $script:UiPainted) {
        Clear-Host
        $script:UiPainted = $true
    } else {
        if (-not (Set-UiHome)) { Clear-Host }
    }
    $left = 0
    $winTop = 0
    try {
        $left = [Console]::WindowLeft
        $winTop = [Console]::WindowTop
    } catch { }
    $script:PaintY = $winTop
    Write-Logo $script:UiMode
    Out-UiLine ("    folder     $Root") "DarkGray"
    Out-UiLine ("    launchable $Launchable") "DarkGray"
    Out-UiLine ("    data       $Root\data") "DarkGray"
    Out-UiLine "" "Gray"
    foreach ($name in $script:UiRows.Keys) {
        $row = $script:UiRows[$name]
        Write-StepLine $name $row.State $row.Detail $row.Kind
    }
    Out-UiLine "" "Gray"
    if ($script:UiNote) {
        foreach ($line in ($script:UiNote -split "`r?`n")) {
            $color = "DarkGray"
            if ($line -match "complete") { $color = "Green" }
            elseif ($line -match "^->") { $color = "Cyan" }
            Out-UiLine ("  " + $line) $color
        }
    }
    $layout = Get-PaneLayout
    if ($script:UseSplit) {
        $promptY = $winTop + $layout.Top - 2
        $quitY = $winTop + $layout.Top - 1
        $divY = $winTop + $layout.Div
        while ($script:PaintY -lt $promptY) {
            Write-ConsoleLine $left $script:PaintY "" "Gray"
            $script:PaintY += 1
        }
        $p = $script:PromptText
        if (-not $p) { $p = "" }
        Write-ConsoleLine $left $promptY ("  " + $p) "Yellow"
        Write-QuitHint
        $rule = ([string][char]0x2500) * [Math]::Max(1, (Get-ConsoleWidth) - 1)
        Write-ConsoleLine $left $divY $rule "DarkMagenta"
        Show-LogPane
        try { [Console]::SetCursorPosition($left, $promptY) } catch { }
    } else {
        Write-Host ($script:Esc + "[J") -NoNewline
        Write-QuitHint
    }
}

function Set-UiRow([string]$Name, [string]$State, [string]$Detail, [string]$Kind) {
    $script:UiRows[$Name] = @{ State = $State; Detail = $Detail; Kind = $Kind }
    Show-UiScreen
}

function Reset-Ui([string]$Mode) {
    $script:UiMode = $Mode
    $script:UiRows = [ordered]@{}
    $script:UiNote = ""
    $script:UiPainted = $false
    $script:CompileCrates = New-Object System.Collections.Generic.List[string]
    $script:CheckCrates = New-Object System.Collections.Generic.List[string]
    $script:CargoSeen = New-Object 'System.Collections.Generic.HashSet[string]'
    $script:LaunchTree = New-Object System.Collections.Generic.List[string]
    $script:CargoDone = $false
    $script:CargoTotal = 0
}

$script:ActionLog = $null

function Start-ActionLog([string]$Action) {
    $script:ActionLog = Join-Path $Root ($Action + ".log")
    $script:UseSplit = $true
    $script:PromptText = ""
    $script:LogLines = New-Object System.Collections.Generic.List[string]
    $hdr = @(
        "epg.monster studio",
        ("action: " + $Action),
        ("time: " + (Get-Date -Format "yyyy-MM-dd HH:mm:ss")),
        ("folder: " + $Root),
        ""
    )
    Set-Content -Path $script:ActionLog -Value $hdr -Encoding UTF8
    Add-UiLog ("-- " + $Action + " --")
    Add-UiLog ("log: " + $script:ActionLog)
}

function Write-ActionLog([string]$Text) {
    if (-not $script:ActionLog) { return }
    Add-Content -Path $script:ActionLog -Value $Text -Encoding UTF8
}

function Invoke-Logged([string]$Title, [scriptblock]$Cmd) {
    Write-ActionLog ""
    Write-ActionLog (">> " + $Title)
    Add-UiLog $Title
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $code = 0
    try {
        & $Cmd 2>&1 | ForEach-Object {
            $line = [string]$_
            Write-ActionLog $line
            Add-UiLog $line
            Poll-QuitKey
        }
        if ($null -ne $LASTEXITCODE) { $code = [int]$LASTEXITCODE }
        Write-ActionLog ("exit: " + $code)
    } catch {
        $code = 1
        Write-ActionLog ("error: " + $_)
        Add-UiLog ("error: " + $_)
        Write-ActionLog ("exit: " + $code)
    } finally {
        $ErrorActionPreference = $prev
    }
    return $code
}

function Invoke-Quiet([scriptblock]$Cmd, [string]$FailMsg) {
    $log = Join-Path $env:TEMP "epg-monster-studio-install.log"
    Add-UiLog $FailMsg
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Cmd *> $log
    } finally {
        $ErrorActionPreference = $prev
    }
    if (Test-Path $log) {
        Get-Content $log -ErrorAction SilentlyContinue | ForEach-Object {
            Write-ActionLog $_
            Add-UiLog $_
        }
    }
    Write-ActionLog ("exit: " + $LASTEXITCODE)
    if ($LASTEXITCODE -ne 0) {
        $script:UiNote = $FailMsg
        Show-UiScreen
        throw $FailMsg
    }
}

function Show-Usage {
    Write-Banner "help"
    Write-Host @"
  .\studio.ps1                 install + start
  .\studio.ps1 --install       Node, Rust, gcc, ffmpeg, mpv/VLC; build the .exe
  .\studio.ps1 --shortcuts     Desktop + Start Menu
  .\studio.ps1 --uninstall     stop, remove shortcuts + launchable; optional tools
  .\studio.ps1 --start         build UI, run the launchable
  .\studio.ps1 --stop          stop
  .\studio.ps1 --restart       stop then start
  .\studio.ps1 --help

  --install uses winget for Node and Rust; scoop then winget for ffmpeg / mpv / VLC
  --uninstall prompts for studio plus Node, Rust, ffmpeg, mpv, VLC
  .\data is never deleted
  each action writes .\install.log / .\uninstall.log / .\start.log / ...
"@ -ForegroundColor Gray
    Write-QuitHint
}

function Test-Running {
    if (-not (Test-Path $PidFile)) { return $false }
    $procId = (Get-Content $PidFile -ErrorAction SilentlyContinue | Select-Object -First 1)
    if (-not $procId) { return $false }
    $procId = $procId.Trim()
    if (-not $procId) { return $false }
    return [bool](Get-Process -Id $procId -ErrorAction SilentlyContinue)
}

function Stop-Studio {
    $n = 0
    if (Test-Path $PidFile) {
        $procId = (Get-Content $PidFile -ErrorAction SilentlyContinue | Select-Object -First 1)
        if ($procId) {
            $procId = $procId.Trim()
            $parsedPid = 0
            if ([int]::TryParse($procId, [ref]$parsedPid)) {
                $target = Get-Process -Id $parsedPid -ErrorAction SilentlyContinue
                if ($target) {
                    Stop-Process -Id $target.Id -Force -ErrorAction SilentlyContinue
                    $n += 1
                }
            }
        }
        Remove-Item $PidFile -Force -ErrorAction SilentlyContinue
    }
    $studioProcs = Get-Process -Name "epg-monster-studio" -ErrorAction SilentlyContinue
    foreach ($proc in $studioProcs) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        $n += 1
    }
    if ($n -gt 0) { Write-Step "app" "stopped" "" "ok" } else { Write-Step "app" "not running" "" "skip" }
}

function Test-Cmd([string]$Name) {
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-NativeText([scriptblock]$Cmd) {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $out = @(& $Cmd 2>&1 | ForEach-Object { [string]$_ })
        foreach ($line in $out) {
            if ([string]::IsNullOrWhiteSpace($line)) { continue }
            if ($line -match '^(info|warning|warn|debug):') { continue }
            if ($line -match 'NativeCommandError|CategoryInfo|FullyQualifiedErrorId|At .*\.ps1:') { continue }
            return $line.Trim()
        }
        return $null
    } catch {
        return $null
    } finally {
        $ErrorActionPreference = $prev
    }
}

function Show-PromptCursor {
    try {
        $layout = Get-PaneLayout
        $left = [Console]::WindowLeft
        $top = [Console]::WindowTop
        $y = $top + $layout.Top - 2
        $x = $left + 2 + $script:PromptText.Length
        $maxX = $left + [Math]::Max(0, (Get-ConsoleWidth) - 2)
        if ($x -gt $maxX) { $x = $maxX }
        [Console]::SetCursorPosition($x, $y)
        [Console]::CursorVisible = $true
    } catch { }
}

function Confirm-Yes([string]$Prompt) {
    $script:PromptText = "? $Prompt [Y/n] "
    Show-UiScreen
    Show-PromptCursor
    $r = Read-PromptChar
    $script:PromptText = ""
    Show-UiScreen
    return [string]::IsNullOrWhiteSpace($r) -or $r -match '^[Yy]'
}

function Confirm-No([string]$Prompt) {
    $script:PromptText = "? $Prompt [y/N] "
    Show-UiScreen
    Show-PromptCursor
    $r = Read-PromptChar
    $script:PromptText = ""
    Show-UiScreen
    return $r -match '^[Yy]'
}

function Invoke-Soft([scriptblock]$Cmd) {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { & $Cmd } finally { $ErrorActionPreference = $prev }
}

function Read-InstallState {
    $state = @{
        written = ""
        folder = $Root
        tools = @{}
    }
    if (-not (Test-Path $ToolStatePath)) { return $state }
    try {
        $j = Get-Content $ToolStatePath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($j.written) { $state.written = [string]$j.written }
        if ($j.folder) { $state.folder = [string]$j.folder }
        $src = $j.tools
        if (-not $src) { $src = $j }
        foreach ($p in $src.PSObject.Properties) {
            if (@("written", "folder", "tools") -contains $p.Name) { continue }
            $val = $p.Value
            if ($val -is [string]) {
                $state.tools[$p.Name] = @{ how = [string]$val }
            } else {
                $rec = @{}
                foreach ($q in $val.PSObject.Properties) {
                    $rec[$q.Name] = [string]$q.Value
                }
                $state.tools[$p.Name] = $rec
            }
        }
    } catch { }
    return $state
}

function Save-InstallState($state) {
    if (-not $state.tools -or $state.tools.Count -eq 0) {
        if (Test-Path $ToolStatePath) { Remove-Item -Force $ToolStatePath -ErrorAction SilentlyContinue }
        return
    }
    $state.written = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $state.folder = $Root
    ConvertTo-Json -InputObject $state -Depth 6 | Set-Content -Path $ToolStatePath -Encoding UTF8
}

function Get-ToolRecord([string]$Key) {
    $s = Read-InstallState
    if ($s.tools.ContainsKey($Key)) { return $s.tools[$Key] }
    return $null
}

function Remember-Tool {
    param(
        [string]$Key,
        [string]$How,
        [string]$Path,
        [string]$ScoopName,
        [string]$WingetId,
        [string]$Cmd,
        [string]$Ffprobe
    )
    $s = Read-InstallState
    $rec = @{}
    if ($s.tools.ContainsKey($Key) -and $s.tools[$Key] -is [hashtable]) {
        $rec = $s.tools[$Key]
    }
    if ($How) { $rec.how = $How }
    if ($Path) { $rec.path = $Path }
    if ($ScoopName) { $rec.scoopName = $ScoopName }
    if ($WingetId) { $rec.wingetId = $WingetId }
    if ($Cmd) { $rec.cmd = $Cmd }
    if ($Ffprobe) { $rec.ffprobe = $Ffprobe }
    $s.tools[$Key] = $rec
    Save-InstallState $s
}

function Forget-Tool([string]$Key) {
    $s = Read-InstallState
    if ($s.tools.ContainsKey($Key)) { $s.tools.Remove($Key) }
    Save-InstallState $s
}

function Detect-HowFromPath([string]$Path) {
    if (-not $Path) { return "system" }
    if ($Path -match '\\scoop\\') { return "scoop" }
    if ($Path -match 'WinGet|WindowsApps') { return "winget" }
    if ($Path -match '\\.cargo\\') { return "rustup" }
    return "system"
}

function Scoop-NameFromPath([string]$Path) {
    if ($Path -match 'scoop\\apps\\([^\\]+)\\') { return $Matches[1] }
    return $null
}

function Cmd-Source([string]$Name) {
    $c = Get-Command $Name -ErrorAction SilentlyContinue
    if ($c -and $c.Source) { return [string]$c.Source }
    return $null
}

function Snapshot-InstallState {
    Refresh-Path
    $node = Cmd-Source "node"
    if (-not $node) {
        foreach ($p in @(
            (Join-Path $env:USERPROFILE "scoop\apps\nodejs\current\node.exe"),
            (Join-Path $env:USERPROFILE "scoop\apps\nodejs-lts\current\node.exe")
        )) { if (Test-RealExe $p) { $node = $p; break } }
    }
    if ($node) {
        $prev = Get-ToolRecord "node"
        $how = $null
        if ($prev) { $how = $prev.how }
        if (-not $how) { $how = Detect-HowFromPath $node }
        $sn = Scoop-NameFromPath $node
        if (-not $sn) { $sn = "nodejs-lts" }
        Remember-Tool -Key "node" -How $how -Path $node -Cmd "node" -ScoopName $sn -WingetId "OpenJS.NodeJS.LTS"
    }
    $cargo = Cmd-Source "cargo"
    if ($cargo) {
        $prev = Get-ToolRecord "rust"
        $how = $null
        if ($prev) { $how = $prev.how }
        if (-not $how) { $how = Detect-HowFromPath $cargo }
        Remember-Tool -Key "rust" -How $how -Path $cargo -Cmd "cargo" -ScoopName "rustup" -WingetId "Rustlang.Rustup"
    }
    $ffdir = Get-FfmpegDir
    if ($ffdir) {
        $ff = Join-Path $ffdir "ffmpeg.exe"
        $fp = Join-Path $ffdir "ffprobe.exe"
        $prev = Get-ToolRecord "ffmpeg"
        $how = $null
        if ($prev) { $how = $prev.how }
        if (-not $how) { $how = Detect-HowFromPath $ff }
        Remember-Tool -Key "ffmpeg" -How $how -Path $ff -Ffprobe $fp -Cmd "ffmpeg" -ScoopName "ffmpeg" -WingetId "Gyan.FFmpeg"
    }
    $mpv = Get-MpvPath
    if ($mpv) {
        $prev = Get-ToolRecord "mpv"
        $how = $null
        if ($prev) { $how = $prev.how }
        if (-not $how) { $how = Detect-HowFromPath $mpv }
        $sn = Scoop-NameFromPath $mpv
        if (-not $sn) { $sn = "mpv" }
        Remember-Tool -Key "mpv" -How $how -Path $mpv -Cmd "mpv" -ScoopName $sn -WingetId "shinchiro.mpv"
    }
    $vlc = Get-VlcPath
    if ($vlc) {
        $prev = Get-ToolRecord "vlc"
        $how = $null
        if ($prev) { $how = $prev.how }
        if (-not $how) { $how = Detect-HowFromPath $vlc }
        $sn = Scoop-NameFromPath $vlc
        if (-not $sn) { $sn = "vlc" }
        Remember-Tool -Key "vlc" -How $how -Path $vlc -Cmd "vlc" -ScoopName $sn -WingetId "VideoLAN.VLC"
    }
    Write-ActionLog ("wrote " + $ToolStatePath)
}

function Refresh-Path {
    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $out = New-Object System.Collections.Generic.List[string]
    $add = {
        param([string]$chunk)
        if (-not $chunk) { return }
        foreach ($piece in ($chunk -split ';')) {
            $p = $piece.Trim().Trim('"')
            if (-not $p) { continue }
            if ($seen.Add($p)) { $out.Add($p) }
        }
    }
    & $add (Join-Path $env:USERPROFILE "scoop\shims")
    & $add (Join-Path $env:USERPROFILE ".cargo\bin")
    & $add (Join-Path $Root "node_modules\.bin")
    $mingw = "S:\toolchains\winlibs\mingw64\bin"
    if (Test-Path $mingw) { & $add $mingw }
    & $add ([System.Environment]::GetEnvironmentVariable("Path", "User"))
    & $add ([System.Environment]::GetEnvironmentVariable("Path", "Machine"))
    $joined = ($out -join ";")
    if ($joined.Length -gt 24000) {
        Write-ActionLog ("PATH rebuilt unique length=" + $joined.Length)
    }
    $env:Path = $joined
}

function Install-ScoopIfNeeded {
    if (Test-Cmd "scoop") { return $true }
    if (-not (Confirm-Yes "Install Scoop (user-local ffmpeg / mpv / VLC)?")) { return $false }
    $null = Invoke-Logged "install scoop" {
        Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force
        Invoke-RestMethod -Uri "https://get.scoop.sh" | Invoke-Expression
    }
    Refresh-Path
    return (Test-Cmd "scoop")
}

function Install-Winget {
    param(
        [string]$Label,
        [string]$Key,
        [string]$WingetId,
        [string[]]$ExtraIds
    )
    if (-not (Test-Cmd "winget")) {
        Write-ActionLog "winget not on PATH; cannot install $Label"
        return $false
    }
    if (-not (Confirm-Yes "Install $Label with winget ($WingetId)?")) {
        return $false
    }
    $ids = @($WingetId)
    if ($ExtraIds) { $ids = $ids + $ExtraIds }
    foreach ($id in $ids) {
        if (-not $id) { continue }
        $null = Invoke-Logged ("winget install " + $id) {
            winget install --id $id -e --accept-package-agreements --accept-source-agreements
        }
        Refresh-Path
        Remember-Tool $Key "winget"
        return $true
    }
    return $false
}

function Install-ScoopOrWinget {
    param(
        [string]$Label,
        [string]$Key,
        [string]$ScoopName,
        [string]$WingetId,
        [string]$ScoopBucket
    )
    if (Test-Cmd "scoop") {
        if (Confirm-Yes "Install $Label with scoop ($ScoopName)?") {
            if ($ScoopBucket) { Invoke-Logged ("scoop bucket add " + $ScoopBucket) { scoop bucket add $ScoopBucket } }
            Invoke-Logged ("scoop install " + $ScoopName) { scoop install $ScoopName }
            Refresh-Path
            Remember-Tool $Key "scoop"
            return $true
        }
    }
    if (Test-Cmd "winget") {
        if (Confirm-Yes "Install $Label with winget ($WingetId)?" ) {
            Invoke-Logged ("winget install " + $WingetId) {
                winget install --id $WingetId -e --accept-package-agreements --accept-source-agreements
            }
            Refresh-Path
            Remember-Tool $Key "winget"
            return $true
        }
    }
    return $false
}

function Uninstall-ScoopOrWinget {
    param(
        [string]$Label,
        [string]$Key,
        [string]$ScoopName,
        [string]$WingetId,
        [string[]]$ExtraWingetIds,
        [string[]]$ExtraScoopNames,
        [scriptblock]$StillThere
    )
    $rec = Get-ToolRecord $Key
    $how = ""
    if ($rec) {
        $how = [string]$rec.how
        if ($rec.scoopName) { $ScoopName = [string]$rec.scoopName }
        if ($rec.wingetId) { $WingetId = [string]$rec.wingetId }
    }
    Write-ActionLog ""
    Write-ActionLog ("uninstall " + $Label + " (recorded=" + $how + " path=" + $(if ($rec) { $rec.path } else { "" }) + ")")
    $order = @()
    if ($how -eq "scoop") {
        $order += @("scoop", "winget")
    } elseif ($how -eq "winget") {
        $order += @("winget", "scoop")
    } else {
        $order += @("winget", "scoop")
    }
    if ($Key -eq "rust") {
        $order = @("winget", "rustup", "scoop")
    }
    $seen = @{}
    $methods = @()
    foreach ($m in $order) {
        if (-not $seen.ContainsKey($m)) {
            $seen[$m] = $true
            $methods += $m
        }
    }
    foreach ($m in $methods) {
        if ($m -eq "rustup") {
            $ruExe = $null
            if (Test-Cmd "rustup") { $ruExe = "rustup" }
            elseif ($rec -and $rec.path) {
                $cand = Join-Path (Split-Path ([string]$rec.path)) "rustup.exe"
                if (Test-Path $cand) { $ruExe = $cand }
            }
            if ($ruExe) {
                Invoke-Logged "rustup self uninstall --yes" { & $ruExe self uninstall --yes }
                Refresh-Path
                if ($StillThere -and -not (& $StillThere)) { break }
                Invoke-Logged "rustup self uninstall -y" { & $ruExe self uninstall -y }
                Refresh-Path
                if ($StillThere -and -not (& $StillThere)) { break }
                Invoke-Logged "echo y | rustup self uninstall" { cmd.exe /c "echo y| `"$ruExe`" self uninstall" }
            }
        } elseif ($m -eq "scoop" -and (Test-Cmd "scoop")) {
            $names = @($ScoopName)
            if ($ExtraScoopNames) { $names = $names + $ExtraScoopNames }
            if ($Key -eq "node") { $names = $names + @("nodejs", "nodejs-lts") }
            $uniq = New-Object 'System.Collections.Generic.List[string]'
            $seenN = @{}
            foreach ($sn in $names) {
                if ($sn -and -not $seenN.ContainsKey($sn)) { $seenN[$sn] = $true; $uniq.Add($sn) }
            }
            $names = $uniq
            foreach ($sn in $names) {
                if ($sn) {
                    Invoke-Logged ("scoop uninstall " + $sn) { scoop uninstall $sn }
                    Refresh-Path
                    if ($StillThere -and -not (& $StillThere)) { break }
                }
            }
        } elseif ($m -eq "winget" -and (Test-Cmd "winget")) {
            $ids = @($WingetId)
            if ($ExtraWingetIds) { $ids = $ids + $ExtraWingetIds }
            foreach ($id in $ids) {
                if ($id) {
                    Invoke-Logged ("winget uninstall " + $id) { winget uninstall --id $id -e --accept-source-agreements }
                    Refresh-Path
                    if ($StillThere -and -not (& $StillThere)) { break }
                }
            }
        }
        Refresh-Path
        if ($StillThere -and -not (& $StillThere)) { break }
    }
    if ($StillThere -and (& $StillThere)) {
        return
    }
    Forget-Tool $Key
}

function Use-GnuToolchain {
    $env:RUSTUP_TOOLCHAIN = "stable-x86_64-pc-windows-gnu"
    Refresh-Path
}

function First-ExistingFile([string[]]$Paths) {
    foreach ($p in $Paths) {
        if ($p -and (Test-Path $p) -and -not (Test-Path $p -PathType Container)) { return $p }
    }
    return $null
}

function Test-RealExe([string]$Path) {
    if (-not $Path) { return $false }
    if (-not (Test-Path $Path)) { return $false }
    if (Test-Path $Path -PathType Container) { return $false }
    try {
        return ((Get-Item $Path).Length -ge 8192)
    } catch {
        return $false
    }
}

function First-ExistingApp([string[]]$Paths) {
    foreach ($p in $Paths) {
        if (Test-RealExe $p) { return $p }
    }
    return $null
}

function Get-FfmpegDir {
    $here = Join-Path $Root "tools\ffmpeg"
    $candidates = @(
        $here,
        (Join-Path $env:USERPROFILE "scoop\apps\ffmpeg\current\bin"),
        "C:\ProgramData\chocolatey\bin",
        "C:\ffmpeg\bin",
        "C:\Program Files\ffmpeg\bin"
    )
    $wp = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
    if (Test-Path $wp) {
        Get-ChildItem $wp -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "Gyan.FFmpeg*" } | ForEach-Object {
            Get-ChildItem $_.FullName -Directory -ErrorAction SilentlyContinue | ForEach-Object {
                $candidates += (Join-Path $_.FullName "bin")
            }
        }
    }
    if (Test-Cmd "ffmpeg") {
        $ff = (Get-Command ffmpeg).Source
        if ($ff) { $candidates = @((Split-Path $ff)) + $candidates }
    }
    foreach ($d in $candidates) {
        if (-not $d) { continue }
        $ff = Join-Path $d "ffmpeg.exe"
        $fp = Join-Path $d "ffprobe.exe"
        if ((Test-RealExe $ff) -and (Test-RealExe $fp)) { return $d }
    }
    return $null
}

function Get-MpvPath {
    $hits = @(
        (Join-Path $env:USERPROFILE "scoop\apps\mpv\current\mpv.exe"),
        "C:\Program Files\mpv\mpv.exe",
        "C:\Program Files\MPV Player\mpv.exe",
        "C:\Program Files (x86)\mpv\mpv.exe"
    )
    if (Test-Cmd "mpv") {
        $src = [string](Get-Command mpv).Source
        if ($src) {
            if ($src -match '\.com$') {
                $hits = @([System.IO.Path]::ChangeExtension($src, ".exe")) + $hits
            } else {
                $hits = @($src) + $hits
            }
        }
    }
    return First-ExistingApp $hits
}

function Get-VlcPath {
    $hits = @(
        "C:\Program Files\VideoLAN\VLC\vlc.exe",
        "C:\Program Files (x86)\VideoLAN\VLC\vlc.exe",
        (Join-Path $env:USERPROFILE "scoop\apps\vlc\current\vlc.exe")
    )
    if (Test-Cmd "vlc") {
        $src = [string](Get-Command vlc).Source
        if ($src) { $hits = @($src) + $hits }
    }
    return First-ExistingApp $hits
}

function Get-CargoTargetDir {
    try {
        $json = cargo metadata --format-version 1 --no-deps --offline --manifest-path src-tauri/Cargo.toml 2>$null
        if ($json) {
            $meta = $json | ConvertFrom-Json
            if ($meta.target_directory) { return [string]$meta.target_directory }
        }
    } catch { }
    if ($env:CARGO_TARGET_DIR) { return $env:CARGO_TARGET_DIR }
    return (Join-Path $Root "src-tauri\target")
}

function Get-CargoReleaseExe {
    $leaf = "epg-monster-studio.exe"
    $td = Get-CargoTargetDir
    $candidates = @(
        (Join-Path $td (Join-Path "x86_64-pc-windows-gnu" (Join-Path "release" $leaf))),
        (Join-Path $td (Join-Path "release" $leaf)),
        (Join-Path $Root (Join-Path "src-tauri\target" (Join-Path "x86_64-pc-windows-gnu" (Join-Path "release" $leaf)))),
        (Join-Path $Root (Join-Path "src-tauri\target" (Join-Path "release" $leaf)))
    )
    foreach ($c in $candidates) {
        if ($c -and (Test-Path $c)) { return $c }
    }
    return $null
}

function Get-CargoCrateName([string]$PackageId) {
    if ([string]::IsNullOrWhiteSpace($PackageId)) { return $null }
    if ($PackageId -match '#([^@\s"]+)@([^@"\s]+)') {
        return "$($Matches[1]) v$($Matches[2])"
    }
    if ($PackageId -match '^([^@\s"]+)@([^@"\s]+)') {
        return "$($Matches[1]) v$($Matches[2])"
    }
    if ($PackageId -match '(\S+)\s+(\d+\.\d+[^\s)]*)') {
        return "$($Matches[1]) v$($Matches[2])"
    }
    return $null
}

function Get-CargoUnitTotal {
    try {
        $json = cargo metadata --format-version 1 --offline --manifest-path src-tauri/Cargo.toml 2>$null
        if (-not $json) { return 0 }
        $meta = $json | ConvertFrom-Json
        if ($meta.resolve -and $meta.resolve.nodes) { return [int]$meta.resolve.nodes.Count }
        if ($meta.packages) { return [int]$meta.packages.Count }
    } catch { }
    return 0
}

function Add-CargoCrate([string]$Name, [bool]$Fresh) {
    if ([string]::IsNullOrWhiteSpace($Name)) { return }
    if (-not $script:CargoSeen) {
        $script:CargoSeen = New-Object 'System.Collections.Generic.HashSet[string]'
    }
    $list = $script:CompileCrates
    if ($Fresh) { $list = $script:CheckCrates }
    $list.Add($Name) | Out-Null
    $isNew = $script:CargoSeen.Add($Name)
    if (-not $isNew) { return }
    $n = $script:CargoSeen.Count
    if ($script:CargoTotal -lt $n) { $script:CargoTotal = $n }
    $total = $script:CargoTotal
    $pct = 0
    if ($total -gt 0) { $pct = [int][Math]::Floor(100 * $n / $total) }
    if ($pct -gt 99 -and -not $script:CargoDone) { $pct = 99 }
    $verb = "compile"
    if ($Fresh) { $verb = "check" }
    Set-UiRow "cargo" ("($n/$total  $pct%)") "$verb  $Name" "wait"
}

function Copy-LaunchableSidecars([string]$FromDir) {
    $dllNames = @("WebView2Loader.dll")
    foreach ($name in $dllNames) {
        $src = Join-Path $FromDir $name
        if (-not (Test-Path $src)) {
            $src = Join-Path $Root (Join-Path "src-tauri\windows" $name)
        }
        if (Test-Path $src) {
            Copy-Item -Force $src (Join-Path $Root $name)
        }
    }
}

function Use-NpmBin {
    Refresh-Path
}

function Ensure-NpmDeps {
    $tsc = Join-Path $Root "node_modules\typescript\bin\tsc"
    $mods = Join-Path $Root "node_modules"
    if ((Test-Path $mods) -and (Test-Path $tsc)) {
        Set-UiRow "npm" "ok" "node_modules present" "ok"
        return
    }
    Set-UiRow "npm" "installing" "npm install" "wait"
    Invoke-Quiet { npm install } "npm install failed"
    Set-UiRow "npm" "installed" "node_modules" "ok"
}

function Build-Launchable {
    Use-GnuToolchain
    Use-NpmBin
    Ensure-NpmDeps
    Use-NpmBin
    Set-UiRow "UI build" "building" "npm run build" "wait"
    Invoke-Quiet { npm run build } "npm run build failed"
    Set-UiRow "UI build" "ok" "dist\" "ok"
    $script:CompileCrates.Clear()
    $script:CheckCrates.Clear()
    if (-not $script:CargoSeen) {
        $script:CargoSeen = New-Object 'System.Collections.Generic.HashSet[string]'
    } else {
        $script:CargoSeen.Clear()
    }
    $script:CargoDone = $false
    $script:CargoTotal = Get-CargoUnitTotal
    if ($script:CargoTotal -lt 1) { $script:CargoTotal = 1 }
    Set-UiRow "cargo" ("(0/$($script:CargoTotal)  0%)") "starting" "wait"
    $prevEa = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $failed = $false
    $finished = ""
    try {
        & cargo build -p epg-monster-studio --message-format=json --release --features custom-protocol --target x86_64-pc-windows-gnu --manifest-path src-tauri/Cargo.toml 2>&1 | ForEach-Object {
            Poll-QuitKey
            $line = "$_"
            if ($line -match '^\s*\S+\s+:\s+(.*)$') { $line = $Matches[1] }
            $line = $line.Trim()
            if ($line.StartsWith("{")) {
                try {
                    $msg = $line | ConvertFrom-Json
                    if ($msg.reason -eq "compiler-artifact") {
                        $crate = Get-CargoCrateName ([string]$msg.package_id)
                        $fresh = $false
                        if ($msg.PSObject.Properties.Name -contains "fresh") { $fresh = [bool]$msg.fresh }
                        Add-CargoCrate $crate $fresh
                    } elseif ($msg.reason -eq "build-finished") {
                        if ($msg.PSObject.Properties.Name -contains "success" -and -not $msg.success) { $failed = $true }
                    }
                } catch { }
            } elseif ($line -match 'Compiling\s+(\S+)\s+(v\S+)') {
                Add-CargoCrate "$($Matches[1]) $($Matches[2])" $false
            } elseif ($line -match 'Checking\s+(\S+)\s+(v\S+)') {
                Add-CargoCrate "$($Matches[1]) $($Matches[2])" $true
            } elseif ($line -match 'Finished') {
                $finished = $line
            } elseif ($line -match '(?i)error:') {
                $failed = $true
            }
        }
    } finally {
        $ErrorActionPreference = $prevEa
    }
    if ($LASTEXITCODE -ne 0 -or $failed) { throw "cargo build failed" }
    $script:CargoDone = $true
    $n = $script:CompileCrates.Count + $script:CheckCrates.Count
    $total = [Math]::Max($script:CargoTotal, $n)
    $extra = ""
    if ($finished -match 'in (.+)$') { $extra = "  " + $Matches[1] }
    Set-UiRow "cargo" ("($n/$total  100%)") ("done" + $extra) "ok"
    Set-UiRow "launchable" "(1/2  50%)" "copying exe" "wait"
    $built = Get-CargoReleaseExe
    if (-not $built) { throw "cargo build finished but epg-monster-studio.exe was not found in the target dir." }
    Copy-Item -Force $built $Launchable
    Set-UiRow "launchable" "(2/2  100%)" "copying dll" "wait"
    Copy-LaunchableSidecars (Split-Path $built)
    Set-UiRow "launchable" "(2/2  100%)" "ready" "ok"
}

function Install-Studio {
    Reset-Ui "install"
    foreach ($n in @("Node.js","Rust","gcc","ffmpeg","ffprobe","mpv","VLC","npm","data","UI build","cargo","launchable")) {
        $script:UiRows[$n] = @{ State = "..."; Detail = ""; Kind = "wait" }
    }
    Show-UiScreen

    if (-not (Test-Cmd "node")) {
        Set-UiRow "Node.js" "missing" "need 22+" "fail"
        $null = Install-Winget -Label "Node.js LTS" -Key "node" -WingetId "OpenJS.NodeJS.LTS" -ExtraIds @("OpenJS.NodeJS")
        Refresh-Path
        if (-not (Test-Cmd "node")) { throw "Need Node 22+ on PATH (winget OpenJS.NodeJS.LTS)." }
    }
    Set-UiRow "Node.js" "ok" (Get-NativeText { node -v }) "ok"

    Use-GnuToolchain
    if (-not (Test-Cmd "cargo")) {
        Set-UiRow "Rust" "missing" "need cargo" "fail"
        $null = Install-Winget -Label "Rust (rustup)" -Key "rust" -WingetId "Rustlang.Rustup"
        Refresh-Path
        Use-GnuToolchain
        if (-not (Test-Cmd "cargo")) { throw "Need Rust (cargo) on PATH (winget Rustlang.Rustup)." }
    }
    $null = Invoke-Logged "cargo --version" { cargo --version }
    Set-UiRow "Rust" "ok" (Get-NativeText { cargo --version }) "ok"

    if (Test-Cmd "gcc") {
        Set-UiRow "gcc" "ok" (Get-NativeText { gcc --version }) "ok"
    } else {
        Set-UiRow "gcc" "missing" "GNU Windows builds need MinGW" "warn"
        if ((Test-Cmd "winget") -and (Confirm-Yes "Open WinLibs MinGW download page?")) {
            Start-Process "https://winlibs.com/"
        }
    }

    if (-not (Get-FfmpegDir) -or -not (Get-MpvPath) -or -not (Get-VlcPath)) {
        if (-not (Test-Cmd "scoop")) { $null = Install-ScoopIfNeeded }
    }

    $ffmpegDir = Get-FfmpegDir
    if (-not $ffmpegDir) {
        Set-UiRow "ffmpeg" "missing" "need ffmpeg.exe + ffprobe.exe" "fail"
        Set-UiRow "ffprobe" "missing" "" "fail"
        $null = Install-ScoopOrWinget -Label "ffmpeg (includes ffprobe)" -Key "ffmpeg" -ScoopName "ffmpeg" -WingetId "Gyan.FFmpeg"
        $ffmpegDir = Get-FfmpegDir
        if (-not $ffmpegDir) { throw "Need ffmpeg and ffprobe." }
    }
    Set-UiRow "ffmpeg" "ok" (Join-Path $ffmpegDir "ffmpeg.exe") "ok"
    Set-UiRow "ffprobe" "ok" (Join-Path $ffmpegDir "ffprobe.exe") "ok"

    $mpv = Get-MpvPath
    if ($mpv) {
        Set-UiRow "mpv" "found" $mpv "ok"
    } else {
        Set-UiRow "mpv" "missing" "optional Play engine" "warn"
        if (Install-ScoopOrWinget -Label "mpv" -Key "mpv" -ScoopName "mpv" -WingetId "shinchiro.mpv" -ScoopBucket "extras") {
            Refresh-Path
            $mpv = Get-MpvPath
        }
        if ($mpv) { Set-UiRow "mpv" "ok" $mpv "ok" }
        else { Set-UiRow "mpv" "skipped" "Play needs a path in Settings" "warn" }
    }

    $vlc = Get-VlcPath
    if ($vlc) {
        Set-UiRow "VLC" "found" $vlc "ok"
    } else {
        Set-UiRow "VLC" "missing" "optional Play engine" "warn"
        if (Install-ScoopOrWinget -Label "VLC" -Key "vlc" -ScoopName "vlc" -WingetId "VideoLAN.VLC" -ScoopBucket "extras") {
            Refresh-Path
            $vlc = Get-VlcPath
        }
        if ($vlc) { Set-UiRow "VLC" "ok" $vlc "ok" }
        else { Set-UiRow "VLC" "skipped" "Play needs a path in Settings" "warn" }
    }

    Ensure-NpmDeps
    New-Item -ItemType Directory -Force -Path (Join-Path $Root "data") | Out-Null
    Set-UiRow "data" "ok" (Join-Path $Root "data") "ok"
    Build-Launchable
    Snapshot-InstallState
    $script:UiNote = @"
install complete!

-> .\studio.ps1 --shortcuts to install desktop and menu shortcuts
->  then launch via shortcuts or use  .\studio.ps1 --start
log: .\install.log
"@
    Show-UiScreen
}

function Write-Shortcut([string]$Path, [string]$Target) {
    $folder = Split-Path $Path
    if (-not (Test-Path $folder)) {
        New-Item -ItemType Directory -Force -Path $folder | Out-Null
    }
    $w = New-Object -ComObject WScript.Shell
    $lnk = $w.CreateShortcut($Path)
    $lnk.TargetPath = $Target
    $lnk.WorkingDirectory = $Root
    $lnk.WindowStyle = 1
    $lnk.Description = "epg.monster studio"
    if (Test-Path $IconIco) {
        $lnk.IconLocation = $IconIco
    }
    $lnk.Save()
    Write-Step "shortcut" "wrote" $Path "ok"
}

function Install-Shortcuts {
    Reset-Ui "shortcuts"
    Show-UiScreen
    if (-not (Test-Path $Launchable)) {
        Write-Step "launchable" "missing" "running --install first" "warn"
        Install-Studio
    }
    if (-not (Test-Path $Launchable)) {
        throw "Need $Launchable. Re-run --install."
    }
    Write-Shortcut (Get-DesktopLnk) $Launchable
    Write-Shortcut (Get-StartLnk) $Launchable
    Write-Host ""
    Write-Host "  shortcuts ready. data stays in $Root\data" -ForegroundColor Green
    Write-Host ""
}

function Remove-IfExists([string]$Path) {
    if (Test-Path $Path) {
        Remove-Item -Force $Path -ErrorAction SilentlyContinue
        Write-Step "remove" "deleted" $Path "ok"
        return $true
    }
    Write-Step "remove" "absent" $Path "skip"
    return $false
}

function Test-NodeInstalled {
    Refresh-Path
    $rec = Get-ToolRecord "node"
    if ($rec -and $rec.path -and (Test-Path $rec.path)) { return $true }
    if (Test-Cmd "node") { return $true }
    $hits = @(
        (Join-Path $env:USERPROFILE "scoop\apps\nodejs\current\node.exe"),
        (Join-Path $env:USERPROFILE "scoop\apps\nodejs-lts\current\node.exe"),
        "C:\Program Files\nodejs\node.exe"
    )
    foreach ($p in $hits) {
        if (Test-RealExe $p) { return $true }
    }
    return $false
}

function Uninstall-Studio {
    Reset-Ui "uninstall"
    Refresh-Path
    foreach ($n in @("app","Desktop","Start Menu","exe","Node.js","Rust","ffmpeg","mpv","VLC")) {
        $script:UiRows[$n] = @{ State = "..."; Detail = ""; Kind = "wait" }
    }
    Show-UiScreen
    Out-UiLine "" "Gray"
    Out-UiLine "  What do you want to uninstall?  .\data is never deleted." "Yellow"
    if (Confirm-Yes "Remove studio exe and Desktop / Start Menu shortcuts?") {
        Stop-Studio
        if (Test-Path (Get-DesktopLnk)) {
            Remove-Item -Force (Get-DesktopLnk) -ErrorAction SilentlyContinue
            Set-UiRow "Desktop" "removed" (Get-DesktopLnk) "ok"
        } else { Set-UiRow "Desktop" "absent" "" "skip" }
        if (Test-Path (Get-StartLnk)) {
            Remove-Item -Force (Get-StartLnk) -ErrorAction SilentlyContinue
            Set-UiRow "Start Menu" "removed" (Get-StartLnk) "ok"
        } else { Set-UiRow "Start Menu" "absent" "" "skip" }
        if (Test-Path $Launchable) {
            Remove-Item -Force $Launchable -ErrorAction SilentlyContinue
            Set-UiRow "exe" "removed" $Launchable "ok"
        } else { Set-UiRow "exe" "absent" "" "skip" }
        if (Test-Path $WebViewDll) {
            Remove-Item -Force $WebViewDll -ErrorAction SilentlyContinue
        }
        if (Test-Path $PidFile) {
            Remove-Item -Force $PidFile -ErrorAction SilentlyContinue
        }
        Set-UiRow "app" "removed" "exe + shortcuts" "ok"
    } else {
        Set-UiRow "app" "kept" "" "skip"
        Set-UiRow "Desktop" "kept" "" "skip"
        Set-UiRow "Start Menu" "kept" "" "skip"
        Set-UiRow "exe" "kept" "" "skip"
    }

    if ((Test-NodeInstalled) -and (Confirm-No "Uninstall Node.js too?")) {
        Uninstall-ScoopOrWinget -Label "Node.js" -Key "node" -ScoopName "nodejs-lts" -ExtraScoopNames @("nodejs") -WingetId "OpenJS.NodeJS.LTS" -ExtraWingetIds @("OpenJS.NodeJS") -StillThere { Test-NodeInstalled }
        if (Test-NodeInstalled) { Set-UiRow "Node.js" "failed" "see .\uninstall.log" "fail" }
        else { Set-UiRow "Node.js" "removed" "" "ok" }
    } else { Set-UiRow "Node.js" "kept" "" "skip" }

    if (((Test-Cmd "cargo") -or (Test-Cmd "rustup") -or (Get-ToolRecord "rust")) -and (Confirm-No "Uninstall Rust (rustup) too?")) {
        Uninstall-ScoopOrWinget -Label "Rust" -Key "rust" -ScoopName "rustup" -WingetId "Rustlang.Rustup" -StillThere { (Test-Cmd "cargo") -or (Test-Cmd "rustup") }
        if ((Test-Cmd "cargo") -or (Test-Cmd "rustup")) { Set-UiRow "Rust" "failed" "see .\uninstall.log" "fail" }
        else { Set-UiRow "Rust" "removed" "" "ok" }
    } else { Set-UiRow "Rust" "kept" "" "skip" }

    if (((Get-FfmpegDir) -or (Get-ToolRecord "ffmpeg")) -and (Confirm-No "Uninstall ffmpeg/ffprobe too?")) {
        Uninstall-ScoopOrWinget -Label "ffmpeg" -Key "ffmpeg" -ScoopName "ffmpeg" -WingetId "Gyan.FFmpeg" -StillThere { [bool](Get-FfmpegDir) }
        if (Get-FfmpegDir) { Set-UiRow "ffmpeg" "failed" "see .\uninstall.log" "fail" }
        else { Set-UiRow "ffmpeg" "removed" "" "ok" }
    } else { Set-UiRow "ffmpeg" "kept" "" "skip" }

    if (((Get-MpvPath) -or (Get-ToolRecord "mpv")) -and (Confirm-No "Uninstall mpv too?")) {
        Uninstall-ScoopOrWinget -Label "mpv" -Key "mpv" -ScoopName "mpv" -WingetId "shinchiro.mpv" -StillThere { [bool](Get-MpvPath) }
        if (Get-MpvPath) { Set-UiRow "mpv" "failed" "see .\uninstall.log" "fail" }
        else { Set-UiRow "mpv" "removed" "" "ok" }
    } else { Set-UiRow "mpv" "kept" "" "skip" }

    if (((Get-VlcPath) -or (Get-ToolRecord "vlc")) -and (Confirm-No "Uninstall VLC too?")) {
        Uninstall-ScoopOrWinget -Label "VLC" -Key "vlc" -ScoopName "vlc" -WingetId "VideoLAN.VLC" -StillThere { [bool](Get-VlcPath) }
        if (Get-VlcPath) { Set-UiRow "VLC" "failed" "see .\uninstall.log" "fail" }
        else { Set-UiRow "VLC" "removed" "" "ok" }
    } else { Set-UiRow "VLC" "kept" "" "skip" }

    $script:UiNote = "uninstall complete`n`n.\data was not deleted.`nlog: .\uninstall.log"
    Show-UiScreen
}

function Start-Studio {
    if (Test-Running) {
        $procId = (Get-Content $PidFile | Select-Object -First 1).Trim()
        Write-Step "app" "already running" "pid $procId" "ok"
        return
    }
    $env:EPG_MONSTER_HOME = $Root
    Use-NpmBin
    if (Test-Path $Launchable) {
        Write-Phase "build UI (dist\)"
        Invoke-Quiet { npm run build } "npm run build failed"
        Write-Phase "start"
        $p = Start-Process -FilePath $Launchable -WorkingDirectory $Root -PassThru
        Set-Content -Path $PidFile -Value $p.Id
        Write-Step "app" "started" "pid $($p.Id)" "ok"
        return
    }
    Use-GnuToolchain
    Write-Step "launchable" "missing" "cargo run (or --install for a release .exe)" "warn"
    Write-Phase "build UI (dist\)"
    Invoke-Quiet { npm run build } "npm run build failed"
    $p = Start-Process -FilePath "cargo" -ArgumentList @(
        "run","--features","custom-protocol","--target","x86_64-pc-windows-gnu",
        "--manifest-path","src-tauri/Cargo.toml"
    ) -WorkingDirectory $Root -PassThru
    Set-Content -Path $PidFile -Value $p.Id
    Write-Step "app" "started" "pid $($p.Id) cargo run" "ok"
}

$wantInstall = $false
$wantShortcuts = $false
$wantUninstall = $false
$wantStart = $false
$wantStop = $false
$wantHelp = $false
$unknown = @()
if ($args.Count -eq 0) {
    $wantInstall = $true
    $wantStart = $true
} else {
    foreach ($a in $args) {
        switch -Regex ($a) {
            "^--install$|^install$" { $wantInstall = $true }
            "^--shortcuts$|^shortcuts$" { $wantShortcuts = $true }
            "^--uninstall$|^uninstall$" { $wantUninstall = $true }
            "^--start$|^start$" { $wantStart = $true }
            "^--stop$|^stop$" { $wantStop = $true }
            "^--restart$|^restart$" { $wantStop = $true; $wantStart = $true }
            "^--help$|^-h$|^help$" { $wantHelp = $true }
            default { $unknown += $a }
        }
    }
}

try {
    if ($unknown.Count -gt 0) {
        Write-Host "unknown: $($unknown -join ' ')" -ForegroundColor Red
        Show-Usage
        exit 1
    }
    if ($wantHelp) { Show-Usage; exit 0 }
    if ($wantStop) { Start-ActionLog "stop"; Stop-Studio }
    if ($wantUninstall) { Start-ActionLog "uninstall"; Uninstall-Studio }
    if ($wantInstall) { Start-ActionLog "install"; Install-Studio }
    if ($wantShortcuts) { Start-ActionLog "shortcuts"; Install-Shortcuts }
    if ($wantStart) { Start-ActionLog "start"; Start-Studio }
} finally {
    try { [Console]::CursorVisible = $true } catch { }
}
