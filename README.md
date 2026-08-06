# 🛡️ Soroban Budget Assert

`soroban-budget-assert` provides empirical cost measurement and assertion tooling for Soroban smart contracts.

The workspace contains:

- `budget-macros`, for fast local CPU and memory budget assertions in tests.
- `cargo-budget-report`, for compiling contracts, simulating their calls, and reporting network resource usage.

## `cargo-budget-report`

Run the report command from a Soroban workspace:

```sh
cargo budget-report
```

The tool uses the configured testnet or futurenet network by default. Network and source settings can also be supplied with `--network` and `--source`, or in `budget.toml`.

### Local or standalone RPC nodes

To simulate against a local standalone Soroban RPC node, provide its endpoint and the corresponding network passphrase:

```sh
cargo budget-report \
  --rpc-url http://127.0.0.1:8000/rpc \
  --network-passphrase 'Standalone Network ; February 2017'
```

`--rpc-url <URL>` overrides the testnet/futurenet RPC defaults. `--network-passphrase <PASSPHRASE>` is required whenever `--rpc-url` is used and identifies the network for transaction simulation. This is useful for Docker-based standalone nodes and for testing custom network fee settings without relying on public-network rate limits.

Other commonly used options include:

```text
--network <NETWORK>       Named network, such as testnet or futurenet
--source <SOURCE>         Stellar CLI source account
--json                    Emit JSON output
--csv                     Emit CSV output
--check                   Enforce limits from budget.toml
--init                    Create a budget.toml template
```

## Development

See `CONTRIBUTING.md` for development requirements and quality checks. Before submitting changes, run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Documentation and support

Documentation is available in [`docs/src/`](docs/src/). For support, visit the [Telegram](https://t.me/+Gflo5jZStw1jMjE0) or [Discord](https://discord.gg/5aprtMSyR) communities.
