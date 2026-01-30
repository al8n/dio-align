//! A template for creating Rust open-source repo on GitHub
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![deny(missing_docs)]

// Linux kernel version less than 6.1
#[path = "linux/mod.rs"]
#[cfg(target_os = "linux")]
mod os;

#[path = "apple.rs"]
#[cfg(any(
  target_os = "macos",
  target_os = "ios",
  target_os = "tvos",
  target_os = "watchos",
  target_os = "visionos",
))]
mod os;

#[path = "windows.rs"]
#[cfg(windows)]
mod os;

#[cfg(any(
  target_os = "linux",
  target_os = "macos",
  target_os = "ios",
  target_os = "tvos",
  target_os = "watchos",
  target_os = "visionos",
  windows,
))]
pub use os::*;

/// The information of the direct I/O
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct DirectInfo {
  #[cfg(linux_kernel_6_1)]
  mem_align: u32,
  logical: u32,
  physical: u32,
}

impl DirectInfo {
  #[inline]
  const fn new(
    #[cfg(linux_kernel_6_1)]
    mem_align: u32,
    logical: u32,
    physical: u32,
  ) -> Self {
    Self {
      #[cfg(linux_kernel_6_1)]
      mem_align,
      logical,
      physical,
    }
  }

  /// Returns the memory alignment
  /// 
  /// This is only available on Linux with kernel version `>= 6.1`
  #[cfg_attr(not(tarpaulin), inline(always))]
  #[cfg(linux_kernel_6_1)]
  #[cfg_attr(docsrs, doc(cfg(linux_kernel_6_1)))]
  pub const fn mem_align(&self) -> u32 {
    self.mem_align
  }

  /// Returns the logical block size
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn logical_block_size(&self) -> u32 {
    self.logical
  }

  /// Returns the physical block size
  #[cfg_attr(not(tarpaulin), inline(always))]
  pub const fn physical_block_size(&self) -> u32 {
    self.physical
  }
}


#[test]
fn test_direct_info() {
  use tempfile::NamedTempFile;

  let file = NamedTempFile::new().unwrap();
  let info = fetch(file.path()).unwrap();
  println!(
    "logical: {}, physical: {}",
    info.logical_block_size(),
    info.physical_block_size()
  );
  assert!(info.logical_block_size().is_power_of_two());
  assert!(info.physical_block_size().is_power_of_two());

  #[cfg(linux_kernel_6_1)]
  {
    println!("mem align: {}", info.mem_align());
    assert!(info.mem_align().is_power_of_two());
  }
}
