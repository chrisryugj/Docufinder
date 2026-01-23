#!/bin/bash
#
# Claude Code 글로벌 설정 설치 스크립트
# 사용법: ./install.sh
#

set -e

# 색상 정의
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 로고
echo ""
echo -e "${BLUE}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║        Claude Code 글로벌 설정 설치 스크립트              ║${NC}"
echo -e "${BLUE}║   jh941213 + affaan-m 통합 버전                          ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""

# 변수 설정
CLAUDE_DIR="$HOME/.claude"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 디렉토리 생성
echo -e "${YELLOW}📁 디렉토리 생성 중...${NC}"
mkdir -p "$CLAUDE_DIR"/{agents,commands,rules,hooks}

# 백업 (기존 설정이 있는 경우)
if [ -f "$CLAUDE_DIR/CLAUDE.md" ]; then
    BACKUP_DIR="$CLAUDE_DIR/backup-$(date +%Y%m%d-%H%M%S)"
    echo -e "${YELLOW}📦 기존 설정 백업 중... → $BACKUP_DIR${NC}"
    mkdir -p "$BACKUP_DIR"
    cp -r "$CLAUDE_DIR"/{agents,commands,rules,hooks,CLAUDE.md,settings.json} "$BACKUP_DIR/" 2>/dev/null || true
fi

# Agents 복사
echo -e "${GREEN}🤖 Agents 설치 중...${NC}"
if [ -d "$SCRIPT_DIR/agents" ]; then
    cp "$SCRIPT_DIR/agents/"*.md "$CLAUDE_DIR/agents/" 2>/dev/null || true
    echo "   ✅ $(ls -1 "$SCRIPT_DIR/agents/"*.md 2>/dev/null | wc -l)개 에이전트 설치됨"
fi

# Commands 복사
echo -e "${GREEN}⚡ Commands 설치 중...${NC}"
if [ -d "$SCRIPT_DIR/commands" ]; then
    cp "$SCRIPT_DIR/commands/"*.md "$CLAUDE_DIR/commands/" 2>/dev/null || true
    echo "   ✅ $(ls -1 "$SCRIPT_DIR/commands/"*.md 2>/dev/null | wc -l)개 커맨드 설치됨"
fi

# Rules 복사
echo -e "${GREEN}📏 Rules 설치 중...${NC}"
if [ -d "$SCRIPT_DIR/rules" ]; then
    cp "$SCRIPT_DIR/rules/"*.md "$CLAUDE_DIR/rules/" 2>/dev/null || true
    echo "   ✅ $(ls -1 "$SCRIPT_DIR/rules/"*.md 2>/dev/null | wc -l)개 규칙 설치됨"
fi

# Hooks 복사 및 실행 권한 부여
echo -e "${GREEN}🪝 Hooks 설치 중...${NC}"
if [ -d "$SCRIPT_DIR/hooks" ]; then
    cp "$SCRIPT_DIR/hooks/"*.js "$CLAUDE_DIR/hooks/" 2>/dev/null || true
    cp "$SCRIPT_DIR/hooks/hooks.json" "$CLAUDE_DIR/hooks/" 2>/dev/null || true
    chmod +x "$CLAUDE_DIR/hooks/"*.js 2>/dev/null || true
    echo "   ✅ $(ls -1 "$SCRIPT_DIR/hooks/"*.js 2>/dev/null | wc -l)개 훅 설치됨"
fi

# CLAUDE.md 복사
echo -e "${GREEN}📄 CLAUDE.md 설치 중...${NC}"
if [ -f "$SCRIPT_DIR/CLAUDE.md" ]; then
    cp "$SCRIPT_DIR/CLAUDE.md" "$CLAUDE_DIR/CLAUDE.md"
    echo "   ✅ 글로벌 CLAUDE.md 설치됨"
fi

# settings.json 복사 (없는 경우에만)
echo -e "${GREEN}⚙️  settings.json 확인 중...${NC}"
if [ -f "$SCRIPT_DIR/settings.json" ]; then
    if [ ! -f "$CLAUDE_DIR/settings.json" ]; then
        cp "$SCRIPT_DIR/settings.json" "$CLAUDE_DIR/settings.json"
        echo "   ✅ settings.json 설치됨"
    else
        echo "   ⚠️  기존 settings.json 유지 (수동 병합 필요)"
        echo "   → $SCRIPT_DIR/settings.json 참고"
    fi
fi

# 완료 메시지
echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✅ 설치 완료!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "설치 위치: ${BLUE}$CLAUDE_DIR${NC}"
echo ""
echo "설치된 구성요소:"
echo "  • Agents:   $(ls -1 "$CLAUDE_DIR/agents/"*.md 2>/dev/null | wc -l)개"
echo "  • Commands: $(ls -1 "$CLAUDE_DIR/commands/"*.md 2>/dev/null | wc -l)개"
echo "  • Rules:    $(ls -1 "$CLAUDE_DIR/rules/"*.md 2>/dev/null | wc -l)개"
echo "  • Hooks:    $(ls -1 "$CLAUDE_DIR/hooks/"*.js 2>/dev/null | wc -l)개"
echo ""
echo "다음 단계:"
echo "  1. Claude Code를 재시작하세요"
echo "  2. 새 세션에서 설정이 자동으로 적용됩니다"
echo "  3. /help로 사용 가능한 명령어를 확인하세요"
echo ""
