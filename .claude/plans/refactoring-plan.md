# DocuFinder 리팩토링 계획

**작성일**: 2026-01-17
**상태**: Sprint 1-4 완료, Phase 5 진행예정

---

## 📊 진행 현황

| Sprint/Phase | 내용 | 상태 |
|--------------|------|------|
| Sprint 1 | 토스트 시스템 + 최근검색 개선 | ✅ 완료 |
| Sprint 2 | 사이드바 UI/UX 강화 (즐겨찾기, 통계) | ✅ 완료 |
| Sprint 3 | 인덱싱 진행률 + 취소 | ✅ 완료 |
| Sprint 4 | 파일명 검색 (Everything 스타일) | ✅ 완료 |
| **Phase 5** | **배포 준비** | 🔄 진행예정 |

---

## Phase 5: 배포

| 우선순위 | 작업 | 설명 | 상태 |
|----------|------|------|------|
| P1 | 사용자 테스트 | 전체 기능 검증 (검색, 인덱싱, 파일 열기) | ⬜ |
| P1 | MSI 설치파일 | `pnpm tauri:build` → MSI 생성 | ⬜ |
| P2 | 자동 업데이트 | Tauri Updater 설정 (GitHub Releases 연동) | ⬜ |
| P3 | 코드 서명 | Windows 인증서 적용 (선택) | ⬜ |

### 테스트 체크리스트

- [ ] 하이브리드 검색 (키워드 + 시맨틱)
- [ ] 파일명 검색 (Everything 스타일)
- [ ] 폴더 추가/삭제/즐겨찾기
- [ ] 드라이브 루트 인덱싱 + 취소
- [ ] 파일 열기 + 경로 복사
- [ ] 다크모드 UI
- [ ] 설정 저장/불러오기

### MSI 빌드 명령어

```bash
pnpm tauri:build
# 결과: src-tauri/target/release/bundle/msi/DocuFinder_x.x.x_x64.msi
```

### 자동 업데이트 설정 (tauri.conf.json)

```json
{
  "plugins": {
    "updater": {
      "endpoints": ["https://github.com/user/docufinder/releases/latest/download/latest.json"],
      "pubkey": "YOUR_PUBLIC_KEY"
    }
  }
}
```

---

## 완료된 Sprint 요약

### Sprint 1-2: UI/UX 개선
- 범용 토스트 시스템 (`useToast`)
- 최근검색 시간 배지 + 타입 마이그레이션
- 폴더별 통계 (파일 수, 마지막 인덱싱)
- 즐겨찾기 폴더 핀 고정

### Sprint 3: 인덱싱 시스템
- 실시간 진행률 (Tauri Event)
- 취소 기능 (AtomicBool)
- 드라이브 루트 인덱싱 경고

### Sprint 4: 파일명 검색
- `files_fts` FTS5 테이블
- 4번째 검색 모드 (파일명)
- Windows Long Path 처리

---

## 리스크 및 대응

| 리스크 | 대응 |
|--------|------|
| 시스템 폴더 접근 권한 | 에러 핸들링 + 스킵 |
| 대용량 인덱싱 | 진행률 + 취소 버튼 |
| MSI 서명 없음 | Windows SmartScreen 경고 (사용자 안내) |
