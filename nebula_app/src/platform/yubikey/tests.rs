use super::*;
use std::ffi::{OsStr, OsString};

const KEY_A: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8g";
const KEY_B: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICAfHh0cGxoZGBcWFRQTEhEQDw4NDAsKCQgHBgUEAwIB";

fn tools(root: &Path) -> PivTools {
    PivTools {
        ykman: root.join("ykman"),
        ssh_keygen: root.join("ssh-keygen"),
        ssh_add: root.join("ssh-add"),
        provider: root.join("a space;not-a-shell").join("libykcs11.so"),
    }
}

#[test]
fn command_arguments_and_agent_identity_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let tools = tools(directory.path());
    let agent = AgentContext { socket: directory.path().join("agent socket") };
    let exported = tools.export_command();
    assert_eq!(exported.get_program(), tools.ssh_keygen.as_os_str());
    assert_eq!(
        exported.get_args().collect::<Vec<_>>(),
        vec![OsStr::new("-D"), tools.provider.as_os_str()]
    );
    for load in [true, false] {
        let command = tools.agent_command(&agent, load);
        assert_eq!(command.get_program(), tools.ssh_add.as_os_str());
        let args = command.get_args().collect::<Vec<_>>();
        if load {
            assert_eq!(args, vec![OsStr::new("-s"), tools.provider.as_os_str()]);
        } else {
            assert_eq!(args, vec![OsStr::new("-L")]);
        }
        let env = command.get_envs().collect::<Vec<_>>();
        assert!(env.contains(&(OsStr::new("SSH_AUTH_SOCK"), Some(agent.socket().as_os_str()))));
        assert!(env.contains(&(OsStr::new("SSH_ASKPASS_REQUIRE"), Some(OsStr::new("never")))));
        assert!(!args.iter().any(|arg| *arg == OsStr::new("--pin")));
    }
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(matches!(
            PivTools::from_paths("ykman".into(), "keygen".into(), "add".into(), "module".into()),
            Err(PivError::InvalidPath(Tool::Ykman))
        ));
    } else {
        assert!(matches!(PivTools::discover(), Err(PivError::UnsupportedPlatform)));
    }
}

#[test]
fn device_and_key_output_never_invents_slots_or_silently_discards_errors() {
    assert_eq!(parse_single_serial("12345678\r\n").unwrap(), "12345678");
    assert!(matches!(parse_single_serial(""), Err(PivError::NoDevice)));
    assert!(matches!(parse_single_serial("one"), Err(PivError::InvalidOutput)));
    assert!(matches!(parse_single_serial("123\n456\n"), Err(PivError::MultipleDevices)));
    let output = format!("{KEY_A} first comment\r\n{KEY_B} another\n{KEY_A} changed comment\n");
    assert_eq!(parse_public_keys(&output).unwrap(), vec![KEY_A.to_owned(), KEY_B.to_owned()]);
    assert!(matches!(parse_public_keys("\n"), Err(PivError::NoPublicKeys)));
    assert!(matches!(parse_public_keys("not a key"), Err(PivError::InvalidOutput)));
    assert!(matches!(
        parse_public_keys(&format!("{KEY_A}\nssh-ed25519 invalid-base64\n")),
        Err(PivError::InvalidOutput)
    ));
}

fn python(script: &str, args: &[OsString]) -> Command {
    // setup-python in the existing native workflow supplies this interpreter.
    let mut command = Command::new("python");
    command.args(["-c", script]).args(args);
    command
}

#[test]
fn probes_bound_output_failures_timeouts_and_cancelled_child_lifetimes() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        assert_eq!(
            capture(python("print('ready')", &[]), PROBE_TIMEOUT).await.unwrap().trim(),
            "ready"
        );
        assert!(matches!(
            capture(python("import sys; sys.exit(7)", &[]), PROBE_TIMEOUT).await,
            Err(PivError::CommandFailed(Some(7)))
        ));
        assert!(matches!(
            capture(python("print('x' * 70000)", &[]), PROBE_TIMEOUT).await,
            Err(PivError::OutputLimit)
        ));
        assert!(matches!(
            capture(python("import time; time.sleep(30)", &[]), Duration::from_millis(100)).await,
            Err(PivError::Timeout)
        ));

        let directory = tempfile::tempdir().unwrap();
        let ready = directory.path().join("ready");
        let escaped = directory.path().join("escaped");
        let command = python(
            "import pathlib,sys,time; pathlib.Path(sys.argv[1]).write_text('ready'); time.sleep(3); pathlib.Path(sys.argv[2]).write_text('escaped')",
            &[ready.as_os_str().to_owned(), escaped.as_os_str().to_owned()],
        );
        let task = tokio::spawn(capture(command, PROBE_TIMEOUT));
        let deadline = tokio::time::Instant::now() + PROBE_TIMEOUT;
        while !ready.exists() {
            assert!(tokio::time::Instant::now() < deadline, "probe did not start");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::sleep(Duration::from_millis(3200)).await;
        assert!(!escaped.exists(), "cancelled probe kept running");

        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            let tools = tools(directory.path());
            let agent = AgentContext { socket: directory.path().join("unused") };
            assert!(matches!(
                tools.activate_in_terminal(&agent).await,
                Err(PivError::TerminalRequired)
            ));
        }
    });
}
