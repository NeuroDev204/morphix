imports.gi.versions.Gdk = "3.0";
imports.gi.versions.Gtk = "3.0";

const Gio = imports.gi.Gio;
const morphixMenu = imports.fileItemMenu;

function assertEqual(actual, expected, description) {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) {
        throw new Error(`${description}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    }
}

function localFileItem(uri, fileType = Gio.FileType.REGULAR) {
    return {
        isSpecial: false,
        isDirectory: false,
        file: {
            is_native: () => true,
            query_file_type: () => fileType,
            get_uri: () => uri,
        },
    };
}

const executable = "/usr/bin/morphix";
const firstUri = "file:///tmp/a%20b%0A%27%24%28touch%20owned%29%3B.png";
const secondUri = "file:///tmp/%E9%9B%AA.png";

assertEqual(
    morphixMenu.morphixCommandForSelection([
        localFileItem(firstUri),
        localFileItem(secondUri),
    ]),
    [executable, "menu", "--uris", firstUri, secondUri],
    "multi-file selection keeps each URI in a separate argument"
);
assertEqual(
    morphixMenu.morphixCommandForSelection([]),
    [executable, "tray"],
    "background selection opens the app"
);
assertEqual(
    morphixMenu.morphixCommandForSelection([
        {...localFileItem(firstUri), isDirectory: true},
    ]),
    [executable, "tray", "--unsupported-selection"],
    "directory selection is reported as unsupported"
);
assertEqual(
    morphixMenu.morphixCommandForSelection([
        {
            ...localFileItem("smb://server/share/a.png"),
            file: {
                ...localFileItem("smb://server/share/a.png").file,
                is_native: () => false,
            },
        },
    ]),
    [executable, "tray", "--unsupported-selection"],
    "remote URI selection is reported as unsupported"
);

print("DING Morphix argv checks passed");
