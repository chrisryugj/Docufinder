---
name: hooks
trigger: always
---

# Hooks 규칙

## Hooks 시스템 개요

### 이벤트 유형
| 이벤트 | 발생 시점 | 용도 |
|--------|----------|------|
| SessionStart | 세션 시작 | 컨텍스트 복원 |
| SessionEnd | 세션 종료 | 상태 저장 |
| PreToolUse | 도구 실행 전 | 검증, 차단 |
| PostToolUse | 도구 실행 후 | 후처리 |
| PreCompact | 압축 전 | 상태 백업 |
| Stop | 세션 중단 | 정리 |

### Hooks 응답 유형
```javascript
// 승인
{ decision: 'approve' }

// 차단
{ decision: 'block', reason: '차단 이유' }

// 경고 (진행은 함)
{ decision: 'warn', message: '경고 메시지' }
```

## 활성화된 Hooks

### SessionStart
```javascript
// HANDOFF.md 확인
// 패키지 매니저 감지
// 이전 컨텍스트 로드
```

### PreToolUse (Edit)
```javascript
// 검사 항목:
// - console.log 포함 여부
// - 하드코딩된 시크릿
// - 파일 크기 초과

// 차단 조건:
// - 시크릿 패턴 발견 → block
// - console.log → warn
```

### PreToolUse (Bash - git push)
```javascript
// 검사 항목:
// - 변경사항 리뷰 여부
// - 테스트 실행 여부

// 동작:
// - 리뷰 알림 표시
```

### PostToolUse (Edit - .ts/.tsx)
```javascript
// 동작:
// - TypeScript 타입 체크
// - 린트 실행
// - 에러 즉시 표시
```

### PreCompact
```javascript
// 동작:
// - 현재 상태 자동 저장
// - HANDOFF.md 생성 제안
```

### SessionEnd
```javascript
// 동작:
// - 세션 상태 저장
// - 패턴 분석 (학습)
```

## Hook 스크립트 작성

### 기본 구조
```javascript
#!/usr/bin/env node

const fs = require('fs');

// stdin에서 입력 읽기
const input = JSON.parse(fs.readFileSync(0, 'utf8'));

// 검사 로직
const violations = [];

// 예: console.log 감지
if (/console\.(log|debug)/.test(input.tool_input?.new_string || '')) {
  violations.push('console.log 감지됨');
}

// 결과 출력
if (violations.length > 0) {
  console.log(JSON.stringify({
    decision: 'warn',
    message: violations.join('\n')
  }));
} else {
  console.log(JSON.stringify({ decision: 'approve' }));
}
```

### 입력 형식
```javascript
{
  "tool": "Edit",
  "tool_input": {
    "file_path": "/path/to/file",
    "old_string": "...",
    "new_string": "..."
  },
  "session_context": {
    "cwd": "/project",
    "env": { ... }
  }
}
```

## 설정 방법

### settings.json
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit",
        "command": "node ~/.claude/hooks/pre-edit-check.js"
      },
      {
        "matcher": "Bash",
        "pattern": "git push",
        "command": "node ~/.claude/hooks/git-push-review.js"
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit",
        "pattern": "\\.(ts|tsx)$",
        "command": "node ~/.claude/hooks/post-edit-lint.js"
      }
    ],
    "SessionStart": [
      {
        "command": "node ~/.claude/hooks/session-start.js"
      }
    ],
    "SessionEnd": [
      {
        "command": "node ~/.claude/hooks/session-end.js"
      }
    ]
  }
}
```

### Matcher 문법
```javascript
// 도구 이름
"matcher": "Edit"

// 파일 패턴 (정규식)
"pattern": "\\.(ts|tsx)$"

// 복합 조건
"matcher": "Bash",
"pattern": "git push"
```

## 주의사항

### 성능
```
- Hook은 동기적으로 실행됨
- 긴 작업 피하기 (타임아웃 있음)
- 필요시 비동기 작업은 별도 프로세스
```

### 안전성
```
- block은 신중하게 사용
- 무한 루프 방지
- 에러 시 graceful fallback
```

### 디버깅
```bash
# Hook 테스트
echo '{"tool":"Edit","tool_input":{"new_string":"console.log(x)"}}' | \
  node ~/.claude/hooks/pre-edit-check.js
```
