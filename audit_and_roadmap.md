# Soroban Budget Assert: Audit & Roadmap

Based on open source repository standards, this document serves as a guide to bring the `soroban-budget-assert` project up to the required standard for an open source repository.

> **Status**: Most items are now complete. This file is maintained as a living roadmap; completed entries cite the evidence that satisfies them.

## 1. Audit: What's Missing

### Repository Hygiene (Phase 10)
- [x] **`CONTRIBUTING.md` & `SECURITY.md`**: Both exist. `SECURITY.md` (`SECURITY.md`) covers responsible disclosure and audit-status disclaimer. `CONTRIBUTING.md` (`CONTRIBUTING.md`) guides open-source contributors with setup, testing, and code-quality commands.
- [x] **`README.md` Format**: Rewritten with CI/CD badges, maintainer table (with Telegram contact), community links, banner/logo header, architecture explanation, quick-start commands, and a `contrib.rocks` contributor image (`README.md`).
- [ ] **Branch Protection & Topics**: GitHub repository settings — cannot be verified from the working tree. Check the repository's Settings > Branches and Settings > Tags pages.
- [ ] **Issue Generation**: Pre-planned GitHub issues — cannot be verified from the working tree. Check the repository's Issues tab.
- [ ] **Release Tag**: No `v0.1.0` tag exists (`git tag` returns empty).

### Documentation Site (Phase 11)
- [x] **Dedicated Docs Site**: Scaffolded under `docs/` using mdBook (`docs/book.toml`). The README also links to a published GitBook site at `https://tollcraft.gitbook.io/docs/budget-assert`. Contains end-user guides, developer guides, protocol mechanics, and a macro/CLI reference (`docs/src/`).

### Submission Assets (Phase 12)
- [x] **Demo Video**: Linked in the README — an asciinema recording showing a contract exceeding its budget and failing, followed by a passing assertion (`README.md:9`).
- [ ] **Submission Form Copy**: Structured paragraph for grant submission — not part of the repository working tree. Stored externally.

---

## 2. Roadmap to Completion

### Step 1: Core Hygiene
- [x] Create `SECURITY.md` outlining responsible disclosure and audit status. — Verified at `SECURITY.md`.
- [x] Create `CONTRIBUTING.md` to guide open-source contributors. — Verified at `CONTRIBUTING.md`.
- [x] Rewrite `README.md` to include:
  - Project banner/logo — `README.md:2`
  - CI/CD status badges — `README.md:5-6`
  - Maintainer table with contact info — `README.md:78-80`
  - Concise architecture explanation — `README.md:17-31`
  - Practical quick-start commands — `README.md:34-69`
  - `contrib.rocks` contributors section — `README.md:90`

### Step 2: Repository Configuration
- [ ] Configure GitHub repository settings: add topics (e.g., `soroban`, `stellar`, `testing`) and enforce branch protection. — Cannot be verified from the working tree.
- [ ] Write and run a `gh` CLI script to populate the repository with categorized issues (including Summary, Acceptance Criteria, and Tech Stack). — Cannot be verified from the working tree.
- [ ] Cut and publish a `v0.1.0` release tag. — No tags exist (`git tag`).

### Step 3: Documentation Site
- [x] Scaffold a GitBook (or similar) docs site. — `docs/book.toml` configures mdBook. README additionally links to a published GitBook at `https://tollcraft.gitbook.io/docs/budget-assert`.
- [x] Write the required sections:
  - **Introduction**: Problem and solution with cited figures — `docs/src/README.md`
  - **Mechanics**: How the CLI and macros calculate and assert costs — `docs/src/mechanics.md`
  - **Reference**: Detailed usage of `cargo budget-report` and `#[budget_cpu_lt]` — `docs/src/reference.md`
  - **Developer Guide**: Local setup and extending the tool — `docs/src/developer_guide.md`

### Step 4: Submission Preparation
- [x] Record a short demo video showing a contract exceeding its budget and failing, followed by a passing assertion. — Linked at `README.md:9` (`https://asciinema.org/a/qqC0RysuCDBvfUXC`).
- [ ] Draft the final submission descriptions (1-paragraph plain English description, repo relationships, and planned issue breakdown). — External submission materials; not in the repository.
- [ ] Aggregate all links (live URLs, repo URLs, documentation site, demo video) for final submission. — External submission materials; not in the repository.
