use serde_json::{Value, json};
use std::{
    env, io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};
use windows::Win32::{
    System::{SystemInformation::GetSystemDirectoryW, Threading::CREATE_NO_WINDOW},
    UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify},
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
};

use crate::{Result, native};

const TITLE: &str = "Копировать полный путь";
const OWNER: &str = "ThodHUB.CopyAll.v1";
const MENU_KEYS: [(&str, &str); 4] = [
    (r"Software\Classes\Directory\shell\ThodHUB.CopyAll", "%1"),
    (
        r"Software\Classes\Directory\Background\shell\ThodHUB.CopyAll",
        "%V",
    ),
    (r"Software\Classes\*\shell\ThodHUB.CopyAll", "%1"),
    (r"Software\Classes\Drive\shell\ThodHUB.CopyAll", "%1"),
];
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "ThodProd.CopyAll";
const SETTINGS_KEY: &str = r"Software\ThodProd\CopyAll";
const RUN_OWNER_VALUE: &str = "StartupCommand";
const POWERSHELL_INIT: &str = "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); ";
const PACKAGE_QUERY: &str =
    "Get-AppxPackage -Name ThodProd.CopyAll -Publisher 'CN=ThodProd.CopyAll'";
const CERTIFICATE: &str = "0E85D468BD75DF47E5B93DBE77C5141A2B6BCFED";

pub fn status() -> Result<Value> {
    let classic_count = classic_count()?;
    let modern_supported = supports_modern_menu() && shell_path("CopyAll.msix")?.is_file();
    let modern_version = installed_package()?;
    Ok(json!({
        "contextEnabled": classic_count != 0 || !modern_version.is_empty(),
        "contextReady": classic_count == MENU_KEYS.len()
            && (!modern_supported || modern_version == package_version()),
        "modernEnabled": !modern_version.is_empty(),
        "startupEnabled": owned_startup()?.is_some(),
    }))
}

pub fn install_context() -> Result<()> {
    install_classic()?;
    if supports_modern_menu() && shell_path("CopyAll.msix")?.is_file() {
        if !trusted_certificate()? {
            if !native::trust_prompt() {
                return Err("Классическое меню включено. Установка пункта в первое меню Windows 11 отменена.".into());
            }
            native::elevate_certificate_tool()?;
        }
        register_package()?;
    }
    Ok(())
}

pub fn remove_context() -> Result<()> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let mut removed = false;
    for (path, _) in MENU_KEYS {
        if owns_menu_key(path)? {
            ignore_missing(current_user.delete_subkey_all(path))?;
            removed = true;
        }
    }
    if removed {
        notify_explorer();
    }
    if supports_modern_menu() {
        run_powershell(&format!("{PACKAGE_QUERY} | Remove-AppxPackage"), None)?;
    }
    Ok(())
}

pub fn set_startup(enabled: bool) -> Result<()> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    if !enabled && owned_startup()?.is_none() {
        return Ok(());
    }
    let (run, _) = current_user.create_subkey(RUN_KEY)?;
    let (settings, _) = current_user.create_subkey(SETTINGS_KEY)?;
    if enabled {
        let existing = read_string(&run, RUN_VALUE)?;
        let previous = read_string(&settings, RUN_OWNER_VALUE)?;
        if existing.is_some() && existing != previous {
            return Err("Запись автозапуска CopyAll занята другой программой.".into());
        }
        let command = format!("\"{}\" --startup", env::current_exe()?.display());
        settings.set_value(RUN_OWNER_VALUE, &command)?;
        if let Err(error) = run.set_value(RUN_VALUE, &command) {
            if let Some(previous) = previous {
                settings.set_value(RUN_OWNER_VALUE, &previous)?;
            } else {
                settings.delete_value(RUN_OWNER_VALUE)?;
            }
            return Err(error.into());
        }
    } else {
        ignore_missing(run.delete_value(RUN_VALUE))?;
        ignore_missing(settings.delete_value(RUN_OWNER_VALUE))?;
    }
    Ok(())
}

pub fn repair() -> Result<()> {
    if owned_startup()?.is_some() {
        set_startup(true)?;
    }
    if classic_count()? != 0 {
        install_classic()?;
    }
    // An update refreshes the external location of an already enabled Windows 11 command.
    if !installed_package()?.is_empty() && shell_path("CopyAll.msix")?.is_file() {
        register_package()?;
    }
    Ok(())
}

pub fn startup() -> Result<()> {
    if owned_startup()?.is_some() && classic_count()? != 0 {
        install_classic()?;
    }
    Ok(())
}

fn owned_startup() -> Result<Option<String>> {
    let owned = read_user_string(SETTINGS_KEY, RUN_OWNER_VALUE)?;
    let current = read_user_string(RUN_KEY, RUN_VALUE)?;
    Ok(owned.filter(|command| current.as_ref() == Some(command)))
}

fn install_classic() -> Result<()> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    for (path, _) in MENU_KEYS {
        if let Some(key) = open_user_key(path)? {
            if read_string(&key, "ThodOwner")?.as_deref() != Some(OWNER) {
                return Err("Ключ контекстного меню CopyAll занят другой программой.".into());
            }
        }
    }
    let executable = env::current_exe()?;
    for (path, target) in MENU_KEYS {
        let (key, _) = current_user.create_subkey(path)?;
        key.set_value("ThodOwner", &OWNER)?;
        key.set_value("", &TITLE)?;
        key.set_value("Icon", &format!("\"{}\",0", executable.display()))?;
        key.set_value("MultiSelectModel", &"Single")?;
        let (command, _) = key.create_subkey_with_flags("command", KEY_WRITE)?;
        // The marker prevents a path ending in a backslash (for example C:\) from
        // escaping the closing quote when Windows parses the command line.
        command.set_value(
            "",
            &format!("\"{}\" --menu-path \"{target}|\"", executable.display()),
        )?;
    }
    notify_explorer();
    Ok(())
}

fn classic_count() -> Result<usize> {
    let mut count = 0;
    for (path, _) in MENU_KEYS {
        count += usize::from(owns_menu_key(path)?);
    }
    Ok(count)
}

fn owns_menu_key(path: &str) -> Result<bool> {
    Ok(read_user_string(path, "ThodOwner")?.as_deref() == Some(OWNER))
}

fn open_user_key(path: &str) -> io::Result<Option<RegKey>> {
    optional(RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(path, KEY_READ))
}

fn read_user_string(path: &str, name: &str) -> io::Result<Option<String>> {
    match open_user_key(path)? {
        Some(key) => read_string(&key, name),
        None => Ok(None),
    }
}

fn read_string(key: &RegKey, name: &str) -> io::Result<Option<String>> {
    optional(key.get_value(name))
}

fn optional<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn ignore_missing(result: io::Result<()>) -> io::Result<()> {
    optional(result).map(|_| ())
}

fn notify_explorer() {
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

fn supports_modern_menu() -> bool {
    windows_version::OsVersion::current().build >= 22000
}

fn package_version() -> String {
    format!("{}.0", env!("CARGO_PKG_VERSION"))
}

fn installed_package() -> Result<String> {
    if supports_modern_menu() {
        run_powershell(
            &format!("{PACKAGE_QUERY} | ForEach-Object {{ $_.Version.ToString() }}"),
            None,
        )
    } else {
        Ok(String::new())
    }
}

fn register_package() -> Result<()> {
    let package = shell_path("CopyAll.msix")?;
    let external_location = env::current_exe()?
        .parent()
        .ok_or("Не найдена папка CopyAll.")?
        .to_path_buf();
    run_powershell(
        &format!(
            "$installed = {PACKAGE_QUERY}; if ($installed.Version -eq [version]'{}') {{ $installed | Remove-AppxPackage }}; Add-AppxPackage -Path $env:COPYALL_SHELL_FILE -ExternalLocation $env:COPYALL_SHELL_ROOT -ForceUpdateFromAnyVersion",
            package_version()
        ),
        Some((&package, &external_location)),
    )?;
    Ok(())
}

fn trusted_certificate() -> Result<bool> {
    let output = run_powershell(
        &format!("Test-Path -LiteralPath 'Cert:\\LocalMachine\\TrustedPeople\\{CERTIFICATE}'"),
        None,
    )?;
    Ok(output.eq_ignore_ascii_case("true"))
}

pub fn trust_certificate() -> Result<()> {
    let certificate = shell_path("CopyAll.cer")?;
    run_powershell(
        &format!(
            "$cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new($env:COPYALL_SHELL_FILE); if ($cert.Thumbprint -ne '{CERTIFICATE}') {{ throw 'CopyAll certificate mismatch.' }}; $store = [Security.Cryptography.X509Certificates.X509Store]::new('TrustedPeople','LocalMachine'); $store.Open('ReadWrite'); try {{ $store.Add($cert) }} finally {{ $store.Dispose() }}"
        ),
        Some((&certificate, Path::new(""))),
    )?;
    Ok(())
}

fn shell_path(name: &str) -> Result<PathBuf> {
    Ok(env::current_exe()?
        .parent()
        .ok_or("Не найдена папка CopyAll.")?
        .join("shell")
        .join(name))
}

fn run_powershell(script: &str, paths: Option<(&Path, &Path)>) -> Result<String> {
    let mut system_directory = [0u16; 260];
    let len = unsafe { GetSystemDirectoryW(Some(&mut system_directory)) } as usize;
    if len == 0 || len >= system_directory.len() {
        return Err("Не найдена системная папка Windows.".into());
    }
    use std::os::windows::ffi::OsStringExt;
    let executable = PathBuf::from(std::ffi::OsString::from_wide(&system_directory[..len]))
        .join(r"WindowsPowerShell\v1.0\powershell.exe");
    let mut command = Command::new(executable);
    command.creation_flags(CREATE_NO_WINDOW.0).args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        &format!("{POWERSHELL_INIT}{script}"),
    ]);
    if let Some((file, root)) = paths {
        command
            .env("COPYALL_SHELL_FILE", file)
            .env("COPYALL_SHELL_ROOT", root);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "Расширение Проводника: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
