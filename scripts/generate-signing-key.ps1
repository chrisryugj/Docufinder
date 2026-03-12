$keyPath = "$env:USERPROFILE\.tauri\anything.key"
$keyDir = Split-Path $keyPath -Parent
if (-not (Test-Path $keyDir)) { New-Item -ItemType Directory -Path $keyDir -Force | Out-Null }

$workDir = $PSScriptRoot | Split-Path -Parent

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = "cmd.exe"
$psi.Arguments = "/c npx tauri signer generate -w `"$keyPath`" --force"
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$psi.WorkingDirectory = $workDir

$p = [System.Diagnostics.Process]::Start($psi)
Start-Sleep -Milliseconds 3000
$p.StandardInput.WriteLine("")
Start-Sleep -Milliseconds 500
$p.StandardInput.WriteLine("")
$p.WaitForExit(15000)

$stdout = $p.StandardOutput.ReadToEnd()
$stderr = $p.StandardError.ReadToEnd()

Write-Host "STDOUT:"
Write-Host $stdout
if ($stderr) {
    Write-Host "STDERR:"
    Write-Host $stderr
}

$pubKeyPath = "$keyPath.pub"
if (Test-Path $pubKeyPath) {
    Write-Host "`nPUBLIC_KEY:"
    Get-Content $pubKeyPath
} else {
    Write-Host "`nPublic key file not found at: $pubKeyPath"
}
