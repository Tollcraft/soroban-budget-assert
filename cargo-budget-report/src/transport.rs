use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub trait Transport {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        package_name: &str,
    ) -> Result<String>;

    fn build_invoke_xdr(
        &mut self,
        contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        func_args: &[String],
        package: &str,
    ) -> Result<String>;

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value>;
}

/// Join `parts` with `:` into a single, exactly-sized `String`.
///
/// Fixture keys are built on every recorded or replayed call; sizing the
/// buffer up front avoids the incremental regrowth `format!` does.
fn fixture_key(parts: &[&str]) -> String {
    let len = parts.iter().map(|p| p.len()).sum::<usize>() + parts.len().saturating_sub(1);
    let mut key = String::with_capacity(len);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            key.push(':');
        }
        key.push_str(part);
    }
    key
}

/// Fixture key for a [`Transport::deploy_contract`] response.
pub fn deploy_key(package: &str) -> String {
    fixture_key(&["deploy", package])
}

/// Fixture key for a [`Transport::build_invoke_xdr`] response.
pub fn invoke_key(package: &str, function: &str) -> String {
    fixture_key(&["invoke", package, function])
}

/// Fixture key for a [`Transport::simulate_transaction`] response.
pub fn simulate_key(package: &str, function: &str) -> String {
    fixture_key(&["simulate", package, function])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_the_fixture_format() {
        assert_eq!(deploy_key("pkg"), "deploy:pkg");
        assert_eq!(invoke_key("pkg", "do_work"), "invoke:pkg:do_work");
        assert_eq!(simulate_key("pkg", "do_work"), "simulate:pkg:do_work");
    }

    #[test]
    fn keys_with_empty_parts_keep_their_separators() {
        assert_eq!(deploy_key(""), "deploy:");
        assert_eq!(invoke_key("", ""), "invoke::");
        assert_eq!(simulate_key("pkg", ""), "simulate:pkg:");
    }

    #[test]
    fn keys_are_allocated_at_exact_capacity() {
        for key in [
            deploy_key("pkg"),
            invoke_key("päckäge", "fünctiön"),
            simulate_key("", ""),
        ] {
            assert_eq!(key.capacity(), key.len(), "{key}");
        }
    }

    #[test]
    fn fixture_key_handles_zero_and_one_part() {
        assert_eq!(fixture_key(&[]), "");
        assert_eq!(fixture_key(&["only"]), "only");
    }
}
