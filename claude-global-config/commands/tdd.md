---
name: tdd
description: 테스트 주도 개발 워크플로우
agent: tdd-guide
---

# /tdd 커맨드

## 사용법
```
/tdd [기능 설명]
```

## 예시
```
/tdd 이메일 유효성 검사 함수
/tdd 장바구니 총액 계산
/tdd 사용자 인증 서비스
```

## 동작 (Red-Green-Refactor)

### Phase 1: RED
```
1. 인터페이스/타입 정의
2. 테스트 케이스 작성
3. 테스트 실행 → 실패 확인
```

### Phase 2: GREEN
```
4. 최소한의 구현 코드 작성
5. 테스트 실행 → 통과 확인
```

### Phase 3: REFACTOR
```
6. 코드 정리/개선
7. 테스트 실행 → 여전히 통과 확인
8. 필요시 반복
```

## 출력 형식
```markdown
# TDD: [기능명]

## 1. 인터페이스
```typescript
interface Calculator {
  add(a: number, b: number): number;
}
```

## 2. 테스트 (RED)
```typescript
describe('Calculator', () => {
  test('should add two numbers', () => {
    expect(calculator.add(2, 3)).toBe(5);
  });
});
```

## 3. 상태
🔴 RED: 테스트 작성 완료
다음: 구현 시작
```

## 커버리지 목표
- 일반 코드: 80%+
- 핵심 로직: 90%+
- 보안 코드: 100%

## 자동 후속 작업
- 구현 완료 → `/verify` 자동 실행
- E2E 필요 → `/e2e` 제안

## 관련 커맨드
- `/test-coverage` - 커버리지 확인
- `/e2e` - E2E 테스트
