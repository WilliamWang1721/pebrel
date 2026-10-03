use super::*;

#[cfg(windows)]
#[test]
#[ignore = "fork-only issue 349 probe requires both ordinary and elevated Windows tokens"]
fn issue349_real_token_update_boundary() {
    use sha2::{Digest as _, Sha256};
    const CHILD: &str = "PEBREL_349_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let executable = std::env::current_exe().unwrap();
        let root = tempfile::tempdir_in(executable.parent().unwrap()).unwrap();
        let child = root.path().join("instance.exe");
        std::fs::hard_link(executable, &child).unwrap();
        let config = root.path().join("config");
        std::fs::create_dir(&config).unwrap();
        let output = std::process::Command::new(child)
            .args([
                "--ignored",
                "--exact",
                "update_download::handoff::tests::issue349_real_token_update_boundary",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("PEBREL_CONFIG_DIR", &config)
            .env("NEBULA_CONFIG_DIR", &config)
            .output()
            .unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        return;
    }
    let expected = std::env::var("PEBREL_349_EXPECT_ELEVATED").unwrap() == "1";
    let elevated = crate::platform::elevation::is_elevated().unwrap();
    println!("actual TokenElevation={elevated}; expected={expected}");
    assert_eq!(elevated, expected);
    assert_eq!(crate::platform::elevation::requires_isolation(), expected);
    assert_eq!(
        crate::platform::distribution::current(),
        crate::platform::distribution::Distribution::Direct
    );
    let bytes = b"MZissue349 fixture; never executed";
    let name = "Pebrel-v99.0.0-windows-x64-setup.exe";
    let asset = UpdateAsset {
        version: "99.0.0".into(),
        name: name.into(),
        download_url: format!("https://github.com/Kuddev/pebrel/releases/download/v99.0.0/{name}"),
        size: Some(bytes.len() as u64),
        sha256: Some(Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()),
    };
    let config = nebula_settings::settings_dir();
    if elevated {
        assert!(schedule(&asset).unwrap_err().contains("ordinary Pebrel window"));
        let error = prepare(&asset).err().expect("elevated preparation must fail");
        assert!(error.contains("privileged sessions stay isolated"));
        assert!(
            !config.join("updates").exists(),
            "privileged entry points wrote ordinary update data"
        );
    } else {
        let (_, path) = super::super::download_paths(&asset).unwrap();
        std::fs::write(&path, bytes).unwrap();
        *super::super::session() = Some(super::super::DownloadSession {
            generation: 1,
            asset: asset.clone(),
            status: super::super::DownloadStatus::Ready { path, bytes: bytes.len() as u64 },
        });
        schedule(&asset).unwrap();
        let scheduled: UpdateAsset = read_json(&config.join("updates/install-next.json")).unwrap();
        assert_eq!(scheduled, asset);
        // The isolated copy has no uninstaller marker: preparation must reach
        // the installation check, rather than misclassifying this real token.
        let error = prepare(&asset).err().expect("portable fixture must not install");
        assert!(error.contains("This copy is portable"), "{error}");
        assert!(!config.join("updates/handoffs").exists());
    }
}

#[test]
fn update_restore_tickets_cover_success_rollback_and_acknowledgement() {
    // Run in a separate process: changing the settings environment in the shared
    // test runner would race unrelated tests and could touch real user settings.
    if std::env::var_os("PEBREL_RESTORE_TEST_CHILD").is_none() {
        let root = tempfile::tempdir().unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "update_download::handoff::tests::update_restore_tickets_cover_success_rollback_and_acknowledgement", "--nocapture"])
            .env("PEBREL_RESTORE_TEST_CHILD", "1")
            .env("PEBREL_CONFIG_DIR", root.path())
            .env("NEBULA_CONFIG_DIR", root.path())
            .env_remove("PEBREL_UPDATE_RESTORE")
            .output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let config = canonical(&nebula_settings::settings_dir()).unwrap();
    let directory = config.join("updates/handoffs/restore-test");
    std::fs::create_dir_all(&directory).unwrap();
    let executable = canonical(&std::env::current_exe().unwrap()).unwrap();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let mut plan = Plan {
        schema: 1,
        asset: UpdateAsset {
            version: version.clone(),
            name: "fixture".into(),
            download_url: String::new(),
            sha256: None,
            size: None,
        },
        transaction: "restore-test".into(),
        installation: executable.parent().unwrap().to_owned(),
        executable,
        config_directory: config.clone(),
        installer: config.join("fixture"),
        sha256: String::new(),
        bytes: 1,
        version: version.clone(),
        original_version: version,
        guard_path: config.join("guard"),
        participants: Vec::new(),
    };
    let save = |name: &str, value: serde_json::Value| {
        std::fs::write(directory.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    };
    std::fs::write(
        config.join("updates/last-handoff.json"),
        serde_json::to_vec(&directory.join("plan.json")).unwrap(),
    )
    .unwrap();
    save("workspace.json", serde_json::to_value(vec![Session::new(0, Vec::new())]).unwrap());
    for recovered in [false, true] {
        plan.version = if recovered { "99.0.0".into() } else { env!("CARGO_PKG_VERSION").into() };
        save("plan.json", serde_json::to_value(&plan).unwrap());
        save(
            "result.json",
            serde_json::json!({"transaction": "wrong", "success": !recovered, "recovered_original": recovered}),
        );
        assert!(restore_ticket().is_none());
        save(
            "result.json",
            serde_json::json!({"transaction": plan.transaction, "success": !recovered, "recovered_original": recovered}),
        );
        assert_eq!(restore_ticket().unwrap().len(), 1);
        acknowledge_restore(2);
        assert!(!directory.join("restored.json").exists());
        acknowledge_restore(1);
        assert!(directory.join("restored.json").exists());
        assert!(restore_ticket().is_none(), "a completed restore must not replay");
        std::fs::remove_file(directory.join("restored.json")).unwrap();
        save("restore-attempts.json", serde_json::json!(3));
        assert!(restore_ticket().is_none(), "crash-loop recovery is bounded");
        save("restore-attempts.json", serde_json::json!(0));
    }
    plan.original_version = "0.0.0".into();
    save("plan.json", serde_json::to_value(&plan).unwrap());
    assert!(restore_ticket().is_none(), "rollback must match the running original version");
}
