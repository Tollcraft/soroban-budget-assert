# Introduction

`soroban-budget-assert` solves a specific problem in the Stellar ecosystem: local resource estimates do not match real network costs, and the error can point in either direction.

Measured on this repo's example contract (`do_expensive_work(10_000)`, testnet ground truth 756,678 CPU instructions):

- Raw Rust test estimates ran **~81% under** the network cost (143,887 locally).
- WASM-mode estimates depend on the build profile: with Cargo's default release profile they ran ~8% *under* the network cost of that build (767,049 vs 832,006), and with the standard Soroban size-optimization profile they run **~19% over** (901,816 vs 756,678).

{% hint style="warning" %}
A developer who trusts local numbers either deploys a contract that exhausts its budget on the public network, or over-provisions against costs that aren't real. Both mistakes come from the same root cause: the only trustworthy number is a network simulation of the exact WASM you deploy.
{% endhint %}

This tool provides both halves of the fix: `cargo budget-report` measures network-simulated resource usage across a whole workspace, and the `budget_macros` assertions pin measured costs into `cargo test` so a cost regression fails CI before it fails on-chain.

{% hint style="info" %}
Scope: the report covers execution resources — CPU instructions and ledger read/write bytes. Those are inputs to the non-refundable resource fee, not a total transaction fee: rent, refundable fees, transaction size, footprint entry counts, and the inclusion fee are not measured. [Measurement scope](reference.md#measurement-scope) sets out the boundary and points at where to find the rest.
{% endhint %}
