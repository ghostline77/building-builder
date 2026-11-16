//! Deterministic stored ZIP bundles for portable editable source projects.
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Seek, Write},
    path::{Path, PathBuf},
};

pub fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        ensure!(
            !entry.file_type()?.is_symlink(),
            "Export source contains a symbolic link"
        );
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn files(root: &Path, items: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files(&entry.path(), items)?;
        } else {
            items.push(entry.path());
        }
    }
    Ok(())
}

/// Stores generated files without lossy transformations. CRCs and ZIP directory
/// offsets are explicit; oversized classic-ZIP output is rejected.
pub fn write(root: &Path, output: &Path) -> Result<()> {
    let mut entries = Vec::new();
    files(root, &mut entries)?;
    entries.sort();
    let count = u16::try_from(entries.len()).context("Too many export files for a ZIP bundle")?;
    let mut file = fs::File::create_new(output)?;
    let mut directory = Vec::new();
    for path in entries {
        let name = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/")
            .into_bytes();
        let name_length = u16::try_from(name.len())?;
        let bytes = fs::read(&path)?;
        let size = u32::try_from(bytes.len()).context("Export file exceeds classic ZIP size")?;
        let crc = crc32fast::hash(&bytes);
        let offset = u32::try_from(file.stream_position()?)?;
        let mut header = 0x04034b50_u32.to_le_bytes().to_vec();
        for value in [20_u16, 0x800, 0, 0, 33] {
            header.extend(value.to_le_bytes());
        }
        for value in [crc, size, size] {
            header.extend(value.to_le_bytes());
        }
        header.extend(name_length.to_le_bytes());
        header.extend(0_u16.to_le_bytes());
        header.extend(&name);
        file.write_all(&header)?;
        file.write_all(&bytes)?;
        directory.extend(0x02014b50_u32.to_le_bytes());
        for value in [20_u16, 20, 0x800, 0, 0, 33] {
            directory.extend(value.to_le_bytes());
        }
        for value in [crc, size, size] {
            directory.extend(value.to_le_bytes());
        }
        for value in [name_length, 0, 0, 0, 0] {
            directory.extend(value.to_le_bytes());
        }
        directory.extend(0_u32.to_le_bytes());
        directory.extend(offset.to_le_bytes());
        directory.extend(name);
    }
    let directory_offset = u32::try_from(file.stream_position()?)?;
    let directory_size = u32::try_from(directory.len())?;
    file.write_all(&directory)?;
    file.write_all(&0x06054b50_u32.to_le_bytes())?;
    for value in [0_u16, 0, count, count] {
        file.write_all(&value.to_le_bytes())?;
    }
    file.write_all(&directory_size.to_le_bytes())?;
    file.write_all(&directory_offset.to_le_bytes())?;
    file.write_all(&0_u16.to_le_bytes())?;
    file.sync_all()?;
    Ok(())
}
