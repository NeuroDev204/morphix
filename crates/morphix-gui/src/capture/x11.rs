use super::{classify_window, CaptureError, Key, KeyInjector, Target};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{self, AtomEnum, ConnectionExt, KeyButMask},
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
    CURRENT_TIME, NONE,
};

pub struct X11Session {
    connection: RustConnection,
    root: u32,
}

fn failed(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::KeysFailed(error.to_string())
}

impl X11Session {
    pub fn connect() -> Result<Self, CaptureError> {
        let (connection, screen) = x11rb::connect(None).map_err(failed)?;
        let root = connection.setup().roots[screen].root;
        connection
            .xtest_get_version(2, 2)
            .map_err(failed)?
            .reply()
            .map_err(failed)?;
        Ok(Self { connection, root })
    }

    fn atom(&self, name: &str) -> Result<u32, CaptureError> {
        Ok(self
            .connection
            .intern_atom(false, name.as_bytes())
            .map_err(failed)?
            .reply()
            .map_err(failed)?
            .atom)
    }

    pub fn selection_owner(&self) -> Result<u32, CaptureError> {
        Ok(self
            .connection
            .get_selection_owner(self.atom("CLIPBOARD")?)
            .map_err(failed)?
            .reply()
            .map_err(failed)?
            .owner)
    }

    fn property(&self, window: u32, name: &str) -> Result<Vec<u8>, CaptureError> {
        Ok(self
            .connection
            .get_property(false, window, self.atom(name)?, AtomEnum::ANY, 0, 4096)
            .map_err(failed)?
            .reply()
            .map_err(failed)?
            .value)
    }

    pub fn active_window_target(&self) -> Result<Target, CaptureError> {
        let active = self
            .connection
            .get_property(
                false,
                self.root,
                self.atom("_NET_ACTIVE_WINDOW")?,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .map_err(failed)?
            .reply()
            .map_err(failed)?;
        let window = active
            .value32()
            .and_then(|mut values| values.next())
            .unwrap_or(NONE);
        if window == NONE {
            return Ok(Target::Other);
        }
        let classes = self
            .property(window, "WM_CLASS")?
            .split(|b| *b == 0)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect::<Vec<_>>();
        let title = self.property(window, "_NET_WM_NAME")?;
        Ok(classify_window(&classes, &String::from_utf8_lossy(&title)))
    }

    fn keycode(&self, key: Key) -> Result<u8, CaptureError> {
        let symbol = match key {
            Key::Ctrl => 0xffe3,
            Key::Shift => 0xffe1,
            Key::C => 0x63,
            Key::F7 => 0xffc4,
        };
        let setup = self.connection.setup();
        let mapping = self
            .connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .map_err(failed)?
            .reply()
            .map_err(failed)?;
        mapping
            .keysyms
            .chunks(usize::from(mapping.keysyms_per_keycode))
            .position(|symbols| symbols.contains(&symbol))
            .map(|offset| setup.min_keycode + offset as u8)
            .ok_or_else(|| failed(format!("Không tìm thấy phím {key:?}")))
    }
}

impl KeyInjector for X11Session {
    fn modifiers_held(&mut self) -> Result<bool, CaptureError> {
        let mask = self
            .connection
            .query_pointer(self.root)
            .map_err(failed)?
            .reply()
            .map_err(failed)?
            .mask;
        Ok(mask.intersects(
            KeyButMask::SHIFT | KeyButMask::CONTROL | KeyButMask::MOD1 | KeyButMask::MOD4,
        ))
    }

    fn key_event(&mut self, key: Key, pressed: bool) -> Result<(), CaptureError> {
        let code = self.keycode(key)?;
        self.connection
            .xtest_fake_input(
                if pressed {
                    xproto::KEY_PRESS_EVENT
                } else {
                    xproto::KEY_RELEASE_EVENT
                },
                code,
                CURRENT_TIME,
                self.root,
                0,
                0,
                0,
            )
            .map_err(failed)?
            .check()
            .map_err(failed)?;
        self.connection.flush().map_err(failed)
    }
}
