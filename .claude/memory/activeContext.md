# DocuFinder 현재 컨텍스트

## 프로젝트 상태
- **Phase 1**: 완료 (기반 구축)
- **Phase 2**: 완료 (파일 파서)
- **Phase 3**: 완료 (시맨틱 검색)
- **Phase 4**: 완료 (고급 기능)

## 완료된 작업

### Phase 1 (기반 구축)
- Tauri 프로젝트 셋업
- AppState (DB 경로 공유)
- DB CRUD 함수 (파일, 청크, 폴더)
- 인덱싱 파이프라인 (폴더 순회 → 파싱 → DB 저장)
- FTS5 검색 + 파일 정보 조인
- 프론트엔드 UI (폴더 선택, 검색, 결과 표시)

### Phase 2 (파일 파서)
| 파서 | 파일 | 라이브러리 |
|------|------|------------|
| HWPX | `parsers/hwpx.rs` | zip + quick-xml |
| DOCX | `parsers/docx.rs` | zip + quick-xml |
| XLSX | `parsers/xlsx.rs` | calamine |
| PDF | `parsers/pdf.rs` | pdf-extract |
| TXT/MD | `parsers/txt.rs` | 내장 |

### Phase 3 (시맨틱 검색)
| 컴포넌트 | 파일 | 라이브러리 |
|----------|------|------------|
| 임베더 | `embedder/mod.rs` | ort 2.0.0-rc.11 + tokenizers |
| 벡터 인덱스 | `search/vector.rs` | usearch 2.23 |
| 하이브리드 검색 | `search/hybrid.rs` | RRF 알고리즘 |

**주요 기능**:
- multilingual-e5-small 모델 (384차원, 118MB)
- 청크 임베딩 → usearch HNSW 인덱스
- 시맨틱 검색 (벡터 유사도)
- 하이브리드 검색 (FTS + 벡터 + RRF 병합)

### Phase 4 (고급 기능)
| 기능 | 파일 | 설명 |
|------|------|------|
| 파일 감시 | `indexer/manager.rs` | notify 기반, 디바운스 처리 |
| 증분 인덱싱 | `indexer/manager.rs` | 백그라운드 스레드에서 자동 처리 |
| 하이라이트 | `App.tsx` | highlight_ranges → mark 태그 |
| 검색 모드 | `App.tsx` | keyword/semantic/hybrid 선택 |
| 파일 열기 | `App.tsx` | shell:open 연동 (기존) |

## 다음 작업 (Phase 5)
1. MSI 설치파일 생성
2. 자동 업데이트 설정
3. 사용자 가이드 문서

## 핵심 파일
| 파일 | 역할 |
|------|------|
| `src-tauri/src/lib.rs` | AppState (embedder, vector_index, watch_manager) |
| `src-tauri/src/embedder/mod.rs` | ONNX 임베딩 생성 |
| `src-tauri/src/search/vector.rs` | usearch 벡터 인덱스 |
| `src-tauri/src/search/hybrid.rs` | RRF 병합 |
| `src-tauri/src/indexer/pipeline.rs` | 인덱싱 (FTS + 벡터) |
| `src-tauri/src/indexer/manager.rs` | 파일 감시 + 증분 인덱싱 |
| `src-tauri/src/commands/search.rs` | 검색 커맨드 (keyword, semantic, hybrid) |

## 모델 다운로드
시맨틱 검색을 사용하려면 모델 파일이 필요합니다:
```
{앱 데이터 폴더}/models/multilingual-e5-small/
├── model.onnx       (118MB)
└── tokenizer.json   (17MB)
```

HuggingFace에서 다운로드:
- https://huggingface.co/intfloat/multilingual-e5-small

## 실행 방법
```bash
pnpm tauri:dev
```

## 마지막 업데이트
2026-01-14
