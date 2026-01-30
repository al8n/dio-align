use std::{
  ffi::{c_void, OsStr},
  io,
  mem,
  os::windows::ffi::OsStrExt,
  path::Path,
};

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, HANDLE};
use windows::Win32::Storage::FileSystem::{
  CreateFileW, GetDiskFreeSpaceW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
  FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
  OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
  IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, StorageAccessAlignmentProperty,
  STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
};

use super::DirectInfo;

struct Handle(HANDLE);

impl Drop for Handle {
  fn drop(&mut self) {
    unsafe {
      let _ = CloseHandle(self.0);
    }
  }
}

#[inline]
fn wide_len(buf: &[u16]) -> usize {
  buf.iter().position(|&c| c == 0).unwrap_or(buf.len())
}

#[inline]
fn to_wide(input: &OsStr) -> Vec<u16> {
  let mut wide: Vec<u16> = input.encode_wide().collect();
  wide.push(0);
  wide
}

fn get_volume_path(path: &Path) -> io::Result<Vec<u16>> {
  let path_wide = to_wide(path.as_os_str());
  let mut buffer = vec![0u16; 260];

  loop {
    let ok = unsafe {
      GetVolumePathNameW(
        PCWSTR(path_wide.as_ptr()),
        PWSTR(buffer.as_mut_ptr()),
        buffer.len() as u32,
      )
    }
    .as_bool();

    if ok {
      let len = wide_len(&buffer);
      buffer.truncate(len);
      buffer.push(0);
      return Ok(buffer);
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_MORE_DATA.0 as i32) {
      buffer.resize(buffer.len() * 2, 0);
      continue;
    }

    return Err(err);
  }
}

fn get_volume_name(volume_path: &[u16]) -> io::Result<Vec<u16>> {
  let mut buffer = vec![0u16; 260];

  loop {
    let ok = unsafe {
      GetVolumeNameForVolumeMountPointW(
        PCWSTR(volume_path.as_ptr()),
        PWSTR(buffer.as_mut_ptr()),
        buffer.len() as u32,
      )
    }
    .as_bool();

    if ok {
      let len = wide_len(&buffer);
      buffer.truncate(len);
      buffer.push(0);
      return Ok(buffer);
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(ERROR_MORE_DATA.0 as i32) {
      buffer.resize(buffer.len() * 2, 0);
      continue;
    }

    return Err(err);
  }
}

fn get_logical_block_size(volume_path: &[u16]) -> io::Result<u32> {
  let mut sectors_per_cluster = 0u32;
  let mut bytes_per_sector = 0u32;
  let mut free_clusters = 0u32;
  let mut total_clusters = 0u32;

  let ok = unsafe {
    GetDiskFreeSpaceW(
      PCWSTR(volume_path.as_ptr()),
      &mut sectors_per_cluster,
      &mut bytes_per_sector,
      &mut free_clusters,
      &mut total_clusters,
    )
  }
  .as_bool();

  if ok {
    Ok(bytes_per_sector)
  } else {
    Err(io::Error::last_os_error())
  }
}

fn volume_device_path(volume_name: &[u16]) -> Option<Vec<u16>> {
  let len = wide_len(volume_name);
  let mut name = String::from_utf16_lossy(&volume_name[..len]);
  name = name.trim_end_matches('\\').to_string();

  let device = if let Some(rest) = name.strip_prefix(r"\\?\") {
    format!(r"\\.\{}", rest)
  } else if name.starts_with(r"\\.\") {
    name
  } else if name.len() >= 2 && name.as_bytes().get(1) == Some(&b':') {
    format!(r"\\.\{}", &name[..2])
  } else {
    return None;
  };

  Some(to_wide(OsStr::new(&device)))
}

fn open_volume(device_path: &[u16]) -> io::Result<Handle> {
  let handle = unsafe {
    CreateFileW(
      PCWSTR(device_path.as_ptr()),
      FILE_GENERIC_READ,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
      None,
      OPEN_EXISTING,
      FILE_ATTRIBUTE_NORMAL,
      None,
    )
  }
  .map_err(|_| io::Error::last_os_error())?;

  Ok(Handle(handle))
}

fn query_alignment(handle: HANDLE) -> io::Result<STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR> {
  let query = STORAGE_PROPERTY_QUERY {
    PropertyId: StorageAccessAlignmentProperty,
    QueryType: PropertyStandardQuery,
    AdditionalParameters: [0],
  };

  let mut desc: STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR = unsafe { mem::zeroed() };
  let mut bytes_returned = 0u32;

  let ok = unsafe {
    DeviceIoControl(
      handle,
      IOCTL_STORAGE_QUERY_PROPERTY,
      Some(&query as *const _ as *const c_void),
      mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
      Some(&mut desc as *mut _ as *mut c_void),
      mem::size_of::<STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR>() as u32,
      Some(&mut bytes_returned),
      None,
    )
  }
  .as_bool();

  if ok {
    Ok(desc)
  } else {
    Err(io::Error::last_os_error())
  }
}

fn get_physical_block_size(volume_path: &[u16]) -> io::Result<Option<u32>> {
  let volume_name = match get_volume_name(volume_path) {
    Ok(name) => name,
    Err(_) => return Ok(None),
  };

  let device_path = match volume_device_path(&volume_name) {
    Some(path) => path,
    None => return Ok(None),
  };

  let handle = match open_volume(&device_path) {
    Ok(handle) => handle,
    Err(_) => return Ok(None),
  };

  let desc = match query_alignment(handle.0) {
    Ok(desc) => desc,
    Err(_) => return Ok(None),
  };

  if desc.BytesPerPhysicalSector == 0 {
    return Ok(None);
  }

  Ok(Some(desc.BytesPerPhysicalSector))
}

/// Fetch direct I/O information
pub fn fetch<P: AsRef<Path>>(path: P) -> io::Result<DirectInfo> {
  let volume_path = get_volume_path(path.as_ref())?;
  let logical_size = get_logical_block_size(&volume_path)?;
  let physical_size = get_physical_block_size(&volume_path)?.unwrap_or(logical_size);

  Ok(DirectInfo::new(logical_size, physical_size))
}
