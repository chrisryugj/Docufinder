# 2026-04-17 — HWPX DRM COM 팝업 우회

## 증상

회사 PC(Windows + 한컴오피스 설치)에서 DRM HWPX 인덱싱 시 한글 보안 경고 다이얼로그(#32770 타이틀 "한글") 노출. 집 PC에서는 같은 kordoc v2.4.0 코드가 팝업 없이 정상 동작.

## 원인 규명

한컴오피스의 **FilePathChecker**는 COM 호출로 파일을 Open할 때 파일 경로가 "신뢰 영역"(`%TEMP%`, `%APPDATA%`, `%USERPROFILE%\Documents` 등) 밖이면 접근 허용 다이얼로그를 띄움. 

`$hwp.RegisterModule('FilePathCheckerModule', 'FilePathCheckerModuleExample')` 호출은 `FilePathCheckerModuleExample` COM 클래스가 레지스트리에 등록된 **개발 환경**(Hancom SDK 설치 PC)에서만 작동. 일반 사용자 환경에서는 해당 ProgID가 등록되지 않아 silent fail → 팝업 그대로 노출.

집 PC는 Hancom SDK 또는 유사 도구로 DLL이 등록된 상태였을 가능성. 회사 PC에서는 미등록 (`[System.Type]::GetTypeFromProgID('FilePathCheckerModuleExample')` → null).

## 시도한 우회 (실패)

| 방법 | 결과 |
|------|------|
| `SetMessageBoxMode(0xFFFF0001)` | 한컴 내부 메시지박스만 억제. #32770 외부 다이얼로그엔 효과 없음 |
| Runspace 기반 Win32 FindWindow + PostMessage(Enter) 워처 | 자식 PowerShell 프로세스의 desktop 컨텍스트에서 FindWindow가 일관되게 매칭 못함 |
| Runspace 기반 FindWindowEx + BM_CLICK 버튼 타겟 | 동일 이유로 불안정 |
| `isEncryptedHwpx` 조건 완화 (Contents/* 암호화 시에만 DRM으로 판정) | 해당 파일은 본문 전체(Contents/header.xml, Contents/section0.xml, settings.xml) 모두 실제 AES-256 암호화되어 있어 조건 완화 무효. JS 파싱 경로 불가 |

## 최종 해결 (성공)

**원본 파일을 `%TEMP%\hwp-com-<guid>\<파일명>`으로 복사한 뒤 COM Open**. `%TEMP%`는 한컴 신뢰 영역이라 FilePathChecker가 경고를 띄우지 않음. 작업 후 임시 폴더 정리.

추가: `$hwp.Quit()` + GC로 좀비 `Hwp.exe` 프로세스 누적 방지 (기존엔 `ReleaseComObject`만 있어 COM 호출마다 Hwp.exe가 남음 — 테스트 중 8개 누적 확인).

## 커밋

### kordoc (github.com/chrisryugj/kordoc, main)

| 해시 | 내용 |
|------|------|
| `7352663` | feat: HWPX DRM COM fallback 최초 도입 (v2.4.0, 2026-04-17 00:06) |
| `50bcd30` | (실패 시도) SetMessageBoxMode 추가 |
| `431beda` | (실패 시도) 워처 기반 BM_CLICK |
| `cf26183` | **%TEMP% 복사 우회 도입 (핵심 해결)** |
| `060f3d7` | **Quit() + GC로 좀비 프로세스 방지** |

### Docufinder (github.com/chrisryugj/Docufinder, main)

| 해시 | 내용 |
|------|------|
| `e7ab86b` | fix(ui): 설정 모달 토글 설명 문구 단어 중간 줄바꿈 방지 (wordBreak: keep-all + flex-shrink-0) |

Docufinder는 kordoc을 번들로 포함([src-tauri/resources/kordoc/](../../src-tauri/resources/kordoc/)). 번들 자체는 `.gitignore`에 포함되어 있어 commit 대상 아님. 빌드 시 `pnpm run bundle-kordoc` 실행으로 재생성.

## 검증

CLI 직접 호출(Docufinder 번들 경로):
```bash
node src-tauri/target/debug/resources/kordoc/cli.js "D:/anything test/서울시 식품안전관리 시행계획 2025.hwpx" --format json --silent
```
→ `success: true` + 마크다운 42페이지 정상 추출, 팝업 없음.

Hwp.exe 프로세스 수: 실행 전 8개(좀비) → 실행 후 3개(남은 건 한컴 서비스, 작업 무관).

## 집에서 이어갈 작업

1. `cd d:/AI_Project/kordoc && git pull` — 최신 반영
2. `cd d:/AI_Project/Docufinder && git pull` — 설정 UI fix 반영
3. Docufinder: `pnpm tauri:dev` **재시작** (실행 중 번들 반영 안 되므로)
4. 재인덱싱해서 팝업 없음 + 정상 추출 확인

## 관련 파일

- [kordoc com-fallback.ts](../../../kordoc/src/hwpx/com-fallback.ts) — 핵심 수정
- [kordoc parser.ts:178-192](../../../kordoc/src/hwpx/parser.ts) — DRM 감지 및 COM fallback 호출
- [Docufinder kordoc.rs](../../src-tauri/src/parsers/kordoc.rs) — kordoc 사이드카 Rust 래퍼
- [Docufinder bundle-kordoc.ps1](../../scripts/bundle-kordoc.ps1) — 번들 스크립트
