# Changelog

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
