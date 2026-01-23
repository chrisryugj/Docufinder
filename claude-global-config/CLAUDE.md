# Claude Code 글로벌 설정

> jh941213 + affaan-m 통합 버전 - 두 프로젝트의 장점만 결합

---

## 핵심 원칙

```
1. Claude = 똑똑한 주니어 개발자
   → 큰 작업은 작게 나누어 위임

2. Plan First
   → 복잡한 작업 전 반드시 계획 수립

3. 컨텍스트는 신선한 우유
   → 80k 토큰 도달 전 /handoff 실행

4. 자동화 우선
   → 반복 작업은 Hooks가 처리

5. 검증 루프 필수
   → 모든 구현 후 /verify 실행
```

---

## 에이전트 (10개)

| 트리거 키워드 | 에이전트 | 역할 |
|--------------|----------|------|
| 계획, 설계 | `planner` | 전략적 계획 수립 |
| 아키텍처, 구조 | `architect` | 시스템 설계 |
| UI, 컴포넌트 | `frontend-developer` | React/TS/Tailwind |
| 리뷰, 검토 | `code-reviewer` | 코드 품질 분석 |
| 보안, 취약점 | `security-reviewer` | OWASP 기반 검토 |
| 테스트, TDD | `tdd-guide` | 테스트 주도 개발 |
| 빌드 에러 | `build-error-resolver` | 자동 에러 수정 |
| E2E, 통합테스트 | `e2e-runner` | Playwright E2E |
| 정리, 데드코드 | `refactor-cleaner` | 미사용 코드 제거 |
| 문서, README | `doc-updater` | 문서 동기화 |

---

## 슬래시 커맨드 (16개)

### 필수 커맨드
| 커맨드 | 설명 |
|--------|------|
| `/plan` | 작업 계획 수립 |
| `/verify` | 린트+테스트+빌드 검증 |
| `/build-fix` | 빌드 에러 자동 수정 |
| `/review` | 코드 리뷰 |

### 세션 관리
| 커맨드 | 설명 |
|--------|------|
| `/handoff` | 세션 인계 문서 생성 |
| `/checkpoint` | 상태 저장점 생성 |
| `/compact-guide` | 토큰 관리 가이드 |

### 개발 워크플로우
| 커맨드 | 설명 |
|--------|------|
| `/tdd` | 테스트 주도 개발 |
| `/frontend` | UI 개발 워크플로우 |
| `/e2e` | E2E 테스트 생성 |
| `/commit-push-pr` | Git 통합 워크플로우 |

### 코드 정리
| 커맨드 | 설명 |
|--------|------|
| `/refactor-clean` | 데드코드 제거 |
| `/simplify` | 복잡한 코드 단순화 |
| `/update-docs` | 문서 동기화 |

### 학습
| 커맨드 | 설명 |
|--------|------|
| `/learn` | 패턴 추출 및 저장 |
| `/test-coverage` | 커버리지 분석 |

---

## 자동화 규칙

### 자동 에이전트 위임
- 복잡한 기능 요청 → `planner` 먼저 호출
- UI 작업 → `frontend-developer` 위임
- 빌드 실패 → `build-error-resolver` 자동 호출
- 코드 완성 후 → `/verify` 자동 실행

### 자동 검증 (Hooks)
- 모든 Edit 후 → 보안 검사 (시크릿, console.log)
- TypeScript 파일 변경 시 → 타입 체크
- git push 전 → 변경사항 확인 알림

### 자동 알림
- 보안 위험 감지 시 → 즉시 경고
- 컨텍스트 70% 도달 시 → 정리 권장
- 작업 완료 시 → 다음 단계 제안

---

## 코딩 규칙

### 크기 제한
```
- 함수: 50줄 이하
- 파일: 800줄 이하
- 파라미터: 4개 이하
```

### TypeScript
```
- strict mode 필수
- any 금지 (unknown 사용)
- 명시적 타입 선언
```

### 불변성
```
- const 우선 (let 최소화)
- 배열/객체 변이 금지
- 순수 함수 지향
```

---

## 보안 규칙

### 절대 금지
```
❌ 하드코딩된 시크릿 (API 키, 비밀번호)
❌ .env 파일 커밋
❌ console.log (프로덕션)
❌ main 브랜치 직접 푸시
```

### 필수 사항
```
✅ 환경변수로 시크릿 관리
✅ 사용자 입력 검증
✅ SQL 파라미터 바인딩
✅ XSS 방지 (HTML 이스케이프)
```

---

## 테스트 규칙

### 커버리지 목표
```
| 영역 | 최소 | 권장 |
|------|------|------|
| 일반 | 80% | 90% |
| 핵심 로직 | 90% | 100% |
| 보안 코드 | 100% | 100% |
```

### TDD 사이클
```
RED → GREEN → REFACTOR → 반복
```

---

## 세션 관리

### 토큰 상태별 조치
```
50% 미만  → 정상 작업
50-70%    → /checkpoint 권장
70-85%    → /handoff 권장
85% 이상  → /handoff 필수, 새 세션 시작
```

### 새 세션 시작 시
```
"HANDOFF.md 읽고 이어서 작업해줘"
```

---

## 기술 스택

### Frontend
- React 18+, TypeScript strict, Tailwind CSS
- shadcn/ui, Zustand/Jotai, TanStack Query

### Backend
- Rust, Node.js, Python
- Tauri (Desktop)

### Database
- SQLite, PostgreSQL, MongoDB

---

## 언어 설정

```
- 코드/변수명: 영어
- 주석: 한국어 가능
- 커밋 메시지: 한국어 가능
- 문서: 한국어 가능
```
