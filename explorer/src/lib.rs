use std::{
    ffi::{OsString, c_void},
    io::{BufWriter, Write},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
use windows::{
    Win32::{
        Foundation::{
            BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG,
            E_NOTIMPL, E_POINTER, HMODULE, S_FALSE, S_OK,
        },
        System::{
            Com::{CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl},
            LibraryLoader::{
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW,
                GetModuleHandleExW,
            },
            SystemServices::SFGAO_FILESYSTEM,
        },
        UI::Shell::{
            ECS_ENABLED, ECS_HIDDEN, IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl,
            IShellItemArray, SHStrDupW, SIATTRIBFLAGS_AND, SIGDN_FILESYSPATH,
        },
    },
    core::{Error, GUID, HRESULT, IUnknown, Interface, PCWSTR, PWSTR, Result, implement, w},
};

const CLASS_ID: GUID = GUID::from_u128(0xdb86a616_f972_4d28_99b6_cb1087621b51);
static LIVE_OBJECTS: AtomicUsize = AtomicUsize::new(0);
static SERVER_LOCKS: AtomicUsize = AtomicUsize::new(0);

struct Lifetime;

impl Lifetime {
    fn new() -> Self {
        LIVE_OBJECTS.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        LIVE_OBJECTS.fetch_sub(1, Ordering::Release);
    }
}

#[implement(IExplorerCommand)]
struct CopyPaths(Lifetime);

impl IExplorerCommand_Impl for CopyPaths_Impl {
    fn GetTitle(&self, _: Option<&IShellItemArray>) -> Result<PWSTR> {
        unsafe { SHStrDupW(w!("Копировать полный путь")) }
    }

    fn GetIcon(&self, _: Option<&IShellItemArray>) -> Result<PWSTR> {
        let exe = worker_path()?;
        let icon: Vec<u16> = exe.as_os_str().encode_wide().chain([44, 48, 0]).collect();
        unsafe { SHStrDupW(PCWSTR(icon.as_ptr())) }
    }

    fn GetToolTip(&self, _: Option<&IShellItemArray>) -> Result<PWSTR> {
        unsafe {
            SHStrDupW(w!("Скопировать полные пути, включая содержимое папок"))
        }
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLASS_ID)
    }

    fn GetState(&self, items: Option<&IShellItemArray>, _: BOOL) -> Result<u32> {
        if let Some(items) = items {
            if let Ok(attributes) =
                unsafe { items.GetAttributes(SIATTRIBFLAGS_AND, SFGAO_FILESYSTEM) }
            {
                if attributes.contains(SFGAO_FILESYSTEM) {
                    return Ok(ECS_ENABLED.0 as u32);
                }
            }
        }
        Ok(ECS_HIDDEN.0 as u32)
    }

    fn Invoke(&self, items: Option<&IShellItemArray>, _: Option<&IBindCtx>) -> Result<()> {
        let items = items.ok_or(E_INVALIDARG)?;
        let count = unsafe { items.GetCount()? };
        if count == 0 {
            return Err(E_INVALIDARG.into());
        }

        // Pass all selected paths through a pipe, so Explorer's selection is independent of
        // the Windows command-line length. The worker performs all filesystem enumeration.
        let mut child = Command::new(worker_path()?)
            .arg("--selection")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(io_error)?;
        let mut pipe = BufWriter::new(child.stdin.take().ok_or(E_FAIL)?);
        let mut bytes = Vec::new();
        let sent = (|| {
            for index in 0..count {
                let item = unsafe { items.GetItemAt(index)? };
                let path = ShellString(unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? });
                bytes.clear();
                bytes.extend(
                    unsafe { path.0.as_wide() }
                        .iter()
                        .flat_map(|unit| unit.to_le_bytes()),
                );
                bytes.extend_from_slice(&[0, 0]);
                pipe.write_all(&bytes).map_err(io_error)?;
            }
            pipe.flush().map_err(io_error)
        })();
        if sent.is_err() {
            let _ = child.kill();
        }
        sent
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(0)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

struct ShellString(PWSTR);

impl Drop for ShellString {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(Some(self.0.0.cast())) };
    }
}

fn io_error(error: std::io::Error) -> Error {
    let code = error
        .raw_os_error()
        .map(|code| HRESULT::from_win32(code as u32))
        .unwrap_or(E_FAIL);
    Error::new(code, error.to_string())
}

fn worker_path() -> Result<PathBuf> {
    let mut module = HMODULE::default();
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(worker_path as *const () as *const u16),
            &mut module,
        )?;
    }
    let mut path = vec![0; 32768];
    let len = unsafe { GetModuleFileNameW(module, &mut path) } as usize;
    if len == 0 || len >= path.len() {
        return Err(Error::from_win32());
    }
    let mut path = PathBuf::from(OsString::from_wide(&path[..len]));
    path.set_file_name("CopyAll.Worker.exe");
    Ok(path)
}

#[implement(IClassFactory)]
struct Factory(Lifetime);

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Option<&IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = CopyPaths(Lifetime::new()).into();
        unsafe { command.query(iid, object).ok() }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        if lock.as_bool() {
            SERVER_LOCKS.fetch_add(1, Ordering::Relaxed);
        } else {
            SERVER_LOCKS.fetch_sub(1, Ordering::Release);
        }
        Ok(())
    }
}

/// # Safety
/// COM supplies valid pointers for the identifiers and a writable output pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    class: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if class.is_null() || iid.is_null() || object.is_null() {
        return E_POINTER;
    }
    unsafe { *object = std::ptr::null_mut() };
    if unsafe { *class } != CLASS_ID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory(Lifetime::new()).into();
    unsafe { factory.query(iid, object) }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE_OBJECTS.load(Ordering::Acquire) == 0 && SERVER_LOCKS.load(Ordering::Acquire) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
