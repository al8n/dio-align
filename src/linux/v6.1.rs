use rustix::{
  fd::AsFd,
  fs::{AtFlags, StatxFlags, statx},
};
use smol_str::format_smolstr;

use std::{fs::OpenOptions, io, path::Path};

use super::{super::DirectInfo, read_block_size, PHYSICAL_BLOCK_SIZE_FILE_NAME, BLOCK_PATH, ABS_BLOCK_PATH, QUEUE_PATH};

/// Fetch direct I/O information
pub fn fetch<P: AsRef<Path>>(path: P) -> io::Result<DirectInfo> {
  let path = path.as_ref();
  let abs = path.canonicalize()?;

  let parent_dir = abs.parent().ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("path '{}' has no parent directory", abs.display()),
    )
  })?;

  let parent_fd = OpenOptions::new().read(true).open(parent_dir)?;

  let file_name = abs.file_name().ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("path '{}' has no file name", abs.display()),
    )
  })?;

  let stx = statx(
    parent_fd.as_fd(),
    file_name,
    AtFlags::empty(),
    StatxFlags::DIOALIGN,
  )?;

  let mask_ok = stx.stx_mask & StatxFlags::DIOALIGN.bits() != 0;
  let values_valid = stx.stx_dio_mem_align > 0 && stx.stx_dio_offset_align > 0;

  if !(mask_ok && values_valid) {
    return Err(io::Error::new(
      io::ErrorKind::Unsupported,
      format!(
        "filesystem at '{}' does not support DIOALIGN (virtual/network filesystem?)",
        abs.display()
      ),
    ));
  }

  let mem_align = stx.stx_dio_mem_align;
  let logical_blk_size = stx.stx_dio_offset_align;
  let major = stx.stx_dev_major;
  let minor = stx.stx_dev_minor;

  let mut current = {
    let path = format_smolstr!("{ABS_BLOCK_PATH}/{major}:{minor}");
    let path: &str = path.as_ref();
    let path: &Path = path.as_ref();
    path.canonicalize()?
  };

  loop {
    let queue = current.join(QUEUE_PATH);

    let physical_blk_path = queue.join(PHYSICAL_BLOCK_SIZE_FILE_NAME);
    if physical_blk_path.exists() {
      let physical_blk_size = read_block_size(&physical_blk_path).unwrap_or(logical_blk_size);
      return Ok(DirectInfo::new(mem_align, logical_blk_size, physical_blk_size));
    }

    match current.parent() {
      Some(p) if p.file_name().is_some_and(|n| n.ne(BLOCK_PATH)) => current = p.to_path_buf(),
      _ => {
        return Err(io::Error::new(io::ErrorKind::Unsupported, "cannot find the physical block size information"))
      },
    }
  }
}
