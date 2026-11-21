use std::{
    env, fs,
    io::{self, BufWriter, Seek, SeekFrom, Write},
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

use crate::native;

const REPARSE_POINT: u32 = 0x400;
const BUFFER_SIZE: usize = 64 * 1024;
const UTF16_BOM: [u8; 2] = [0xff, 0xfe];

pub fn run(paths: Vec<PathBuf>) -> crate::Result<()> {
    let (mut output, skipped) = collect_paths(paths)?;
    if let Err(clipboard_error) = output.copy_to_clipboard() {
        let file = output.into_file()?;
        native::open_file(&file).map_err(|open_error| {
            format!(
                "Не удалось скопировать список ({clipboard_error}) или открыть TXT ({open_error}). Файл сохранён: {}",
                file.display()
            )
        })?;
    }
    if skipped != 0 {
        native::message(
            &format!("Список готов. Не удалось прочитать элементы: {skipped}."),
            native::MB_ICONWARNING,
        );
    }
    Ok(())
}

fn collect_paths(paths: Vec<PathBuf>) -> crate::Result<(Output, u64)> {
    if paths.is_empty() {
        return Err("Проводник не передал ни одного пути.".into());
    }

    let mut output = BufWriter::with_capacity(BUFFER_SIZE, Output::Memory(Vec::new()));
    let mut skipped = 0u64;
    let mut line = Vec::new();
    for path in paths {
        let path = std::path::absolute(path)?;
        write_path(&mut output, &mut line, &path)?;
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {
                skipped += append_directory(&path, &mut output, &mut line)?;
            }
            Ok(_) => {}
            Err(_) => skipped += 1,
        }
    }

    let output = output.into_inner().map_err(|error| error.into_error())?;
    Ok((output, skipped))
}

fn append_directory(
    path: &Path,
    output: &mut BufWriter<Output>,
    line: &mut Vec<u8>,
) -> io::Result<u64> {
    let mut skipped = 0;
    let mut directories = Vec::new();
    match fs::read_dir(path) {
        Ok(entries) => directories.push(entries),
        Err(_) => return Ok(1),
    }
    while let Some(entries) = directories.last_mut() {
        match entries.next() {
            Some(Ok(entry)) => {
                let path = entry.path();
                if let Output::File(file) = output.get_ref() {
                    if path.file_name() == file.path().file_name() {
                        continue;
                    }
                }
                write_path(output, line, &path)?;
                match entry.metadata() {
                    Ok(metadata)
                        if metadata.is_dir() && metadata.file_attributes() & REPARSE_POINT == 0 =>
                    {
                        match fs::read_dir(path) {
                            Ok(children) => directories.push(children),
                            Err(_) => skipped += 1,
                        }
                    }
                    Ok(_) => {}
                    Err(_) => skipped += 1,
                }
            }
            Some(Err(_)) => skipped += 1,
            None => {
                directories.pop();
            }
        }
    }
    Ok(skipped)
}

fn write_path(output: &mut impl Write, line: &mut Vec<u8>, path: &Path) -> io::Result<()> {
    line.clear();
    line.extend(path.as_os_str().encode_wide().flat_map(u16::to_le_bytes));
    line.extend_from_slice(&[b'\r', 0, b'\n', 0]);
    output.write_all(line)
}

enum Output {
    Memory(Vec<u8>),
    File(NamedTempFile),
}

impl Output {
    fn copy_to_clipboard(&mut self) -> crate::Result<()> {
        match self {
            Self::Memory(bytes) => native::clipboard(bytes.len() as u64, &mut bytes.as_slice()),
            Self::File(file) => {
                let len = file.as_file().metadata()?.len() - UTF16_BOM.len() as u64;
                file.seek(SeekFrom::Start(UTF16_BOM.len() as u64))?;
                native::clipboard(len, file)
            }
        }
    }

    fn into_file(self) -> io::Result<PathBuf> {
        let file = match self {
            Self::Memory(bytes) => spool(&bytes)?,
            Self::File(file) => file,
        };
        // Keep the export available after the worker exits, including if the viewer fails to open.
        Ok(file.into_temp_path().keep()?)
    }
}

fn spool(bytes: &[u8]) -> io::Result<NamedTempFile> {
    let directory = env::temp_dir().join("CopyAll");
    fs::create_dir_all(&directory)?;
    let mut file = tempfile::Builder::new()
        .prefix("CopyAll-")
        .suffix(".txt")
        .tempfile_in(directory)?;
    file.write_all(&UTF16_BOM)?;
    file.write_all(bytes)?;
    Ok(file)
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Self::Memory(buffer) = self {
            if bytes.len() <= BUFFER_SIZE - buffer.len() {
                return buffer.write(bytes);
            }
            *self = Self::File(spool(buffer)?);
        }
        match self {
            Self::File(file) => file.write(bytes),
            Self::Memory(_) => unreachable!(),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Memory(_) => Ok(()),
            Self::File(file) => file.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_every_path_beyond_the_previous_limits() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        for index in 0..20_001 {
            fs::File::create(
                root.path()
                    .join(format!("файл с длинным именем {index:05}.txt")),
            )?;
        }
        let mut deep = root.path().to_path_buf();
        for _ in 0..80 {
            deep.push("deep");
        }
        fs::create_dir_all(&deep)?;
        let last = deep.join("конец.txt");
        fs::File::create(&last)?;

        let (output, skipped) = collect_paths(vec![root.path().to_path_buf()])?;
        assert_eq!(skipped, 0);
        assert!(matches!(output, Output::File(_)));
        let saved = output.into_file()?;
        let contents = fs::read(&saved);
        fs::remove_file(saved)?;
        let contents = contents?;
        assert!(contents.starts_with(&UTF16_BOM));
        assert!(contents.len() > 2_000_000);
        let units: Vec<u16> = contents[2..]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .collect();
        let text = String::from_utf16(&units)?;
        assert_eq!(text.lines().count(), 20_083);
        assert!(text.lines().any(|line| Path::new(line) == last));
        Ok(())
    }
}
