# 자체서명 코드서명 인증서 생성 스크립트
# Docufinder MSI 서명용

$certName = "Docufinder Code Signing"
$certStore = "Cert:\CurrentUser\My"

# 기존 인증서 확인
$existing = Get-ChildItem $certStore | Where-Object { $_.Subject -eq "CN=$certName" }
if ($existing) {
    Write-Host "기존 인증서 발견: $($existing.Thumbprint)"
    Write-Host "만료일: $($existing.NotAfter)"
    $existing | Select-Object Thumbprint, Subject, NotAfter | Format-Table
    exit 0
}

# 자체서명 코드서명 인증서 생성 (2년 유효)
$cert = New-SelfSignedCertificate `
    -Subject "CN=$certName" `
    -Type CodeSigningCert `
    -CertStoreLocation $certStore `
    -NotAfter (Get-Date).AddYears(2) `
    -KeyUsage DigitalSignature `
    -FriendlyName "Docufinder MSI Signing"

$thumbprint = $cert.Thumbprint
Write-Host ""
Write-Host "=== 인증서 생성 완료 ==="
Write-Host "Thumbprint: $thumbprint"
Write-Host "Subject: $($cert.Subject)"
Write-Host "만료일: $($cert.NotAfter)"
Write-Host ""

# .cer 파일 내보내기 (사내 배포용 - 공개키만)
$cerPath = Join-Path $PSScriptRoot "docufinder-codesign.cer"
Export-Certificate -Cert $cert -FilePath $cerPath | Out-Null
Write-Host ".cer 내보내기: $cerPath"
Write-Host ""
Write-Host "=== 사내 PC 설치 방법 ==="
Write-Host "1. docufinder-codesign.cer 파일을 대상 PC에 복사"
Write-Host "2. 더블클릭 > 인증서 설치 > 로컬 컴퓨터 > 신뢰할 수 있는 루트 인증 기관"
Write-Host "   또는 GPO로 일괄 배포"
Write-Host ""
Write-Host "=== tauri.conf.json 설정 ==="
Write-Host "certificateThumbprint: `"$thumbprint`""
