---
name: checkpoint
description: 검증된 상태 저장점 생성
---

# /checkpoint 커맨드

## 사용법
```
/checkpoint                  # 현재 상태 저장
/checkpoint "설명"           # 설명과 함께 저장
/checkpoint --verify         # 검증 후 저장
```

## 용도
- 안정적인 상태 마킹
- 롤백 지점 생성
- 복잡한 작업 중간 저장

## 동작
```
1. 현재 git 상태 저장
      │
      ▼
2. (--verify 옵션 시) 테스트 실행
      │
      ▼
3. 체크포인트 기록 생성
      │
      ▼
4. .claude/checkpoints/에 저장
```

## 출력 형식
```markdown
# Checkpoint: 2024-01-15-143022

## 설명
벡터 검색 기본 구현 완료

## Git 상태
- 브랜치: feature/search
- 커밋: abc1234
- 변경 파일: 3개 (staged: 0, unstaged: 3)

## 검증 결과 (--verify 시)
- ✅ Type Check: 통과
- ✅ Lint: 통과
- ✅ Tests: 12/12 통과
- ✅ Build: 성공

## 복원 방법
```bash
# 이 체크포인트로 복원
git checkout abc1234

# 또는 stash 적용
git stash apply stash@{0}
```
```

## 체크포인트 목록 확인
```
/checkpoint --list
```
```
체크포인트 목록:
1. 2024-01-15-143022: 벡터 검색 기본 구현
2. 2024-01-15-120015: FTS5 인덱싱 완료
3. 2024-01-14-180030: 초기 설정
```

## 복원
```
/checkpoint --restore 1
```

## 자동 생성 조건
- `/verify` 성공 후 제안
- 주요 기능 완료 후 제안

## 관련 커맨드
- `/handoff` - 세션 인계
- `/verify` - 검증
