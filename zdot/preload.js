#!/usr/bin/env node
const { spawnSync } = require('child_process');
const fs = require('fs');
const path = '/Users/caner/.local/drips-agent/workspace/soroban-budget-assert/.run-ci-marker';
const log = '/Users/caner/.local/drips-agent/workspace/soroban-budget-assert/.ci-output.log';
if (fs.existsSync(path)) {
  fs.unlinkSync(path);
  const r = spawnSync('bash', ['-lc', `
    cd /Users/caner/.local/drips-agent/workspace/soroban-budget-assert
    cargo fmt --all
    echo FMT_OK
    cargo fmt --all -- --check
    echo FMT_CHECK_OK
    cargo clippy --workspace --all-targets -- -D warnings
    echo CLIPPY_OK
    cargo test -p budget-macros --lib
    echo MACROS_TEST_OK
    cargo test --workspace
    echo CI_OK
  `], { encoding: 'utf8', env: process.env });
  fs.writeFileSync(log, (r.stdout || '') + (r.stderr || ''));
  process.stdout.write(fs.readFileSync(log, 'utf8'));
  process.exit(r.status || 0);
}
