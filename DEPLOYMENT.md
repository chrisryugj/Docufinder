# Anything 사내 배포 가이드

## 시스템 요구사항 (설치 대상 PC)

| 항목 | 요구사항 | 비고 |
|------|---------|------|
| **OS** | Windows 10 21H2 이상 / Windows 11 | WebView2 런타임 기본 탑재 |
| **WebView2** | 자동 포함 (21H2+) | 구형 Windows 10은 [수동 설치](https://developer.microsoft.com/ko-kr/microsoft-edge/webview2/) 필요 |
| **VC++ Runtime** | 2015-2022 x64 | MSI 설치 시 자동 포함 |
| **디스크** | 최소 500MB 여유 | 앱 ~300MB + DB/인덱스 가변 |
| **RAM** | 4GB 이상 권장 | ONNX 모델 로드 시 ~500MB 사용 |
| **인터넷** | 최초 실행 시 불필요 | 모델은 MSI에 번들됨. OTA 업데이트에만 필요 |

---

## 빌드 & 배포

### 1. 사전 요구사항 (빌드 PC)
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
결과물: `src-tauri/target/release/bundle/msi/Anything_1.0.0_x64_ko-KR.msi`

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
`tauri.conf.json`에 코드 서명 설정:
```json
{
  "bundle": {
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": "https://timestamp.digicert.com",
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

### 방식 A: OTA 자동 업데이트 (권장)

`tauri-plugin-updater`를 통한 GitHub Releases 기반 OTA 업데이트.
앱이 시작 3초 후 + 6시간 주기로 자동 체크하며, 사용자에게 배너로 알림.

#### 초기 설정 (1회)

**1. 서명 키 생성**
```powershell
npx tauri signer generate -w $env:USERPROFILE\.tauri\anything.key
```
- 비밀번호 입력 (빈 문자열도 가능)
- 생성 파일:
  - `~/.tauri/anything.key` — 비밀키 (절대 공유 금지)
  - `~/.tauri/anything.key.pub` — 공개키

> 이미 `scripts/generate-signing-key.cjs`로 자동 생성 가능:
> ```powershell
> node scripts/generate-signing-key.cjs
> ```

**2. GitHub Secrets 등록**

GitHub 레포 → Settings → Secrets and variables → Actions에 추가:

| Secret 이름 | 값 |
|-------------|-----|
| `TAURI_SIGNING_PRIVATE_KEY` | `~/.tauri/anything.key` 파일 내용 전체 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 키 생성 시 입력한 비밀번호 (빈 문자열이면 빈 값) |

**3. 공개키 확인**

`tauri.conf.json`의 `plugins.updater.pubkey`에 공개키가 등록되어 있는지 확인:
```json
{
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/chrisryugj/Docufinder/releases/latest/download/latest.json"
      ],
      "pubkey": "<~/.tauri/anything.key.pub 두 번째 줄>"
    }
  }
}
```

#### 릴리스 배포 절차

```powershell
# 1. 버전 번호 업데이트 (3곳 모두 동일하게)
#    - src-tauri/Cargo.toml      → version = "1.6.0"
#    - src-tauri/tauri.conf.json  → "version": "1.6.0"
#    - package.json               → "version": "1.6.0"

# 2. 커밋
git add -A
git commit -m "v1.6.0: 변경사항 요약"

# 3. 태그 생성 + 푸시
git tag v1.6.0
git push origin main --tags
```

태그 푸시 시 **GitHub Actions가 자동으로**:
1. 프론트엔드 빌드 (`pnpm build`)
2. ONNX 모델 다운로드
3. Rust 프로덕션 빌드 (`tauri build`)
4. MSI 서명 (서명 키 사용)
5. 업데이트 아티팩트 생성 (`.msi.zip` + `.msi.zip.sig` + `latest.json`)
6. GitHub Release (Draft) 생성

#### 릴리스 게시

1. GitHub → Releases → Draft 릴리스 확인
2. 릴리스 노트 작성 (변경사항 요약)
3. **Publish release** 클릭

게시 후 기존 사용자의 앱이 자동으로 업데이트 감지 → 배너 표시 → 원클릭 설치.

#### 업데이트 아티팩트 구성

| 파일 | 설명 |
|------|------|
| `Anything_x.y.z_x64_ko-KR.msi` | 일반 설치파일 (신규 설치용) |
| `Anything_x.y.z_x64_ko-KR.msi.zip` | 업데이트 번들 (OTA용) |
| `Anything_x.y.z_x64_ko-KR.msi.zip.sig` | 서명 파일 (무결성 검증) |
| `latest.json` | 업데이트 매니페스트 (버전, URL, 서명 정보) |

#### 사용자 경험

- 앱 시작 후 자동 업데이트 체크 (3초 후)
- 새 버전 발견 시 상단 배너: "새 버전 vX.Y.Z 사용 가능"
- "지금 설치" 클릭 → 다운로드 진행률 표시 → 자동 재시작
- "나중에" 클릭 → 세션 동안 숨김 (6시간 후 재알림)
- 네트워크 오류 시 조용히 스킵 (오프라인 환경 대응)

### 방식 B: 수동 배포

인터넷 차단 환경이나 추가 통제가 필요한 경우:

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
| OTA 업데이트 서명 | ✅ (Ed25519 + minisign 검증) |
