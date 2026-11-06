# Chạy khi cài và khi gỡ, với quyền của người dùng hiện tại (không phải admin).
# Cài: đăng ký gói sparse trỏ tới thư mục cài để Explorer thấy menu Morphix. Gỡ: bỏ đăng ký.
param(
    [string] $AppDir,
    [switch] $Remove
)
$ErrorActionPreference = "Stop"
Get-AppxPackage -Name "Morphix.ContextMenu" -ErrorAction SilentlyContinue | Remove-AppxPackage
if (-not $Remove) {
    Add-AppxPackage -Path (Join-Path $AppDir "morphix-shell.msix") -ExternalLocation $AppDir
}
