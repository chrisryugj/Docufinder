---
name: update-docs
description: 문서 자동 동기화
agent: doc-updater
---

# /update-docs 커맨드

## 사용법
```
/update-docs                 # 전체 문서 동기화
/update-docs --readme        # README만
/update-docs --api           # API 문서만
/update-docs --changelog     # CHANGELOG만
```

## 대상 문서
- README.md
- API 문서
- CHANGELOG.md
- JSDoc/Rustdoc

## 동작
```
1. 코드 변경 감지 (git diff)
      │
      ▼
2. 영향받는 문서 식별
      │
      ▼
3. 자동 업데이트 생성
      │
      ▼
4. 수동 필요 항목 표시
```

## 자동 감지 항목
| 코드 변경 | 문서 영향 |
|----------|----------|
| 새 API 엔드포인트 | API.md, README |
| 환경변수 추가 | README, .env.example |
| 함수 시그니처 변경 | JSDoc |
| 새 기능 추가 | README, CHANGELOG |

## 출력 형식
```
📝 문서 동기화 분석

감지된 변경:
├── 새 API: GET /api/search
├── 환경변수: SEARCH_API_KEY
└── 함수 변경: searchDocuments()

자동 업데이트:
├── ✅ README.md - 환경변수 섹션
├── ✅ API.md - 새 엔드포인트
└── ✅ .env.example

수동 필요:
└── ⚠️ CHANGELOG.md - 변경 내용 요약

[적용하시겠습니까? Y/n]
```

## CHANGELOG 형식
```markdown
## [Unreleased]

### Added
- 하이브리드 검색 API (`GET /api/search`)
- `SEARCH_API_KEY` 환경변수 지원

### Changed
- `searchDocuments` 함수에 옵션 파라미터 추가
```

## 관련 커맨드
- `/commit-push-pr` - 문서 포함 커밋
