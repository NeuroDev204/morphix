#!/usr/bin/env bash
# Kiểm tra binary morphix-gui đã nhúng UI đã build (ui/dist) thay vì nạp dev server.
# WHY: thiếu feature custom-protocol thì Tauri nạp devUrl localhost:5173 và không nhúng asset, nên
# cửa sổ báo "Could not connect to localhost". Hai điều kiện dưới đây đều phải đúng:
#   1. có tên asset đã băm của bundle (assets/index-*.js) và khay.html: chứng minh ui/dist được nhúng;
#   2. không còn URL localhost:5173 nào ngoài các đường dẫn tương đối (icons/, crates/): Tauri codegen
#      vẫn ghi chúng vào binary ngay cả khi release (đã thấy trong bản Linux và Windows), không phải đích điều hướng.
# Usage: scripts/check-gui-embeds-ui.sh <đường dẫn binary>
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <binary>" >&2
  exit 2
fi
bin="$1"
[ -f "$bin" ] || { echo "không thấy binary: $bin" >&2; exit 2; }

# WHY: strings không có thì không kiểm tra được, nên báo lỗi thay vì coi như đạt.
command -v strings >/dev/null || { echo "cần lệnh strings (binutils)" >&2; exit 2; }

strs="$(strings -n 6 "$bin")"
embedded="$(grep -c -E 'assets/index-[A-Za-z0-9_-]+\.js' <<<"$strs" || true)"
khay="$(grep -c -F 'khay.html' <<<"$strs" || true)"
dev_urls="$(grep -o -E 'http://localhost:5173[A-Za-z0-9_./@-]*' <<<"$strs" | grep -v -E '^http://localhost:5173/(icons|crates)/' | wc -l || true)"

status=0
if [ "$embedded" -eq 0 ] || [ "$khay" -eq 0 ]; then
  echo "FAIL: $bin không nhúng ui/dist (assets/index-*.js: $embedded, khay.html: $khay)" >&2
  status=1
fi
if [ "$dev_urls" -ne 0 ]; then
  echo "FAIL: $bin còn $dev_urls URL http://localhost:5173 ngoài đường dẫn icon" >&2
  status=1
fi
[ "$status" -eq 0 ] && echo "OK: $bin nhúng ui/dist (assets/index-*.js: $embedded, khay.html: $khay), không có URL dev"
exit "$status"
