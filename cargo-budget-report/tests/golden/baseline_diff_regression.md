### Baseline comparison

**1 regressed · 1 improved · 1 within tolerance · 0 unchanged** · 1 new · 1 stale

| Function | Metric | Baseline | Current | Change | Change % | Dir | Status |
|---|---|--:|--:|--:|--:|:-:|:--|
| `amm::swap` | cpu_instructions | 1,000,000 | 1,500,000 | +500,000 | +50.00% | ^ | BREACH (max 1,100,000) |
| `amm::swap` | read_bytes | 4,096 | 2,048 | -2,048 | -50.00% | v | improved |
| `amm::swap` | write_bytes | 512 | 540 | +28 | +5.47% | ^ | within tolerance |

**New functions** (no baseline entry — re-run `--record-baseline` to capture): `amm::brand_new`

**Stale entries** (in baseline, not in current WASM — re-run `--record-baseline` to clean up): `amm::removed`
