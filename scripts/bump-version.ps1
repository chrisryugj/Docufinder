# bump-version.ps1 — Update version in 3 files simultaneously
# Usage: .\scripts\bump-version.ps1 2.1.0
#        .\scripts\bump-version.ps1 2.1.0 -DryRun

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string]$Version,

    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# Detect project root (based on package.json)
$packageJson = Join-Path $PSScriptRoot "..\package.json"
if (-not (Test-Path $packageJson)) {
    $packageJson = Join-Path (Get-Location) "package.json"
}
$projectRoot = Split-Path -Parent (Resolve-Path $packageJson)

$files = @(
    @{ Path = "$projectRoot\package.json";              Pattern = '"version": "[^"]*"';    Replace = "`"version`": `"$Version`"" },
    # (?m) 필수 — 없으면 `^` 가 파일 선두에만 걸려 Cargo.toml 은 조용히 스킵된다.
    # 그러면 package.json 만 올라가 release.sh 의 버전 일치 검사를 통과한 채로
    # 바이너리 버전만 옛 값으로 배포된다. 들여쓴 의존성 `version = ` 은 열 0 이 아니라 안 걸린다.
    @{ Path = "$projectRoot\src-tauri\Cargo.toml";      Pattern = '(?m)^version = "[^"]*"'; Replace = "version = `"$Version`"" },
    @{ Path = "$projectRoot\src-tauri\tauri.conf.json";  Pattern = '"version": "[^"]*"';   Replace = "`"version`": `"$Version`"" }
)

if ($DryRun) {
    # ${Version} 로 감싼다 — `$Version:` 는 PowerShell 이 드라이브 한정 변수로 파싱해
    # 스크립트 전체가 ParserError 로 죽었다 (DryRun 여부와 무관하게 실행 불가였음).
    Write-Host "[DRY RUN] Would update to v${Version}:" -ForegroundColor Magenta
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
