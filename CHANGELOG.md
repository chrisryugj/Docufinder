# Changelog

## [2.5.0] - 2026-04-20

**OneDrive·네트워크 폴더 대응 + 모델 번들 (오프라인/회사망 친화)**

### 추가
- **ONNX Runtime + PaddleOCR 모델 MSI 번들** — 첫 실행 시 인터넷 차단 환경에서도 즉시 시맨틱 검색·OCR 가능. 회사망/방화벽으로 huggingface·github 다운로드가 막혀도 동작.
- **OneDrive(클라우드 placeholder) 차단** — Files-On-Demand 로 클라우드에만 있는 파일은 본문 파싱을 자동으로 skip. 인덱서가 모르는 사이 수십 GB 를 끌어내리던 사고 방지. 파일명·크기·수정일은 정상 인덱싱(파일명 검색 가능).
- **네트워크 폴더(UNC, `\\server\share`) 정식 지원** — UNC 경로 정규화(dunce), 30초 주기 PollWatcher 분기, kordoc/HWP 파서 호환. SMB 위에서 inotify 가 동작 안 하는 한계를 폴링으로 우회.
- **`dunce` 의존성 추가** — Windows extended-length(`\\?\`) / UNC prefix 일관 정규화.

### 개선
- 폴더 등록 시 `dunce::canonicalize` 사용 — 네트워크 경로에서 표준 canonicalize 가 수십 초 block 되던 문제 해소.
- 인덱싱 결과에 `cloud_skipped_count` 추가 — 본문 skip 통계와 실패를 분리.

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB

---

## [2.4.0] - 2026-04-20

**최초 배포 (Public release)**

내 PC 문서를 통째로 검색하는 로컬 검색 엔진. 파일명 몰라도, 열어보지 않아도 문서 **내용**으로 찾습니다.

### 핵심 기능
- 문서 내용 검색 (SQLite FTS5, 1초 이내)
- 파일명 검색 (Everything 스타일, 인메모리 캐시)
- 시맨틱/하이브리드 검색 (KoSimCSE ONNX 768차원)
- AI 질의응답 + 문서 요약 (Gemini API, 선택)
- 문서 버전 자동 그룹핑 (lineage) + 버전 간 diff
- 실시간 파일 감시 (`.gitignore` 자동 존중)
- HWPX/DOCX/XLSX/PPTX/PDF/이미지(OCR)/텍스트 지원

### 시스템 요구사항
Windows 10 (21H2+) / Windows 11 · RAM 8GB 이상 (16GB 권장) · 디스크 여유 1GB
