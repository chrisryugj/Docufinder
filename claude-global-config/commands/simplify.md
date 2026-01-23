---
name: simplify
description: 복잡한 코드 단순화
---

# /simplify 커맨드

## 사용법
```
/simplify                    # 변경된 파일 대상
/simplify [파일경로]         # 특정 파일
/simplify --aggressive       # 적극적 리팩토링
```

## 대상
- 긴 함수 (50줄 초과)
- 복잡한 조건문
- 중복 로직
- 깊은 중첩

## 동작
```
1. 복잡도 분석
   - Cyclomatic complexity
   - 함수 길이
   - 중첩 깊이
      │
      ▼
2. 개선 제안 생성
      │
      ▼
3. 사용자 승인
      │
      ▼
4. 리팩토링 적용
```

## 개선 예시

### 긴 함수 분할
```typescript
// Before: 80줄 함수
function processOrder(order) {
  // 검증 로직 20줄
  // 계산 로직 30줄
  // 저장 로직 30줄
}

// After: 분할된 함수들
function validateOrder(order) { ... }
function calculateTotal(order) { ... }
function saveOrder(order) { ... }
function processOrder(order) {
  validateOrder(order);
  const total = calculateTotal(order);
  return saveOrder({ ...order, total });
}
```

### 조건문 단순화
```typescript
// Before
if (user && user.isActive && user.role === 'admin') {
  if (permission && permission.canEdit) {
    // ...
  }
}

// After
const isActiveAdmin = user?.isActive && user?.role === 'admin';
const hasEditPermission = permission?.canEdit;

if (isActiveAdmin && hasEditPermission) {
  // ...
}
```

## 출력 형식
```
📊 복잡도 분석: src/api/orders.ts

문제점:
├── processOrder: 85줄 (권장: 50줄)
├── calculateDiscount: 복잡도 15 (권장: 10)
└── 중첩 깊이 4 (권장: 3)

제안:
1. processOrder → 3개 함수로 분할
2. calculateDiscount → 조건문 추출
3. 중첩 → early return 패턴

[적용하시겠습니까? Y/n]
```

## 관련 커맨드
- `/refactor-clean` - 데드코드 제거
- `/review` - 리뷰
