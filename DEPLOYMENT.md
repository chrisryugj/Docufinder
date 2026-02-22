# Anything 사내 배포 가이드

## 빌드 & 배포

### 1. 사전 요구사항
- Node.js 22+ (LTS)
- Rust 1.92+
- pnpm 10+
- Visual Studio Build Tools 2022+ (C++ 빌드 도구)

### 2. 빌드
```bash
pnpm install
pnpm run download-model
pnpm tauri:build
```
결과물: `src-tauri/target/release/bundle/msi/Anything_0.1.0_x64_ko-KR.msi`

### 3. 모델 파일
빌드 시 `pnpm run download-model`로 ONNX 모델을 다운로드합니다.
- 인터넷 차단 환경: 아래 경로에 수동 배치

| 모델 | 경로 | 파일 |
|------|------|------|
| KoSimCSE (임베딩) | `src-tauri/models/kosimcse-roberta-multitask/` | `model.onnx`, `tokenizer.json`, `onnxruntime.dll` |
| MiniLM (재정렬) | `src-tauri/models/ms-marco-MiniLM-L6-v2/` | `model.onnx`, `tokenizer.json` |

---

## 코드 서명 (필수)

### 왜 필요한가?
- **서명 없이 배포하면** Windows SmartScreen이 설치를 차단
- 사내 보안 정책에서 미서명 실행 파일 차단 가능

### 현재 설정
`tauri.conf.json`에 코드 서명이 설정되어 있음:
```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "5674CABFCAD28D70087D03DD6422436501D02B91",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com",
      "wix": {
        "language": "ko-KR"
      }
    }
  }
}
```

### 인증서 변경 방법

#### Option A: 자체 서명 인증서 (사내 배포용)
```powershell
# 1. 인증서 생성
$cert = New-SelfSignedCertificate -Subject "CN=MyCompany Code Signing" `
  -Type CodeSigningCert -CertStoreLocation Cert:\CurrentUser\My

# 2. Thumbprint 확인
$cert.Thumbprint

# 3. 인증서를 신뢰할 수 있는 루트에 추가 (GPO로 배포 권장)
Export-Certificate -Cert $cert -FilePath "MyCompany-CodeSigning.cer"

# 4. tauri.conf.json의 certificateThumbprint 업데이트
```

#### Option B: 공인 인증서 (외부 배포용)
- DigiCert, Sectigo 등에서 코드 서명 인증서 구매

---

## 업데이트 배포

현재 자동 업데이트는 비활성화 상태 (`createUpdaterArtifacts: false`).
사내 네트워크 외부 통신 차단 환경에 맞춰 수동 배포 방식 채택.

### 권장 방법
1. **공유 네트워크 드라이브**: MSI를 사내 공유 폴더에 배치
2. **SCCM/Intune**: 기업 소프트웨어 배포 도구 사용
3. **수동 배포**: MSI 파일 직접 배포

---

## 앱 데이터 위치
- DB/인덱스: `%APPDATA%/com.anything.app/`
- 로그: `%APPDATA%/com.anything.app/logs/`
- 크래시 로그: `%APPDATA%/com.anything.app/crash.log`
- 모델: 앱 설치 경로 내 `models/` (번들 리소스)

### 완전 제거 시
MSI 제거 후 `%APPDATA%/com.anything.app/` 폴더 수동 삭제

---

## 보안 사항

| 항목 | 상태 |
|------|------|
| 압축 폭탄 방어 | ✅ (크기/비율/엔트리 제한) |
| 모델 무결성 검증 | ✅ (SHA-256) |
| CSP 정책 | ✅ (`script-src 'self'`) |
| SQL Injection 방어 | ✅ (파라미터 바인딩) |
| Path Traversal 방어 | ✅ (canonicalize) |
| 크래시 핸들러 | ✅ (panic hook → crash.log) |
| 프로덕션 console.log 제거 | ✅ (esbuild drop) |
| 외부 통신 차단 | ✅ (updater 비활성화) |
