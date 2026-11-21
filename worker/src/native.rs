use std::{io::Read, mem::size_of, os::windows::ffi::OsStrExt, path::Path, thread, time::Duration};

use windows::{
    Win32::{
        Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HWND, WAIT_OBJECT_0},
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalLock, GlobalUnlock},
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::{GetExitCodeProcess, WaitForSingleObject},
        },
        UI::{
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW, ShellExecuteW},
            WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, HWND_MESSAGE, IDYES, MB_DEFBUTTON2, MB_OK,
                MB_YESNO, MESSAGEBOX_STYLE, MessageBoxW, SW_HIDE, SW_SHOWNORMAL,
            },
        },
    },
    core::{HSTRING, PCWSTR, w},
};

pub use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_ICONWARNING};

pub fn clipboard(len: u64, source: &mut impl Read) -> crate::Result<()> {
    let size = usize::try_from(len)?
        .checked_add(2)
        .filter(|size| *size <= isize::MAX as usize)
        .ok_or("Список слишком большой.")?;
    let mut memory_status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // Check the current system capacity before faulting in a potentially very large block.
    if unsafe { GlobalMemoryStatusEx(&mut memory_status) }.is_ok()
        && size as u64
            > memory_status
                .ullAvailPhys
                .min(memory_status.ullAvailPageFile)
    {
        return Err("Недостаточно свободной памяти для буфера обмена.".into());
    }

    let mut memory = ClipboardMemory(unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, size)? });
    let pointer = unsafe { GlobalLock(memory.0) }.cast::<u8>();
    if pointer.is_null() {
        return Err(windows::core::Error::from_win32().into());
    }
    let contents = unsafe { std::slice::from_raw_parts_mut(pointer, size) };
    let (text, terminator) = contents.split_at_mut(size - 2);
    let read = source.read_exact(text);
    terminator.fill(0);
    let _ = unsafe { GlobalUnlock(memory.0) };
    read?;

    let window = ClipboardWindow(unsafe {
        CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("CopyAll"),
            Default::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            None,
            None,
        )?
    });
    for attempt in 0..20 {
        if unsafe { OpenClipboard(window.0) }.is_ok() {
            let _clipboard = OpenedClipboard;
            unsafe {
                EmptyClipboard()?;
                const CF_UNICODETEXT: u32 = 13;
                SetClipboardData(CF_UNICODETEXT, HANDLE(memory.0.0))?;
            }
            memory.0 = HGLOBAL::default(); // Windows now owns the memory.
            return Ok(());
        }
        if attempt < 19 {
            thread::sleep(Duration::from_millis(50));
        }
    }
    Err("Буфер обмена занят другой программой.".into())
}

struct ClipboardMemory(HGLOBAL);

impl Drop for ClipboardMemory {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { GlobalFree(self.0) };
        }
    }
}

struct ClipboardWindow(HWND);

impl Drop for ClipboardWindow {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct OpenedClipboard;

impl Drop for OpenedClipboard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

pub fn open_file(path: &Path) -> crate::Result<()> {
    let path = wide_path(path);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!("код Windows {}", result.0 as isize).into());
    }
    Ok(())
}

pub fn trust_prompt() -> bool {
    unsafe {
        MessageBoxW(
            None,
            w!(
                "Для пункта в первом меню Windows 11 нужно добавить сертификат издателя ThodProd.CopyAll в «Доверенные лица» компьютера. Windows запросит права администратора. Продолжить?"
            ),
            w!("CopyAll"),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}

pub fn elevate_certificate_tool() -> crate::Result<()> {
    let executable = wide_path(&std::env::current_exe()?);
    let mut request = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(executable.as_ptr()),
        lpParameters: w!("--trust-shell-certificate"),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut request)? };
    let process = request.hProcess;
    if process.is_invalid() {
        return Err("Не удалось запустить установку сертификата.".into());
    }
    let mut code = 1;
    let completed = unsafe { WaitForSingleObject(process, 120_000) } == WAIT_OBJECT_0;
    let result = unsafe { GetExitCodeProcess(process, &mut code) };
    let _ = unsafe { CloseHandle(process) };
    if completed && result.is_ok() && code == 0 {
        Ok(())
    } else {
        Err("Windows не подтвердила установку сертификата.".into())
    }
}

pub fn message(text: &str, icon: MESSAGEBOX_STYLE) {
    unsafe { MessageBoxW(None, &HSTRING::from(text), w!("CopyAll"), MB_OK | icon) };
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain([0]).collect()
}
