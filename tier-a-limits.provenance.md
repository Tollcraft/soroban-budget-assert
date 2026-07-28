# tier-a-limits provenance

Hand-composed fixture for the propagation PR; replace by running
`cargo budget-report --derive-limits` against a fresh Tier B report.

- Source Tier B JSON: `cargo-budget-report/fixtures/tier_b_report_sample.json`
- Margins (cpu, memory, read, write): `1.5000`, `1.2500`, `2.0000`, `3.0000`
- Build profile of source WASM: `release`
- Generated at (UTC): `2026-07-27T00:00:00Z` (placeholder)

This file is auto-generated once the propagation toolchain lands.
Re-run `cargo budget-report --derive-limits` to refresh. The columns
are the inputs and result of every Tier A limit;
`tier_a_limit = ceil(tier_b_value × margin_metric)`.

| Key | Tier B value | Margin | Tier A limit |
|---|---:|---:|---:|
| `TIER_A__AMM_POOL_CONTRACT__DEPOSIT__CPU` | 200000 | 1.5000 | 300000 |
| `TIER_A__AMM_POOL_CONTRACT__DEPOSIT__MEM` | 1850000 | 1.2500 | 2312500 |
| `TIER_A__AMM_POOL_CONTRACT__DEPOSIT__READ` | 2150 | 2.0000 | 4300 |
| `TIER_A__AMM_POOL_CONTRACT__DEPOSIT__WRITE` | 600 | 3.0000 | 1800 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__CPU` | 756678 | 1.5000 | 1135017 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__MEM` | 1700000 | 1.2500 | 2125000 |
| `TIER_A__AMM_POOL_CONTRACT__EXTEND_INSTANCE_TTL__CPU` | 22000 | 1.5000 | 33000 |
| `TIER_A__AMM_POOL_CONTRACT__EXTEND_INSTANCE_TTL__MEM` | 1250000 | 1.2500 | 1562500 |
| `TIER_A__AMM_POOL_CONTRACT__EXTEND_INSTANCE_TTL__READ` | 350 | 2.0000 | 700 |
| _omitted_ | 0 | 3.0000 | 0 (`ceil_apply(0, m) == 0`; assertion intentionally absent, see [`derive.rs`](cargo-budget-report/src/derive.rs)) |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__CPU` | 90000 | 1.5000 | 135000 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__MEM` | 1850000 | 1.2500 | 2312500 |
| `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU` | 600000 | 1.5000 | 900000 |
| `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__MEM` | 5550000 | 1.2500 | 6937500 |
| `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__READ` | 6450 | 2.0000 | 12900 |
| `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__WRITE` | 1800 | 3.0000 | 5400 |
| `TIER_A__AMM_POOL_CONTRACT__SWAP__CPU` | 200000 | 1.5000 | 300000 |
| `TIER_A__AMM_POOL_CONTRACT__SWAP__MEM` | 1850000 | 1.2500 | 2312500 |
| `TIER_A__AMM_POOL_CONTRACT__WITHDRAW__CPU` | 200000 | 1.5000 | 300000 |
| `TIER_A__AMM_POOL_CONTRACT__WITHDRAW__MEM` | 1850000 | 1.2500 | 2312500 |

