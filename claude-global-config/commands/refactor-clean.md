---
name: refactor-clean
description: 데드코드 제거 및 코드 정리
agent: refactor-cleaner
---

# /refactor-clean 커맨드

## 사용법
```
/refactor-clean              # 전체 스캔 (dry-run)
/refactor-clean [경로]       # 특정 경로만
/refactor-clean --apply      # 안전한 항목 자동 적용
/refactor-clean --interactive # 항목별 확인
```

## 검사 대상
- 미사용 import
- 미사용 변수/함수
- 미사용 타입
- 주석 처리된 코드
- 중복 코드

## 동작
```
1. 전체 코드 스캔
      │
      ▼
2. 참조 그래프 생성
      │
      ▼
3. 미사용 항목 식별
      │
      ▼
4. 안전성 검증
      │
      ▼
5. 정리 적용 (옵션)
```

## 출력 형식
```
🧹 코드 정리 분석

발견된 미사용 항목: 15개
├── Imports: 8개
├── Variables: 4개
├── Functions: 2개
└── 주석 코드: 1개

✅ 안전 삭제 가능: 12개
⚠️ 확인 필요: 3개

예상 효과:
- 제거 코드: ~120줄
- 번들 감소: ~1.5KB

[--apply로 적용 또는 --interactive로 개별 확인]
```

## 안전 장치
```
✅ 안전:
- 내부에서만 사용, 참조 없음
- export 안 됨
- 주석 코드

⚠️ 주의:
- export된 항목
- 테스트에서 참조

❌ 스킵:
- 동적 import 대상
- 설정에서 참조
```

## 관련 커맨드
- `/simplify` - 코드 단순화
- `/verify` - 정리 후 검증
