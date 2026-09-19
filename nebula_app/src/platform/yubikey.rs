#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::io;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;

#[cfg(target_os = "macos")]
const DEFAULT_PROVIDERS: &[&str] =
    &["/opt/homebrew/lib/libykcs11.dylib", "/usr/local/lib/libykcs11.dylib"];

#[cfg(target_os = "linux")]
const DEFAULT_PROVIDERS: &[&str] = &[
    "/usr/local/lib/libykcs11.so",
    "/usr/lib/x86_64-linux-gnu/libykcs11.so",
    "/usr/lib/aarch64-linux-gnu/libykcs11.so",
    "/usr/lib/libykcs11.so",
    "/usr/lib64/libykcs11.so",
];

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn activate(provider: Option<&Path>) -> io::Result<()> {
    let provider = provider
        .map(PathBuf::from)
        .or_else(|| DEFAULT_PROVIDERS.iter().map(PathBuf::from).find(|path| path.is_file()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "libykcs11 was not found; pass its path with --provider",
            )
        })?;

    let status = Command::new("ssh-add").arg("-s").arg(provider).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ssh-add exited with {status}")))
    }
}
