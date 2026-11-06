# Đóng gói, ký và cài gói sparse của menu Windows 11 cho người dùng hiện tại.
# Dùng: powershell -File package\register.ps1 -Thumbprint <dấu vân tay chứng chỉ ký> -DllDir <thư mục chứa morphix_shell.dll>
# Chứng chỉ phải đã được tin cậy ở kho máy (LocalMachine\Root), cần một lần với quyền admin.
param(
    [Parameter(Mandatory)] [string] $Thumbprint,
    [Parameter(Mandatory)] [string] $DllDir
)
$ErrorActionPreference = "Stop"
$package = $PSScriptRoot
$staging = Join-Path $env:TEMP "morphix-shell-pkg"
$sdk = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64"

Get-AppxPackage -Name "Morphix.ContextMenu" -ErrorAction SilentlyContinue | Remove-AppxPackage
if (Test-Path $staging) { Remove-Item $staging -Recurse -Force }
New-Item -ItemType Directory -Path $staging | Out-Null
Copy-Item (Join-Path $package "AppxManifest.xml") $staging
Copy-Item (Join-Path $package "Assets") $staging -Recurse

$msix = Join-Path $staging "morphix-shell.msix"
& "$sdk\makeappx.exe" pack /d $staging /p $msix /nv /o | Out-Null
& "$sdk\signtool.exe" sign /fd SHA256 /sm /s My /sha1 $Thumbprint $msix | Out-Null
Add-AppxPackage -Path $msix -ExternalLocation (Resolve-Path $DllDir).Path
Get-AppxPackage -Name "Morphix.ContextMenu" | Select-Object Name, Version, Status | Format-List
