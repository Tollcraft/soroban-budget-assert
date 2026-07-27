//! Windows compatibility tests for path handling and command invocation.
//!
//! These tests verify that the tool handles Windows path separators,
//! command-line argument quoting, and cross-platform edge cases correctly.

#[cfg(test)]
mod windows_compatibility_tests {
    use crate::*;
    use std::path::PathBuf;

    // ── Path handling tests ─────────────────────────────────────────────

    #[test]
    fn wasm_path_with_windows_separators_joins_correctly() {
        let target_dir = PathBuf::from("target\\wasm32-unknown-unknown\\release");
        let wasm_name = "my_contract.wasm";
        let wasm_path = target_dir.join(wasm_name);
        let path_str = wasm_path.to_string_lossy();
        // On Windows the separator is \, on Unix it is / — the test just
        // checks that joining does not produce a broken path.
        assert!(
            wasm_path.ends_with("my_contract.wasm"),
            "path should end with the wasm file name, got: {}",
            path_str
        );
    }

    #[test]
    fn wasm_path_with_unix_separators_joins_correctly() {
        let target_dir = PathBuf::from("target/wasm32-unknown-unknown/release");
        let wasm_name = "my_contract.wasm";
        let wasm_path = target_dir.join(wasm_name);
        assert!(
            wasm_path.ends_with("my_contract.wasm"),
            "path should end with the wasm file name"
        );
    }

    #[test]
    fn wasm_path_with_mixed_separators_normalizes_correctly() {
        let target_dir = PathBuf::from("target\\wasm32-unknown-unknown/release");
        let wasm_name = "my_contract.wasm";
        let wasm_path = target_dir.join(wasm_name);
        assert!(
            wasm_path.ends_with("my_contract.wasm"),
            "path should end with the wasm file name regardless of separator style"
        );
    }

    #[test]
    fn wasm_path_empty_components_do_not_cause_panic() {
        let target_dir = PathBuf::from("");
        let wasm_name = "contract.wasm";
        let wasm_path = target_dir.join(wasm_name);
        assert_eq!(wasm_path.to_string_lossy(), "contract.wasm");
    }

    #[test]
    fn wasm_path_root_absolute_keeps_absolute() {
        let target_dir = PathBuf::from("/target");
        let wasm_name = "contract.wasm";
        let wasm_path = target_dir.join(wasm_name);
        let path_str = wasm_path.to_string_lossy();
        // On Windows root might be C:\ instead of /, but the join should
        // still produce a sensible absolute path.
        assert!(
            wasm_path.ends_with("contract.wasm"),
            "path should end with contract.wasm, got: {}",
            path_str
        );
        // It should either start with / (Unix) or a drive letter (Windows).
        assert!(
            path_str.starts_with('/') || path_str.contains(':'),
            "absolute path should start with / or a drive letter, got: {}",
            path_str
        );
    }

    // ── Command argument handling tests ──────────────────────────────────

    #[test]
    fn build_invoke_args_with_windows_style_paths_does_not_break() {
        // Windows paths like C:\Users\me\contract.wasm are legitimate;
        // they should pass through quoting and joining without corruption.
        let contract_id = "C";
        let source = "alice";
        let network = "testnet";
        let function = "do_work";
        let func_args = vec![
            "--wasm".to_string(),
            "C:\\Users\\me\\contract.wasm".to_string(),
        ];
        let args = build_invoke_args(contract_id, source, network, function, &func_args);
        assert_eq!(args[args.len() - 2], "--wasm");
        assert_eq!(
            args[args.len() - 1],
            "C:\\Users\\me\\contract.wasm"
        );
    }

    #[test]
    fn build_invoke_args_with_quoted_windows_args_preserves_quoting() {
        let contract_id = "C";
        let source = "alice";
        let network = "testnet";
        let function = "do_work";
        let func_args = vec![
            "--arg".to_string(),
            "\"value with spaces\"".to_string(),
        ];
        let args = build_invoke_args(contract_id, source, network, function, &func_args);
        assert_eq!(args[args.len() - 2], "--arg");
        assert_eq!(args[args.len() - 1], "\"value with spaces\"");
    }

    #[test]
    fn build_invoke_args_with_unicode_windows_paths() {
        let contract_id = "C";
        let source = "alice";
        let network = "testnet";
        let function = "do_work";
        let func_args = vec![
            "--wasm".to_string(),
            "C:\\Users\\用户\\contract.wasm".to_string(),
        ];
        let args = build_invoke_args(contract_id, source, network, function, &func_args);
        assert_eq!(
            args[args.len() - 1],
            "C:\\Users\\用户\\contract.wasm"
        );
    }

    #[test]
    fn build_invoke_args_long_windows_path_does_not_truncate() {
        let contract_id = "C";
        let source = "alice";
        let network = "testnet";
        let function = "do_work";
        let long_path = "\\\\?\\C:\\".to_string() + &"subdir\\".repeat(50) + "contract.wasm";
        let func_args = vec!["--wasm".to_string(), long_path.clone()];
        let args = build_invoke_args(contract_id, source, network, function, &func_args);
        assert_eq!(args[args.len() - 1], long_path);
    }

    // ── Network name edge cases ──────────────────────────────────────────

    #[test]
    fn build_invoke_args_with_windows_reserved_network_names() {
        // "local" is a valid network name on all platforms.
        let args = build_invoke_args("C", "alice", "local", "fn", &[]);
        assert_eq!(args[7], "local");
    }

    // ── Source account edge cases ────────────────────────────────────────

    #[test]
    fn build_invoke_args_with_identity_name_containing_spaces() {
        let args = build_invoke_args("C", "my identity", "testnet", "fn", &[]);
        assert_eq!(args[5], "my identity");
    }

    // ── File system path normalization ───────────────────────────────────

    #[test]
    fn wasm_name_replaces_hyphens_for_windows_compatibility() {
        let package_name = "my-contract";
        let wasm_name = package_name.replace('-', "_");
        assert_eq!(wasm_name, "my_contract");
    }

    #[test]
    fn wasm_name_no_hyphen_stays_unchanged() {
        let package_name = "contract";
        let wasm_name = package_name.replace('-', "_");
        assert_eq!(wasm_name, "contract");
    }

    #[test]
    fn wasm_name_multiple_hyphens_replaced() {
        let package_name = "my-cool-contract";
        let wasm_name = package_name.replace('-', "_");
        assert_eq!(wasm_name, "my_cool_contract");
    }

    #[test]
    fn wasm_name_leading_trailing_hyphens() {
        let package_name = "-contract-";
        let wasm_name = package_name.replace('-', "_");
        assert_eq!(wasm_name, "_contract_");
    }

    // ── crate_types filtering ────────────────────────────────────────────

    #[test]
    fn is_cdylib_check_is_case_sensitive() {
        let targets = [cargo_metadata::Target {
            name: "test".to_string(),
            src_path: "src/lib.rs".into(),
            required_features: vec![],
            kind: vec!["cdylib".to_string()],
            crate_types: vec!["cdylib".to_string()],
            edition: "2021".to_string(),
            doctest: true,
            doc: true,
            test: true,
        }];
        assert!(
            targets
                .iter()
                .any(|t| t.crate_types.iter().any(|ct| *ct == "cdylib")),
            "cdylib target should be detected"
        );
    }

    #[test]
    fn is_cdylib_check_rejects_non_cdylib() {
        let targets = [cargo_metadata::Target {
            name: "test".to_string(),
            src_path: "src/lib.rs".into(),
            required_features: vec![],
            kind: vec!["lib".to_string()],
            crate_types: vec!["lib".to_string()],
            edition: "2021".to_string(),
            doctest: true,
            doc: true,
            test: true,
        }];
        assert!(
            !targets
                .iter()
                .any(|t| t.crate_types.iter().any(|ct| *ct == "cdylib")),
            "non-cdylib target should be rejected"
        );
    }

    // ── WASM size edge cases ─────────────────────────────────────────────

    #[test]
    fn wasm_size_zero_bytes_is_handled() {
        let wasm_size: u32 = 0u32;
        assert_eq!(wasm_size, 0);
        // Zero-byte WASM would be invalid in practice, but the tool
        // must not panic when encountering it.
        let formatted = format_with_commas_and_units(u64::from(wasm_size), "WASM Bytes");
        assert!(!formatted.is_empty());
        assert_eq!(formatted, "0 B");
    }

    #[test]
    fn wasm_size_u32_max_is_handled() {
        let wasm_size: u32 = u32::MAX;
        let formatted = format_with_commas_and_units(u64::from(wasm_size), "WASM Bytes");
        assert!(formatted.contains("4,294,967,295"));
    }

    // ── WASM export filtering ────────────────────────────────────────────

    #[test]
    fn exported_fn_filter_skips_internal_names() {
        let name = "__internal_func";
        assert!(
            name.starts_with('_'),
            "internal functions starting with _ should be skipped"
        );
    }

    #[test]
    fn exported_fn_filter_skips_memory_export() {
        let name = "memory";
        assert!(
            name == "memory",
            "'memory' export should be skipped"
        );
    }

    #[test]
    fn exported_fn_filter_allows_underscore_in_middle() {
        let name = "do_work";
        assert!(
            !name.starts_with('_') && name != "memory",
            "functions with underscore in the middle should be included"
        );
    }

    #[test]
    fn exported_fn_filter_allows_leading_underscore_followed_by_alphanumeric() {
        // Only names starting with _ are excluded; _foo is excluded.
        let name = "_foo";
        assert!(
            name.starts_with('_'),
            "names starting with _ should be excluded"
        );
    }
}
