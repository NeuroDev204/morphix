import logging

import gi

gi.require_version("Gtk", "4.0")
# WHY thử 4.1 trước: Nautilus 47+ (Fedora, Arch...) nạp sẵn API 4.1 nên yêu cầu 4.0 sẽ lỗi;
# Nautilus 46 (Ubuntu 24.04) chỉ có 4.0. MenuProvider giống nhau ở cả hai bản.
try:
    gi.require_version("Nautilus", "4.1")
except ValueError:
    gi.require_version("Nautilus", "4.0")

from gi.repository import Gio, GObject, Gtk, Nautilus


LOG: logging.Logger = logging.getLogger("morphix.nautilus")
EXECUTABLE: str = "/usr/bin/morphix"
START_FAILED: str = "Không thể mở Morphix. Hãy kiểm tra cài đặt rồi thử lại."


def command_for_selection(uris: list[str], all_regular_local: bool) -> list[str]:
    if not uris:
        return [EXECUTABLE, "tray"]
    if all_regular_local:
        return [EXECUTABLE, "menu", "--uris", *uris]
    return [EXECUTABLE, "tray", "--unsupported-selection"]


class MorphixMenuProvider(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, files: list[Nautilus.FileInfo]) -> list[Nautilus.MenuItem]:
        uris: list[str] = [file_info.get_uri() for file_info in files]
        all_regular_local: bool = bool(files) and all(
            file_info.get_uri_scheme() == "file"
            and file_info.get_file_type() == Gio.FileType.REGULAR
            for file_info in files
        )
        return [self._menu_item("file", command_for_selection(uris, all_regular_local))]

    def get_background_items(self, current_folder: Nautilus.FileInfo) -> list[Nautilus.MenuItem]:
        del current_folder
        return [self._menu_item("background", command_for_selection([], True))]

    def _menu_item(self, context: str, argv: list[str]) -> Nautilus.MenuItem:
        item: Nautilus.MenuItem = Nautilus.MenuItem(
            name=f"MorphixMenuProvider::{context}",
            label="Morphix",
        )
        item.connect("activate", self._launch, argv)
        return item

    def _launch(self, _item: Nautilus.MenuItem, argv: list[str]) -> None:
        try:
            Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE)
        except Exception:
            LOG.exception("không khởi chạy được Morphix từ menu Nautilus")
            try:
                Gtk.AlertDialog.new(START_FAILED).show(None)
            except Exception:
                LOG.exception("không hiện được thông báo lỗi khởi chạy Morphix")
