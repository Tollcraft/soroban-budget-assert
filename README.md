# Soroban Budget Assert Landing Page

This directory contains the source code for the official `soroban-budget-assert` landing page.

## 🚀 Overview

The landing page is a lightweight, responsive, single-page website built with standard static HTML5, CSS3, and JavaScript. It has zero external build or runtime dependencies, making it simple and maintainable for Rust developers.

### Structure

- `index.html`: Main single-page document containing hero section, problem statement, cost-gap comparison metrics, two-tier architecture overview, quick start code blocks, asciinema demo embed, and community links.
- `styles.css`: Custom CSS styles including design system tokens, responsive grid layouts, animations, and dark mode aesthetics.

## 🛠️ Local Development & Preview

Since the site consists of static files, you can view it directly by opening `index.html` in any web browser, or serve it using any simple static HTTP server:

```bash
# Using Python
python3 -m http.server 8000 --directory site

# Or using npx serve / static-server
npx serve site
```

Then visit `http://localhost:8000` in your browser.

## 🚢 Deployment

The site is automatically deployed to GitHub Pages (`gh-pages` root) via GitHub Actions whenever changes are merged into `main`.

Deployment workflow: `.github/workflows/docs.yml`.
