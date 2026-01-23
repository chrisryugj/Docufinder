---
name: agents
trigger: always
---

# 에이전트 위임 규칙

## 자동 위임 조건

### 키워드 기반 트리거
| 사용자 요청 패턴 | 위임 에이전트 |
|-----------------|---------------|
| "계획", "설계", "어떻게 구현" | planner |
| "아키텍처", "구조", "시스템 설계" | architect |
| "UI", "컴포넌트", "화면", "버튼" | frontend-developer |
| "리뷰", "검토", "코드 봐줘" | code-reviewer |
| "보안", "취약점", "안전" | security-reviewer |
| "테스트", "TDD", "커버리지" | tdd-guide |
| "빌드 에러", "컴파일 실패" | build-error-resolver |
| "E2E", "통합 테스트" | e2e-runner |
| "정리", "데드코드", "미사용" | refactor-cleaner |
| "문서", "README" | doc-updater |

### 상황 기반 트리거
| 상황 | 자동 호출 |
|------|----------|
| 복잡한 기능 요청 | planner |
| 빌드 실패 | build-error-resolver |
| 구현 완료 | /verify → code-reviewer |
| 보안 이슈 감지 | security-reviewer |

## 위임 원칙

### 단일 책임
```
✅ 하나의 작업 = 하나의 에이전트
❌ 여러 에이전트 동시 호출 (순차 처리)
```

### 연쇄 위임 제한
```
최대 위임 깊이: 3단계

예시:
planner → frontend-developer → tdd-guide (OK)
planner → architect → frontend-developer → tdd-guide (제한)
```

### 완료 후 검증
```
모든 에이전트 작업 완료 후:
1. 결과 검증
2. 필요시 다음 에이전트 호출
3. 최종 /verify 실행
```

## 에이전트별 도구 권한

| 에이전트 | Read | Write | Edit | Bash | Glob | Grep |
|---------|:----:|:-----:|:----:|:----:|:----:|:----:|
| planner | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| architect | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| frontend-developer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| code-reviewer | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| security-reviewer | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ |
| tdd-guide | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| build-error-resolver | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| e2e-runner | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| refactor-cleaner | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| doc-updater | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |

## 워크플로우 예시

### 새 기능 개발
```
1. [자동] planner 호출
   ↓
2. [자동] 계획에 UI 포함 → frontend-developer
   ↓
3. [자동] 구현 완료 → /verify
   ↓
4. [자동] 보안 코드 포함 → security-reviewer
   ↓
5. [자동] 완료 → code-reviewer
   ↓
6. [제안] /commit-push-pr
```

### 버그 수정
```
1. 버그 분석
   ↓
2. [권장] tdd-guide (회귀 테스트)
   ↓
3. 수정 구현
   ↓
4. [자동] /verify
   ↓
5. [제안] /commit-push-pr
```

### 리팩토링
```
1. [자동] refactor-cleaner (데드코드)
   ↓
2. 리팩토링 실행
   ↓
3. [자동] /verify
   ↓
4. [자동] code-reviewer
```

## 에이전트 중단

### 중단 조건
```
- 3회 연속 실패
- 사용자 취소 요청
- 위험한 작업 감지
```

### 중단 후 처리
```
1. 현재 상태 저장
2. 사용자에게 상황 보고
3. 수동 개입 요청
```
