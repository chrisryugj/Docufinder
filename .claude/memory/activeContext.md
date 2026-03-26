# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1~5**: 완료
- **Phase 6**: 완료 (3대 기능 + 프로덕션 리뷰 + 리팩토링)
- **v2.1~v2.8**: 모두 커밋+푸시 완료
- **프로덕션 리뷰 1차**: 커밋 완료 (f5bdbd8)
- **프로덕션 리뷰 2차**: 커밋 완료 (2d5f5f4)
- **프로덕션 리뷰 3차**: 커밋 완료 (3ec38b1)

## 마지막 업데이트
2026-03-23 (프로덕션 리뷰 3차 — 보안 3건 + 품질 5건 수정)

---

## 현재 상태 요약

**빌드 상태**: cargo check 통과 + tsc --noEmit 통과
**커밋 상태**: 리뷰 3차 커밋 완료 (3ec38b1, 5파일 +167/-256)
**통합 테스트**: 미실행 (`pnpm tauri:dev` 실제 동작 확인 필요)

### ✅ 프로덕션 리뷰 3차 수정 내역 (8건)

| # | 심각도 | 항목 | 변경 내용 |
|---|--------|------|----------|
| 1 | SEC-H | package_zip 경로 검증 | canonicalize() + symlink 방지 + BLOCKED_PATH_PATTERNS 차단 |
| 2 | SEC-H | export_csv 개행 누락 | 500자 초과 시에도 `replace('\n', " ")` 적용 |
| 3 | SEC-M | open_url 입력 검증 | 제어문자·공백·탭 검증 추가 |
| 4 | QUAL-H | index.rs 코드 중복 | `extract_indexing_context()` + `maybe_start_auto_vector()` 헬퍼 (~90줄 x3 → 공통화) |
| 5 | QUAL-H | file.rs 플랫폼 분기 중복 | `open_with_default()` 공통 함수 (60줄+ 제거) |
| 6 | QUAL-M | useSearch.ts sweepTimer HMR | `import.meta.hot.dispose()` 정리 |
| 7 | QUAL-M | autoComplete.close 비안정 참조 | `useRef` 패턴으로 안정화 |
| 8 | QUAL-M | aiAutoRef useEffect 의존성 | eslint-disable 주석 + 의도 명시 |

**수정 파일 (4):**
- `src-tauri/src/commands/export.rs` — ZIP 경로 검증 + CSV 개행 수정
- `src-tauri/src/commands/file.rs` — URL 검증 + open_with_default 통합
- `src-tauri/src/commands/index.rs` — extract_indexing_context 공통 헬퍼
- `src/App.tsx` — autoComplete ref + eslint 주석
- `src/hooks/useSearch.ts` — HMR dispose

### 🔍 여전히 보류 중인 항목

- **H-2**: OCR 모델 SHA-256 해시 채우기 — 실제 다운로드 후 로그에서 확인 필요
- **SEC-H2**: 하드코딩 admin code 9812 — 내부앱 수준 (문서화됨)
- **SEC-M4**: API 키 plaintext 저장 — Windows DPAPI 통합은 별도 feature

### 📋 다음 할 일

**P1 (우선):**
- [x] 리뷰 3차 코드 커밋 (3ec38b1)
- [ ] `pnpm tauri:dev` 통합 테스트 — 전체 기능 동작 확인
- [ ] OCR 토글 ON → 이미지 폴더 인덱싱 + 스캔 PDF 인덱싱 → 검색
- [ ] SHA-256 해시 채우기 — OCR 모델 최초 다운로드 시 로그 확인 → `model_downloader.rs` 기입

**P2 (후속):**
- [ ] HWP 변환 테스트 (한글 설치된 PC)
- [ ] MSI 빌드 → VM 설치 테스트

**P3 (장기):**
- [ ] 관계맵 (D3 그래프) — 미구현
- [ ] AI 스트리밍 응답 (v2.6 후속)

### 🧹 미커밋 잔여
- `HwpxConverterSetup.exe` 미추적 (번들링 여부 결정 필요)
- `docs/` 디렉토리 미추적
- `.claude/memory/` 변경 미커밋

## 핵심 설계 결정

### OCR 통합 아키텍처
- **parse_file 확장**: `Option<&OcrEngine>` 파라미터 추가 — 기존 호출은 None으로 무영향
- **설정 기반 주입**: `ocr_enabled=false` → OCR 엔진 None → 기존과 100% 동일 동작
- **파이프라인 전달 체인**: container → index_service → pipeline/sync → parse_file
- **이미지 확장자 분리**: `SUPPORTED_EXTENSIONS`에 추가 안 함, OCR 있을 때만 partition에 포함
- **스캔 PDF OCR**: lopdf로 임베디드 이미지 직접 추출 (pdfium 불필요, 추가 DLL 없음)
- **성능 보호**: MAX_OCR_PAGES=20, MAX_FILE_SIZE=100MB, 이미지 2000px 리사이즈

### CSV export 아키텍처 (리뷰 2차)
- 프론트엔드 DOM Blob → **Rust 백엔드** `export_csv` 커맨드로 이전
- XLSX export와 동일 패턴: Tauri save dialog → Rust 파일 쓰기
- 수식 주입 방어(=, +, -, @)는 Rust `escape_csv()`에서 처리

### 인덱싱 명령 리팩토링 (리뷰 3차)
- `extract_indexing_context()`: 설정/서비스/DB경로/제외목록을 단일 lock에서 추출하는 공통 헬퍼
- `maybe_start_auto_vector()`: 벡터 자동 시작 판단+실행 공통화
- add_folder/reindex_folder/resume_indexing 3곳에서 사용

---

## 다음 세션 이어가기 프롬프트

```
Docufinder(Anything) 프로젝트 — 로컬 문서 검색 앱 (Tauri 2 + React).

프로덕션 리뷰 1~3차 모두 커밋 완료. 빌드 통과 (cargo check + tsc).
main 브랜치 3커밋 ahead (origin 대비).

다음 할 일:
1. `pnpm tauri:dev` 통합 테스트 — 전체 기능 동작 확인
2. OCR 테스트: 설정에서 토글 ON → 이미지 폴더 인덱싱 + 스캔 PDF → 검색
3. SHA-256 해시 채우기 (OCR 모델 최초 다운로드 로그 → model_downloader.rs)
4. HWP 변환 테스트 (한글 설치 PC)
5. MSI 빌드 → VM 설치 테스트

컨텍스트: .claude/memory/activeContext.md
```
