# Anything 사내 배포 가이드

## 빌드 & 배포

### 1. 사전 요구사항
- Node.js 18+
- Rust 1.75+
- pnpm 10+
- Visual Studio Build Tools (C++ 빌드 도구)

### 2. 빌드
```bash
pnpm install
pnpm tauri:build
```
결과물: `src-tauri/target/release/bundle/msi/Anything_0.1.0_x64_ko-KR.msi`

### 3. 모델 파일
빌드 시 `pnpm run download-model`이 자동 실행되어 ONNX 모델을 다운로드합니다.
- 인터넷 차단 환경: `src-tauri/models/multilingual-e5-small/` 디렉토리에 수동 배치
- 필요 파일: `model.onnx`, `tokenizer.json`, `onnxruntime.dll`

---

## 코드 서명 (필수)

### 왜 필요한가?
- **서명 없이 배포하면** Windows SmartScreen이 설치를 차단
- 사내 보안 정책에서 미서명 실행 파일 차단 가능

### 설정 방법

#### Option A: 자체 서명 인증서 (사내 배포용)
```powershell
# 1. 인증서 생성
$cert = New-SelfSignedCertificate -Subject "CN=MyCompany Code Signing" `
  -Type CodeSigningCert -CertStoreLocation Cert:\CurrentUser\My

# 2. Thumbprint 확인
$cert.Thumbprint

# 3. 인증서를 신뢰할 수 있는 루트에 추가 (GPO로 배포 권장)
Export-Certificate -Cert $cert -FilePath "MyCompany-CodeSigning.cer"
```

#### Option B: 공인 인증서 (외부 배포용)
- DigiCert, Sectigo 등에서 코드 서명 인증서 구매

### tauri.conf.json 설정
```json
{
  "bundle": {
    "windows": {
      "wix": {
        "language": "ko-KR"
      },
      "certificateThumbprint": "YOUR_THUMBPRINT_HERE",
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com"
    }
  }
}
```

### 환경변수로 설정 (CI/CD)
```bash
TAURI_SIGNING_PRIVATE_KEY=<key>
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<password>
```

---

## 업데이트 배포

현재 자동 업데이트는 비활성화 상태 (사내 네트워크 외부 통신 차단).

### 권장 방법
1. **공유 네트워크 드라이브**: MSI를 사내 공유 폴더에 배치
2. **SCCM/Intune**: 기업 소프트웨어 배포 도구 사용
3. **수동 배포**: MSI 파일 직접 배포

---

## 앱 데이터 위치
- DB/인덱스: `%APPDATA%/com.anything.app/`
- 로그: `%APPDATA%/com.anything.app/logs/`
- 크래시 로그: `%APPDATA%/com.anything.app/crash.log`
- 모델: `%APPDATA%/com.anything.app/models/`

### 완전 제거 시
MSI 제거 후 `%APPDATA%/com.anything.app/` 폴더 수동 삭제
