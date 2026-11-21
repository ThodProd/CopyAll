#![windows_subsystem = "windows"]

mod copy;
mod integration;
mod native;

use std::{
    env,
    ffi::OsString,
    io::{self, BufRead, Read, Write},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    process::ExitCode,
    thread::{self, JoinHandle},
};

use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let method = args.next();
    let command = method.as_deref().and_then(|value| value.to_str());
    let result = match command {
        None if method.is_none() => serve(),
        Some("--register-integrations") => integration::repair(),
        Some("--uninstall-context") => {
            integration::set_startup(false).and_then(|()| integration::remove_context())
        }
        Some("--startup") => integration::startup(),
        Some("--trust-shell-certificate") => integration::trust_certificate(),
        Some("--context-copy") => copy::run(args.map(PathBuf::from).collect()),
        Some("--selection") => read_selection().and_then(copy::run),
        Some("--menu-path") => menu_path(args.next()).and_then(|path| copy::run(vec![path])),
        _ => Err("Неизвестная команда CopyAll.".into()),
    };
    if let Err(error) = result {
        let text = error.to_string();
        let _ = writeln!(io::stderr().lock(), "{text}");
        if matches!(
            command,
            Some("--context-copy" | "--menu-path" | "--selection" | "--trust-shell-certificate")
        ) {
            native::message(&text, native::MB_ICONERROR);
        }
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn read_selection() -> Result<Vec<PathBuf>> {
    let mut bytes = Vec::new();
    io::stdin().lock().read_to_end(&mut bytes)?;
    if bytes.len() % 2 != 0 || !bytes.ends_with(&[0, 0]) {
        return Err("Проводник передал неполный список путей.".into());
    }
    let mut paths = Vec::new();
    let mut path = Vec::new();
    for bytes in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        if unit == 0 {
            if path.is_empty() {
                return Err("Проводник передал пустой путь.".into());
            }
            paths.push(PathBuf::from(OsString::from_wide(&path)));
            path.clear();
        } else {
            path.push(unit);
        }
    }
    Ok(paths)
}

fn menu_path(arg: Option<OsString>) -> Result<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    let arg = arg.ok_or("Проводник не передал путь.")?;
    let mut path: Vec<u16> = arg.encode_wide().collect();
    if path.pop() != Some(u16::from(b'|')) || path.is_empty() {
        return Err("Проводник передал неполный путь.".into());
    }
    Ok(PathBuf::from(OsString::from_wide(&path)))
}

fn serve() -> Result<()> {
    let mut job = None;
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let line = line?;
        let request: serde_json::Result<Value> = serde_json::from_str(&line);
        let (id, result) = match request {
            Ok(request) => (request["id"].clone(), dispatch(&request, &mut job)),
            Err(error) => (Value::Null, Err(error.into())),
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": error.to_string()}})
            }
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

type SettingsJob = JoinHandle<Result<Value>>;

fn dispatch(request: &Value, job: &mut Option<SettingsJob>) -> Result<Value> {
    match request["method"].as_str() {
        Some("describe") => {
            Ok(json!({"name": "CopyAll", "version": env!("CARGO_PKG_VERSION"), "api": 1}))
        }
        Some("settings.poll") => {
            let Some(active) = job else {
                return Err("Нет активной операции.".into());
            };
            if !active.is_finished() {
                return Ok(json!({"running": true}));
            }
            let result = job
                .take()
                .unwrap()
                .join()
                .map_err(|_| "Операция прервана.")??;
            Ok(json!({"running": false, "state": result}))
        }
        method => {
            let action: fn() -> Result<()> = match method {
                Some("settings.status") => || Ok(()),
                Some("context.install") => integration::install_context,
                Some("context.remove") => integration::remove_context,
                Some("startup.install") => || integration::set_startup(true),
                Some("startup.remove") => || integration::set_startup(false),
                _ => return Err("Неизвестная команда CopyAll.".into()),
            };
            if job.as_ref().is_some_and(|active| !active.is_finished()) {
                return Ok(json!({"running": true}));
            }
            *job = Some(thread::spawn(move || {
                action()?;
                integration::status()
            }));
            Ok(json!({"running": true}))
        }
    }
}
