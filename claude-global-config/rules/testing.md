---
name: testing
trigger: always
---

# 테스트 규칙

## 필수 사항

### 테스트 커버리지 목표
```
| 영역 | 최소 | 권장 |
|------|------|------|
| 전체 | 80% | 90% |
| 핵심 로직 | 90% | 100% |
| 보안 코드 | 100% | 100% |
| 유틸리티 | 90% | 100% |
```

### 테스트 작성 시점
```
✅ 필수:
- 새 기능 = 새 테스트
- 버그 수정 = 회귀 테스트
- 리팩토링 전 = 기존 동작 테스트 확보

권장:
- TDD (테스트 먼저)
```

## 테스트 구조

### AAA 패턴
```typescript
test('should calculate total with discount', () => {
  // Arrange (준비)
  const items = [{ price: 100 }, { price: 200 }];
  const discount = 0.1;

  // Act (실행)
  const result = calculateTotal(items, discount);

  // Assert (검증)
  expect(result).toBe(270);
});
```

### 네이밍 규칙
```typescript
// 패턴: should [동작] when [조건]
test('should return empty array when input is null', () => {});
test('should throw error when user is not authorized', () => {});

// 또는 한국어
test('입력이 null이면 빈 배열을 반환한다', () => {});
```

### describe 구조
```typescript
describe('UserService', () => {
  describe('create', () => {
    test('should create user with valid data', () => {});
    test('should throw on duplicate email', () => {});
  });

  describe('findById', () => {
    test('should return user when exists', () => {});
    test('should return null when not found', () => {});
  });
});
```

## 테스트 유형

### Unit (70%)
```
- 단일 함수/클래스
- 빠른 실행
- 외부 의존성 모킹
```

### Integration (20%)
```
- 모듈 간 연동
- DB 연결 (테스트 DB)
- API 엔드포인트
```

### E2E (10%)
```
- 전체 사용자 흐름
- 실제 브라우저
- 느리지만 신뢰도 높음
```

## 금지 사항

### 테스트 간 의존
```typescript
// ❌ 금지: 테스트 순서 의존
let sharedUser;
test('create user', () => { sharedUser = createUser(); });
test('update user', () => { updateUser(sharedUser); }); // sharedUser 의존

// ✅ 권장: 독립적 테스트
test('update user', () => {
  const user = createTestUser(); // 자체 setup
  updateUser(user);
});
```

### 과도한 모킹
```typescript
// ❌ 구현 세부사항 모킹
jest.spyOn(service, 'privateMethod');

// ✅ 외부 의존성만 모킹
jest.mock('./external-api');
```

### 테스트에서 로직
```typescript
// ❌ 테스트에 조건문
if (result.length > 0) {
  expect(result[0]).toBe(expected);
}

// ✅ 명확한 assertion
expect(result).toHaveLength(1);
expect(result[0]).toBe(expected);
```

## 경계값 테스트

```typescript
describe('validateAge', () => {
  // 정상 경계
  test('0세 허용', () => {});
  test('120세 허용', () => {});

  // 비정상 경계
  test('음수 거부', () => {});
  test('121 이상 거부', () => {});

  // 특수 케이스
  test('null 처리', () => {});
  test('undefined 처리', () => {});
});
```

## 비동기 테스트

```typescript
// async/await 사용
test('should fetch user', async () => {
  const user = await fetchUser('123');
  expect(user.name).toBe('John');
});

// 타임아웃 설정
test('should complete within 5s', async () => {
  // ...
}, 5000);
```
