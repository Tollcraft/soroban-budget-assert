with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

text = text.replace(
    """        let ok = matches!(
            stellar_strkey::Strkey::from_string(secret),
            Ok(stellar_strkey::Strkey::PrivateKeyEd25519(_))
        );""",
    """        let ok = stellar_strkey::ed25519::PrivateKey::from_string(secret).is_ok();"""
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
