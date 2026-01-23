# Claude Code 글로벌 설정 - 상세 비교 분석

> jh941213/my-claude-code-asset vs affaan-m/everything-claude-code 완전 비교

---

## 1. 저장소 개요

| 항목 | jh941213 | affaan-m |
|------|----------|----------|
| **Stars** | 40+ | 100+ |
| **배경** | 한국 개발자 개인 설정 | Anthropic 해커톤 우승자, 10개월+ 사용 |
| **언어** | 한국어 | 영어 |
| **설치 방식** | 수동 복사 / 스크립트 | Plugin marketplace 지원 |
| **테스트** | 없음 | 테스트 스위트 포함 |
| **크로스 플랫폼** | 부분 지원 | Windows/macOS/Linux 완전 지원 |

---

## 2. Agents 완전 비교

### 2.1 전체 목록

| Agent | jh941213 | affaan-m | 통합 채택 | 채택 이유 |
|-------|:--------:|:--------:|:--------:|-----------|
| planner | ✅ | ✅ | ✅ | 둘 다 보유, 기능 유사 |
| architect | ✅ | ✅ | ✅ | 둘 다 보유, 기능 유사 |
| code-reviewer | ✅ | ✅ | ✅ | 둘 다 보유, 기능 유사 |
| security-reviewer | ✅ | ✅ | ✅ | 둘 다 보유, 기능 유사 |
| tdd-guide | ✅ | ✅ | ✅ | 둘 다 보유, affaan-m이 더 상세 |
| **frontend-developer** | ✅ | ❌ | ✅ | jh941213 고유, React/TS 전문 |
| **build-error-resolver** | ❌ | ✅ | ✅ | affaan-m 고유, 자동 빌드 수정 |
| **e2e-runner** | ❌ | ✅ | ✅ | affaan-m 고유, Playwright E2E |
| **refactor-cleaner** | ❌ | ✅ | ✅ | affaan-m 고유, 데드코드 제거 |
| **doc-updater** | ❌ | ✅ | ✅ | affaan-m 고유, 문서 동기화 |

### 2.2 채택/제외 분석

#### ✅ 채택한 것들

**jh941213에서 채택:**
- `frontend-developer`: React/TypeScript/Tailwind 전문 에이전트. affaan-m에는 없는 UI 개발 특화 에이전트

**affaan-m에서 채택:**
- `build-error-resolver`: 빌드 에러 자동 분석/수정. 실용성 높음
- `e2e-runner`: Playwright 기반 E2E 테스트. 프로덕션 품질에 필수
- `refactor-cleaner`: 미사용 코드 자동 식별/제거
- `doc-updater`: 코드 변경에 따른 문서 자동 업데이트

#### ❌ 제외한 것 없음
- 모든 에이전트 채택 (총 10개)

---

## 3. Commands 완전 비교

### 3.1 전체 목록

| Command | jh941213 | affaan-m | 통합 채택 | 채택 이유 |
|---------|:--------:|:--------:|:--------:|-----------|
| /plan | ✅ | ✅ | ✅ | 필수, 작업 계획 |
| /verify | ✅ | ✅ | ✅ | 필수, 검증 루프 |
| /tdd | ✅ | ✅ | ✅ | affaan-m이 더 상세 |
| /build-fix | ✅ | ✅ | ✅ | 둘 다 보유 |
| /review | ✅ | ✅ (code-review) | ✅ | 이름만 다름 |
| **커밋/PR 관련** |
| /commit-push-pr | ✅ | ❌ | ✅ | jh941213 고유, Git 통합 |
| **세션 관리** |
| /handoff | ✅ | ❌ | ✅ | jh941213 고유, 세션 인계 |
| /compact-guide | ✅ | ❌ | ✅ | jh941213 고유, 토큰 관리 |
| /checkpoint | ❌ | ✅ | ✅ | affaan-m 고유, 상태 저장 |
| **코드 정리** |
| /simplify | ✅ | ❌ | ✅ | jh941213 고유, 코드 단순화 |
| /refactor-clean | ❌ | ✅ | ✅ | affaan-m 고유, 데드코드 |
| **UI 개발** |
| /frontend | ✅ | ❌ | ✅ | jh941213 고유, UI 워크플로우 |
| **테스트** |
| /e2e | ❌ | ✅ | ✅ | affaan-m 고유, E2E 테스트 |
| /test-coverage | ❌ | ✅ | ✅ | affaan-m 고유, 커버리지 |
| **학습/패턴** |
| /learn | ❌ | ✅ | ✅ | affaan-m 고유, 패턴 추출 |
| /eval | ❌ | ✅ | ⚠️ | 평가 특화, 선택적 |
| **문서화** |
| /update-docs | ❌ | ✅ | ✅ | affaan-m 고유, 문서 갱신 |
| /update-codemaps | ❌ | ✅ | ⚠️ | 코드맵 특화, 선택적 |
| **기타** |
| /setup-pm | ❌ | ✅ | ✅ | 패키지 매니저 설정 |
| /orchestrate | ❌ | ✅ | ⚠️ | 복잡한 워크플로우, 선택적 |

### 3.2 채택/제외 분석

#### ✅ 필수 채택 (16개)
| Command | 출처 | 이유 |
|---------|------|------|
| /plan | 둘 다 | 필수 워크플로우 |
| /verify | 둘 다 | 품질 보장 필수 |
| /tdd | 둘 다 | TDD 워크플로우 |
| /build-fix | 둘 다 | 빌드 에러 해결 |
| /review | 둘 다 | 코드 리뷰 |
| /commit-push-pr | jh941213 | Git 워크플로우 통합 (매우 실용적) |
| /handoff | jh941213 | 세션 인계 (컨텍스트 관리 핵심) |
| /compact-guide | jh941213 | 토큰 관리 가이드 |
| /checkpoint | affaan-m | 상태 저장점 |
| /simplify | jh941213 | 코드 단순화 |
| /refactor-clean | affaan-m | 데드코드 제거 |
| /frontend | jh941213 | UI 개발 워크플로우 |
| /e2e | affaan-m | E2E 테스트 |
| /test-coverage | affaan-m | 커버리지 확인 |
| /learn | affaan-m | 패턴 학습 |
| /update-docs | affaan-m | 문서 동기화 |

#### ⚠️ 선택적 채택 (3개)
| Command | 출처 | 이유 |
|---------|------|------|
| /eval | affaan-m | 평가 특화, 일반 프로젝트에서 불필요할 수 있음 |
| /update-codemaps | affaan-m | 코드맵 특화, 대형 프로젝트에만 유용 |
| /orchestrate | affaan-m | 매우 복잡한 워크플로우용 |

#### ❌ 제외 (1개)
| Command | 출처 | 제외 이유 |
|---------|------|----------|
| /setup-pm | affaan-m | 패키지 매니저 자동 감지로 대체 가능 |

---

## 4. Rules 완전 비교

### 4.1 전체 목록

| Rule | jh941213 | affaan-m | 통합 채택 | 채택 이유 |
|------|:--------:|:--------:|:--------:|-----------|
| security | ✅ | ✅ | ✅ | 둘 다, affaan-m이 더 상세 |
| coding-style | ✅ | ✅ | ✅ | jh941213 한국어 지원 추가 |
| testing | ✅ | ✅ | ✅ | affaan-m이 커버리지 상세 |
| git-workflow | ✅ | ✅ | ✅ | 둘 다 유사 |
| performance | ✅ | ✅ | ✅ | 둘 다 유사 |
| **agents** | ❌ | ✅ | ✅ | affaan-m 고유, 위임 규칙 |
| **hooks** | ❌ | ✅ | ✅ | affaan-m 고유, hooks 가이드 |
| **patterns** | ❌ | ✅ | ✅ | affaan-m 고유, 디자인 패턴 |

### 4.2 채택/제외 분석

#### ✅ 전부 채택 (8개)
모든 규칙 채택. affaan-m의 추가 규칙(agents, hooks, patterns)이 유용함.

---

## 5. Skills 완전 비교

### 5.1 jh941213 Skills (11개)

| Skill | 설명 | 채택 | 이유 |
|-------|------|:----:|------|
| react-patterns | React 컴포넌트 패턴 | ✅ | 필수 |
| typescript-advanced-types | TS 고급 타입 | ✅ | 필수 |
| tailwind-design-system | Tailwind 시스템 | ✅ | 필수 |
| shadcn-ui | shadcn/ui 사용법 | ✅ | UI 라이브러리 |
| ui-ux-pro-max | UI/UX 가이드 | ✅ | 디자인 품질 |
| web-design-guidelines | 웹 디자인 표준 | ✅ | 디자인 표준 |
| vercel-react-best-practices | Vercel 최적화 | ⚠️ | Vercel 사용시만 |
| api-design-principles | API 설계 원칙 | ✅ | 백엔드 필수 |
| fastapi-templates | FastAPI 템플릿 | ⚠️ | Python 백엔드시만 |
| async-python-patterns | Python 비동기 | ⚠️ | Python 사용시만 |
| python-testing-patterns | Python 테스트 | ⚠️ | Python 사용시만 |

### 5.2 affaan-m Skills (11개)

| Skill | 설명 | 채택 | 이유 |
|-------|------|:----:|------|
| frontend-patterns | 프론트엔드 패턴 | ✅ | 필수 |
| backend-patterns | 백엔드 패턴 | ✅ | 필수 |
| coding-standards | 코딩 표준 | ✅ | 품질 |
| tdd-workflow | TDD 워크플로우 | ✅ | 테스트 |
| verification-loop | 검증 루프 | ✅ | 품질 보장 |
| security-review | 보안 리뷰 | ✅ | 보안 |
| continuous-learning | 지속 학습 | ✅ | 패턴 학습 |
| strategic-compact | 전략적 압축 | ✅ | 컨텍스트 관리 |
| eval-harness | 평가 프레임워크 | ⚠️ | 평가 특화 |
| clickhouse-io | ClickHouse DB | ❌ | 특정 DB 전용 |
| project-guidelines-example | 프로젝트 예시 | ❌ | 예시 파일 |

### 5.3 통합 Skills 목록

#### ✅ 필수 채택 (12개)
| Skill | 출처 | 범주 |
|-------|------|------|
| react-patterns | jh941213 | Frontend |
| typescript-advanced | jh941213 | Frontend |
| tailwind-system | jh941213 | Frontend |
| shadcn-ui | jh941213 | Frontend |
| frontend-patterns | affaan-m | Frontend |
| backend-patterns | affaan-m | Backend |
| api-design | jh941213 | Backend |
| coding-standards | affaan-m | Quality |
| tdd-workflow | affaan-m | Testing |
| verification-loop | affaan-m | Quality |
| security-review | affaan-m | Security |
| strategic-compact | affaan-m | Context |

#### ⚠️ 선택적 (스택에 따라)
| Skill | 출처 | 조건 |
|-------|------|------|
| fastapi-templates | jh941213 | Python 백엔드 |
| async-python-patterns | jh941213 | Python |
| python-testing-patterns | jh941213 | Python |
| vercel-best-practices | jh941213 | Vercel 배포 |

#### ❌ 제외
| Skill | 출처 | 제외 이유 |
|-------|------|----------|
| clickhouse-io | affaan-m | 특정 DB 전용 |
| project-guidelines-example | affaan-m | 단순 예시 파일 |
| eval-harness | affaan-m | 평가 특화 |

---

## 6. Hooks 완전 비교

### 6.1 Hooks 지원

| 항목 | jh941213 | affaan-m |
|------|:--------:|:--------:|
| Hooks 시스템 | ❌ 없음 | ✅ 완전 지원 |
| PreToolUse | ❌ | ✅ |
| PostToolUse | ❌ | ✅ |
| SessionStart | ❌ | ✅ |
| SessionEnd | ❌ | ✅ |
| PreCompact | ❌ | ✅ |
| Stop | ❌ | ✅ |

### 6.2 affaan-m Hooks 상세

| Hook | 트리거 | 기능 | 채택 |
|------|--------|------|:----:|
| dev-server-block | PreToolUse | tmux 외 dev 서버 차단 | ⚠️ |
| long-running-reminder | PreToolUse | 긴 명령어 tmux 권장 | ✅ |
| git-push-review | PreToolUse | push 전 리뷰 알림 | ✅ |
| documentation-guard | PreToolUse | 불필요한 .md 생성 차단 | ✅ |
| compaction-suggestion | PreToolUse | 압축 제안 | ✅ |
| pre-compact | PreCompact | 압축 전 상태 저장 | ✅ |
| session-start | SessionStart | 이전 컨텍스트 로드 | ✅ |
| pr-creation | PostToolUse | PR 생성 후 URL 로그 | ✅ |
| code-formatting | PostToolUse | 자동 Prettier | ⚠️ |
| typescript-validation | PostToolUse | TS 파일 검증 | ✅ |
| console-log-warning | PostToolUse | console.log 경고 | ✅ |
| session-end | SessionEnd | 세션 상태 저장 | ✅ |
| evaluate-session | SessionEnd | 세션 패턴 평가 | ✅ |
| stop-validation | Stop | 종료 전 검증 | ✅ |

### 6.3 통합 Hooks

#### ✅ 필수 채택 (10개)
- `session-start.js`: 세션 시작시 HANDOFF.md 확인
- `session-end.js`: 세션 종료시 상태 저장
- `pre-compact.js`: 압축 전 상태 저장
- `pre-edit-check.js`: console.log, 하드코딩 시크릿 감지
- `post-edit-lint.js`: 에디트 후 린트
- `typescript-check.js`: TS 파일 타입 체크
- `git-push-review.js`: push 전 확인
- `compaction-warning.js`: 압축 경고
- `documentation-guard.js`: 불필요한 문서 생성 방지
- `evaluate-session.js`: 세션 패턴 분석

#### ⚠️ 선택적
- `dev-server-block.js`: tmux 강제 (환경에 따라)
- `code-formatting.js`: 자동 Prettier (프로젝트 설정에 따라)

---

## 7. 기타 기능 비교

### 7.1 고유 기능

| 기능 | jh941213 | affaan-m | 통합 채택 |
|------|:--------:|:--------:|:--------:|
| HANDOFF.md 프로토콜 | ✅ | ❌ | ✅ |
| 한국어 지원 | ✅ | ❌ | ✅ |
| Plugin marketplace | ❌ | ✅ | ❌ (수동 설치) |
| 테스트 스위트 | ❌ | ✅ | ⚠️ |
| MCP 통합 | ❌ | ✅ | ⚠️ |
| 크로스 플랫폼 스크립트 | ⚠️ | ✅ | ✅ |

### 7.2 MCP 통합 (affaan-m)

| MCP | 용도 | 채택 |
|-----|------|:----:|
| GitHub | PR, Issue 관리 | ✅ (gh CLI로 대체 가능) |
| Supabase | DB 관리 | ⚠️ Supabase 사용시만 |
| Vercel | 배포 | ⚠️ Vercel 사용시만 |
| Railway | 배포 | ⚠️ Railway 사용시만 |

---

## 8. 최종 통합 요약

### 8.1 통합 통계

| 항목 | jh941213 | affaan-m | 통합 |
|------|:--------:|:--------:|:----:|
| Agents | 6 | 9 | **10** |
| Commands | 10 | 15 | **17** |
| Rules | 5 | 8 | **8** |
| Skills | 11 | 11 | **12+선택** |
| Hooks | 0 | 14 | **12** |

### 8.2 출처별 기여도

```
통합 설정 구성:
├── jh941213 고유 기여 (30%)
│   ├── frontend-developer 에이전트
│   ├── /commit-push-pr, /handoff, /compact-guide, /frontend, /simplify
│   ├── HANDOFF.md 프로토콜
│   └── 한국어 지원
│
├── affaan-m 고유 기여 (45%)
│   ├── build-error-resolver, e2e-runner, refactor-cleaner, doc-updater
│   ├── /checkpoint, /e2e, /learn, /test-coverage, /refactor-clean, /update-docs
│   ├── Hooks 시스템 전체
│   └── agents, hooks, patterns 규칙
│
└── 공통 (25%)
    ├── planner, architect, code-reviewer, security-reviewer, tdd-guide
    ├── /plan, /verify, /tdd, /build-fix, /review
    └── security, coding-style, testing, git-workflow, performance 규칙
```

### 8.3 버린 것들과 이유

| 항목 | 출처 | 버린 이유 |
|------|------|----------|
| clickhouse-io skill | affaan-m | 특정 DB 전용, 범용성 없음 |
| project-guidelines-example | affaan-m | 단순 예시 파일 |
| eval-harness skill | affaan-m | 평가 특화, 일반 개발에 불필요 |
| /setup-pm | affaan-m | 자동 감지로 충분 |
| Plugin marketplace 방식 | affaan-m | 수동 설치가 더 유연 |
| Supabase/Vercel/Railway MCP | affaan-m | 특정 서비스 전용 |

---

## 9. 권장 사용 시나리오

### 9.1 프로젝트 유형별 권장

| 프로젝트 유형 | 권장 추가 Skills | 권장 Commands |
|--------------|------------------|---------------|
| React SPA | react-patterns, shadcn-ui | /frontend |
| Next.js | vercel-best-practices | /frontend, /e2e |
| Tauri Desktop | rust-patterns (추가 필요) | /build-fix |
| Python Backend | fastapi-templates, async-python | /tdd |
| Full Stack | 전부 | 전부 |

### 9.2 워크플로우 권장

```
새 기능 개발:
1. /plan → 계획 수립
2. /tdd → 테스트 먼저
3. /frontend (UI) 또는 직접 구현
4. /verify → 검증
5. /review → 리뷰
6. /commit-push-pr → Git 완료

세션 관리:
1. 80k 토큰 접근 시 → /handoff
2. 복잡한 작업 중간 → /checkpoint
3. 새 세션 시작 → HANDOFF.md 읽기

코드 정리:
1. /refactor-clean → 데드코드 제거
2. /simplify → 복잡한 코드 단순화
3. /update-docs → 문서 동기화
```

---

## 10. 결론

### jh941213 강점
- 한국어 개발자 친화적
- HANDOFF 프로토콜로 세션 관리 체계화
- Git 워크플로우 통합 (/commit-push-pr)
- UI 개발 전문 에이전트

### affaan-m 강점
- Hooks 자동화 시스템
- 빌드/테스트 자동 해결 에이전트
- 더 많은 Commands와 체계적 구조
- 10개월 실사용 경험 반영

### 통합 설정의 가치
- 두 저장소의 장점만 결합
- 불필요하거나 특정 서비스 전용 기능 제외
- 한국어 지원 + 자동화의 최적 조합
- 범용적으로 모든 프로젝트에 적용 가능
