---
name: frontend
description: 프론트엔드 개발 워크플로우
agent: frontend-developer
---

# /frontend 커맨드

## 사용법
```
/frontend [UI 요구사항]
```

## 예시
```
/frontend 로그인 폼 컴포넌트
/frontend 검색 결과 카드 리스트
/frontend 반응형 네비게이션 바
```

## 동작
```
1. 요구사항 분석
      │
      ▼
2. 컴포넌트 설계
   - Props 인터페이스
   - 상태 관리 전략
      │
      ▼
3. 구현
   - TypeScript + Tailwind
   - shadcn/ui 활용
      │
      ▼
4. 접근성 검증
   - ARIA
   - 키보드 네비게이션
      │
      ▼
5. 반응형 확인
```

## 출력물
```
📦 생성된 파일:
├── src/components/LoginForm.tsx
├── src/components/LoginForm.test.tsx
└── src/types/auth.ts

✨ 컴포넌트 미리보기:
[컴포넌트 설명 및 Props 문서]
```

## 기술 스택
- React 18+ (Hooks)
- TypeScript strict
- Tailwind CSS
- shadcn/ui

## 체크리스트
- [ ] TypeScript 타입 안전
- [ ] 반응형 (mobile-first)
- [ ] 다크모드 지원
- [ ] 접근성 (a11y)
- [ ] 키보드 네비게이션

## 관련 커맨드
- `/verify` - 구현 후 검증
- `/tdd` - 테스트 먼저 작성
