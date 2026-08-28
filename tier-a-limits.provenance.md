# tier-a-limits provenance

- Source Tier B JSON: `/tmp/tier_b_report.json`
- Margins (cpu, memory, read, write): `1.5000`, `1.2500`, `2.0000`, `3.0000`
- Generated at (UTC): `2026-08-27T08:13:22Z`

This file is auto-generated. Re-run `cargo budget-report --derive-limits` to refresh. The columns are the inputs and result of every Tier A limit; `tier_a_limit = ceil(tier_b_value × margin_metric)`.

| Key | Tier B value | Margin | Tier A limit |
|---|---:|---:|---:|
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__CPU` | 1627002 | 1.5000 | 2440503 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__WRITE` | 0 | 3.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__CPU` | 1945128 | 1.5000 | 2917692 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__WRITE` | 932 | 3.0000 | 2796 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__CPU` | 1578562 | 1.5000 | 2367843 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__WRITE` | 208 | 3.0000 | 624 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__CPU` | 1542328 | 1.5000 | 2313492 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__WRITE` | 0 | 3.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__CPU` | 1559174 | 1.5000 | 2338761 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__WRITE` | 0 | 3.0000 | 0 |

