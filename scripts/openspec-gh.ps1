# Map an OpenSpec change folder to GitHub issues (epic + tasks.md checkboxes).
# Usage: .\scripts\openspec-gh.ps1 [-Change] <folder> [--dry-run] [--no-close]
# Creates missing epic/task issues, writes openspec/changes/<name>/github.md,
# and closes issues whose tasks.md lines are marked [x].
$ErrorActionPreference = "Stop"
$Change = $null
$DryRun = $false
$CloseDone = $true
for ($i = 0; $i -lt $args.Count; $i++) {
    $a = [string]$args[$i]
    if ($a -eq "--dry-run") { $DryRun = $true; continue }
    if ($a -eq "--no-close") { $CloseDone = $false; continue }
    if ($a -eq "-Change" -or $a -eq "--change") {
        if ($i + 1 -lt $args.Count) { $Change = [string]$args[$i + 1]; $i++ }
        continue
    }
    if ($a -notmatch "^-") { $Change = $a }
}
if (-not $Change) {
    Write-Host "usage: .\scripts\openspec-gh.ps1 <openspec/changes folder> [--dry-run] [--no-close]"
    exit 1
}

$Root = Split-Path $PSScriptRoot -Parent
if (-not $Root) { $Root = (Get-Location).Path }
$changeDir = Join-Path $Root (Join-Path "openspec\changes" $Change)
if (-not (Test-Path $changeDir)) {
    throw "missing change folder: $changeDir"
}
$tasksPath = Join-Path $changeDir "tasks.md"
$mapPath = Join-Path $changeDir "github.md"
$proposalPath = Join-Path $changeDir "proposal.md"

function Ensure-Label([string]$Name, [string]$Color, [string]$Desc) {
    $existing = gh label list --json name --jq ".[].name" 2>$null
    if ($existing -split "`n" -contains $Name) { return }
    gh label create $Name --color $Color --description $Desc 2>$null | Out-Null
}

function Get-IssueLabels {
    $labels = @("openspec", "v2")
    if ($Change -match "install") { $labels += "install-scripts" }
    if ($Change -match "v3|ghoul") { $labels = @("openspec", "v3") }
    return $labels
}

function Invoke-GhIssueCreate([string]$Title, [string]$Body, [string[]]$Labels) {
    $ghArgs = @("issue", "create", "--title", $Title, "--body", $Body)
    foreach ($lab in $Labels) { $ghArgs += @("--label", $lab) }
    return & gh @ghArgs
}

function Close-MappedIssue([string]$Num, [string]$Reason) {
    if (-not $Num -or $Num -eq "0") { return }
    if ($DryRun) {
        Write-Host "would close #$Num"
        return
    }
    $state = gh issue view $Num --json state --jq .state 2>$null
    if ($state -ne "OPEN") { return }
    gh issue close $Num --comment $Reason | Out-Null
    Write-Host "closed #$Num"
}

Ensure-Label "openspec" "0E8A16" "OpenSpec change"
Ensure-Label "v2" "1D76DB" "v2 tester scope"
Ensure-Label "v3" "5319E7" "parked for v3"
Ensure-Label "install-scripts" "FBCA04" "studio.ps1 / studio.sh"
Ensure-Label "tracking" "C5DEF5" "process / board, not product UI"

$epicNum = $null
$taskMap = @{}
if (Test-Path $mapPath) {
    foreach ($line in Get-Content $mapPath) {
        if ($line -match "^epic:\s*(\d+)") { $epicNum = $Matches[1]; continue }
        if ($line -match "^\|\s*task\s*\|\s*issue\s*\|") { continue }
        if ($line -match "^\|\s*-+\s*\|\s*-+\s*\|") { continue }
        if ($line -match "^\|\s*(.+?)\s*\|\s*(\d+)\s*\|") {
            $taskMap[$Matches[1].Trim()] = $Matches[2]
        }
    }
}

$proposal = ""
if (Test-Path $proposalPath) {
    $proposal = (Get-Content $proposalPath -Raw)
    if ($proposal.Length -gt 2500) { $proposal = $proposal.Substring(0, 2500) + "`n..." }
}

if (-not $epicNum) {
    $body = @"
OpenSpec change: ``openspec/changes/$Change/``

$proposal

Do not put access keys or provider stream URLs in comments.
"@
    $epicLabels = @("openspec", "v2", "tracking")
    if ($Change -match "install") { $epicLabels += "install-scripts" }
    if ($Change -match "v3|ghoul") { $epicLabels = @("openspec", "v3", "tracking") }
    if ($DryRun) {
        Write-Host "would create epic: [openspec] $Change"
        $epicNum = "0"
    } else {
        $url = Invoke-GhIssueCreate "[openspec] $Change" $body $epicLabels
        if ($url -match "/issues/(\d+)") { $epicNum = $Matches[1] }
        Write-Host "epic #$epicNum $url"
    }
}

$taskRows = @()
if (Test-Path $tasksPath) {
    foreach ($line in Get-Content $tasksPath) {
        if ($line -match "^- \[([ xX])\] (.+)$") {
            $done = ($Matches[1] -ne " ")
            $text = $Matches[2].Trim()
            $taskRows += [pscustomobject]@{ Done = $done; Text = $text }
        }
    }
}

$taskLabels = Get-IssueLabels
foreach ($row in $taskRows) {
    $t = $row.Text
    if (-not $taskMap.ContainsKey($t)) {
        if ($row.Done) { continue }
        $taskBody = @"
Parent: #$epicNum
Change: ``openspec/changes/$Change/``
Task: $t
"@
        if ($DryRun) {
            Write-Host "would create task: $t"
            $taskMap[$t] = "0"
        } else {
            $url = Invoke-GhIssueCreate "[$Change] $t" $taskBody $taskLabels
            $num = "0"
            if ($url -match "/issues/(\d+)") { $num = $Matches[1] }
            $taskMap[$t] = $num
            Write-Host "task #$num $t"
        }
    }
    if ($CloseDone -and $row.Done -and $taskMap.ContainsKey($t)) {
        Close-MappedIssue $taskMap[$t] "Checkbox marked done in openspec/changes/$Change/tasks.md."
    }
}

$md = New-Object System.Collections.Generic.List[string]
[void]$md.Add("# GitHub mapping")
[void]$md.Add("")
[void]$md.Add("epic: $epicNum")
[void]$md.Add("")
[void]$md.Add("| task | issue |")
[void]$md.Add("|------|-------|")
$written = @{}
foreach ($row in $taskRows) {
    if (-not $taskMap.ContainsKey($row.Text)) { continue }
    [void]$md.Add("| $($row.Text) | $($taskMap[$row.Text]) |")
    $written[$row.Text] = $true
}
foreach ($k in $taskMap.Keys) {
    if ($written.ContainsKey($k)) { continue }
    [void]$md.Add("| $k | $($taskMap[$k]) |")
}
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($mapPath, (($md -join "`n") + "`n"), $utf8)
Write-Host "wrote $mapPath"
