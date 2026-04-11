# bump-version.ps1 — 3곳의 버전을 동시에 업데이트
# 사용법: .\scripts\bump-version.ps1 2.1.0
#         .\scripts\bump-version.ps1 2.1.0 -DryRun

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,

    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# 프로젝트 루트 탐지 (package.json 기준)
$packageJson = Join-Path $PSScriptRoot "..\package.json"
if (-not (Test-Path $packageJson)) {
    $packageJson = Join-Path (Get-Location) "package.json"
}
$projectRoot = Split-Path -Parent (Resolve-Path $packageJson)

$files = @(
    @{ Path = "$projectRoot\package.json";              Pattern = '"version": "[^"]*"';    Replace = "`"version`": `"$Version`"" },
    @{ Path = "$projectRoot\src-tauri\Cargo.toml";      Pattern = '^version = "[^"]*"';   Replace = "version = `"$Version`"" },
    @{ Path = "$projectRoot\src-tauri\tauri.conf.json";  Pattern = '"version": "[^"]*"';   Replace = "`"version`": `"$Version`"" }
)

if ($DryRun) {
    Write-Host "[DRY RUN] Would update to v$Version:" -ForegroundColor Magenta
}

foreach ($file in $files) {
    if (-not (Test-Path $file.Path)) {
        Write-Warning "File not found: $($file.Path)"
        continue
    }
    $content = Get-Content $file.Path -Raw -Encoding UTF8
    $updated = $content -replace $file.Pattern, $file.Replace
    if ($content -eq $updated) {
        Write-Warning "No change in $($file.Path)"
    } elseif ($DryRun) {
        Write-Host "  [DRY] $($file.Path) -> v$Version" -ForegroundColor Yellow
    } else {
        [System.IO.File]::WriteAllText($file.Path, $updated, [System.Text.UTF8Encoding]::new($false))
        Write-Host "Updated $($file.Path) -> v$Version" -ForegroundColor Green
    }
}

Write-Host ""
if ($DryRun) {
    Write-Host "Dry run complete. No files were modified." -ForegroundColor Magenta
} else {
    Write-Host "Version bumped to v$Version in all 3 files." -ForegroundColor Cyan
    Write-Host "Next steps:" -ForegroundColor Yellow
    Write-Host "  1. Update CHANGELOG.md" -ForegroundColor Yellow
    Write-Host "  2. Run 'cargo check' to update Cargo.lock" -ForegroundColor Yellow
    Write-Host "  3. Commit and tag: git tag v${Version}" -ForegroundColor Yellow
}
