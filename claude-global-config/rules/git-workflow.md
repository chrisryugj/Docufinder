---
name: git-workflow
trigger: always
---

# Git 워크플로우 규칙

## 브랜치 전략

### 브랜치 네이밍
```
main (또는 master)  - 항상 배포 가능
├── feature/*       - 새 기능 개발
├── fix/*           - 버그 수정
├── refactor/*      - 리팩토링
├── docs/*          - 문서 작업
└── chore/*         - 기타 (의존성 등)

예시:
- feature/user-authentication
- fix/login-validation
- refactor/api-structure
```

### 브랜치 규칙
```
✅ 허용:
- feature 브랜치에서 개발
- PR을 통한 main 머지

❌ 금지:
- main 직접 푸시
- force push (특히 main)
```

## 커밋 규칙

### Conventional Commits
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### 타입
| 타입 | 설명 | 예시 |
|------|------|------|
| feat | 새 기능 | feat(auth): 소셜 로그인 추가 |
| fix | 버그 수정 | fix(api): null 체크 추가 |
| refactor | 리팩토링 | refactor(utils): 함수 분리 |
| docs | 문서 | docs: API 문서 업데이트 |
| test | 테스트 | test: 유닛 테스트 추가 |
| chore | 기타 | chore: 의존성 업데이트 |
| style | 포맷팅 | style: 들여쓰기 수정 |
| perf | 성능 | perf: 쿼리 최적화 |

### 좋은 커밋 메시지
```
✅ 좋은 예:
feat(search): 하이브리드 검색 기능 구현

- FTS5 키워드 검색 추가
- 벡터 시맨틱 검색 추가
- RRF 병합 알고리즘 구현

Closes #123

❌ 나쁜 예:
- "fix"
- "update"
- "WIP"
- "asdf"
```

### 커밋 크기
```
✅ 권장:
- 하나의 논리적 변경 = 하나의 커밋
- 작고 자주 커밋
- 리뷰 가능한 크기

❌ 피해야 할 것:
- 여러 기능을 하나의 커밋에
- 너무 큰 커밋
- 동작하지 않는 상태로 커밋
```

## 금지 사항

### 커밋 금지 파일
```
❌ 절대 금지:
- .env (환경변수, 시크릿)
- node_modules/
- *.log
- .DS_Store
- credentials.json
- *.pem, *.key

⚠️ 주의:
- 빌드 결과물 (dist/, build/)
- IDE 설정 (.idea/, .vscode/)
- 바이너리 파일
```

### 커밋 금지 패턴
```
❌ 금지:
- console.log (디버깅용)
- debugger
- 주석 처리된 코드 (대량)
- TODO 없는 임시 코드
```

## PR 규칙

### PR 생성
```markdown
## Summary
[1-3줄 요약]

## Changes
- [변경 내용 목록]

## Test Plan
- [ ] 테스트 항목

## Screenshots (UI 변경 시)
[스크린샷]

## Related Issues
Closes #123
```

### PR 체크리스트
```
✅ 머지 전 확인:
- [ ] 테스트 통과
- [ ] 빌드 성공
- [ ] 코드 리뷰 승인
- [ ] 충돌 해결
- [ ] 문서 업데이트 (필요시)
```

### 머지 전략
```
✅ 권장:
- Squash and merge (기능 브랜치)
- Rebase (깔끔한 히스토리)

상황별:
- 작은 기능: Squash
- 큰 기능 (히스토리 유지): Merge commit
```

## 되돌리기

### 안전한 방법
```bash
# 커밋 되돌리기 (새 커밋 생성)
git revert <commit>

# 스테이징 취소
git reset HEAD <file>
```

### 위험한 방법 (주의)
```bash
# ⚠️ 히스토리 변경 (로컬에서만)
git reset --hard <commit>

# 🚨 절대 금지 (공유 브랜치에서)
git push --force origin main
```
