#![allow(dead_code)]

fn validate_in_list(
    value: &str,
    list: &[&str],
    label: &str,
    is_allow_list: bool,
) -> Result<(), String> {
    let found = list.contains(&value);
    match (found, is_allow_list) {
        (true, true) | (false, false) => Ok(()),
        (false, true) => Err(format!(
            "unsupported {} '{}': expected one of {:?}",
            label, value, list
        )),
        (true, false) => Err(format!(
            "invalid {} '{}': must not be one of {:?}",
            label, value, list
        )),
    }
}

pub fn validate_network(network: &str) -> Result<(), String> {
    validate_in_list(network, &["testnet", "futurenet", "local"], "network", true)
}

pub fn validate_function_name(name: &str) -> Result<(), String> {
    validate_in_list(
        name,
        &["memory", "_constructor", "_init"],
        "function name",
        false,
    )
}

pub fn validate_source(source: &str) -> Result<(), String> {
    validate_in_list(
        source,
        &["", " ", "\t", "\n", "\r"],
        "source account",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_network_testnet() {
        assert!(validate_network("testnet").is_ok());
    }

    #[test]
    fn validate_network_futurenet() {
        assert!(validate_network("futurenet").is_ok());
    }

    #[test]
    fn validate_network_local() {
        assert!(validate_network("local").is_ok());
    }

    #[test]
    fn validate_network_invalid() {
        assert!(validate_network("mainnet").is_err());
    }

    #[test]
    fn validate_network_empty() {
        assert!(validate_network("").is_err());
    }

    #[test]
    fn validate_function_name_valid() {
        assert!(validate_function_name("do_work").is_ok());
    }

    #[test]
    fn validate_function_name_memory() {
        assert!(validate_function_name("memory").is_err());
    }

    #[test]
    fn validate_function_name_constructor() {
        assert!(validate_function_name("_constructor").is_err());
    }

    #[test]
    fn validate_function_name_init() {
        assert!(validate_function_name("_init").is_err());
    }

    #[test]
    fn validate_function_name_empty() {
        assert!(validate_function_name("").is_ok());
    }

    #[test]
    fn validate_source_valid() {
        assert!(validate_source("alice").is_ok());
    }

    #[test]
    fn validate_source_empty() {
        assert!(validate_source("").is_err());
    }

    #[test]
    fn validate_source_space() {
        assert!(validate_source(" ").is_err());
    }

    #[test]
    fn validate_source_tab() {
        assert!(validate_source("\t").is_err());
    }
}
