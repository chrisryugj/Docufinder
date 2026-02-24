# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/lang/ko/).

## [1.0.0] - 2026-02-22

### Added
- 하이브리드 검색: 키워드(FTS5) + 시맨틱(벡터) + RRF 병합 + Cross-Encoder 재정렬
- Everything 스타일 파일명 검색 (인메모리 캐시)
- 실시간 폴더 감시 + 증분 인덱싱 (notify 8)
- 2단계 인덱싱: FTS 즉시 완료 → 벡터 백그라운드 처리
- 인덱싱 진행률 실시간 표시 + 취소 버튼
- 즐겨찾기 폴더 핀 고정
- HDD/SSD 자동 감지 + 적응형 스레딩
- 지원 파일 형식: HWPX, DOCX, XLSX, PDF, TXT
- KoSimCSE-roberta-multitask 임베딩 (768차원)
- ms-marco-MiniLM-L6-v2 재정렬 모델
- Lindera 2.0 한국어 형태소 분석
- 다크모드 / 라이트모드 / 시스템 테마
- 색상 프리셋 커스터마이징
- CSP 보안 정책 적용
- 압축 폭탄 방어 (크기/비율/엔트리 제한)
- SHA-256 모델 무결성 검증
- HuggingFace 자동 모델 다운로드
- MSI 설치 파일 빌드 (코드 서명 지원)
- 크래시 핸들러 (panic hook → crash.log, 7일 로테이션)
