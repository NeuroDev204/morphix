//! Menu chuột phải kiểu Windows 11 (IExplorerCommand). Mục "Morphix" là lệnh gốc có submenu; mỗi việc là một
//! lệnh riêng nên Explorer báo đúng việc được bấm. Chỉ hoạt động khi gói sparse (`package/AppxManifest.xml`) đã cài.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::path::PathBuf;

use windows::core::{implement, Interface, Ref, GUID, HRESULT, PWSTR};
use windows::Win32::Foundation::{
    CLASS_E_NOAGGREGATION, E_INVALIDARG, E_NOTIMPL, E_OUTOFMEMORY, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoRegisterClassObject, CoTaskMemAlloc, CoTaskMemFree, IBindCtx, IClassFactory,
    IClassFactory_Impl, CLSCTX_LOCAL_SERVER, COINIT_MULTITHREADED, REGCLS_MULTIPLEUSE,
};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl,
    IShellItemArray, ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, SIGDN_FILESYSPATH,
};
use windows_core::{ComObject, IUnknown, BOOL};

use crate::entries::{shell_menu, ShellMenu};
use crate::windows::dispatch_request;
use morphix_ipc::MenuRequest;

/// CLSID của lệnh menu Windows 11; trùng với `com:Class` trong `package/AppxManifest.xml`.
pub const CLSID_MORPHIX_COMMAND: GUID = GUID::from_u128(0x9b4e2c1a_7d3f_4e8b_a6c5_2f1e0d9c8b7a);

#[derive(Clone)]
enum Role {
    /// Mục "Morphix" có submenu.
    Root,
    /// Một việc trong submenu, với id và nhãn đã có trong catalog.
    Action { id: String, label: String },
}

#[implement(IExplorerCommand)]
struct MorphixCommand {
    role: Role,
    /// File đang chọn, lưu khi Explorer hỏi trạng thái để submenu biết cần liệt kê việc nào.
    paths: RefCell<Vec<PathBuf>>,
}

impl MorphixCommand {
    fn new(role: Role, paths: Vec<PathBuf>) -> Self {
        Self {
            role,
            paths: RefCell::new(paths),
        }
    }
}

impl IExplorerCommand_Impl for MorphixCommand_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> windows_core::Result<PWSTR> {
        match &self.role {
            Role::Root => co_task_string("Morphix"),
            Role::Action { label, .. } => co_task_string(label),
        }
    }

    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> windows_core::Result<GUID> {
        Err(E_NOTIMPL.into())
    }

    fn GetState(&self, items: Ref<IShellItemArray>, _slow: BOOL) -> windows_core::Result<u32> {
        let paths = paths_of(items);
        let state = match &self.role {
            Role::Root => match shell_menu(&paths) {
                ShellMenu::Items(_) => ECS_ENABLED,
                _ => ECS_HIDDEN,
            },
            Role::Action { .. } => ECS_ENABLED,
        };
        *self.paths.borrow_mut() = paths;
        Ok(state.0 as u32)
    }

    fn Invoke(
        &self,
        items: Ref<IShellItemArray>,
        _context: Ref<IBindCtx>,
    ) -> windows_core::Result<()> {
        match &self.role {
            Role::Root => Err(E_NOTIMPL.into()),
            Role::Action { id, .. } => dispatch_request(MenuRequest {
                paths: paths_of(items),
                action: Some(id.clone()),
            }),
        }
    }

    fn GetFlags(&self) -> windows_core::Result<u32> {
        Ok(match self.role {
            Role::Root => ECF_HASSUBCOMMANDS.0 as u32,
            Role::Action { .. } => ECF_DEFAULT.0 as u32,
        })
    }

    fn EnumSubCommands(&self) -> windows_core::Result<IEnumExplorerCommand> {
        let Role::Root = self.role else {
            return Err(E_NOTIMPL.into());
        };
        let paths = self.paths.borrow().clone();
        let children: Vec<IExplorerCommand> = match shell_menu(&paths) {
            ShellMenu::Items(items) => items
                .into_iter()
                .map(|item| {
                    let role = Role::Action {
                        id: item.id,
                        label: item.label,
                    };
                    ComObject::new(MorphixCommand::new(role, paths.clone())).into_interface()
                })
                .collect(),
            _ => Vec::new(),
        };
        Ok(ComObject::new(MorphixEnum {
            items: children,
            next: Cell::new(0),
        })
        .into_interface())
    }
}

/// Liệt kê các việc con; Explorer đọc theo từng lô bằng `Next`.
#[implement(IEnumExplorerCommand)]
struct MorphixEnum {
    items: Vec<IExplorerCommand>,
    next: Cell<usize>,
}

impl IEnumExplorerCommand_Impl for MorphixEnum_Impl {
    fn Next(&self, celt: u32, out: *mut Option<IExplorerCommand>, fetched: *mut u32) -> HRESULT {
        if out.is_null() {
            return E_INVALIDARG;
        }
        let start = self.next.get();
        let available = self.items.len().saturating_sub(start);
        let count = (celt as usize).min(available);
        for offset in 0..count {
            unsafe {
                out.add(offset)
                    .write(Some(self.items[start + offset].clone()))
            };
        }
        if !fetched.is_null() {
            unsafe { *fetched = count as u32 };
        }
        self.next.set(start + count);
        if count == celt as usize {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> windows_core::Result<()> {
        let start = self.next.get();
        if start + celt as usize > self.items.len() {
            self.next.set(self.items.len());
            return Err(S_FALSE.into());
        }
        self.next.set(start + celt as usize);
        Ok(())
    }

    fn Reset(&self) -> windows_core::Result<()> {
        self.next.set(0);
        Ok(())
    }

    fn Clone(&self) -> windows_core::Result<IEnumExplorerCommand> {
        Ok(ComObject::new(MorphixEnum {
            items: self.items.clone(),
            next: Cell::new(self.next.get()),
        })
        .into_interface())
    }
}

/// Đường dẫn của các file trong mảng mục Explorer truyền vào. Mục không đọc được tên thì bỏ qua.
fn paths_of(items: Ref<IShellItemArray>) -> Vec<PathBuf> {
    let Ok(array) = items.ok() else {
        return Vec::new();
    };
    let count = unsafe { array.GetCount() }.unwrap_or(0);
    (0..count)
        .filter_map(|index| {
            let item = unsafe { array.GetItemAt(index) }.ok()?;
            let name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.ok()?;
            let text = unsafe { name.to_string() }.ok();
            unsafe { CoTaskMemFree(Some(name.0 as *const c_void)) };
            text.map(PathBuf::from)
        })
        .collect()
}

/// Chuỗi cấp phát bằng `CoTaskMemAlloc`, vì Explorer tự giải phóng bằng `CoTaskMemFree`.
fn co_task_string(text: &str) -> windows_core::Result<PWSTR> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let memory = unsafe { CoTaskMemAlloc(wide.len() * 2) } as *mut u16;
    if memory.is_null() {
        return Err(E_OUTOFMEMORY.into());
    }
    unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), memory, wide.len()) };
    Ok(PWSTR(memory))
}

/// Factory cho lệnh gốc; `DllGetClassObject` trả về khi Explorer hỏi đúng CLSID.
#[implement(IClassFactory)]
struct CommandClassFactory;

impl IClassFactory_Impl for CommandClassFactory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        output: *mut *mut c_void,
    ) -> windows_core::Result<()> {
        if outer.as_ref().is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if iid.is_null() || output.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let command: IExplorerCommand =
            ComObject::new(MorphixCommand::new(Role::Root, Vec::new())).into_interface();
        let unknown: IUnknown = command.cast()?;
        unsafe {
            (Interface::vtable(&unknown).QueryInterface)(Interface::as_raw(&unknown), iid, output)
        }
        .ok()
    }

    fn LockServer(&self, _lock: BOOL) -> windows_core::Result<()> {
        Ok(())
    }
}

pub fn class_factory() -> IClassFactory {
    ComObject::new(CommandClassFactory).into_interface()
}

/// Chạy máy chủ COM ngoài tiến trình cho menu Windows 11: `morphix.exe -Embedding`, do Explorer khởi động qua gói
/// sparse. Đăng ký lớp rồi giữ tiến trình sống để phục vụ Explorer.
pub fn serve_command() -> windows_core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    let factory: IClassFactory = class_factory();
    unsafe {
        CoRegisterClassObject(
            &CLSID_MORPHIX_COMMAND,
            &factory,
            CLSCTX_LOCAL_SERVER,
            REGCLS_MULTIPLEUSE,
        )?;
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
