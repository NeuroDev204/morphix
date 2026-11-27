// Chạy trong GNOME Shell (extension.js của bản chép DING). Khi người dùng bật lại DING gốc (ví dụ Ubuntu Settings →
// Desktop icons) thì bản chép Morphix tự nhường: hai bản không bao giờ cùng bật (hai tiến trình vẽ desktop, trùng tên D-Bus).
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const FORK = 'morphix-desktop@morphix';
const DING = 'ding@rastersoft.com';

let settings = null;
let changedId = 0;
let idleId = 0;

function yieldToDing() {
    const enabled = settings.get_strv('enabled-extensions');
    if (!enabled.includes(DING) || !enabled.includes(FORK))
        return;
    const disabled = settings.get_strv('disabled-extensions').filter(uuid => uuid !== FORK);
    settings.set_strv('enabled-extensions', enabled.filter(uuid => uuid !== FORK));
    settings.set_strv('disabled-extensions', [...disabled, FORK]);
}

export function start() {
    stop();
    settings = new Gio.Settings({schema_id: 'org.gnome.shell'});
    changedId = settings.connect('changed::enabled-extensions', () => {
        // WHY idle: không đổi gsettings ngay trong tín hiệu thay đổi của chính nó.
        if (idleId)
            return;
        idleId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            idleId = 0;
            if (settings)
                yieldToDing();
            return GLib.SOURCE_REMOVE;
        });
    });
}

export function stop() {
    if (idleId) {
        GLib.source_remove(idleId);
        idleId = 0;
    }
    if (settings && changedId)
        settings.disconnect(changedId);
    changedId = 0;
    settings = null;
}
