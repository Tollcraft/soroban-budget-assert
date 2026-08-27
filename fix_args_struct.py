import re

with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"(replay: None,\n)",
    r"\1            rpc_url: None,\n            network_passphrase: None,\n            no_deploy_cache: false,\n            offline: false,\n",
    text
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
