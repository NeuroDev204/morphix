import json
import logging
import subprocess

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
# WHY hạn chờ ngắn: danh sách việc được hỏi ngay lúc Nautilus dựng menu chuột phải, nên chờ lâu là làm Files đứng.
LIST_TIMEOUT_SECONDS: float = 0.3
# Khớp giới hạn của menu Explorer: không đọc nội dung quá số file này.
MAX_FILES: int = 100
TOO_MANY_LABEL: str = "Chọn quá nhiều file (tối đa 100)"


def command_for_selection(uris: list[str], all_regular_local: bool) -> list[str]:
    if not uris:
        return [EXECUTABLE, "tray"]
    if all_regular_local:
        return [EXECUTABLE, "menu", "--uris", *uris]
    return [EXECUTABLE, "tray", "--unsupported-selection"]


def list_command(uris: list[str]) -> list[str]:
    return [EXECUTABLE, "--json", "menu", "--list", "--uris", *uris]


def action_command(action_id: str, uris: list[str]) -> list[str]:
    return [EXECUTABLE, "menu", "--action", action_id, "--uris", *uris]


def parse_listing(stdout: str | None) -> tuple[bool, list[dict]] | None:
    """Đọc JSON của `menu --list`; `None` khi hỏng hoặc thiếu khoá, để menu lùi về mục mở Khay."""
    try:
        data: object = json.loads(stdout)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return None
    if not isinstance(data, dict):
        return None
    too_many: object = data.get("too_many")
    items: object = data.get("items")
    if not isinstance(too_many, bool) or not isinstance(items, list):
        return None
    parsed: list[dict] = []
    for item in items:
        if (
            not isinstance(item, dict)
            or not isinstance(item.get("id"), str)
            or not isinstance(item.get("label"), str)
        ):
            return None
        parsed.append(
            {"id": item["id"], "label": item["label"], "is_tool": bool(item.get("is_tool", False))}
        )
    return too_many, parsed


class MorphixMenuProvider(GObject.GObject, Nautilus.MenuProvider):
    # Kết quả hỏi danh sách việc của lựa chọn gần nhất, theo tuple URI. WHY nhớ cả kết quả hỏng: Nautilus gọi lại
    # `get_file_items` nhiều lần cho cùng một lựa chọn, nên không được chờ hết hạn lần nữa.
    _cache: tuple[tuple[str, ...], tuple[bool, list[dict]] | None] | None = None

    def get_file_items(self, files: list[Nautilus.FileInfo]) -> list[Nautilus.MenuItem]:
        uris: list[str] = [file_info.get_uri() for file_info in files]
        all_regular_local: bool = bool(files) and all(
            file_info.get_uri_scheme() == "file"
            and file_info.get_file_type() == Gio.FileType.REGULAR
            for file_info in files
        )
        if not all_regular_local:
            return [self._menu_item("file", command_for_selection(uris, all_regular_local))]
        if len(files) > MAX_FILES:
            return [self._submenu_item([self._disabled_item(TOO_MANY_LABEL)])]
        listing = self._listing(uris)
        if listing is None:
            return [self._menu_item("file", command_for_selection(uris, True))]
        too_many, actions = listing
        if too_many:
            return [self._submenu_item([self._disabled_item(TOO_MANY_LABEL)])]
        if not actions:
            return []
        return [
            self._submenu_item(
                [
                    self._action_item(action["id"], action["label"], uris)
                    for action in actions
                ]
            )
        ]

    def get_background_items(self, current_folder: Nautilus.FileInfo) -> list[Nautilus.MenuItem]:
        del current_folder
        return [self._menu_item("background", command_for_selection([], True))]

    def _listing(self, uris: list[str]) -> tuple[bool, list[dict]] | None:
        key: tuple[str, ...] = tuple(uris)
        if self._cache is not None and self._cache[0] == key:
            return self._cache[1]
        listing: tuple[bool, list[dict]] | None = None
        try:
            result = subprocess.run(
                list_command(uris),
                capture_output=True,
                text=True,
                timeout=LIST_TIMEOUT_SECONDS,
                check=False,
            )
            if result.returncode == 0:
                listing = parse_listing(result.stdout)
        except (subprocess.TimeoutExpired, OSError) as error:
            LOG.warning("không lấy được danh sách việc của Morphix: %s", error)
        self._cache = (key, listing)
        return listing

    def _submenu_item(self, children: list[Nautilus.MenuItem]) -> Nautilus.MenuItem:
        item: Nautilus.MenuItem = Nautilus.MenuItem(name="MorphixMenuProvider::file", label="Morphix")
        submenu: Nautilus.Menu = Nautilus.Menu()
        for child in children:
            submenu.append_item(child)
        item.set_submenu(submenu)
        return item

    def _disabled_item(self, label: str) -> Nautilus.MenuItem:
        return Nautilus.MenuItem(
            name="MorphixMenuProvider::too-many", label=label, sensitive=False
        )

    def _action_item(self, action_id: str, label: str, uris: list[str]) -> Nautilus.MenuItem:
        item: Nautilus.MenuItem = Nautilus.MenuItem(
            name=f"MorphixMenuProvider::action::{action_id}", label=label
        )
        item.connect("activate", self._launch, action_command(action_id, uris))
        return item

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
