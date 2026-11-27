// Menu Morphix cho bản chép DING (chạy trong tiến trình ding.js, legacy imports, Gtk3).
// Cùng hành vi với integration/nautilus/morphix_menu.py; @MORPHIX@ được `morphix install-integration` thay bằng đường dẫn binary.
/* exported addFileItems, addBackgroundItem */

var MORPHIX = '@MORPHIX@';
var LIST_TIMEOUT_SECONDS = 0.3;
var MAX_FILES = 100;
var TOO_MANY_LABEL = 'Chọn quá nhiều file (tối đa 100)';

var commandForSelection = function(uris, allRegularLocal) {
    if (uris.length === 0)
        return [MORPHIX, 'tray'];
    if (allRegularLocal)
        return [MORPHIX, 'menu', '--uris', ...uris];
    return [MORPHIX, 'tray', '--unsupported-selection'];
};

var listCommand = function(uris) {
    return [MORPHIX, '--json', 'menu', '--list', '--uris', ...uris];
};

var actionCommand = function(id, uris) {
    return [MORPHIX, 'menu', '--action', id, '--uris', ...uris];
};

// null khi JSON hỏng hoặc thiếu khoá, để menu lùi về mục mở Morphix.
var parseListing = function(stdout) {
    let data;
    try {
        data = JSON.parse(stdout);
    } catch (error) {
        return null;
    }
    if (!data || typeof data.too_many !== 'boolean' || !Array.isArray(data.items))
        return null;
    let items = [];
    for (let item of data.items) {
        if (!item || typeof item.id !== 'string' || typeof item.label !== 'string')
            return null;
        items.push({id: item.id, label: item.label, isTool: item.is_tool === true});
    }
    return {tooMany: data.too_many, items};
};

var spawn = function(argv) {
    const Gio = imports.gi.Gio;
    try {
        Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE);
    } catch (error) {
        log(`Morphix: không mở được: ${error}`);
    }
};

var list = function(uris) {
    const Gio = imports.gi.Gio;
    try {
        let proc = Gio.Subprocess.new(
            ['timeout', String(LIST_TIMEOUT_SECONDS), ...listCommand(uris)],
            Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        let [, stdout] = proc.communicate_utf8(null, null);
        return proc.get_successful() ? parseListing(stdout) : null;
    } catch (error) {
        log(`Morphix: không lấy được danh sách việc: ${error}`);
        return null;
    }
};

var isRegularLocal = function(item) {
    const Gio = imports.gi.Gio;
    try {
        return !item.isSpecial && !item.isDirectory && item.file.is_native() &&
            item.file.query_file_type(Gio.FileQueryInfoFlags.NONE, null) === Gio.FileType.REGULAR;
    } catch (error) {
        return false;
    }
};

var newItem = function(label, argv) {
    const Gtk = imports.gi.Gtk;
    let item = new Gtk.MenuItem({label});
    if (argv)
        item.connect('activate', () => spawn(argv));
    return item;
};

var addFileItems = function(fileItemMenu, fileItem) {
    const Gtk = imports.gi.Gtk;
    let selection = fileItemMenu._desktopManager.getCurrentSelection(false);
    let fileItems = selection && selection.length > 0 ? selection : [fileItem];
    let uris = fileItems.map(f => f.file.get_uri());
    let allRegular = fileItems.every(isRegularLocal);
    let top = new Gtk.MenuItem({label: 'Morphix'});
    if (!allRegular) {
        top = newItem('Morphix', commandForSelection(uris, false));
    } else if (uris.length > MAX_FILES) {
        let sub = new Gtk.Menu();
        let disabled = newItem(TOO_MANY_LABEL, null);
        disabled.set_sensitive(false);
        sub.add(disabled);
        top.set_submenu(sub);
    } else {
        let listing = list(uris);
        if (listing === null) {
            top = newItem('Morphix', commandForSelection(uris, true));
        } else if (listing.tooMany) {
            let sub = new Gtk.Menu();
            let disabled = newItem(TOO_MANY_LABEL, null);
            disabled.set_sensitive(false);
            sub.add(disabled);
            top.set_submenu(sub);
        } else if (listing.items.length === 0) {
            return;
        } else {
            let sub = new Gtk.Menu();
            for (let action of listing.items)
                sub.add(newItem(action.label, actionCommand(action.id, uris)));
            top.set_submenu(sub);
        }
    }
    fileItemMenu._menu.add(top);
    fileItemMenu._addSeparator();
};

var addBackgroundItem = function(menu) {
    menu.add(newItem('Morphix', commandForSelection([], true)));
};
