# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1**: 완료 (기반 구축)
- **Phase 2**: 완료 (파일 파서)
- **Phase 3**: 완료 (시맨틱 검색)
- **Phase 4**: 완료 (고급 기능)
- **Phase 5**: 완료 (배포 준비 + 프로덕션 리뷰 4차 88/100)

## 마지막 업데이트
2026-02-22 (수익화 전략 v3 수립)

---

## 현재 상태 요약

**프로덕션 종합 점수**: 88/100
**배포 상태**: MSI 빌드 + 코드 서명 설정 완료
**수익화 전략**: `.claude/plans/crispy-doodling-aurora.md` (v3)

### 남은 P2 작업 (3건)
- [ ] 이중 FS 순회 통합 (index.rs, pipeline.rs)
- [ ] data_root 설정 기능 (container.rs, settings.rs)
- [ ] PDF timeout 5s→3s (parsers/pdf.rs)

### 수익화 다음 단계
- [ ] 겸직허가 비공식 문의
- [ ] 부모 법인 사업 목적 추가 등기 준비
- [ ] GitHub Sponsors / Buy Me a Coffee 설정
- [ ] korean-law-mcp Pro 기능 설계

---

## 핵심 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/lib.rs` | AppContainer 초기화 + 크래시 핸들러 |
| `src-tauri/src/application/container.rs` | DI 컨테이너 |
| `src-tauri/src/application/services/search_service.rs` | 검색 비즈니스 로직 |
| `src-tauri/src/indexer/pipeline.rs` | FTS 인덱싱 파이프라인 |
| `src-tauri/src/indexer/vector_worker.rs` | 벡터 인덱싱 워커 (Drop 구현) |
| `src-tauri/src/indexer/background_parser.rs` | 백그라운드 파싱 (유휴 감지) |
| `src-tauri/src/model_downloader.rs` | 모델 다운로드 + SHA-256 검증 |
| `src-tauri/src/utils/disk_info.rs` | SSD/HDD 감지 (OnceLock 캐싱) |
| `src-tauri/src/parsers/` | 문서 파서 (압축 폭탄 방어 + 사이즈 리밋) |
| `src-tauri/src/search/vector.rs` | 벡터 인덱스 (TOCTOU 수정, 단일 write lock) |
| `src-tauri/src/search/filename_cache.rs` | 파일명 인메모리 캐시 |
| `src-tauri/src/tokenizer/lindera_ko.rs` | 한국어 형태소 분석 |
| `src-tauri/src/reranker/mod.rs` | 시맨틱 결과 재정렬 |
| `src/App.tsx` | 프론트엔드 앱 |
| `src/ErrorBoundary.tsx` | 에러 바운더리 |

## 실행 방법
```bash
pnpm tauri:dev      # 개발 모드
pnpm tauri:build    # MSI 빌드
```
