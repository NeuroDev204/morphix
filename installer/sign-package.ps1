# Dev side: đóng gói và ký gói sparse của menu Windows 11 bằng chứng chỉ đã tin cậy ở kho máy.
# Chạy trên máy phát hành (cần chứng chỉ có khoá riêng). Kết quả: installer\package\morphix-shell.msix
param([Parameter(Mandatory)] [string] $Thumbprint)
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
$sdk = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64"
$staging = Join-Path $here "package\staging"
$msix = Join-Path $here "package\morphix-shell.msix"
if (Test-Path $msix) { Remove-Item $msix }
& "$sdk\makeappx.exe" pack /d $staging /p $msix /nv /o | Out-Null
& "$sdk\signtool.exe" sign /fd SHA256 /sm /s My /sha1 $Thumbprint $msix | Out-Null
# Xuất chứng chỉ công khai cho trình cài đặt (không có khoá riêng).
$cert = Get-ChildItem Cert:\LocalMachine\My | Where-Object Thumbprint -eq $Thumbprint
Export-Certificate -Cert $cert -FilePath (Join-Path $here "morphix-dev.cer") | Out-Null
"signed: $msix"
