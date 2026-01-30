use rustix::{fs::{open, OFlags, Mode}, io::read};

use std::{io, path::Path};

pub use impl_::*;

// Linux kernel version greater than 6.1
#[path = "v6.1.rs"]
#[cfg(linux_kernel_6_1)]
mod impl_;

// Linux kernel version less than 6.1
#[path = "old.rs"]
#[cfg(not(linux_kernel_6_1))]
mod impl_;

const PHYSICAL_BLOCK_SIZE_FILE_NAME: &str = "physical_block_size";
const QUEUE_PATH: &str = "queue";
const BLOCK_PATH: &str = "block";
const ABS_BLOCK_PATH: &str = "/sys/dev/block";

#[inline]
fn read_block_size<P: AsRef<Path>>(path: P) -> io::Result<u32> {
  let fd = open(path.as_ref().as_os_str(), OFlags::RDONLY, Mode::empty())?;
  let mut buf = [0; 64];
  let num = read(&fd, &mut buf)?;
  let size = core::str::from_utf8(&buf[..num]).map_err(|e| {
    io::Error::new(io::ErrorKind::InvalidData, e)
  })?;
  size.trim().parse().map_err(|e| {
    io::Error::new(io::ErrorKind::InvalidData, e)
  })
}
