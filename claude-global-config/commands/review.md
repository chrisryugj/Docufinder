---
name: review
description: 코드 리뷰 실행
agent: code-reviewer
---

# /review 커맨드

## 사용법
```
/review                      # 변경된 파일 리뷰
/review [파일경로]           # 특정 파일 리뷰
/review --security           # 보안 집중 리뷰
/review --performance        # 성능 집중 리뷰
```

## 예시
```
/review
/review src/api/users.ts
/review --security src/auth/
```

## 동작
1. 대상 파일 식별 (git diff 또는 명시적 경로)
2. `code-reviewer` 에이전트 호출
3. 품질/보안/성능 관점 분석
4. 문제점 및 개선사항 리포트

## 체크 항목
- **품질**: 가독성, 구조, DRY, 네이밍
- **보안**: 입력 검증, 인젝션, 시크릿
- **성능**: 복잡도, N+1, 메모리
- **TypeScript**: any, null safety

## 출력 형식
```markdown
# 코드 리뷰 결과

## 요약
- 전체 평가: ⭐⭐⭐⭐☆
- 🔴 Critical: 0개
- 🟡 Warning: 2개
- 🔵 Info: 3개

## 문제점
| 심각도 | 파일:라인 | 문제 | 제안 |
|--------|-----------|------|------|
| 🟡 | api.ts:42 | any 사용 | 타입 정의 |

## 잘된 점
- ✅ 일관된 코딩 스타일
```

## 옵션
- `--security`: `security-reviewer` 추가 호출
- `--performance`: 성능 분석 강화
- `--strict`: 모든 경고를 에러로 취급

## 관련 커맨드
- `/verify` - 자동 검증
- `/commit-push-pr` - 리뷰 후 커밋
