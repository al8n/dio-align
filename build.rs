use std::{
  env::{self, var},
  process::Command,
};

fn main() {
  // Don't rerun this on changes other than build.rs, as we only depend on
  // the rustc version.
  println!("cargo:rerun-if-changed=build.rs");

  // Check for `--features=tarpaulin`.
  let tarpaulin = var("CARGO_FEATURE_TARPAULIN").is_ok();

  if tarpaulin {
    use_feature("tarpaulin");
  } else {
    // Always rerun if these env vars change.
    println!("cargo:rerun-if-env-changed=CARGO_TARPAULIN");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARPAULIN");

    // Detect tarpaulin by environment variable
    if env::var("CARGO_TARPAULIN").is_ok() || env::var("CARGO_CFG_TARPAULIN").is_ok() {
      use_feature("tarpaulin");
    }
  }

  // attempt os
  match env::var("CARGO_CFG_TARGET_OS").as_deref() {
    Ok("linux") => check_kernel(),
    Ok("macos" | "ios" | "tvos" | "watchos" | "visionos") => use_feature("apple"),
    _ => {}
  }

  // Rerun this script if any of our features or configuration flags change,
  // or if the toolchain we used for feature detection changes.
  println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TARPAULIN");
}

fn use_feature(feature: &str) {
  println!("cargo:rustc-cfg={}", feature);
}

fn check_kernel() {
  // fetch the version of the kernel
  let output = Command::new("uname")
    .arg("-r")
    .output()
    .expect("Failed to execute uname");

  let kernel_version = core::str::from_utf8(&output.stdout).expect("Invalid UTF-8 output");

  if let Some(version) = kernel_version.split('-').next() {
    let version_parts: Vec<&str> = version.split('.').collect();
    if version_parts.len() >= 2 {
      let major_version: u32 = version_parts[0].parse().unwrap_or(0);
      let minor_version: u32 = version_parts[1].parse().unwrap_or(0);

      if major_version > 6 || (major_version == 6 && minor_version >= 1) {
        use_feature("linux_kernel_6_1");
      }
    }
  }
}
