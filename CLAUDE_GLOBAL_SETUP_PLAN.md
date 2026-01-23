# Claude Code 글로벌 설정 통합 계획서

> jh941213/my-claude-code-asset + affaan-m/everything-claude-code 장점 통합

---

## 목차

1. [개요](#1-개요)
2. [디렉토리 구조](#2-디렉토리-구조)
3. [Agents 설정](#3-agents-설정)
4. [Commands 설정](#4-commands-설정)
5. [Rules 설정](#5-rules-설정)
6. [Hooks 설정](#6-hooks-설정)
7. [Skills 설정](#7-skills-설정)
8. [settings.json 설정](#8-settingsjson-설정)
9. [CLAUDE.md 글로벌 템플릿](#9-claudemd-글로벌-템플릿)
10. [설치 스크립트](#10-설치-스크립트)

---

## 1. 개요

### 1.1 설계 철학

| 원칙 | 설명 | 출처 |
|------|------|------|
| **Claude = 똑똑한 주니어** | 큰 작업을 작은 단위로 분할하여 위임 | jh941213 |
| **Plan First** | 복잡한 작업 전 반드시 계획 수립 | 둘 다 |
| **컨텍스트는 신선한 우유** | 80-100k 토큰 도달 전 리셋 | jh941213 |
| **자동화 우선** | Hooks로 반복 작업 자동화 | affaan-m |
| **검증 루프 필수** | 모든 작업 후 /verify 실행 | 둘 다 |

### 1.2 통합 목표

```
✅ 한국어 친화적 워크플로우 (jh941213)
✅ 이벤트 기반 자동화 (affaan-m)
✅ 세션 간 컨텍스트 유지 (jh941213 HANDOFF)
✅ 빌드/테스트 자동 해결 (affaan-m)
✅ 보안 및 코드 품질 강제 (둘 다)
```

---

## 2. 디렉토리 구조

```
~/.claude/
├── CLAUDE.md                    # 글로벌 설정 (자동 로드)
├── settings.json                # 권한, hooks, MCP 설정
├── agents/                      # 특화 에이전트 (9개)
│   ├── planner.md
│   ├── architect.md
│   ├── frontend-developer.md
│   ├── code-reviewer.md
│   ├── security-reviewer.md
│   ├── tdd-guide.md
│   ├── build-error-resolver.md
│   ├── refactor-cleaner.md
│   └── doc-updater.md
├── commands/                    # 슬래시 커맨드 (12개)
│   ├── plan.md
│   ├── verify.md
│   ├── review.md
│   ├── tdd.md
│   ├── build-fix.md
│   ├── frontend.md
│   ├── commit-push-pr.md
│   ├── handoff.md
│   ├── checkpoint.md
│   ├── learn.md
│   ├── refactor-clean.md
│   └── simplify.md
├── rules/                       # 자동 적용 규칙 (6개)
│   ├── security.md
│   ├── coding-style.md
│   ├── testing.md
│   ├── git-workflow.md
│   ├── agent-delegation.md
│   └── performance.md
└── skills/                      # 기술 스킬셋 (8개)
    ├── react-patterns.md
    ├── typescript-advanced.md
    ├── tailwind-system.md
    ├── rust-patterns.md
    ├── tauri-development.md
    ├── api-design.md
    ├── testing-strategies.md
    └── git-advanced.md
```

---

## 3. Agents 설정

### 3.1 planner.md
```markdown
---
name: planner
description: 복잡한 기능 구현을 위한 전략적 계획 수립
tools: [Read, Glob, Grep, WebSearch]
---

# Planner Agent

## 역할
복잡한 작업을 분석하고 실행 가능한 단계로 분해

## 출력 형식
1. **목표 분석**: 최종 목표 명확화
2. **현재 상태**: 코드베이스 분석 결과
3. **단계별 계획**: 번호가 매겨진 실행 단계
4. **의존성**: 각 단계 간 의존 관계
5. **위험 요소**: 잠재적 문제점 및 대응 방안

## 규칙
- 각 단계는 30분 이내 완료 가능해야 함
- 테스트 단계 반드시 포함
- 롤백 계획 명시
```

### 3.2 architect.md
```markdown
---
name: architect
description: 시스템 설계 및 구조 결정
tools: [Read, Glob, Grep, WebSearch]
---

# Architect Agent

## 역할
시스템 아키텍처 설계 및 기술적 의사결정

## 분석 항목
1. **현재 구조**: 기존 아키텍처 파악
2. **확장성**: 향후 확장 고려사항
3. **성능**: 병목 지점 및 최적화 방안
4. **보안**: 잠재적 취약점
5. **유지보수성**: 코드 구조 개선점

## 출력 형식
- 다이어그램 (Mermaid)
- 장단점 비교표
- 권장 사항 및 근거
```

### 3.3 frontend-developer.md (jh941213)
```markdown
---
name: frontend-developer
description: React/TypeScript/Tailwind 기반 고품질 UI 구현
tools: [Read, Write, Edit, Glob, Grep, Bash]
---

# Frontend Developer Agent

## 역할
사용자 경험 중심의 프론트엔드 구현

## 워크플로우
1. **디자인 분석**: 요구사항 → UI 컴포넌트 도출
2. **컴포넌트 설계**: 재사용 가능한 구조
3. **구현**: TypeScript + Tailwind
4. **접근성**: ARIA, 키보드 네비게이션
5. **반응형**: 모바일 우선 설계

## 기술 스택
- React 18+ (Hooks, Suspense)
- TypeScript strict mode
- Tailwind CSS
- shadcn/ui 컴포넌트

## 금지 사항
- any 타입 사용
- inline 스타일
- 하드코딩된 색상값
```

### 3.4 code-reviewer.md
```markdown
---
name: code-reviewer
description: 코드 품질 및 보안 분석
tools: [Read, Glob, Grep]
---

# Code Reviewer Agent

## 역할
코드 품질, 보안, 성능 관점에서 리뷰

## 체크리스트
### 품질
- [ ] 함수 길이 50줄 이하
- [ ] 파일 길이 800줄 이하
- [ ] 명확한 함수/변수 네이밍
- [ ] 적절한 에러 처리
- [ ] 불변성 패턴 준수

### 보안
- [ ] 입력값 검증
- [ ] SQL/XSS/Command Injection 방지
- [ ] 하드코딩된 시크릿 없음
- [ ] 적절한 권한 검사

### 성능
- [ ] N+1 쿼리 없음
- [ ] 불필요한 리렌더링 없음
- [ ] 메모리 누수 없음

## 출력 형식
| 심각도 | 파일:라인 | 문제 | 제안 |
```

### 3.5 security-reviewer.md
```markdown
---
name: security-reviewer
description: 보안 취약점 식별 및 대응 방안 제시
tools: [Read, Glob, Grep]
---

# Security Reviewer Agent

## 역할
OWASP Top 10 기반 보안 취약점 분석

## 검사 항목
1. **인젝션**: SQL, NoSQL, OS Command, LDAP
2. **인증 결함**: 세션 관리, 비밀번호 정책
3. **민감 데이터 노출**: 암호화, 전송 보안
4. **XXE**: XML 외부 엔티티
5. **접근 제어**: 권한 상승, IDOR
6. **보안 설정 오류**: 기본값, 불필요한 기능
7. **XSS**: Reflected, Stored, DOM-based
8. **역직렬화**: 안전하지 않은 역직렬화
9. **알려진 취약점**: 의존성 버전 검사
10. **로깅/모니터링**: 감사 추적

## 출력 형식
| 위험도 | 취약점 | 위치 | 영향 | 권장 조치 |
```

### 3.6 tdd-guide.md
```markdown
---
name: tdd-guide
description: 테스트 주도 개발 방법론 가이드
tools: [Read, Write, Edit, Glob, Grep, Bash]
---

# TDD Guide Agent

## 역할
Red-Green-Refactor 사이클 가이드

## 워크플로우
1. **Red**: 실패하는 테스트 먼저 작성
2. **Green**: 테스트 통과하는 최소 코드
3. **Refactor**: 코드 정리 (테스트 유지)

## 테스트 원칙
- 하나의 테스트 = 하나의 동작
- AAA 패턴: Arrange, Act, Assert
- 테스트 커버리지 80% 이상 목표
- 경계값 및 예외 케이스 포함

## 테스트 종류
| 종류 | 비율 | 도구 |
|------|------|------|
| Unit | 70% | Jest, Vitest, pytest |
| Integration | 20% | Supertest, TestClient |
| E2E | 10% | Playwright, Cypress |
```

### 3.7 build-error-resolver.md (affaan-m)
```markdown
---
name: build-error-resolver
description: 빌드/컴파일 에러 자동 분석 및 해결
tools: [Read, Write, Edit, Glob, Grep, Bash]
---

# Build Error Resolver Agent

## 역할
빌드 실패 시 원인 분석 및 자동 수정

## 워크플로우
1. **에러 수집**: 빌드 로그 전체 분석
2. **원인 분류**: 타입/의존성/설정/문법
3. **우선순위화**: 연쇄 에러 루트 원인 먼저
4. **수정 적용**: 자동 수정 가능한 것 먼저
5. **검증**: 재빌드로 수정 확인

## 지원 빌드 시스템
- TypeScript (tsc)
- Rust (cargo)
- Vite/Webpack
- Tauri

## 에러 패턴 DB
| 에러 패턴 | 원인 | 해결책 |
|-----------|------|--------|
| TS2307 | 모듈 못 찾음 | import 경로 수정 |
| E0432 | Rust unresolved import | use 문 수정 |
| E0599 | 메서드 없음 | impl 블록 확인 |
```

### 3.8 refactor-cleaner.md (affaan-m)
```markdown
---
name: refactor-cleaner
description: 데드코드 제거 및 코드 정리
tools: [Read, Write, Edit, Glob, Grep, Bash]
---

# Refactor Cleaner Agent

## 역할
사용되지 않는 코드 식별 및 제거

## 검사 항목
1. **미사용 import**: 사용되지 않는 모듈
2. **미사용 변수**: 선언 후 미참조
3. **미사용 함수**: 호출되지 않는 함수
4. **미사용 타입**: 참조되지 않는 타입 정의
5. **중복 코드**: 유사 로직 통합 가능 여부
6. **주석 처리된 코드**: 삭제 대상

## 안전 장치
- 삭제 전 참조 검색 필수
- export된 항목은 신중하게
- 테스트 실행 후 삭제 확정

## 출력 형식
| 파일 | 라인 | 항목 | 이유 | 액션 |
```

### 3.9 doc-updater.md (affaan-m)
```markdown
---
name: doc-updater
description: 코드 변경에 따른 문서 자동 동기화
tools: [Read, Write, Edit, Glob, Grep]
---

# Doc Updater Agent

## 역할
코드와 문서의 일관성 유지

## 대상 문서
- README.md
- API 문서
- CHANGELOG.md
- JSDoc / Rustdoc 주석

## 동기화 항목
1. **API 변경**: 함수 시그니처 변경 반영
2. **설정 변경**: 환경변수, 설정 파일
3. **의존성 변경**: 버전 업데이트
4. **기능 추가/삭제**: 문서 섹션 갱신

## 출력 형식
변경이 필요한 문서 목록과 구체적 수정 내용
```

---

## 4. Commands 설정

### 4.1 plan.md
```markdown
---
name: plan
description: 작업 계획 수립
---

# /plan 커맨드

## 사용법
```
/plan [작업 설명]
```

## 동작
1. planner 에이전트 호출
2. 현재 코드베이스 분석
3. 단계별 실행 계획 생성
4. 예상 위험 요소 식별

## 출력
- 번호가 매겨진 작업 목록
- 각 작업의 예상 복잡도
- 의존성 그래프
```

### 4.2 verify.md
```markdown
---
name: verify
description: 린트, 테스트, 빌드 검증
---

# /verify 커맨드

## 사용법
```
/verify [--lint] [--test] [--build] [--all]
```

## 동작
1. **Lint**: ESLint, Clippy 실행
2. **Test**: 테스트 스위트 실행
3. **Build**: 프로덕션 빌드 검증
4. **Type Check**: TypeScript, Rust 타입 검사

## 프로젝트별 명령어
| 프레임워크 | Lint | Test | Build |
|-----------|------|------|-------|
| React/Vite | `pnpm lint` | `pnpm test` | `pnpm build` |
| Tauri | `cargo clippy` | `cargo test` | `pnpm tauri:build` |
| Next.js | `pnpm lint` | `pnpm test` | `pnpm build` |

## 실패 시
- build-error-resolver 에이전트 자동 호출
- 에러 수정 후 재검증
```

### 4.3 review.md
```markdown
---
name: review
description: 코드 리뷰 실행
---

# /review 커맨드

## 사용법
```
/review [파일경로] [--security] [--performance]
```

## 동작
1. 변경된 파일 식별 (git diff)
2. code-reviewer 에이전트 호출
3. --security 옵션 시 security-reviewer 추가 호출

## 출력
마크다운 테이블 형식의 리뷰 결과
```

### 4.4 tdd.md
```markdown
---
name: tdd
description: 테스트 주도 개발 워크플로우
---

# /tdd 커맨드

## 사용법
```
/tdd [기능 설명]
```

## 동작
1. tdd-guide 에이전트 호출
2. 실패하는 테스트 먼저 작성
3. 최소 구현 코드 작성
4. 테스트 통과 확인
5. 리팩토링

## 사이클
Red → Green → Refactor → 반복
```

### 4.5 build-fix.md
```markdown
---
name: build-fix
description: 빌드 에러 자동 수정
---

# /build-fix 커맨드

## 사용법
```
/build-fix
```

## 동작
1. 빌드 실행 및 에러 캡처
2. build-error-resolver 에이전트 호출
3. 에러 자동 수정
4. 재빌드로 검증
5. 실패 시 반복 (최대 3회)
```

### 4.6 frontend.md (jh941213)
```markdown
---
name: frontend
description: 프론트엔드 개발 워크플로우
---

# /frontend 커맨드

## 사용법
```
/frontend [UI 요구사항]
```

## 동작
1. 요구사항 분석 → 컴포넌트 설계
2. frontend-developer 에이전트 호출
3. 컴포넌트 구현
4. 스토리북/테스트 작성

## 출력
- 컴포넌트 파일
- 타입 정의
- 테스트 파일
```

### 4.7 commit-push-pr.md (jh941213)
```markdown
---
name: commit-push-pr
description: Git 워크플로우 통합 실행
---

# /commit-push-pr 커맨드

## 사용법
```
/commit-push-pr [--no-pr]
```

## 동작
1. `git status` 확인
2. `git diff` 분석
3. 커밋 메시지 자동 생성 (한국어)
4. `git add` + `git commit`
5. `git push`
6. PR 생성 (--no-pr 없으면)

## 커밋 메시지 형식
```
<type>(<scope>): <description>

<body>
```

타입: feat, fix, refactor, docs, test, chore
```

### 4.8 handoff.md (jh941213)
```markdown
---
name: handoff
description: 세션 인계 문서 생성
---

# /handoff 커맨드

## 사용법
```
/handoff
```

## 동작
현재 세션의 컨텍스트를 HANDOFF.md로 저장

## 출력 내용
```markdown
# HANDOFF - [날짜]

## 완료된 작업
- ...

## 진행 중인 작업
- ...

## 다음 단계
- ...

## 주의사항
- ...

## 관련 파일
- ...
```

## 용도
- 컨텍스트 리셋 전
- 다음 세션으로 인계
- 80k 토큰 도달 시
```

### 4.9 checkpoint.md (affaan-m)
```markdown
---
name: checkpoint
description: 검증 상태 저장
---

# /checkpoint 커맨드

## 사용법
```
/checkpoint [설명]
```

## 동작
1. 현재 git 상태 저장
2. 모든 테스트 실행
3. 빌드 상태 확인
4. 체크포인트 기록

## 용도
- 안정적인 상태 마킹
- 롤백 지점 생성
```

### 4.10 learn.md (affaan-m)
```markdown
---
name: learn
description: 세션 중 패턴 추출
---

# /learn 커맨드

## 사용법
```
/learn [패턴 설명]
```

## 동작
1. 현재 세션의 코드 패턴 분석
2. 재사용 가능한 패턴 추출
3. skills/ 디렉토리에 저장

## 용도
- 프로젝트별 패턴 학습
- 반복 작업 템플릿화
```

### 4.11 refactor-clean.md (affaan-m)
```markdown
---
name: refactor-clean
description: 데드코드 제거
---

# /refactor-clean 커맨드

## 사용법
```
/refactor-clean [경로]
```

## 동작
1. refactor-cleaner 에이전트 호출
2. 미사용 코드 식별
3. 안전한 삭제 실행
4. 테스트로 검증
```

### 4.12 simplify.md (jh941213)
```markdown
---
name: simplify
description: 코드 단순화
---

# /simplify 커맨드

## 사용법
```
/simplify [파일경로]
```

## 동작
1. 복잡도 분석
2. 리팩토링 제안
3. 사용자 승인 후 적용

## 대상
- 긴 함수 분할
- 중복 로직 추출
- 복잡한 조건문 단순화
```

---

## 5. Rules 설정

### 5.1 security.md
```markdown
---
name: security
trigger: always
---

# 보안 규칙

## 금지 사항
- 하드코딩된 시크릿, API 키, 비밀번호
- .env 파일 커밋
- 사용자 입력 미검증 사용
- SQL 문자열 직접 조합
- eval(), dangerouslySetInnerHTML 무분별 사용

## 필수 사항
- 환경변수로 시크릿 관리
- 사용자 입력 sanitization
- HTTPS 강제
- CORS 적절히 설정
- CSP 헤더 설정 (웹앱)

## Tauri 특화
- IPC 핸들러에서 입력 검증 필수
- 파일 시스템 접근 권한 최소화
- allowlist 명시적 설정
```

### 5.2 coding-style.md
```markdown
---
name: coding-style
trigger: always
---

# 코딩 스타일 규칙

## 일반
- 함수 50줄 이하
- 파일 800줄 이하
- 명확한 네이밍 (의미 있는 이름)
- 주석: 코드로 설명 안 되는 것만

## TypeScript
- strict mode 필수
- any 금지 (unknown 사용)
- 명시적 타입 선언
- interface 선호 (type보다)

## Rust
- clippy 경고 0개 유지
- unwrap() 지양 (? 연산자 사용)
- 명시적 에러 타입

## 불변성
- const 우선 (let 최소화)
- 배열/객체 변이 금지
- 순수 함수 지향

## 한국어 (선택)
- 커밋 메시지 한국어 가능
- 주석 한국어 가능
- 코드/변수명은 영어
```

### 5.3 testing.md
```markdown
---
name: testing
trigger: always
---

# 테스트 규칙

## 필수 사항
- 새 기능 = 새 테스트
- 버그 수정 = 회귀 테스트
- 커버리지 80% 이상 목표

## 테스트 구조
- AAA 패턴: Arrange, Act, Assert
- 하나의 테스트 = 하나의 동작
- 독립적 실행 가능

## 네이밍
```
test_[기능]_[시나리오]_[예상결과]
it('should [동작] when [조건]')
```

## 금지 사항
- 테스트 간 상태 공유
- 외부 의존성 직접 호출 (mock 사용)
- console.log로 디버깅
```

### 5.4 git-workflow.md
```markdown
---
name: git-workflow
trigger: always
---

# Git 워크플로우 규칙

## 브랜치
- main: 항상 배포 가능
- feature/*: 기능 개발
- fix/*: 버그 수정
- refactor/*: 리팩토링

## 커밋
- Conventional Commits 형식
- 작은 단위로 자주 커밋
- 의미 있는 메시지

## 금지 사항
- main 직접 푸시
- force push (--force)
- 커밋에 console.log 포함
- .env, node_modules 커밋

## PR
- 리뷰 전 self-review
- 테스트 통과 필수
- 충돌 해결 후 머지
```

### 5.5 agent-delegation.md
```markdown
---
name: agent-delegation
trigger: always
---

# 에이전트 위임 규칙

## 자동 위임 조건
| 상황 | 에이전트 |
|------|----------|
| 복잡한 기능 계획 | planner |
| 아키텍처 결정 | architect |
| UI 구현 | frontend-developer |
| 코드 리뷰 요청 | code-reviewer |
| 보안 검토 필요 | security-reviewer |
| TDD 요청 | tdd-guide |
| 빌드 실패 | build-error-resolver |
| 코드 정리 | refactor-cleaner |
| 문서 업데이트 | doc-updater |

## 위임 원칙
- 하나의 작업 = 하나의 에이전트
- 에이전트 완료 후 결과 검증
- 연쇄 위임 최대 3단계
```

### 5.6 performance.md
```markdown
---
name: performance
trigger: always
---

# 성능 규칙

## 컨텍스트 관리
- 80k 토큰 도달 전 /handoff 실행
- 불필요한 파일 읽기 최소화
- 활성 MCP 80개 이하 유지

## 코드 성능
- N+1 쿼리 금지
- 불필요한 리렌더링 방지 (React.memo, useMemo)
- 대용량 데이터 페이지네이션/가상화
- 이미지 lazy loading

## 빌드 성능
- 트리 셰이킹 활성화
- 코드 스플리팅 적용
- 불필요한 의존성 제거
```

---

## 6. Hooks 설정

### 6.1 hooks.json
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit",
        "script": "node ~/.claude/hooks/pre-edit-check.js"
      },
      {
        "matcher": "Write",
        "script": "node ~/.claude/hooks/pre-write-check.js"
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit",
        "script": "node ~/.claude/hooks/post-edit-lint.js"
      }
    ],
    "SessionStart": [
      {
        "script": "node ~/.claude/hooks/session-start.js"
      }
    ],
    "SessionEnd": [
      {
        "script": "node ~/.claude/hooks/session-end.js"
      }
    ],
    "ContextCompaction": [
      {
        "script": "node ~/.claude/hooks/compaction-warning.js"
      }
    ]
  }
}
```

### 6.2 pre-edit-check.js
```javascript
#!/usr/bin/env node
// 에디트 전 검사: console.log, 하드코딩된 시크릿 감지

const fs = require('fs');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const { new_string } = input.tool_input;

const violations = [];

// console.log 감지
if (/console\.(log|debug|info)/.test(new_string)) {
  violations.push('⚠️ console.log 감지됨 - 프로덕션 코드에서 제거 필요');
}

// 하드코딩된 시크릿 패턴
const secretPatterns = [
  /api[_-]?key\s*[:=]\s*['"][^'"]+['"]/i,
  /password\s*[:=]\s*['"][^'"]+['"]/i,
  /secret\s*[:=]\s*['"][^'"]+['"]/i,
];

for (const pattern of secretPatterns) {
  if (pattern.test(new_string)) {
    violations.push('🚨 하드코딩된 시크릿 감지됨!');
    break;
  }
}

if (violations.length > 0) {
  console.log(JSON.stringify({
    decision: 'block',
    reason: violations.join('\n')
  }));
} else {
  console.log(JSON.stringify({ decision: 'approve' }));
}
```

### 6.3 session-start.js
```javascript
#!/usr/bin/env node
// 세션 시작 시 HANDOFF.md 존재 확인

const fs = require('fs');
const path = require('path');

const handoffPath = path.join(process.cwd(), 'HANDOFF.md');

if (fs.existsSync(handoffPath)) {
  console.log('📋 HANDOFF.md 발견 - 이전 세션 컨텍스트를 확인하세요.');
  console.log(`경로: ${handoffPath}`);
}
```

### 6.4 compaction-warning.js
```javascript
#!/usr/bin/env node
// 컨텍스트 압축 경고

console.log(`
⚠️ 컨텍스트가 압축되었습니다.

권장 조치:
1. /handoff 실행하여 현재 상태 저장
2. 새 세션 시작 고려
3. 불필요한 파일 참조 줄이기
`);
```

---

## 7. Skills 설정

### 7.1 react-patterns.md
```markdown
---
name: react-patterns
---

# React 패턴 스킬

## 컴포넌트 구조
```tsx
// 1. imports
// 2. types
// 3. constants
// 4. helper functions
// 5. component
// 6. exports
```

## Hooks 패턴
- useState: 단순 상태
- useReducer: 복잡한 상태
- useMemo/useCallback: 성능 최적화
- useRef: DOM 참조, 이전값 저장

## 상태 관리
- 로컬: useState, useReducer
- 전역: Context, Zustand, Jotai
- 서버: TanStack Query, SWR

## 에러 처리
- ErrorBoundary로 감싸기
- Suspense로 로딩 처리
```

### 7.2 rust-patterns.md
```markdown
---
name: rust-patterns
---

# Rust 패턴 스킬

## 에러 처리
```rust
// thiserror로 커스텀 에러
#[derive(thiserror::Error, Debug)]
pub enum MyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ? 연산자로 전파
fn do_something() -> Result<(), MyError> {
    let file = File::open("path")?;
    Ok(())
}
```

## 비동기
```rust
// tokio 런타임
#[tokio::main]
async fn main() {
    let result = async_fn().await;
}
```

## 구조체 패턴
```rust
// Builder 패턴
impl MyStructBuilder {
    pub fn new() -> Self { ... }
    pub fn field(mut self, value: T) -> Self { ... }
    pub fn build(self) -> MyStruct { ... }
}
```
```

### 7.3 tauri-development.md
```markdown
---
name: tauri-development
---

# Tauri 개발 스킬

## IPC 커맨드
```rust
#[tauri::command]
async fn my_command(
    state: State<'_, AppState>,
    param: String,
) -> Result<Response, String> {
    // 입력 검증 필수
    if param.is_empty() {
        return Err("Invalid param".into());
    }
    Ok(Response { ... })
}
```

## 프론트엔드 호출
```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<Response>('my_command', { param: 'value' });
```

## 파일 시스템
```rust
use tauri::api::path::app_data_dir;

let data_dir = app_data_dir(&config)?;
```

## 빌드 명령어
- 개발: `pnpm tauri:dev`
- 빌드: `pnpm tauri:build`
- Rust 체크: `cargo check --manifest-path src-tauri/Cargo.toml`
```

---

## 8. settings.json 설정

```json
{
  "permissions": {
    "allow": [
      "Bash(npm:*)",
      "Bash(pnpm:*)",
      "Bash(cargo:*)",
      "Bash(git:*)",
      "Bash(gh:*)",
      "Read",
      "Write",
      "Edit",
      "Glob",
      "Grep"
    ],
    "deny": [
      "Bash(rm -rf /)",
      "Bash(sudo *)",
      "Bash(chmod 777 *)"
    ]
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit",
        "command": "node ~/.claude/hooks/pre-edit-check.js"
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit",
        "command": "node ~/.claude/hooks/post-edit-lint.js"
      }
    ],
    "SessionStart": [
      {
        "command": "node ~/.claude/hooks/session-start.js"
      }
    ]
  },
  "agents": {
    "maxConcurrent": 3,
    "defaultTimeout": 300000
  },
  "context": {
    "warningThreshold": 80000,
    "autoCompactThreshold": 100000
  }
}
```

---

## 9. CLAUDE.md 글로벌 템플릿

```markdown
# Claude Code 글로벌 설정

## 핵심 원칙
- Claude = 똑똑한 주니어 개발자
- 큰 작업은 작게 나누어 위임
- Plan First: 복잡한 작업 전 반드시 계획
- 컨텍스트 80k 토큰 전 /handoff 실행

## 사용 가능한 에이전트
| 명령 | 에이전트 | 용도 |
|------|----------|------|
| 복잡한 기능 | planner | 계획 수립 |
| 설계 결정 | architect | 아키텍처 |
| UI 개발 | frontend-developer | React/TS |
| 코드 리뷰 | code-reviewer | 품질 검사 |
| 보안 검토 | security-reviewer | 취약점 |
| TDD | tdd-guide | 테스트 주도 |
| 빌드 에러 | build-error-resolver | 자동 수정 |
| 코드 정리 | refactor-cleaner | 데드코드 |
| 문서 갱신 | doc-updater | README 등 |

## 슬래시 커맨드
- `/plan` - 작업 계획
- `/verify` - 린트/테스트/빌드
- `/review` - 코드 리뷰
- `/tdd` - 테스트 주도 개발
- `/build-fix` - 빌드 에러 수정
- `/frontend` - UI 개발
- `/commit-push-pr` - Git 워크플로우
- `/handoff` - 세션 인계
- `/checkpoint` - 상태 저장
- `/refactor-clean` - 데드코드 제거

## 코딩 규칙
- 함수 50줄, 파일 800줄 이하
- TypeScript strict, any 금지
- 불변성 패턴 (const 우선)
- 테스트 커버리지 80%+

## 보안 규칙
- 하드코딩된 시크릿 금지
- 사용자 입력 검증 필수
- .env 커밋 금지

## 기술 스택
- Frontend: React, TypeScript, Tailwind
- Backend: Rust, Node.js, Python
- Desktop: Tauri
- DB: SQLite, PostgreSQL
```

---

## 10. 설치 스크립트

### 10.1 install.sh
```bash
#!/bin/bash

# Claude Code 글로벌 설정 설치 스크립트

set -e

CLAUDE_DIR="$HOME/.claude"

echo "🚀 Claude Code 글로벌 설정 설치 시작..."

# 디렉토리 생성
mkdir -p "$CLAUDE_DIR"/{agents,commands,rules,skills,hooks}

# 설정 파일 복사 (이 스크립트와 같은 디렉토리에 있다고 가정)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# CLAUDE.md 복사
if [ -f "$SCRIPT_DIR/CLAUDE.md" ]; then
    cp "$SCRIPT_DIR/CLAUDE.md" "$CLAUDE_DIR/CLAUDE.md"
    echo "✅ CLAUDE.md 설치 완료"
fi

# settings.json 복사
if [ -f "$SCRIPT_DIR/settings.json" ]; then
    cp "$SCRIPT_DIR/settings.json" "$CLAUDE_DIR/settings.json"
    echo "✅ settings.json 설치 완료"
fi

# agents 복사
if [ -d "$SCRIPT_DIR/agents" ]; then
    cp "$SCRIPT_DIR/agents/"*.md "$CLAUDE_DIR/agents/"
    echo "✅ Agents 설치 완료"
fi

# commands 복사
if [ -d "$SCRIPT_DIR/commands" ]; then
    cp "$SCRIPT_DIR/commands/"*.md "$CLAUDE_DIR/commands/"
    echo "✅ Commands 설치 완료"
fi

# rules 복사
if [ -d "$SCRIPT_DIR/rules" ]; then
    cp "$SCRIPT_DIR/rules/"*.md "$CLAUDE_DIR/rules/"
    echo "✅ Rules 설치 완료"
fi

# skills 복사
if [ -d "$SCRIPT_DIR/skills" ]; then
    cp "$SCRIPT_DIR/skills/"*.md "$CLAUDE_DIR/skills/"
    echo "✅ Skills 설치 완료"
fi

# hooks 복사 및 실행 권한
if [ -d "$SCRIPT_DIR/hooks" ]; then
    cp "$SCRIPT_DIR/hooks/"*.js "$CLAUDE_DIR/hooks/"
    chmod +x "$CLAUDE_DIR/hooks/"*.js
    echo "✅ Hooks 설치 완료"
fi

echo ""
echo "🎉 설치 완료!"
echo ""
echo "설치 위치: $CLAUDE_DIR"
echo ""
echo "다음 명령으로 확인하세요:"
echo "  ls -la $CLAUDE_DIR"
```

### 10.2 Windows용 install.ps1
```powershell
# Claude Code 글로벌 설정 설치 스크립트 (Windows)

$ClaudeDir = "$env:USERPROFILE\.claude"

Write-Host "🚀 Claude Code 글로벌 설정 설치 시작..." -ForegroundColor Cyan

# 디렉토리 생성
$Dirs = @("agents", "commands", "rules", "skills", "hooks")
foreach ($Dir in $Dirs) {
    New-Item -ItemType Directory -Force -Path "$ClaudeDir\$Dir" | Out-Null
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# 파일 복사
$Items = @(
    @{Src="CLAUDE.md"; Dst="$ClaudeDir\CLAUDE.md"},
    @{Src="settings.json"; Dst="$ClaudeDir\settings.json"}
)

foreach ($Item in $Items) {
    if (Test-Path "$ScriptDir\$($Item.Src)") {
        Copy-Item "$ScriptDir\$($Item.Src)" -Destination $Item.Dst -Force
        Write-Host "✅ $($Item.Src) 설치 완료" -ForegroundColor Green
    }
}

# 디렉토리 복사
foreach ($Dir in $Dirs) {
    if (Test-Path "$ScriptDir\$Dir") {
        Copy-Item "$ScriptDir\$Dir\*" -Destination "$ClaudeDir\$Dir\" -Force
        Write-Host "✅ $Dir 설치 완료" -ForegroundColor Green
    }
}

Write-Host ""
Write-Host "🎉 설치 완료!" -ForegroundColor Green
Write-Host "설치 위치: $ClaudeDir" -ForegroundColor Yellow
```

---

## 부록: 출처 매핑

| 항목 | jh941213 | affaan-m | 통합 |
|------|:--------:|:--------:|:----:|
| planner | ✅ | ✅ | ✅ |
| architect | ✅ | ✅ | ✅ |
| frontend-developer | ✅ | ❌ | ✅ |
| code-reviewer | ✅ | ✅ | ✅ |
| security-reviewer | ✅ | ✅ | ✅ |
| tdd-guide | ✅ | ✅ | ✅ |
| build-error-resolver | ❌ | ✅ | ✅ |
| refactor-cleaner | ❌ | ✅ | ✅ |
| doc-updater | ❌ | ✅ | ✅ |
| /commit-push-pr | ✅ | ❌ | ✅ |
| /handoff | ✅ | ❌ | ✅ |
| /checkpoint | ❌ | ✅ | ✅ |
| /learn | ❌ | ✅ | ✅ |
| Hooks 시스템 | ❌ | ✅ | ✅ |
| 한국어 지원 | ✅ | ❌ | ✅ |
| HANDOFF 프로토콜 | ✅ | ❌ | ✅ |

---

## 다음 단계

1. 이 계획서 검토 및 수정
2. 각 파일 실제 생성
3. 설치 스크립트 테스트
4. 프로젝트별 커스터마이징 (프로젝트 CLAUDE.md)
