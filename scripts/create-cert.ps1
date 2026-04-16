# Self-signed code signing certificate generation script
# For Docufinder MSI signing

$certName = "Docufinder Code Signing"
$certStore = "Cert:\CurrentUser\My"

# Check for existing certificate
$existing = Get-ChildItem $certStore | Where-Object { $_.Subject -eq "CN=$certName" }
if ($existing) {
    Write-Host "Existing certificate found: $($existing.Thumbprint)"
    Write-Host "Expires: $($existing.NotAfter)"
    $existing | Select-Object Thumbprint, Subject, NotAfter | Format-Table
    exit 0
}

# Generate self-signed code signing certificate (valid for 2 years)
$cert = New-SelfSignedCertificate `
    -Subject "CN=$certName" `
    -Type CodeSigningCert `
    -CertStoreLocation $certStore `
    -NotAfter (Get-Date).AddYears(2) `
    -KeyUsage DigitalSignature `
    -FriendlyName "Docufinder MSI Signing"

$thumbprint = $cert.Thumbprint
Write-Host ""
Write-Host "=== Certificate Created ==="
Write-Host "Thumbprint: $thumbprint"
Write-Host "Subject: $($cert.Subject)"
Write-Host "Expires: $($cert.NotAfter)"
Write-Host ""

# Export .cer file (for internal distribution - public key only)
$cerPath = Join-Path $PSScriptRoot "docufinder-codesign.cer"
Export-Certificate -Cert $cert -FilePath $cerPath | Out-Null
Write-Host ".cer exported: $cerPath"
Write-Host ""
Write-Host "=== Installation on Target PCs ==="
Write-Host "1. Copy docufinder-codesign.cer to the target PC"
Write-Host "2. Double-click > Install Certificate > Local Machine > Trusted Root Certification Authorities"
Write-Host "   Or deploy via GPO"
Write-Host ""
Write-Host "=== tauri.conf.json Configuration ==="
Write-Host "certificateThumbprint: `"$thumbprint`""
