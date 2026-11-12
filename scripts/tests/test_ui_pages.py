from __future__ import annotations

import unittest
from pathlib import Path

UI = Path(__file__).resolve().parents[2] / "crates" / "morphix-gui" / "ui"


class UiPagesTests(unittest.TestCase):
    def test_every_page_declares_an_icon_so_the_webview_never_requests_a_missing_favicon(self) -> None:
        # WHY: thiếu khai báo thì WebView xin /favicon.ico và Tauri ghi "ERROR asset not found: favicon.ico" mỗi lần mở.
        for page in ("khay.html", "settings.html"):
            self.assertIn('<link rel="icon" href="data:,">', (UI / page).read_text(encoding="utf-8"), page)


if __name__ == "__main__":
    unittest.main()
