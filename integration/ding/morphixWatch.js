// Chạy trong GNOME Shell (extension.js của bản chép DING). Khi người dùng bật lại DING gốc (ví dụ Ubuntu Settings →
// Desktop icons) thì bản chép Morphix tự nhường: hai bản không bao giờ cùng bật (hai tiến trình vẽ desktop, trùng tên D-Bus).
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const FORK = 'morphix-desktop@morphix';
const DING = 'ding@rastersoft.com';

let settings = null;
let changedIds = [];
let idleId = 0;
let before = null;

function read() {
    return {
        enabled: settings.get_strv('enabled-extensions'),
        disabled: settings.get_strv('disabled-extensions'),
    };
}

// WHY so với trạng thái trước chứ không chỉ nhìn danh sách: DING của Ubuntu là extension của phiên, bật lại nó có thể chỉ là
// gỡ nó khỏi disabled-extensions (không thêm vào enabled-extensions); còn ở bản khác thì là thêm vào enabled-extensions.
function dingWokeUp(now) {
    return (now.enabled.includes(DING) && !before.enabled.includes(DING)) ||
        (before.disabled.includes(DING) && !now.disabled.includes(DING));
}

function check() {
    const now = read();
    const woke = dingWokeUp(now);
    before = now;
    if (!woke || !now.enabled.includes(FORK))
        return;
    settings.set_strv('enabled-extensions', now.enabled.filter(uuid => uuid !== FORK));
    settings.set_strv('disabled-extensions', [...now.disabled.filter(uuid => uuid !== FORK), FORK]);
}

export function start() {
    stop();
    settings = new Gio.Settings({schema_id: 'org.gnome.shell'});
    before = read();
    for (const key of ['enabled-extensions', 'disabled-extensions']) {
        changedIds.push(settings.connect(`changed::${key}`, () => {
            // WHY idle: không đổi gsettings ngay trong tín hiệu thay đổi của chính nó.
            if (idleId)
                return;
            idleId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                idleId = 0;
                if (settings)
                    check();
                return GLib.SOURCE_REMOVE;
            });
        }));
    }
}

export function stop() {
    if (idleId) {
        GLib.source_remove(idleId);
        idleId = 0;
    }
    if (settings) {
        for (const id of changedIds)
            settings.disconnect(id);
    }
    changedIds = [];
    settings = null;
    before = null;
}
