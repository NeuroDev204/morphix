from __future__ import annotations

import importlib
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


class FakeMenuItem:
    def __init__(self, *, name: str, label: str) -> None:
        self.name: str = name
        self.label: str = label
        self.callback: object | None = None
        self.user_data: tuple[object, ...] = ()

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


class NautilusMenuTests(unittest.TestCase):
    def setUp(self) -> None:
        FakeSubprocess.calls.clear()
        FakeSubprocess.error = None
        FakeAlertDialog.instances.clear()

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


if __name__ == "__main__":
    unittest.main()
