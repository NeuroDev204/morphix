pub const CLSID_STRING: &str = "{2131F98F-8888-464C-B3D7-80E2BBEEDDE1}";
pub const DLL_FILE_NAME: &str = "morphix_shell.dll";

#[cfg(windows)]
extern crate windows_core;

#[cfg(windows)]
mod command;
#[cfg(windows)]
pub use command::serve_command;
#[cfg(windows)]
mod windows;

pub mod entries;
