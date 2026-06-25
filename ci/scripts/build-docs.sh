#!/usr/bin/env bash
# Build the repository documentation site into $REPO_ROOT/public/:
#   - a landing index.html with the coverage report and the document/*.md guides
# Published to GitHub Pages / GitLab Pages by the docs workflow.

source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$REPO_ROOT"

pub="$REPO_ROOT/public"
rm -rf "$pub"; mkdir -p "$pub"

guides_html=""
for g in document/*.md environment/README.md CLAUDE.md; do
    [ -f "$g" ] && guides_html+="    <li><code>$g</code></li>\n"
done

# Fold in the coverage report if a previous step produced it: copy the HTML report + the
# self-hosted badge under public/, and render the per-crate table inline on the landing
# page. This is how coverage is "rendered nicely" — served straight from Pages, no
# third-party coverage service.
coverage_html=""
covsrc="$REPO_ROOT/.ci-coverage"
if [ -d "$covsrc/html" ]; then
    cp -r "$covsrc/html" "$pub/coverage"
    [ -f "$covsrc/coverage.svg" ] && cp "$covsrc/coverage.svg" "$pub/coverage.svg"
    total="$(sed 's/[^0-9.]//g' "$covsrc/coverage.txt" 2>/dev/null || echo '')"
    coverage_html="<h2>Coverage</h2>\n"
    coverage_html+="<p><img src=\"coverage.svg\" alt=\"coverage\"> "
    coverage_html+="<a href=\"coverage/\">full line-by-line report</a> — counts dead code.</p>\n"
    if [ -f "$covsrc/coverage-crates.md" ]; then
        # Cheap markdown-table -> HTML-table conversion for the per-crate breakdown.
        coverage_html+="<table><tr><th>Crate</th><th>Lines</th><th>Covered</th><th>Coverage</th></tr>\n"
        while IFS='|' read -r _ crate lines covered pct _; do
            case "$crate" in *Crate*|*---*|"") continue;; esac
            crate="$(printf '%s' "$crate" | sed 's/^ *`//;s/` *$//')"
            coverage_html+="<tr><td><code>$crate</code></td><td>$(echo "$lines"|tr -d ' ')</td><td>$(echo "$covered"|tr -d ' ')</td><td>$(echo "$pct"|tr -d ' ')</td></tr>\n"
        done < "$covsrc/coverage-crates.md"
        coverage_html+="</table>\n"
    fi
fi

{
    cat <<'HTML'
<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>y5 — repository documentation</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:48rem;margin:3rem auto;padding:0 1rem}
h1{font-size:1.6rem}code{background:#f2f2f2;padding:.1em .3em;border-radius:3px}
a{color:#06c}ul{padding-left:1.2rem}
table{border-collapse:collapse;width:100%;font-size:14px}
th,td{border:1px solid #ddd;padding:.3em .6em}td:not(:first-child),th:not(:first-child){text-align:right}
</style></head><body>
<h1>y5 compositor — documentation</h1>
<p>Coverage report plus the in-repo reference guides.</p>
HTML
    printf '%b' "$coverage_html"
    cat <<'HTML'
<h2>Reference guides</h2><ul>
HTML
    printf '%b' "$guides_html"
    cat <<'HTML'
</ul></body></html>
HTML
} > "$pub/index.html"

log "docs site ready: $pub/index.html"
