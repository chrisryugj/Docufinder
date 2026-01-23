---
name: e2e
description: E2E 테스트 생성 및 실행
agent: e2e-runner
---

# /e2e 커맨드

## 사용법
```
/e2e                         # 전체 E2E 실행
/e2e [사용자 흐름]           # 특정 흐름 테스트 생성
/e2e --run                   # 기존 테스트 실행
/e2e --report                # 리포트 확인
```

## 예시
```
/e2e 로그인 후 대시보드 접근
/e2e 상품 검색 및 장바구니 추가
/e2e --run auth
```

## 동작
```
1. 사용자 흐름 분석
      │
      ▼
2. Page Object 생성/업데이트
      │
      ▼
3. 테스트 코드 생성
      │
      ▼
4. 멀티 브라우저 실행
   - Chrome, Firefox, Safari
      │
      ▼
5. 결과 리포트 생성
```

## 출력물
```
🧪 E2E 테스트 결과

실행: 24개 테스트
├── ✅ Chrome: 24/24
├── ✅ Firefox: 24/24
└── ✅ Safari: 24/24

📊 리포트: ./playwright-report/index.html
```

## Page Object Model
```typescript
// 자동 생성되는 Page Object
export class LoginPage {
  readonly emailInput: Locator;
  readonly passwordInput: Locator;
  readonly submitButton: Locator;

  async login(email: string, password: string) {
    await this.emailInput.fill(email);
    await this.passwordInput.fill(password);
    await this.submitButton.click();
  }
}
```

## 셀렉터 규칙
```
✅ 권장: data-testid
❌ 금지: 자동생성 클래스, 깊은 CSS 셀렉터
```

## 실패 시 출력
```
❌ 실패: auth.spec.ts > should logout
- 스크린샷: [링크]
- 비디오: [링크]
- 원인: Timeout waiting for selector
- 수정 제안: waitForSelector 추가
```

## 관련 커맨드
- `/tdd` - 유닛 테스트
- `/verify` - 전체 검증
