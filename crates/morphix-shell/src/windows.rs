use std::{
    cell::RefCell,
    ffi::OsString,
    mem::size_of,
    os::windows::{ffi::OsStringExt, process::CommandExt},
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use morphix_ipc::MenuRequest;

use windows::{
    core::{
        implement, ComObject, Error, IUnknown, Interface, Result, BOOL, GUID, HRESULT, HSTRING,
        PCSTR, PCWSTR, PWSTR,
    },
    Win32::{
        Foundation::HMODULE,
        System::{
            Com::{
                IClassFactory, IClassFactory_Impl, IDataObject, DVASPECT_CONTENT, FORMATETC,
                STGMEDIUM, TYMED_HGLOBAL,
            },
            LibraryLoader::{
                GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                GET_MODULE_HANDLE_EX_FLAG_PIN,
            },
            Ole::{ReleaseStgMedium, CF_HDROP},
        },
        UI::{
            Shell::{
                DragQueryFileW, IContextMenu, IContextMenu_Impl, IShellExtInit, IShellExtInit_Impl,
                CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, HDROP,
            },
            WindowsAndMessaging::{
                CreatePopupMenu, DestroyMenu, InsertMenuItemW, MessageBoxW, HMENU, MB_ICONERROR,
                MB_OK, MENUITEMINFOW, MFS_DISABLED, MFT_SEPARATOR, MIIM_FTYPE, MIIM_ID, MIIM_STATE,
                MIIM_STRING, MIIM_SUBMENU,
            },
        },
    },
};

use crate::entries::{child_for_verb, child_slots, shell_menu, ShellItem, ShellMenu, MAX_FILES};

const CLSID_MORPHIX_SHELL: GUID = GUID::from_u128(0x2131f98f_8888_464c_b3d7_80e2bbeedde1);
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111_u32 as i32);
const CLASS_E_NOAGGREGATION: HRESULT = HRESULT(0x80040110_u32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x80070057_u32 as i32);
const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);
const CMIC_MASK_UNICODE: u32 = 0x0000_4000;
const GCS_VERBA: u32 = 0;
const GCS_VERBW: u32 = 4;
const GCS_HELPTEXTW: u32 = 5;

/// Tên verb ổn định của một mục: không đổi giữa các lần gọi QueryContextMenu.
fn verb_name(id: &str) -> String {
    format!("morphix.{id}")
}
const MENU_LABEL: &str = "Morphix";

static LIVE_OBJECTS: AtomicU32 = AtomicU32::new(0);
static SERVER_LOCKS: AtomicU32 = AtomicU32::new(0);

#[implement(IClassFactory)]
struct ShellClassFactory;

impl ShellClassFactory {
    fn new() -> Self {
        LIVE_OBJECTS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for ShellClassFactory {
    fn drop(&mut self) {
        LIVE_OBJECTS.fetch_sub(1, Ordering::Release);
    }
}

impl IClassFactory_Impl for ShellClassFactory_Impl {
    fn CreateInstance(
        &self,
        outer: windows::core::Ref<IUnknown>,
        iid: *const GUID,
        output: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        if outer.as_ref().is_some() {
            return Err(Error::from_hresult(CLASS_E_NOAGGREGATION));
        }
        if iid.is_null() || output.is_null() {
            return Err(Error::from_hresult(E_INVALIDARG));
        }

        let extension: IContextMenu = ComObject::new(ShellContextMenu::new()).into_interface();
        let unknown: IUnknown = extension.cast()?;
        unsafe {
            (Interface::vtable(&unknown).QueryInterface)(Interface::as_raw(&unknown), iid, output)
                .ok()
        }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            SERVER_LOCKS.fetch_add(1, Ordering::Relaxed);
        } else {
            let _ = SERVER_LOCKS.try_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(1)
            });
        }
        Ok(())
    }
}

#[implement(IShellExtInit, IContextMenu)]
struct ShellContextMenu {
    paths: RefCell<Vec<OsString>>,
    /// Các mục con của lần `QueryContextMenu` gần nhất; chỉ số `k` ứng với id `first_id + 1 + k`.
    items: RefCell<Vec<ShellItem>>,
}

impl ShellContextMenu {
    fn new() -> Self {
        LIVE_OBJECTS.fetch_add(1, Ordering::Relaxed);
        Self {
            paths: RefCell::new(Vec::new()),
            items: RefCell::new(Vec::new()),
        }
    }

    /// Dựng mục "Morphix" cùng submenu vào `menu`. Trả số id đã dùng, tính cả mục cha (0 nghĩa là không chèn gì).
    fn build_menu(&self, menu: HMENU, index: u32, first_id: u32, last_id: u32) -> Result<u32> {
        self.items.borrow_mut().clear();
        let paths: Vec<PathBuf> = self.paths.borrow().iter().map(PathBuf::from).collect();
        let kind = shell_menu(&paths);
        if kind == ShellMenu::Hidden || child_slots(first_id, last_id, 1) == 0 {
            return Ok(0);
        }
        let submenu = unsafe { CreatePopupMenu()? };
        match self.fill_submenu(submenu, &kind, first_id, last_id) {
            Ok(0) => {
                unsafe {
                    let _ = DestroyMenu(submenu);
                }
                return Ok(0);
            }
            Ok(_) => {}
            Err(error) => {
                unsafe {
                    let _ = DestroyMenu(submenu);
                }
                self.items.borrow_mut().clear();
                return Err(error);
            }
        }
        let mut label = wide(MENU_LABEL);
        let parent = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_ID | MIIM_STRING | MIIM_SUBMENU,
            wID: first_id,
            hSubMenu: submenu,
            dwTypeData: PWSTR(label.as_mut_ptr()),
            cch: label.len() as u32 - 1,
            ..Default::default()
        };
        if let Err(error) = unsafe { InsertMenuItemW(menu, index, true, &parent) } {
            unsafe {
                let _ = DestroyMenu(submenu);
            }
            self.items.borrow_mut().clear();
            return Err(error);
        }
        Ok(1 + self.items.borrow().len().max(1) as u32)
    }

    /// Chèn các mục con vào `submenu`, trả số mục đã chèn (không tính đường kẻ).
    fn fill_submenu(
        &self,
        submenu: HMENU,
        kind: &ShellMenu,
        first_id: u32,
        last_id: u32,
    ) -> Result<u32> {
        match kind {
            ShellMenu::Hidden => Ok(0),
            ShellMenu::TooMany => {
                let label = format!("Chọn quá nhiều file (tối đa {MAX_FILES})");
                insert_string_item(submenu, 0, first_id + 1, &label, true)?;
                Ok(1)
            }
            ShellMenu::Items(items) => {
                let slots = child_slots(first_id, last_id, items.len());
                let mut position = 0u32;
                let mut shown: Vec<ShellItem> = Vec::new();
                let mut separated = false;
                for item in items.iter().take(slots) {
                    if item.is_tool && !separated && !shown.is_empty() {
                        insert_separator(submenu, position)?;
                        position += 1;
                        separated = true;
                    }
                    let id = first_id + 1 + shown.len() as u32;
                    insert_string_item(submenu, position, id, &item.label, false)?;
                    position += 1;
                    shown.push(item.clone());
                }
                let count = shown.len() as u32;
                *self.items.borrow_mut() = shown;
                Ok(count)
            }
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn insert_string_item(
    submenu: HMENU,
    position: u32,
    id: u32,
    label: &str,
    disabled: bool,
) -> Result<()> {
    let mut text = wide(label);
    let item = MENUITEMINFOW {
        cbSize: size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_ID | MIIM_STRING | MIIM_STATE,
        fState: if disabled {
            MFS_DISABLED
        } else {
            Default::default()
        },
        wID: id,
        dwTypeData: PWSTR(text.as_mut_ptr()),
        cch: text.len() as u32 - 1,
        ..Default::default()
    };
    unsafe { InsertMenuItemW(submenu, position, true, &item) }
}

fn insert_separator(submenu: HMENU, position: u32) -> Result<()> {
    let item = MENUITEMINFOW {
        cbSize: size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE,
        fType: MFT_SEPARATOR,
        ..Default::default()
    };
    unsafe { InsertMenuItemW(submenu, position, true, &item) }
}

impl Drop for ShellContextMenu {
    fn drop(&mut self) {
        LIVE_OBJECTS.fetch_sub(1, Ordering::Release);
    }
}

impl IShellExtInit_Impl for ShellContextMenu_Impl {
    fn Initialize(
        &self,
        _folder: *const windows::Win32::UI::Shell::Common::ITEMIDLIST,
        data_object: windows::core::Ref<IDataObject>,
        _class_key: windows::Win32::System::Registry::HKEY,
    ) -> Result<()> {
        let data_object = data_object.ok()?;
        let format = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };
        let medium = StgMediumGuard(unsafe { data_object.GetData(&format)? });
        let paths = read_hdrop_paths(&medium.0)?;
        self.paths.replace(paths);
        Ok(())
    }
}

impl IContextMenu_Impl for ShellContextMenu_Impl {
    fn QueryContextMenu(
        &self,
        menu: HMENU,
        index: u32,
        first_id: u32,
        last_id: u32,
        flags: u32,
    ) -> HRESULT {
        if !is_explorer_host()
            || self.paths.borrow().is_empty()
            || first_id > last_id
            || flags & windows::Win32::UI::Shell::CMF_DEFAULTONLY != 0
        {
            return HRESULT(0);
        }
        // WHY: giá trị trả về là số id đã dùng tính từ `first_id`; trả sai sẽ làm id của Morphix đè lên extension khác.
        match self.build_menu(menu, index, first_id, last_id) {
            Ok(used) => HRESULT(used as i32),
            Err(error) => error.code(),
        }
    }

    fn InvokeCommand(&self, command: *const CMINVOKECOMMANDINFO) -> Result<()> {
        if command.is_null() {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        // WHY: verb dạng chuỗi (HIWORD khác 0) là lệnh của người khác hoặc script; chỉ nhận offset số của mục mình.
        // WHY: Explorer gọi bằng mã số (offset) hoặc bằng tên verb do GetCommandString trả về; nhận cả hai.
        let (verb, verb_name_asked) = unsafe {
            let info = &*command;
            if info.fMask & CMIC_MASK_UNICODE != 0
                && info.cbSize as usize >= size_of::<CMINVOKECOMMANDINFOEX>()
            {
                let pointer = (*(command as *const CMINVOKECOMMANDINFOEX)).lpVerbW.0;
                let text = (pointer as usize > 0xFFFF)
                    .then(|| PCWSTR(pointer).to_string().unwrap_or_default());
                (pointer as usize, text)
            } else {
                let pointer = info.lpVerb.0;
                let text = (pointer as usize > 0xFFFF)
                    .then(|| PCSTR(pointer).to_string().unwrap_or_default());
                (pointer as usize, text)
            }
        };
        let id = {
            let items = self.items.borrow();
            let index = match verb_name_asked {
                Some(name) => items.iter().position(|item| verb_name(&item.id) == name),
                None => child_for_verb(verb, items.len()),
            }
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
            items[index].id.clone()
        };
        dispatch_request(MenuRequest {
            paths: self.paths.borrow().iter().map(PathBuf::from).collect(),
            action: Some(id),
        })
    }

    fn GetCommandString(
        &self,
        command_id: usize,
        kind: u32,
        _reserved: *const u32,
        name: windows::core::PSTR,
        max: u32,
    ) -> Result<()> {
        if name.is_null() || max == 0 {
            return Ok(());
        }
        // WHY: tên verb ổn định để Explorer gọi đúng mục khi không dùng mã số.
        if kind == GCS_VERBW || kind == GCS_VERBA {
            let items = self.items.borrow();
            let index = child_for_verb(command_id, items.len())
                .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
            let verb = verb_name(&items[index].id);
            if kind == GCS_VERBW {
                let wide: Vec<u16> = verb.encode_utf16().collect();
                let count = wide.len().min(max as usize - 1);
                unsafe {
                    let out = name.0 as *mut u16;
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), out, count);
                    *out.add(count) = 0;
                }
            } else {
                let bytes = verb.as_bytes();
                let count = bytes.len().min(max as usize - 1);
                unsafe {
                    let out = name.0;
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, count);
                    *out.add(count) = 0;
                }
            }
            return Ok(());
        }
        if kind != GCS_HELPTEXTW {
            return Ok(());
        }
        let items = self.items.borrow();
        let index = child_for_verb(command_id, items.len())
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let help: Vec<u16> = items[index].label.encode_utf16().collect();
        let count = help.len().min(max as usize - 1);
        unsafe {
            let out = name.0 as *mut u16;
            std::ptr::copy_nonoverlapping(help.as_ptr(), out, count);
            *out.add(count) = 0;
        }
        Ok(())
    }
}

fn read_hdrop_paths(medium: &STGMEDIUM) -> Result<Vec<OsString>> {
    if medium.tymed != TYMED_HGLOBAL.0 as u32 {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let drop = unsafe { HDROP(medium.u.hGlobal.0 as *mut _) };
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        let length = unsafe { DragQueryFileW(drop, index, None) } as usize;
        if length == 0 {
            continue;
        }
        let mut path = vec![0u16; length + 1];
        let written = unsafe { DragQueryFileW(drop, index, Some(&mut path)) } as usize;
        if written > length {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        path.truncate(written);
        paths.push(OsString::from_wide(&path));
    }
    Ok(paths)
}

struct StgMediumGuard(STGMEDIUM);

impl Drop for StgMediumGuard {
    fn drop(&mut self) {
        unsafe { ReleaseStgMedium(&mut self.0) };
    }
}

fn shell_dll_path() -> Result<std::path::PathBuf> {
    let address = DllGetClassObject as *const () as *const u16;
    let mut module = HMODULE::default();
    unsafe {
        GetModuleHandleExW(
            // shortcut: pin workers' code until Explorer exits; revisit if hot unloading is required.
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            PCWSTR(address),
            &mut module,
        )?;
    }
    let mut path = vec![0u16; 32_768];
    let length = unsafe { GetModuleFileNameW(Some(module), &mut path) } as usize;
    if length == 0 {
        return Err(Error::from_thread());
    }
    if length >= path.len() {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    path.truncate(length);
    Ok(std::path::PathBuf::from(OsString::from_wide(&path)))
}

fn is_explorer_host() -> bool {
    let mut path = vec![0u16; 32_768];
    let length = unsafe { GetModuleFileNameW(None, &mut path) } as usize;
    if length == 0 || length >= path.len() {
        return false;
    }
    let executable = OsString::from_wide(&path[..length]);
    Path::new(&executable)
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("explorer.exe"))
}

/// Gửi yêu cầu cho GUI trên luồng riêng để không chặn Explorer. Lỗi sau đó hiện bằng hộp thoại,
/// vì lúc này người dùng đã bấm mục và không còn gì khác báo cho họ biết.
pub(crate) fn dispatch_request(request: MenuRequest) -> Result<()> {
    let gui = shell_dll_path()?.with_file_name("morphix-gui.exe");
    std::thread::Builder::new()
        .name("morphix-shell-ipc".to_string())
        .spawn(move || {
            if let Err(error) = send_request_or_start_gui(&request, &gui) {
                log_debug(&format!("Morphix shell IPC failed: {error}"));
                show_error(&format!("Morphix không chạy được việc đã chọn: {error}"));
            }
        })
        .map(|_| ())
        .map_err(|error| {
            log_debug(&format!(
                "Morphix shell could not start IPC worker: {error}"
            ));
            Error::from_hresult(E_FAIL)
        })
}

fn send_request_or_start_gui(request: &MenuRequest, gui: &Path) -> std::result::Result<(), String> {
    let socket = morphix_ipc::socket_name();
    match morphix_ipc::send_request(&socket, request) {
        Ok(()) => return Ok(()),
        Err(error) => log_debug(&format!("Morphix GUI not connected, starting it: {error}")),
    }
    std::process::Command::new(gui)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .spawn()
        .map_err(|error| format!("không khởi động được {}: {error}", gui.display()))?;
    let mut last_error = String::new();
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(20));
        match morphix_ipc::send_request(&socket, request) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!("GUI không nhận yêu cầu: {last_error}"))
}

fn show_error(message: &str) {
    let text = HSTRING::from(message);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            windows::core::w!("Morphix"),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn log_debug(message: &str) {
    unsafe {
        windows::Win32::System::Diagnostics::Debug::OutputDebugStringW(
            &windows::core::HSTRING::from(message),
        );
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    class_id: *const GUID,
    iid: *const GUID,
    output: *mut *mut std::ffi::c_void,
) -> HRESULT {
    if class_id.is_null() || iid.is_null() || output.is_null() {
        return E_INVALIDARG;
    }
    let factory: IClassFactory = if unsafe { *class_id == CLSID_MORPHIX_SHELL } {
        ComObject::new(ShellClassFactory::new()).into_interface()
    } else if unsafe { *class_id == crate::command::CLSID_MORPHIX_COMMAND } {
        crate::command::class_factory()
    } else {
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    let unknown: IUnknown = match factory.cast() {
        Ok(unknown) => unknown,
        Err(error) => return error.code(),
    };
    unsafe {
        (Interface::vtable(&unknown).QueryInterface)(Interface::as_raw(&unknown), iid, output)
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_OBJECTS.load(Ordering::Acquire) == 0 && SERVER_LOCKS.load(Ordering::Acquire) == 0 {
        HRESULT(0)
    } else {
        HRESULT(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_gui_executable_reports_a_reason() {
        let request = MenuRequest {
            paths: vec![PathBuf::from(r"C:\image.png")],
            action: Some("convert:webp".to_string()),
        };
        let missing_gui = Path::new(r"C:\missing-morphix-test\morphix-gui.exe");
        let error = send_request_or_start_gui(&request, missing_gui).unwrap_err();
        assert!(error.contains("không khởi động được"), "{error}");
    }
}
