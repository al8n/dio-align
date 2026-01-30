use rustix::fs::statfs;

use std::{io, path::Path};

use super::DirectInfo;

/// Fetch direct I/O alignment information for a path.
pub fn fetch<P: AsRef<Path>>(path: P) -> io::Result<DirectInfo> {
  let fs = statfs(path.as_ref())?;
  let logical_size = fs.f_bsize as u32;
  let physical_size = fs.f_iosize as u32;

  Ok(DirectInfo::new(logical_size, physical_size))
}
