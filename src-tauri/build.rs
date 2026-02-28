fn main() {
  println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

  if let Ok(output) = std::process::Command::new("xcrun")
    .args(["--toolchain", "default", "--find", "swift"])
    .output()
  {
    let swift_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(toolchain) = std::path::Path::new(&swift_path)
      .parent()
      .and_then(|path| path.parent())
    {
      let lib_path = toolchain.join("lib/swift/macosx");
      if lib_path.exists() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
      }
    }
  }

  tauri_build::build()
}
