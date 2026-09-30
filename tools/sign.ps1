# Signs one file (the app or its installer) for the release workflow, which calls it through
# Tauri's signCommand. It uses the certificate the workflow wrote to KEYLUME_SIGNING_PFX, with a
# timestamp so the signature outlives the certificate. See docs/RELEASING.md for other ways to sign.
param([Parameter(Mandatory = $true)][string] $File)
$ErrorActionPreference = "Stop"
$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found (Windows SDK)" }
& $signtool.FullName sign /f $env:KEYLUME_SIGNING_PFX /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd sha256 /tr http://timestamp.digicert.com /td sha256 $File
if ($LASTEXITCODE -ne 0) { throw "signtool failed for $File" }
