# Soroban Budget Assert: Audit & Roadmap

Based on the `stellar_wave_builder_master_prompt.md.pdf` standards, this document serves as a guide to bring the `soroban-budget-assert` project up to the required standard for a Drips Wave repo approval.

## 1. Audit: What's Missing

### Repository Hygiene (Phase 10)
- **`CONTRIBUTING.md` & `SECURITY.md`**: Entirely missing. Required for responsible disclosure, scope, and audit status disclaimers.
- **`README.md` Format**: Lacks the approved structure. Missing a banner/logo, CI/CD badges, a maintainer table (with Telegram contacts), community links, and a `contrib.rocks` contributor image.
- **Branch Protection & Topics**: GitHub repository settings lack discoverability topics and branch protection rules (requiring PRs and CI status checks).
- **Issue Generation**: Missing a batch of pre-planned GitHub issues categorized by complexity and tech stack.
- **Release Tag**: Missing a semantic release tag (e.g., `v0.1.0`).

### Documentation Site (Phase 11)
- **Dedicated Docs Site**: Missing a separate GitBook (or equivalent) documentation site. The site must contain end-user guides, developer guides, protocol mechanics, and a macro/CLI reference.

### Submission Assets (Phase 12)
- **Demo Video**: Missing a short end-to-end demo video showing the tool in action.
- **Submission Form Copy**: Missing a structured paragraph for the grant submission, repo relationship description, and live URLs.

---

## 2. Roadmap to Completion

### Step 1: Core Hygiene
- [ ] Create `SECURITY.md` outlining responsible disclosure and audit status.
- [ ] Create `CONTRIBUTING.md` to guide open-source contributors.
- [ ] Rewrite `README.md` to include:
  - Project banner/logo
  - CI/CD status badges
  - Maintainer table with contact info
  - Concise architecture explanation
  - Practical quick-start commands
  - `contrib.rocks` contributors section

### Step 2: Repository Configuration
- [ ] Configure GitHub repository settings: add topics (e.g., `soroban`, `stellar`, `testing`) and enforce branch protection.
- [ ] Write and run a `gh` CLI script to populate the repository with categorized issues (including Summary, Acceptance Criteria, and Tech Stack).
- [ ] Cut and publish a `v0.1.0` release tag.

### Step 3: Documentation Site
- [ ] Scaffold a GitBook (or similar) docs site.
- [ ] Write the required sections:
  - **Introduction**: Problem and solution with cited figures.
  - **Mechanics**: How the CLI and macros calculate and assert costs.
  - **Reference**: Detailed usage of `cargo budget-report` and `#[budget_cpu_lt]`.
  - **Developer Guide**: Local setup and extending the tool.

### Step 4: Submission Preparation
- [ ] Record a short demo video showing a contract exceeding its budget and failing, followed by a passing assertion.
- [ ] Draft the final submission descriptions (1-paragraph plain English description, repo relationships, and planned issue breakdown).
- [ ] Aggregate all links (live URLs, repo URLs, documentation site, demo video) for final submission.
