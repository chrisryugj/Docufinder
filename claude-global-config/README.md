# Claude Code 글로벌 설정 통합 패키지

> jh941213/my-claude-code-asset + affaan-m/everything-claude-code 장점 통합

---

## 빠른 시작

### Linux / macOS
```bash
cd claude-global-config
chmod +x install.sh
./install.sh
```

### Windows (PowerShell)
```powershell
cd claude-global-config
.\install.ps1
```

### 설치 확인
```bash
ls -la ~/.claude/
```

---

## 포함된 구성요소

### Agents (10개)
특화된 작업을 위한 에이전트

| 에이전트 | 역할 | 출처 |
|----------|------|------|
| planner | 전략적 계획 수립 | 공통 |
| architect | 시스템 설계 | 공통 |
| frontend-developer | React/TS/Tailwind UI | jh941213 |
| code-reviewer | 코드 품질 분석 | 공통 |
| security-reviewer | 보안 취약점 검토 | 공통 |
| tdd-guide | 테스트 주도 개발 | 공통 |
| build-error-resolver | 빌드 에러 자동 수정 | affaan-m |
| e2e-runner | Playwright E2E | affaan-m |
| refactor-cleaner | 데드코드 제거 | affaan-m |
| doc-updater | 문서 동기화 | affaan-m |

### Commands (16개)
슬래시 커맨드로 빠른 실행

| 커맨드 | 용도 |
|--------|------|
| `/plan` | 작업 계획 |
| `/verify` | 린트+테스트+빌드 |
| `/tdd` | 테스트 주도 개발 |
| `/build-fix` | 빌드 에러 수정 |
| `/review` | 코드 리뷰 |
| `/commit-push-pr` | Git 워크플로우 |
| `/handoff` | 세션 인계 |
| `/checkpoint` | 상태 저장 |
| `/frontend` | UI 개발 |
| `/e2e` | E2E 테스트 |
| `/refactor-clean` | 코드 정리 |
| `/simplify` | 코드 단순화 |
| `/update-docs` | 문서 갱신 |
| `/learn` | 패턴 학습 |
| `/test-coverage` | 커버리지 |
| `/compact-guide` | 토큰 관리 |

### Rules (8개)
자동으로 적용되는 규칙

- `security` - 보안 규칙 (시크릿, 인젝션 방지)
- `coding-style` - 코딩 스타일 (크기 제한, 네이밍)
- `testing` - 테스트 규칙 (커버리지, TDD)
- `git-workflow` - Git 규칙 (커밋, 브랜치)
- `performance` - 성능 규칙 (토큰, 최적화)
- `agents` - 에이전트 위임 규칙
- `patterns` - 디자인 패턴
- `hooks` - Hooks 사용 가이드

### Hooks (9개)
이벤트 기반 자동화

| Hook | 트리거 | 기능 |
|------|--------|------|
| session-start | 세션 시작 | HANDOFF 확인, 프로젝트 감지 |
| session-end | 세션 종료 | 상태 저장 안내 |
| pre-edit-check | Edit 전 | 보안 검사, console.log 감지 |
| post-edit-lint | Edit 후 | TypeScript 타입 체크 |
| git-push-review | git push 전 | 변경사항 확인 |
| documentation-guard | Write .md | 불필요한 문서 방지 |
| pre-compact | 압축 전 | 상태 백업 |
| compaction-warning | 압축 시 | 경고 메시지 |

---

## 문서 목록

| 문서 | 설명 |
|------|------|
| [ANALYSIS.md](ANALYSIS.md) | 두 저장소 상세 비교 분석 |
| [AUTOMATION_WORKFLOW.md](AUTOMATION_WORKFLOW.md) | 자동화 워크플로우 설계 |
| [CLAUDE.md](CLAUDE.md) | 글로벌 설정 파일 |

---

## 사용법

### 1. 복잡한 기능 개발
```
"로그인 기능 구현해줘"
→ 자동으로 planner 호출 → 계획 수립 → 단계별 구현
```

### 2. 빌드 에러 발생 시
```
빌드 실패 감지 → build-error-resolver 자동 호출 → 에러 분석 및 수정
```

### 3. 세션 관리
```
토큰 70% 도달 → 알림 표시
→ /checkpoint 또는 /handoff 실행 권장
```

### 4. 다음 세션에서 이어서
```
"HANDOFF.md 읽고 이어서 작업해줘"
```

---

## 커스터마이징

### 프로젝트별 설정
프로젝트 루트에 `CLAUDE.md` 생성:
```markdown
# 프로젝트 설정

## 기술 스택
- ...

## 프로젝트 특화 규칙
- ...
```

### 새 에이전트 추가
`~/.claude/agents/my-agent.md` 생성:
```markdown
---
name: my-agent
description: 설명
tools: [Read, Write, ...]
---
# 에이전트 내용
```

### 새 커맨드 추가
`~/.claude/commands/my-command.md` 생성:
```markdown
---
name: my-command
description: 설명
---
# 커맨드 내용
```

---

## 기여 출처

| 구성요소 | jh941213 | affaan-m |
|----------|:--------:|:--------:|
| frontend-developer | ✅ | |
| build-error-resolver | | ✅ |
| e2e-runner | | ✅ |
| refactor-cleaner | | ✅ |
| doc-updater | | ✅ |
| /commit-push-pr | ✅ | |
| /handoff | ✅ | |
| /checkpoint | | ✅ |
| /learn | | ✅ |
| Hooks 시스템 | | ✅ |
| HANDOFF 프로토콜 | ✅ | |
| 한국어 지원 | ✅ | |

---

## 라이선스

MIT License - 자유롭게 사용, 수정, 배포 가능
