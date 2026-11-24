# Soroban Budget Assert Landing Page

This directory contains the source code for the official `soroban-budget-assert` landing page and cost-over-time dashboard.

## 🚀 Overview

The site is a lightweight, responsive static web application built with standard HTML5, CSS3, and vanilla JavaScript. It requires no build step (zero build tooling dependencies), making it simple and maintainable. Runtime assets include Google Fonts for typography on the landing page and Chart.js loaded from CDN on the dashboard page.

### Structure

- `index.html`: Main single-page landing page document containing the hero section, problem statement, cost-gap comparison metrics, two-tier architecture overview, quick start code blocks, asciinema demo embed, and community links.
- `styles.css`: Custom CSS styles for the landing page including design system tokens, responsive grid layouts, animations, and dark mode aesthetics.
- `dashboard.html`: Cost-over-time dashboard that visualizes the budget history dataset across packages, functions, and metrics.
- `assets/dashboard.css`: Dashboard-specific layout and styles for charts, controls, and selectors.
- `assets/dashboard.js`: Client-side data fetching, metric filtering, series pivoting, and Chart.js rendering logic.
- `scripts/validate.sh`: Pre-deployment validation script (invoked as `bash site/scripts/validate.sh`). It checks that every HTML file is well-formed, every JavaScript file parses, every internal `href`/`src`/CSS `url()` reference resolves to a file that exists, and — via `scripts/validate-anchors.sh` — that every in-page and cross-page fragment has a matching target. GitHub Actions runs it in the `validate` job of `.github/workflows/deploy-site.yml` before the site is published; run it locally after editing any dashboard HTML, CSS, or JS asset, or after renaming an element `id`. It requires Node.js (`node` on `PATH`).
- `scripts/validate-anchors.sh`: Resolves `<a href="#fragment">` and `<a href="page.html#fragment">` links against `id` (and `<a name>`) attributes in the destination document, reporting the source file, line, fragment, and destination when a rename breaks navigation. It is invoked by `scripts/validate.sh` and can also be run directly (`bash site/scripts/validate-anchors.sh [SITE_DIR]`).
- `README.md`: Directory overview, file inventory, local development instructions, and deployment details.

### Dashboard data source

The dashboard fetches `./history.json` (overridable with `?history=URL`). The
parameter is deliberately left open — the dashboard is meant to be pointed at
`history.json` files published on other hosts — so the page treats every value
in the fetched document as untrusted text: nothing is inserted as HTML, and a
missing, non-JSON, or wrongly-shaped file produces a distinct on-page error
message rather than a silent blank page.

## 🛠️ Local Development & Preview

Since the site consists of static files, you can view it directly by opening `index.html` or `dashboard.html` in any web browser, or serve it using any simple static HTTP server:

```bash
# Using Python
python3 -m http.server 8000 --directory site

# Or using npx serve / static-server
npx serve site
```

Then visit `http://localhost:8000` (landing page) or `http://localhost:8000/dashboard.html` (dashboard) in your browser.

## 🚢 Deployment

The site is automatically deployed to GitHub Pages (`gh-pages` root) via GitHub Actions whenever changes to `site/**` are merged into `main`.

Deployment workflow: [`.github/workflows/deploy-site.yml`](../.github/workflows/deploy-site.yml).

### How the deployment pieces fit together

1. **`deploy-site.yml` (`validate` job)**: Runs `bash site/scripts/validate.sh` first. It parses every HTML document, syntax-checks the JavaScript, resolves every internal link and asset reference, and checks link fragments against the destination `id` attributes (`scripts/validate-anchors.sh`), so broken navigation fails the run before anything is published.
2. **`budget.yml` (`record-history` job)**: On every push to `main`, verifies the uploaded report is a genuine network-measured measurement (placeholder or mocked reports are declined — the run still succeeds), purges any historical entries that fail the same check, and appends `{commit, timestamp, data}` entries to `history.json` on the `gh-pages` branch. Only real measurements ever reach the dashboard's charts.
3. **`deploy-site.yml` (`deploy` job)**: Publishes the contents of `site/` to `gh-pages` using `peaceiris/actions-gh-pages` with `keep_files: true`, so `history.json` is never deleted or overwritten. Both workflows share the `gh-pages-deploy` concurrency group to prevent race conditions on the `gh-pages` branch.
4. **Client-side Dashboard**: The dashboard page (`dashboard.html`) fetches `history.json` same-origin and pivots the data client-side into `package → function → metric` series — requiring no backend server or build-time data baking.

