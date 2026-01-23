# Claude Code 글로벌 설정 설치 스크립트 (Windows PowerShell)
# 사용법: .\install.ps1

$ErrorActionPreference = "Stop"

# 로고
Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║        Claude Code 글로벌 설정 설치 스크립트              ║" -ForegroundColor Cyan
Write-Host "║   jh941213 + affaan-m 통합 버전                          ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# 변수 설정
$ClaudeDir = "$env:USERPROFILE\.claude"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# 디렉토리 생성
Write-Host "📁 디렉토리 생성 중..." -ForegroundColor Yellow
$Dirs = @("agents", "commands", "rules", "hooks")
foreach ($Dir in $Dirs) {
    New-Item -ItemType Directory -Force -Path "$ClaudeDir\$Dir" | Out-Null
}

# 백업 (기존 설정이 있는 경우)
if (Test-Path "$ClaudeDir\CLAUDE.md") {
    $BackupDir = "$ClaudeDir\backup-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
    Write-Host "📦 기존 설정 백업 중... → $BackupDir" -ForegroundColor Yellow
    New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
    Copy-Item "$ClaudeDir\agents\*" -Destination "$BackupDir\agents\" -Force -ErrorAction SilentlyContinue
    Copy-Item "$ClaudeDir\commands\*" -Destination "$BackupDir\commands\" -Force -ErrorAction SilentlyContinue
    Copy-Item "$ClaudeDir\rules\*" -Destination "$BackupDir\rules\" -Force -ErrorAction SilentlyContinue
    Copy-Item "$ClaudeDir\hooks\*" -Destination "$BackupDir\hooks\" -Force -ErrorAction SilentlyContinue
    Copy-Item "$ClaudeDir\CLAUDE.md" -Destination $BackupDir -Force -ErrorAction SilentlyContinue
    Copy-Item "$ClaudeDir\settings.json" -Destination $BackupDir -Force -ErrorAction SilentlyContinue
}

# Agents 복사
Write-Host "🤖 Agents 설치 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\agents") {
    Copy-Item "$ScriptDir\agents\*.md" -Destination "$ClaudeDir\agents\" -Force
    $AgentCount = (Get-ChildItem "$ScriptDir\agents\*.md" | Measure-Object).Count
    Write-Host "   ✅ ${AgentCount}개 에이전트 설치됨" -ForegroundColor Green
}

# Commands 복사
Write-Host "⚡ Commands 설치 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\commands") {
    Copy-Item "$ScriptDir\commands\*.md" -Destination "$ClaudeDir\commands\" -Force
    $CommandCount = (Get-ChildItem "$ScriptDir\commands\*.md" | Measure-Object).Count
    Write-Host "   ✅ ${CommandCount}개 커맨드 설치됨" -ForegroundColor Green
}

# Rules 복사
Write-Host "📏 Rules 설치 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\rules") {
    Copy-Item "$ScriptDir\rules\*.md" -Destination "$ClaudeDir\rules\" -Force
    $RuleCount = (Get-ChildItem "$ScriptDir\rules\*.md" | Measure-Object).Count
    Write-Host "   ✅ ${RuleCount}개 규칙 설치됨" -ForegroundColor Green
}

# Hooks 복사
Write-Host "🪝 Hooks 설치 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\hooks") {
    Copy-Item "$ScriptDir\hooks\*.js" -Destination "$ClaudeDir\hooks\" -Force
    Copy-Item "$ScriptDir\hooks\hooks.json" -Destination "$ClaudeDir\hooks\" -Force
    $HookCount = (Get-ChildItem "$ScriptDir\hooks\*.js" | Measure-Object).Count
    Write-Host "   ✅ ${HookCount}개 훅 설치됨" -ForegroundColor Green
}

# CLAUDE.md 복사
Write-Host "📄 CLAUDE.md 설치 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\CLAUDE.md") {
    Copy-Item "$ScriptDir\CLAUDE.md" -Destination "$ClaudeDir\CLAUDE.md" -Force
    Write-Host "   ✅ 글로벌 CLAUDE.md 설치됨" -ForegroundColor Green
}

# settings.json 복사 (없는 경우에만)
Write-Host "⚙️  settings.json 확인 중..." -ForegroundColor Green
if (Test-Path "$ScriptDir\settings.json") {
    if (-not (Test-Path "$ClaudeDir\settings.json")) {
        Copy-Item "$ScriptDir\settings.json" -Destination "$ClaudeDir\settings.json" -Force
        Write-Host "   ✅ settings.json 설치됨" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  기존 settings.json 유지 (수동 병합 필요)" -ForegroundColor Yellow
    }
}

# 완료 메시지
Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Green
Write-Host "✅ 설치 완료!" -ForegroundColor Green
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Green
Write-Host ""
Write-Host "설치 위치: $ClaudeDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "다음 단계:"
Write-Host "  1. Claude Code를 재시작하세요"
Write-Host "  2. 새 세션에서 설정이 자동으로 적용됩니다"
Write-Host "  3. /help로 사용 가능한 명령어를 확인하세요"
Write-Host ""
