---
name: commit-push-pr
description: Git 워크플로우 통합 (커밋 → 푸시 → PR)
---

# /commit-push-pr 커맨드

## 사용법
```
/commit-push-pr              # 전체 워크플로우
/commit-push-pr --no-pr      # PR 생성 제외
/commit-push-pr --amend      # 이전 커밋 수정
```

## 동작 순서
```
1. git status 확인
      │
      ▼
2. git diff 분석
      │
      ▼
3. 커밋 메시지 자동 생성
      │
      ▼
4. git add + commit
      │
      ▼
5. git push
      │
      ▼
6. gh pr create (옵션)
```

## 커밋 메시지 형식
```
<type>(<scope>): <description>

<body>

<footer>
```

### 타입
| 타입 | 설명 | 예시 |
|------|------|------|
| feat | 새 기능 | feat(auth): 로그인 기능 추가 |
| fix | 버그 수정 | fix(api): null 처리 수정 |
| refactor | 리팩토링 | refactor(utils): 함수 분리 |
| docs | 문서 | docs: README 업데이트 |
| test | 테스트 | test: 유닛 테스트 추가 |
| chore | 기타 | chore: 의존성 업데이트 |

### 예시
```
feat(search): 하이브리드 검색 기능 구현

- FTS5 키워드 검색 추가
- 벡터 시맨틱 검색 추가
- RRF 병합 알고리즘 구현

Closes #123
```

## 안전 장치
- ⚠️ main/master 브랜치에서 실행 차단
- ⚠️ .env 파일 커밋 경고
- ⚠️ console.log 포함시 경고
- ⚠️ `/verify` 실패시 경고

## PR 생성 형식
```markdown
## Summary
- [변경 내용 요약]

## Changes
- [상세 변경 목록]

## Test Plan
- [ ] 테스트 항목

## Related Issues
Closes #123
```

## 출력 형식
```
📝 변경 사항 분석 중...

변경된 파일: 5개
├── src/api/search.ts (modified)
├── src/components/SearchBar.tsx (modified)
└── ...

💬 커밋 메시지 생성:
feat(search): 하이브리드 검색 기능 구현

✅ 커밋 완료
✅ 푸시 완료
✅ PR 생성: https://github.com/...
```

## 관련 커맨드
- `/verify` - 커밋 전 검증
- `/review` - 커밋 전 리뷰
