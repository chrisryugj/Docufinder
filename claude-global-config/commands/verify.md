---
name: verify
description: 린트, 테스트, 빌드 통합 검증
auto_trigger:
  - after_implementation: true
---

# /verify 커맨드

## 사용법
```
/verify              # 전체 검증
/verify --lint       # 린트만
/verify --test       # 테스트만
/verify --build      # 빌드만
/verify --type       # 타입 체크만
```

## 동작 순서
```
1. TypeScript/Rust 타입 체크
         │
         ▼ (통과)
2. ESLint/Clippy 린트
         │
         ▼ (통과)
3. 테스트 실행
         │
         ▼ (통과)
4. 프로덕션 빌드
         │
         ▼ (통과)
    ✅ 검증 완료
```

## 프로젝트별 명령어

### React/Vite
```bash
# Type Check
npx tsc --noEmit

# Lint
npm run lint

# Test
npm test

# Build
npm run build
```

### Tauri
```bash
# Rust Type Check + Lint
cargo clippy --manifest-path src-tauri/Cargo.toml

# Rust Test
cargo test --manifest-path src-tauri/Cargo.toml

# Frontend + Tauri Build
npm run tauri:build
```

### Next.js
```bash
# Type Check + Lint
npm run lint

# Test
npm test

# Build
npm run build
```

## 실패 시 자동 처리
```
에러 발생
    │
    ▼
┌─────────────────────────────┐
│ build-error-resolver 호출   │
│ - 에러 분석                 │
│ - 자동 수정 시도            │
│ - 재검증                    │
└─────────────────────────────┘
    │
    ▼ (3회 실패 시)
사용자에게 수동 개입 요청
```

## 출력 형식
```
🔍 검증 시작...

✅ Type Check: 통과
✅ Lint: 통과 (경고 2개)
✅ Test: 15/15 통과
✅ Build: 성공

📊 결과: 모든 검증 통과!
```

## 자동 트리거 조건
- 기능 구현 완료 후
- PR 생성 전
- `/commit-push-pr` 실행 전

## 관련 커맨드
- `/build-fix` - 빌드 에러 수정
- `/test-coverage` - 커버리지 확인
