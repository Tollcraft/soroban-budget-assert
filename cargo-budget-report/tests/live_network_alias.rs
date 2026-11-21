use assert_cmd::Command;

fn budget_report_cmd() -> Command {
    Command::cargo_bin("cargo-budget-report").expect("binary should be built")
}

#[test]
fn test_all_supported_network_aliases() {
    let non_mainnet = [
        ("test", "https://soroban-testnet.stellar.org:443"),
        ("testnet", "https://soroban-testnet.stellar.org:443"),
        ("futurenet", "https://rpc-futurenet.stellar.org:443"),
        ("local", "http://localhost:8000/soroban/rpc"),
        ("standalone", "http://localhost:8000/soroban/rpc"),
    ];

    for (alias, expected_url) in non_mainnet {
        let output = budget_report_cmd()
            .args(["budget-report", "--network", alias, "--source", "alice"])
            .output()
            .expect("failed to run command");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("Using RPC endpoint {expected_url}")),
            "alias `{alias}` should resolve to `{expected_url}`, got:\n{stderr}"
        );
    }

    let mainnet_aliases = [
        ("mainnet", "https://soroban.stellar.org:443"),
        ("pubnet", "https://soroban.stellar.org:443"),
    ];

    for (alias, expected_url) in mainnet_aliases {
        let output = budget_report_cmd()
            .args([
                "budget-report",
                "--network",
                alias,
                "--source",
                "alice",
                "--allow-mainnet",
            ])
            .output()
            .expect("failed to run command");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("Using RPC endpoint {expected_url}")),
            "alias `{alias}` should resolve to `{expected_url}`, got:\n{stderr}"
        );
    }
}

#[test]
fn test_unknown_network_alias_error_message() {
    let output_unknown = budget_report_cmd()
        .args([
            "budget-report",
            "--network",
            "  TestNett  ",
            "--source",
            "alice",
            "--allow-mainnet",
        ])
        .output()
        .expect("failed to run command");

    let stderr = String::from_utf8_lossy(&output_unknown.stderr);
    assert!(
        stderr.contains(r#"error: no built-in RPC endpoint is configured for network alias "testnett"; pass --rpc-url and --network-passphrase"#),
        "unknown alias error mismatch, got:\n{stderr}"
    );
}
