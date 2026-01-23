---
name: learn
description: 세션 중 패턴 추출 및 학습
---

# /learn 커맨드

## 사용법
```
/learn                       # 현재 세션 패턴 분석
/learn [패턴 설명]           # 특정 패턴 저장
/learn --list                # 학습된 패턴 목록
```

## 용도
- 프로젝트별 패턴 학습
- 반복 작업 템플릿화
- 코딩 스타일 기억

## 학습 대상
```
- 코드 패턴 (컴포넌트 구조, API 호출 방식)
- 네이밍 컨벤션
- 프로젝트 특화 규칙
- 자주 사용하는 명령어
```

## 동작
```
1. 세션 분석
   - 반복된 패턴 감지
   - 사용자 피드백 수집
      │
      ▼
2. 패턴 추출
      │
      ▼
3. 스킬 파일 생성/업데이트
      │
      ▼
4. 프로젝트 .claude/skills/에 저장
```

## 출력 형식
```
🧠 패턴 학습

감지된 패턴:
1. API 호출 패턴
   - try/catch + toast 알림
   - 사용 횟수: 5회

2. 컴포넌트 구조
   - Props 인터페이스 분리
   - 사용 횟수: 8회

저장할 패턴 선택: [1, 2, all, none]
```

## 저장 형식
```markdown
# 프로젝트 패턴: API 호출

## 패턴
```typescript
async function apiCall<T>(
  fn: () => Promise<T>,
  options?: { successMessage?: string }
): Promise<T | null> {
  try {
    const result = await fn();
    if (options?.successMessage) {
      toast.success(options.successMessage);
    }
    return result;
  } catch (error) {
    toast.error(error.message);
    return null;
  }
}
```

## 사용 예시
```typescript
const user = await apiCall(
  () => fetchUser(id),
  { successMessage: '사용자 정보를 불러왔습니다.' }
);
```
```

## 활용
학습된 패턴은 이후 세션에서 자동 적용됨

## 관련 커맨드
- `/handoff` - 세션 인계
