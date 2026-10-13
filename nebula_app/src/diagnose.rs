//! `pebrel diagnose`: collect the environment details a bug report needs and
//! bundle them, redacted, into a ZIP that can be attached to an issue.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::{env, thread};

use clap::Args;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::platform::dirs;

/// Largest log tail kept per file; the end of a log is where the failure is.
const LOG_TAIL_BYTES: u64 = 512 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const REDACTED: &str = "<redacted>";
const SECRET_WORDS: &[&str] =
    &["password", "passwd", "token", "secret", "api_key", "apikey", "api-key", "authorization"];
/// Settings keys whose values identify machines or networks.
const PRIVATE_KEY_WORDS: &[&str] = &["host", "proxy", "url", "endpoint", "path", "dir"];
const AGENT_CLIS: &[&str] = &["claude", "codex", "gemini", "pi", "omp"];
const ENV_ALLOWLIST: &[&str] = &[
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "LANG",
    "LC_ALL",
    "SHELL",
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "GDK_SCALE",
    "WSL_DISTRO_NAME",
    "APPIMAGE",
    "PEBREL_CONFIG_DIR",
    "PEBREL_DEBUG_LOG",
];

#[derive(Args, Debug)]
pub struct Options {
    /// Directory for the diagnostics ZIP (default: current directory).
    #[clap(long, value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub output: Option<PathBuf>,
}

pub fn run(options: Options) -> i32 {
    let report = report();
    let directory = options.output.unwrap_or_else(|| PathBuf::from("."));
    let archive = match write_archive(directory, &report) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Unable to write the diagnostics archive: {error}");
            return 1;
        },
    };
    println!("{report}");
    println!("Diagnostics archive: {}", archive.display());
    println!("Paste the report above into the issue and drag the archive into the Logs field.");
    println!("Review both for private information before posting.");
    0
}

fn report() -> String {
    let mut out = String::new();
    section(&mut out, "Pebrel version", &format!("{} ({})", env!("VERSION"), build_flavor()));
    section(&mut out, "Operating system", &format!("{} {}", env::consts::OS, env::consts::ARCH));
    section(&mut out, "Install source", &install_source());
    section(&mut out, "Detected shells", &shells());
    section(&mut out, "Configuration", &config_status());
    section(&mut out, "Environment", &environment());
    section(&mut out, "AI agent CLIs", &agent_clis());
    for (title, program, args) in probes() {
        let output = run_command(program, args).unwrap_or_else(|| "unavailable".to_owned());
        out.push_str(&format!(
            "<details><summary>{title}</summary>\n\n```\n{output}\n```\n</details>\n\n"
        ));
    }
    out.push_str("## Non-default settings\n\n```\n");
    out.push_str(&settings());
    out.push_str("\n```\n");
    scrub(&out)
}

fn section(out: &mut String, title: &str, body: &str) {
    out.push_str(&format!("## {title}\n\n{body}\n\n"));
}

fn build_flavor() -> String {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let shell = if cfg!(feature = "legacy-shell") { "legacy-shell" } else { "gpui" };
    format!("{profile} build, {shell}")
}

fn install_source() -> String {
    let exe = env::current_exe().map(|path| path.display().to_string()).unwrap_or_default();
    format!("{} (`{exe}`)", classify_install(&exe, env::var_os("APPIMAGE").is_some()))
}

fn classify_install(exe: &str, appimage: bool) -> &'static str {
    let exe = exe.replace('\\', "/").to_lowercase();
    if appimage {
        "AppImage"
    } else if exe.contains("/target/debug/") || exe.contains("/target/release/") {
        "Built from source (cargo build)"
    } else if exe.contains("/windowsapps/") {
        "MSIX / Microsoft Store"
    } else if exe.contains("/scoop/") {
        "Scoop"
    } else if exe.contains("/homebrew/") || exe.contains("/caskroom/") || exe.contains("/cellar/") {
        "Homebrew"
    } else if exe.contains(".app/contents/macos/") {
        "macOS app bundle (.dmg)"
    } else if exe.contains("/program files") || exe.contains("/appdata/local/programs/") {
        "Windows installer"
    } else if exe.starts_with("/usr/") || exe.starts_with("/opt/") {
        "Linux package (.deb / system install)"
    } else {
        "Unknown (portable or custom location)"
    }
}

fn shells() -> String {
    let mut out = String::new();
    for shell in crate::shell_detect::detect_shells() {
        let version = shell_version(&shell.program).map(|text| format!(" — {text}"));
        let version = version.unwrap_or_default();
        out.push_str(&format!("- {} (`{}`, `{}`){version}\n", shell.name, shell.id, shell.program));
    }
    if out.is_empty() {
        return "none detected".to_owned();
    }
    out
}

/// `wsl.exe` and `cmd` have no `--version` that returns promptly.
fn shell_version(program: &str) -> Option<String> {
    let name = Path::new(program).file_stem()?.to_str()?.to_lowercase();
    if matches!(name.as_str(), "wsl" | "cmd") {
        return None;
    }
    run_command(program, &["--version"]).and_then(|text| text.lines().next().map(str::to_owned))
}

fn config_status() -> String {
    use crate::config::{load_source, source};

    match source::discover(None) {
        Ok(Some(found)) => match load_source(&found) {
            Ok(_) => format!("`{}` is valid", found.primary_path.display()),
            Err(error) => format!("`{}` failed to load: {error}", found.primary_path.display()),
        },
        Ok(None) => "no configuration file".to_owned(),
        Err(error) => format!("configuration discovery failed: {error}"),
    }
}

fn environment() -> String {
    ENV_ALLOWLIST
        .iter()
        .filter_map(|name| env::var(name).ok().map(|value| format!("- `{name}={value}`")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_clis() -> String {
    let finder = if cfg!(windows) { "where" } else { "which" };
    let mut out = String::new();
    for &name in AGENT_CLIS {
        let Some(found) = run_command(finder, &[name]) else {
            continue;
        };
        let path = found.lines().next().unwrap_or_default();
        let version = if cfg!(windows) {
            run_command("cmd", &["/c", name, "--version"])
        } else {
            run_command(path, &["--version"])
        };
        let version = version.and_then(|text| text.lines().next().map(str::to_owned));
        out.push_str(&format!("- {name}: {} `{path}`\n", version.unwrap_or_default()));
    }
    if out.is_empty() {
        return "none found on PATH".to_owned();
    }
    out
}

type Probe = (&'static str, &'static str, &'static [&'static str]);

fn probe(title: &'static str, program: &'static str, args: &'static [&'static str]) -> Probe {
    (title, program, args)
}

/// Exact OS version, hardware and display commands for the current platform.
fn probes() -> Vec<Probe> {
    #[cfg(target_os = "macos")]
    {
        vec![
            probe("System version", "sw_vers", &[]),
            probe("Kernel", "uname", &["-a"]),
            probe("CPU and memory", "sysctl", &["-n", "machdep.cpu.brand_string", "hw.memsize"]),
            probe("Graphics and displays", "system_profiler", &["SPDisplaysDataType"]),
        ]
    }
    #[cfg(windows)]
    {
        vec![
            probe("Windows version", "cmd", &["/c", "ver"]),
            probe("Windows release", "powershell", &POWERSHELL_RELEASE),
            probe("CPU and memory", "powershell", &POWERSHELL_HARDWARE),
            probe("Graphics", "powershell", &POWERSHELL_GRAPHICS),
            probe("Monitors", "powershell", &POWERSHELL_MONITORS),
            probe("DPI (96 = 100%, 144 = 150%)", "reg", &REG_DPI),
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        vec![
            probe("OS release", "cat", &["/etc/os-release"]),
            probe("Kernel", "uname", &["-a"]),
            probe("CPU", "lscpu", &[]),
            probe("Memory", "free", &["-h"]),
            probe("Graphics", "sh", &["-c", "lspci | grep -iE 'vga|3d|display'"]),
            probe("Displays", "xrandr", &["--query"]),
        ]
    }
}

#[cfg(windows)]
const REG_DPI: [&str; 4] =
    ["query", r"HKCU\Control Panel\Desktop\WindowMetrics", "/v", "AppliedDPI"];
#[cfg(windows)]
const POWERSHELL_RELEASE: [&str; 4] = [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    r"Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' | Select-Object ProductName,DisplayVersion,CurrentBuild,UBR | Format-List",
];
#[cfg(windows)]
const POWERSHELL_HARDWARE: [&str; 4] = [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "Get-CimInstance Win32_Processor | Select-Object Name | Format-List; Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory | Format-List",
];
#[cfg(windows)]
const POWERSHELL_GRAPHICS: [&str; 4] = [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate | Format-List",
];
#[cfg(windows)]
const POWERSHELL_MONITORS: [&str; 4] = [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::AllScreens | Select-Object DeviceName,Primary,Bounds | Format-List",
];

fn settings() -> String {
    let Ok(text) = fs::read_to_string(dirs::data_dir().join("pebrel_settings.txt")) else {
        return "no settings file (all defaults)".to_owned();
    };
    text.lines().map(redact_setting).collect::<Vec<_>>().join("\n")
}

fn redact_setting(line: &str) -> String {
    match line.split_once('=') {
        Some((key, _)) if is_private_key(key) => format!("{key}={REDACTED}"),
        _ => line.to_owned(),
    }
}

fn is_private_key(key: &str) -> bool {
    let key = key.to_lowercase();
    PRIVATE_KEY_WORDS.iter().chain(SECRET_WORDS).any(|word| key.contains(word))
}

/// Mask home directory, user name and secret-looking `name: value` lines.
fn scrub(text: &str) -> String {
    let home = dirs::home_dir().map(|path| path.display().to_string());
    let mut text = text.to_owned();
    if let Some(home) = home.filter(|home| home.len() > 1) {
        text = text.replace(&home, "~").replace(&home.replace('\\', "/"), "~");
    }
    let user = env::var("USER").or_else(|_| env::var("USERNAME")).unwrap_or_default();
    if user.len() >= 3 {
        text = text.replace(&user, "<user>");
    }
    text.lines().map(scrub_line).collect::<Vec<_>>().join("\n")
}

fn scrub_line(line: &str) -> String {
    let lower = line.to_lowercase();
    let secret = SECRET_WORDS.iter().any(|word| lower.contains(word));
    match line.find(['=', ':']) {
        Some(index) if secret => format!("{}{REDACTED}", &line[..=index]),
        _ => line.to_owned(),
    }
}

fn write_archive(directory: PathBuf, report: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(&directory)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = directory.join(format!("pebrel-diagnostics-{stamp}.zip"));
    let mut zip = ZipWriter::new(File::create(&path)?);
    let options = SimpleFileOptions::default();
    let mut add = |name: &str, content: &str| -> io::Result<()> {
        zip.start_file(name, options)?;
        zip.write_all(content.as_bytes())
    };
    add("report.md", report)?;
    add("settings.txt", &scrub(&settings()))?;
    for log in log_files() {
        if let Ok(tail) = read_tail(&log) {
            let name = log.file_name().unwrap_or_default().to_string_lossy();
            add(&format!("logs/{name}"), &scrub(&tail))?;
        }
    }
    zip.finish()?;
    Ok(path)
}

/// `pebrel_debug.log` plus the three newest per-process logs in the temp dir.
fn log_files() -> Vec<PathBuf> {
    let mut recent: Vec<_> = fs::read_dir(env::temp_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("Pebrel-") && name.ends_with(".log")
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    recent.sort_by(|a, b| b.0.cmp(&a.0));
    let mut files = vec![dirs::data_dir().join("pebrel_debug.log")];
    files.extend(recent.into_iter().take(3).map(|(_, path)| path));
    files.retain(|path| path.is_file());
    files
}

fn read_tail(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_BYTES)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Run a probe with a deadline so a wedged tool cannot hang the report.
fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let mut child = command.spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        match child.try_wait().ok()? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            },
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let text = String::from_utf8_lossy(&reader.join().ok()?).trim().to_owned();
    (status.success() && !text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_source_is_classified_from_the_executable_path() {
        let cases = [
            ("/Applications/Pebrel.app/Contents/MacOS/pebrel", false, "macOS app bundle (.dmg)"),
            (r"C:\Program Files\Pebrel\pebrel.exe", false, "Windows installer"),
            ("/home/me/pebrel/target/release/pebrel", false, "Built from source (cargo build)"),
            ("/tmp/.mount_x/usr/bin/pebrel", true, "AppImage"),
        ];
        for (exe, appimage, expected) in cases {
            assert_eq!(classify_install(exe, appimage), expected);
        }
    }

    #[test]
    fn secrets_and_private_settings_are_redacted() {
        assert_eq!(scrub_line("api_key = sk-123"), "api_key =<redacted>");
        assert_eq!(redact_setting("ssh_proxy_url=socks5://h:1"), "ssh_proxy_url=<redacted>");
        assert_eq!(redact_setting("keybind=ctrl+a:Copy"), "keybind=ctrl+a:Copy");
    }
}
