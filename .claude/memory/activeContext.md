# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1**: 완료 (기반 구축)
- **Phase 2**: 완료 (파일 파서)
- **Phase 3**: 완료 (시맨틱 검색)
- **Phase 4**: 완료 (고급 기능)
- **Clean Architecture**: Phase 2 완료 (AppContainer 마이그레이션)
- **성능 최적화**: Phase 1~5 완료
- **Phase 5 배포**: ✅ **보안 강화 Phase 1~2 완료, 베타 배포 준비 완료**

## 마지막 업데이트
2026-01-31 (보안 강화 Phase 2 완료 - 파싱 오류 알림, FTS 성능 개선)

---

## 🚀 Phase 5: 사내 배포 준비 (완료)

### ✅ Phase 1 완료 (보안 필수)
| 작업 | 파일 | 설명 |
|------|------|------|
| **업데이터 비활성화** | `tauri.conf.json`, `default.json`, `lib.rs` | 외부 통신 차단 |
| **SHA-256 무결성 검증** | `model_downloader.rs` | 모델/DLL 다운로드 시 해시 검증 |
| **타임아웃 추가** | `model_downloader.rs` | 30초 연결, 5분 읽기 |
| **압축 폭탄 방어** | `hwpx.rs`, `docx.rs`, `xlsx.rs` | uncompressed size, 엔트리 수, 압축비 제한 |

### ✅ Phase 2 완료 (안정성 강화)
| 작업 | 파일 | 설명 |
|------|------|------|
| **파싱 오류 알림** | `App.tsx:141-162` | 토스트로 실패 수 표시 |
| **FTS SSD 모드 강제** | `pipeline.rs:409-421` | HDD에서도 병렬 처리 |
| **PDF 타임아웃 5초** | `pdf.rs:9` | 10초 → 5초 |
| **경로 파싱 수정** | `disk_info.rs:26-36` | `\\?\` 접두사 처리 |

### 📋 Phase 3 (정책 준수 - 옵션)
- [ ] DB 암호화 검토 (SQLCipher) - 정책 요구 시
- [ ] CSP 강화 - 보안팀 요구 시

---

## 📊 인덱싱 테스트 결과 (1695개 파일)

| 항목 | 결과 |
|------|------|
| 성공 | 1558개 (92%) |
| 실패 | 137개 (8%) |
| 실패 원인 | ZIP 손상, PDF 타임아웃, 압축 폭탄 방어 |

---

## 핵심 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/lib.rs` | AppContainer 초기화 |
| `src-tauri/src/indexer/pipeline.rs` | FTS 인덱싱 파이프라인 |
| `src-tauri/src/model_downloader.rs` | 모델 다운로드 + SHA-256 검증 |
| `src-tauri/src/utils/disk_info.rs` | SSD/HDD 감지 |
| `src-tauri/src/parsers/` | 문서 파서 (압축 폭탄 방어 포함) |
| `src/App.tsx` | 프론트엔드 앱 (파싱 오류 토스트) |

## 실행 방법
```bash
pnpm tauri:dev      # 개발 모드
pnpm tauri:build    # MSI 빌드
```
