#!/bin/bash
# Issue Generation Script using GitHub CLI (`gh`)
# Run this script to populate the repository with planned issues.

echo "Creating issues..."

# 1. Documentation Site Scaffold
gh issue create \
  --title "docs(site): scaffold documentation site" \
  --label "enhancement,documentation,good first issue" \
  --body "### Summary
We need a dedicated documentation site separated from the README to host user and developer guides.

### Acceptance Criteria
- [ ] Initialize a GitBook or Docusaurus project in a \`docs/\` directory.
- [ ] Create stub pages for Introduction, Mechanics, Reference, and Developer Guide.
- [ ] Configure GitHub Pages or Vercel deployment.

### Tech Stack
- Markdown / GitBook / Docusaurus"

# 2. CI/CD Release Automation
gh issue create \
  --title "ci(release): automate v0.1.0 tag and release creation" \
  --label "enhancement,ci" \
  --body "### Summary
Automate the GitHub release process when a new version tag is pushed.

### Acceptance Criteria
- [ ] Add a GitHub Actions workflow in \`.github/workflows/release.yml\`.
- [ ] Trigger on push to tags matching \`v*\`.
- [ ] Automatically attach the compiled \`cargo-budget-report\` binary for Linux, macOS, and Windows.

### Tech Stack
- GitHub Actions / Rust"

# 3. Macro Enhancement
gh issue create \
  --title "feat(macro): support dynamic budget limits based on env vars" \
  --label "enhancement,rust" \
  --body "### Summary
Allow \`#[budget_cpu_lt(N)]\` to optionally read an environment variable for its threshold instead of a hardcoded integer.

### Acceptance Criteria
- [ ] Modify the macro to accept a string pointing to an env var (e.g., \`#[budget_cpu_lt(env = \"MAX_CPU\")]\`).
- [ ] Fall back to a default value if the env var is not present.
- [ ] Add corresponding unit tests.

### Tech Stack
- Rust / proc-macro"

echo "Done!"
