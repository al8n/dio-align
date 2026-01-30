use rustix::fs::{
  AtFlags, Mode, OFlags, StatxFlags, ioctl_blkpbszget, ioctl_blksszget, open, statx,
};
use smol_str::format_smolstr;

use std::{io, path::Path};

use super::{
  super::DirectInfo, ABS_BLOCK_PATH, BLOCK_PATH, PHYSICAL_BLOCK_SIZE_FILE_NAME, QUEUE_PATH,
  read_block_size,
};

const LOGICAL_BLOCK_SIZE_FILE_NAME: &str = "logical_block_size";

/// Fetch direct I/O alignment information for a path.
pub fn fetch<P: AsRef<Path>>(path: P) -> io::Result<DirectInfo> {
  let path = path.as_ref();
  let fd = open(path.as_os_str(), OFlags::RDONLY, Mode::empty())?;
  match ioctl_blksszget(&fd) {
    Ok(logical_blk_size) => {
      let physical_blk_size = ioctl_blkpbszget(&fd).unwrap_or(logical_blk_size);
      Ok(DirectInfo::new(logical_blk_size, physical_blk_size))
    }
    Err(_) => {
      let abs = path.canonicalize()?;
      let parent_dir = abs.parent().ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::InvalidInput,
          format!("path '{}' has no parent directory", abs.display()),
        )
      })?;

      let parent_fd = open(parent_dir, OFlags::RDONLY, Mode::empty())?;

      let file_name = abs.file_name().ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::InvalidInput,
          format!("path '{}' has no file name", abs.display()),
        )
      })?;

      let stx = statx(
        parent_fd,
        file_name,
        AtFlags::empty(),
        StatxFlags::BASIC_STATS,
      )?;
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
        let logical_blk_path = queue.join(LOGICAL_BLOCK_SIZE_FILE_NAME);
        if logical_blk_path.exists() {
          let logical_blk_size = read_block_size(&logical_blk_path)?;

          let physical_blk_path = queue.join(PHYSICAL_BLOCK_SIZE_FILE_NAME);
          let physical_blk_size = read_block_size(&physical_blk_path).unwrap_or(logical_blk_size);
          return Ok(DirectInfo::new(logical_blk_size, physical_blk_size));
        }

        match current.parent() {
          Some(p) if p.file_name().is_some_and(|n| n.ne(BLOCK_PATH)) => current = p.to_path_buf(),
          _ => {
            return Err(io::Error::new(
              io::ErrorKind::InvalidInput,
              "cannot find the logical and physical block size information",
            ));
          }
        }
      }
    }
  }
}
