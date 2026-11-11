from __future__ import annotations

import importlib
import json
import subprocess
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch


NAUTILUS_DIR: Path = Path(__file__).resolve().parents[1] / "nautilus"
sys.path.insert(0, str(NAUTILUS_DIR))


class FakeMenuProvider:
    pass


class FakeGObject:
    pass


class FakeMenu:
    def __init__(self) -> None:
        self.items: list[FakeMenuItem] = []

    def append_item(self, item: FakeMenuItem) -> None:
        self.items.append(item)

    def get_items(self) -> list[FakeMenuItem]:
        return list(self.items)


class FakeMenuItem:
    def __init__(self, *, name: str, label: str, sensitive: bool = True) -> None:
        self.name: str = name
        self.label: str = label
        self.sensitive: bool = sensitive
        self.submenu: FakeMenu | None = None
        self.callback: object | None = None
        self.user_data: tuple[object, ...] = ()

    def set_submenu(self, submenu: FakeMenu) -> None:
        self.submenu = submenu

    def connect(self, _signal: str, callback: object, *user_data: object) -> None:
        self.callback = callback
        self.user_data = user_data

    def activate(self) -> None:
        assert callable(self.callback)
        self.callback(self, *self.user_data)


class FakeFileInfo:
    def __init__(self, uri: str, file_type: str = "REGULAR") -> None:
        self.uri: str = uri
        self.file_type: str = file_type

    def get_uri(self) -> str:
        return self.uri

    def get_uri_scheme(self) -> str:
        return self.uri.split(":", 1)[0]

    def get_file_type(self) -> str:
        return self.file_type


class FakeSubprocess:
    calls: list[tuple[list[str], object]] = []
    error: Exception | None = None

    @classmethod
    def new(cls, argv: list[str], flags: object) -> object:
        cls.calls.append((argv, flags))
        if cls.error is not None:
            raise cls.error
        return object()


class FakeAlertDialog:
    instances: list[FakeAlertDialog] = []

    def __init__(self, message: str) -> None:
        self.message: str = message
        self.shown: bool = False
        self.instances.append(self)

    @classmethod
    def new(cls, message: str) -> FakeAlertDialog:
        return cls(message)

    def show(self, parent: object | None) -> None:
        assert parent is None
        self.shown = True


def install_gi_stubs(nautilus_versions: tuple[str, ...] = ("4.0", "4.1")) -> None:
    nautilus: types.SimpleNamespace = types.SimpleNamespace(
        FileInfo=FakeFileInfo,
        Menu=FakeMenu,
        MenuItem=FakeMenuItem,
        MenuProvider=FakeMenuProvider,
    )
    gio: types.SimpleNamespace = types.SimpleNamespace(
        FileType=types.SimpleNamespace(REGULAR="REGULAR"),
        Subprocess=FakeSubprocess,
        SubprocessFlags=types.SimpleNamespace(NONE=object()),
    )
    gtk: types.SimpleNamespace = types.SimpleNamespace(AlertDialog=FakeAlertDialog)
    repository: types.ModuleType = types.ModuleType("gi.repository")
    repository.Gio = gio
    repository.GObject = types.SimpleNamespace(GObject=FakeGObject)
    repository.Gtk = gtk
    repository.Nautilus = nautilus
    gi: types.ModuleType = types.ModuleType("gi")
    gi.repository = repository
    gi.required_versions = []

    def require_version(namespace: str, version: str) -> None:
        if namespace == "Nautilus" and version not in nautilus_versions:
            raise ValueError(f"Namespace Nautilus not available for version {version}")
        gi.required_versions.append((namespace, version))

    gi.require_version = require_version
    sys.modules["gi"] = gi
    sys.modules["gi.repository"] = repository


def load_menu_module(nautilus_versions: tuple[str, ...] = ("4.0", "4.1")) -> types.ModuleType:
    previous: dict[str, types.ModuleType | None] = {
        name: sys.modules.get(name) for name in ("gi", "gi.repository", "morphix_menu")
    }
    install_gi_stubs(nautilus_versions)
    sys.modules.pop("morphix_menu", None)
    try:
        return importlib.import_module("morphix_menu")
    finally:
        for name, module in previous.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module


menu: types.ModuleType = load_menu_module()


LISTING_PNG: str = json.dumps(
    {
        "too_many": False,
        "items": [
            {"id": "merge", "label": "PDF", "is_tool": False},
            {"id": "convert:webp", "label": "WebP", "is_tool": False},
            {"id": "compress", "label": "Nén ảnh", "is_tool": True},
        ],
    }
)


def completed(stdout: str, returncode: int = 0) -> subprocess.CompletedProcess[str]:
    return subprocess.CompletedProcess(args=[], returncode=returncode, stdout=stdout, stderr="")


class NautilusMenuTests(unittest.TestCase):
    def setUp(self) -> None:
        FakeSubprocess.calls.clear()
        FakeSubprocess.error = None
        FakeAlertDialog.instances.clear()
        # WHY mặc định không có morphix: các test cũ kiểm đường lùi (một mục mở Khay), và máy chạy test
        # có thể đã cài /usr/bin/morphix thật.
        patcher = patch.object(menu.subprocess, "run", side_effect=FileNotFoundError)
        self.run = patcher.start()
        self.addCleanup(patcher.stop)

    def test_selection_preserves_argv(self) -> None:
        self.assertEqual(
            menu.command_for_selection(["file:///tmp/a%20b.png"], True),
            ["/usr/bin/morphix", "menu", "--uris", "file:///tmp/a%20b.png"],
        )

    def test_provider_requires_gtk4_and_newest_nautilus_api(self) -> None:
        # WHY 4.1 trước: Nautilus 47+ (Fedora, Arch) nạp sẵn API 4.1, yêu cầu 4.0 thì extension không nạp được.
        self.assertEqual(
            menu.gi.required_versions,
            [("Gtk", "4.0"), ("Nautilus", "4.1")],
        )

    def test_falls_back_to_nautilus_4_0_on_nautilus_46(self) -> None:
        old = load_menu_module(nautilus_versions=("4.0",))
        self.assertEqual(old.gi.required_versions, [("Gtk", "4.0"), ("Nautilus", "4.0")])

    def test_background_opens_app(self) -> None:
        self.assertEqual(
            menu.command_for_selection([], True),
            ["/usr/bin/morphix", "tray"],
        )

    def test_mixed_selection_reports_unsupported(self) -> None:
        self.assertEqual(
            menu.command_for_selection(["file:///tmp/a.png", "smb://host/b.png"], False),
            ["/usr/bin/morphix", "tray", "--unsupported-selection"],
        )

    def test_provider_adds_one_morphix_item_for_file_and_background(self) -> None:
        provider: object = menu.MorphixMenuProvider()
        file_items: list[FakeMenuItem] = provider.get_file_items(
            [FakeFileInfo("file:///tmp/a.png")]
        )
        background_items: list[FakeMenuItem] = provider.get_background_items(
            FakeFileInfo("file:///tmp")
        )
        self.assertEqual([item.label for item in file_items], ["Morphix"])
        self.assertEqual([item.label for item in background_items], ["Morphix"])

    def test_shell_characters_remain_one_uri_argument(self) -> None:
        uri: str = "file:///tmp/%E1%BA%A2nh%0A%22%24%28%29%3B.png"
        provider: object = menu.MorphixMenuProvider()
        item: FakeMenuItem = provider.get_file_items([FakeFileInfo(uri)])[0]
        item.activate()
        self.assertEqual(
            FakeSubprocess.calls[0][0],
            ["/usr/bin/morphix", "menu", "--uris", uri],
        )
        self.assertEqual(FakeAlertDialog.instances, [])

    def test_nonregular_or_remote_selection_uses_unsupported_route(self) -> None:
        provider: object = menu.MorphixMenuProvider()
        for file_info in [
            FakeFileInfo("file:///tmp/folder", "DIRECTORY"),
            FakeFileInfo("smb://host/share/a.png"),
        ]:
            with self.subTest(uri=file_info.get_uri()):
                FakeSubprocess.calls.clear()
                items: list[FakeMenuItem] = provider.get_file_items([file_info])
                items[0].activate()
                self.assertEqual(
                    FakeSubprocess.calls[0][0],
                    ["/usr/bin/morphix", "tray", "--unsupported-selection"],
                )

    def test_spawn_error_is_logged_and_shown_in_a_fixed_alert(self) -> None:
        FakeSubprocess.error = OSError("spawn details")
        provider: object = menu.MorphixMenuProvider()
        item: FakeMenuItem = provider.get_file_items(
            [FakeFileInfo("file:///tmp/a.png")]
        )[0]
        with self.assertLogs("morphix.nautilus", level="ERROR") as captured:
            item.activate()
        self.assertEqual(len(FakeAlertDialog.instances), 1)
        self.assertTrue(FakeAlertDialog.instances[0].shown)
        self.assertIsNotNone(captured.records[0].exc_info)
        self.assertEqual(
            FakeAlertDialog.instances[0].message,
            "Không thể mở Morphix. Hãy kiểm tra cài đặt rồi thử lại.",
        )

    # ---- submenu động: một mục con cho mỗi việc áp dụng được ----

    def test_list_command(self) -> None:
        self.assertEqual(
            menu.list_command(["file:///tmp/a%20b.png"]),
            ["/usr/bin/morphix", "--json", "menu", "--list", "--uris", "file:///tmp/a%20b.png"],
        )

    def test_action_command(self) -> None:
        self.assertEqual(
            menu.action_command("convert:webp", ["file:///tmp/a.png", "file:///tmp/b.png"]),
            [
                "/usr/bin/morphix",
                "menu",
                "--action",
                "convert:webp",
                "--uris",
                "file:///tmp/a.png",
                "file:///tmp/b.png",
            ],
        )

    def test_parse_listing_valid(self) -> None:
        too_many, items = menu.parse_listing(LISTING_PNG)
        self.assertFalse(too_many)
        self.assertEqual(
            [(item["id"], item["label"], item["is_tool"]) for item in items],
            [("merge", "PDF", False), ("convert:webp", "WebP", False), ("compress", "Nén ảnh", True)],
        )

    def test_parse_listing_rejects_garbage(self) -> None:
        for text in ["", "not json", "{}", '{"too_many": false}', '{"too_many": 1, "items": []}',
                     '{"too_many": false, "items": [{"id": 3, "label": "x"}]}', "[]", None]:
            with self.subTest(text=text):
                self.assertIsNone(menu.parse_listing(text))

    def test_valid_listing_becomes_one_submenu_with_an_item_per_action(self) -> None:
        self.run.side_effect = None
        self.run.return_value = completed(LISTING_PNG)
        provider: object = menu.MorphixMenuProvider()
        items: list[FakeMenuItem] = provider.get_file_items([FakeFileInfo("file:///tmp/a.png")])
        self.assertEqual([item.label for item in items], ["Morphix"])
        sub = items[0].submenu.get_items()
        self.assertEqual([item.label for item in sub], ["PDF", "WebP", "Nén ảnh"])
        sub[1].activate()
        self.assertEqual(
            FakeSubprocess.calls[0][0],
            ["/usr/bin/morphix", "menu", "--action", "convert:webp", "--uris", "file:///tmp/a.png"],
        )

    def test_empty_listing_adds_no_menu_item(self) -> None:
        self.run.side_effect = None
        self.run.return_value = completed('{"too_many": false, "items": []}')
        provider: object = menu.MorphixMenuProvider()
        self.assertEqual(provider.get_file_items([FakeFileInfo("file:///tmp/a.rar")]), [])

    def test_too_many_files_do_not_call_cli(self) -> None:
        files = [FakeFileInfo(f"file:///tmp/{n}.png") for n in range(menu.MAX_FILES + 1)]
        provider: object = menu.MorphixMenuProvider()
        items: list[FakeMenuItem] = provider.get_file_items(files)
        self.run.assert_not_called()
        sub = items[0].submenu.get_items()
        self.assertEqual([item.label for item in sub], [menu.TOO_MANY_LABEL])
        self.assertFalse(sub[0].sensitive)
        self.assertEqual(menu.TOO_MANY_LABEL, "Chọn quá nhiều file (tối đa 100)")

    def test_timeout_falls_back_to_tray_item(self) -> None:
        self.run.side_effect = subprocess.TimeoutExpired(cmd="morphix", timeout=0.3)
        provider: object = menu.MorphixMenuProvider()
        items: list[FakeMenuItem] = provider.get_file_items([FakeFileInfo("file:///tmp/a.png")])
        self.assertEqual([item.label for item in items], ["Morphix"])
        self.assertIsNone(items[0].submenu)
        items[0].activate()
        self.assertEqual(
            FakeSubprocess.calls[0][0], ["/usr/bin/morphix", "menu", "--uris", "file:///tmp/a.png"]
        )
        self.assertEqual(self.run.call_args.kwargs["timeout"], menu.LIST_TIMEOUT_SECONDS)
        self.assertEqual(menu.LIST_TIMEOUT_SECONDS, 0.3)

    def test_failed_or_garbled_cli_falls_back_to_tray_item(self) -> None:
        provider: object = menu.MorphixMenuProvider()
        for result in [completed(LISTING_PNG, returncode=1), completed("không phải JSON")]:
            with self.subTest(stdout=result.stdout, code=result.returncode):
                self.run.side_effect = None
                self.run.return_value = result
                provider._cache = None
                items = provider.get_file_items([FakeFileInfo("file:///tmp/a.png")])
                self.assertEqual([item.label for item in items], ["Morphix"])
                self.assertIsNone(items[0].submenu)

    def test_second_call_with_same_selection_uses_cache(self) -> None:
        self.run.side_effect = None
        self.run.return_value = completed(LISTING_PNG)
        provider: object = menu.MorphixMenuProvider()
        selection = [FakeFileInfo("file:///tmp/a.png")]
        provider.get_file_items(selection)
        provider.get_file_items(selection)
        self.assertEqual(self.run.call_count, 1)
        provider.get_file_items([FakeFileInfo("file:///tmp/b.png")])
        self.assertEqual(self.run.call_count, 2)

    def test_a_failed_listing_is_cached_too_so_nautilus_is_not_stalled_twice(self) -> None:
        self.run.side_effect = subprocess.TimeoutExpired(cmd="morphix", timeout=0.3)
        provider: object = menu.MorphixMenuProvider()
        selection = [FakeFileInfo("file:///tmp/a.png")]
        provider.get_file_items(selection)
        provider.get_file_items(selection)
        self.assertEqual(self.run.call_count, 1)


if __name__ == "__main__":
    unittest.main()
