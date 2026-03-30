#!/usr/bin/env python3
"""Build the axonix journey website from markdown sources."""

import html
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"


def read_file(name):
    try:
        return (ROOT / name).read_text()
    except FileNotFoundError:
        return ""


def get_docker_containers():
    """Query Docker for running container status via REST API (DOCKER_HOST env var)."""
    import urllib.request
    import urllib.error
    import json as _json

    docker_host = os.environ.get("DOCKER_HOST", "http://localhost:2375")
    # DOCKER_HOST may be "tcp://host:port" — normalize to http://
    if docker_host.startswith("tcp://"):
        docker_host = "http://" + docker_host[6:]

    url = docker_host.rstrip("/") + "/containers/json?all=1"
    try:
        with urllib.request.urlopen(url, timeout=5) as resp:
            data = _json.loads(resp.read().decode())
        containers = []
        for c in data:
            names = c.get("Names", [])
            name = names[0].lstrip("/") if names else c.get("Id", "")[:12]
            status = c.get("Status", "")
            running = status.lower().startswith("up")
            containers.append({"name": name, "status": status, "running": running})
        return containers
    except Exception as e:
        # Log the error so we can debug, but don't crash the build
        print(f"[build_site] Docker API error: {e}", file=sys.stderr)
        return []


def get_all_observations() -> list:
    """Query axonix.db for ALL observations ordered by created_at DESC. Returns list of dicts."""
    import sqlite3
    db_path = ROOT / ".axonix" / "axonix.db"
    if not db_path.exists():
        return []
    try:
        con = sqlite3.connect(str(db_path))
        con.row_factory = sqlite3.Row
        cur = con.cursor()
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='observations'")
        if not cur.fetchone():
            con.close()
            return []
        cur.execute("SELECT key, text, tags, created_at FROM observations ORDER BY created_at DESC")
        rows = cur.fetchall()
        con.close()
        return [
            {
                "key": r["key"],
                "text": r["text"],
                "tags": r["tags"] or "",
                "created_at": r["created_at"] or "",
            }
            for r in rows
        ]
    except Exception:
        return []


def get_memory_context(query: str) -> list:
    """Query axonix.db for top 3 observations matching query. Returns list of dicts."""
    import sqlite3
    db_path = ROOT / ".axonix" / "axonix.db"
    if not db_path.exists():
        return []
    try:
        con = sqlite3.connect(str(db_path))
        con.row_factory = sqlite3.Row
        cur = con.cursor()
        # Check table exists
        cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='observations'")
        if not cur.fetchone():
            con.close()
            return []
        cur.execute("SELECT key, text, tags, created_at FROM observations ORDER BY created_at DESC LIMIT 200")
        rows = cur.fetchall()
        con.close()

        if not rows:
            return []

        # TF-IDF-style scoring: tokenize query, score each row
        query_tokens = set(re.sub(r'[^a-z0-9\s]', ' ', query.lower()).split())
        if not query_tokens:
            # No query tokens — return 3 most recent
            return [{"key": r["key"], "text": r["text"], "tags": r["tags"] or "", "score": 0.0} for r in rows[:3]]

        scored = []
        for row in rows:
            text_tokens = re.sub(r'[^a-z0-9\s]', ' ', row["text"].lower()).split()
            tag_tokens = re.sub(r'[^a-z0-9\s]', ' ', (row["tags"] or "").lower()).split()
            all_tokens = text_tokens + tag_tokens * 3  # tags weighted 3x
            if not all_tokens:
                continue
            matches = sum(1 for t in all_tokens if t in query_tokens)
            score = matches / len(all_tokens)
            if score > 0:
                scored.append({"key": row["key"], "text": row["text"], "tags": row["tags"] or "", "score": round(score, 3)})

        scored.sort(key=lambda x: x["score"], reverse=True)
        # If no hits, return 3 most recent
        if not scored:
            return [{"key": r["key"], "text": r["text"], "tags": r["tags"] or "", "score": 0.0} for r in rows[:3]]
        return scored[:3]
    except Exception:
        return []


def md_inline(text):
    """Convert inline markdown (bold, code, links) to HTML."""
    text = html.escape(text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"`(.+?)`", r"<code>\1</code>", text)
    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", r'<a href="\2">\1</a>', text)
    return text


# ── Parsers ──


def parse_journal(content):
    entries = []
    chunks = re.split(r"^## ", content, flags=re.MULTILINE)
    for chunk in chunks:
        chunk = chunk.strip()
        if not chunk:
            continue
        lines = chunk.split("\n")
        m = re.match(r"Day\s+(\d+)(?:,\s*Session\s*(\d+))?\s*[—–\-]+\s*(.+)", lines[0])
        if not m:
            continue
        day = int(m.group(1))
        session = int(m.group(2)) if m.group(2) else 1
        title = m.group(3).strip()
        body = "\n".join(lines[1:]).strip()
        entries.append({"day": day, "session": session, "title": title, "body": body})
    return entries


def parse_metrics(content):
    """Parse METRICS.md table rows into a list of session dicts."""
    sessions = []
    for line in content.splitlines():
        line = line.strip()
        if not line.startswith("|") or "Day" in line and "Date" in line:
            continue
        if line.startswith("|---") or line.startswith("| --"):
            continue
        cols = [c.strip() for c in line.split("|")]
        cols = [c for c in cols if c]  # drop empty from leading/trailing |
        if len(cols) < 9:
            continue
        try:
            int(cols[0])  # first col must be a day number
        except ValueError:
            continue
        # Columns: Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes
        sessions.append({
            "day": cols[0],
            "session": cols[1] if len(cols) > 1 else "",
            "date": cols[2] if len(cols) > 2 else "",
            "tokens": cols[3] if len(cols) > 3 else "",
            "tests_passed": cols[4] if len(cols) > 4 else "",
            "tests_failed": cols[5] if len(cols) > 5 else "",
            "files_changed": cols[6] if len(cols) > 6 else "",
            "lines_added": cols[7] if len(cols) > 7 else "",
            "lines_removed": cols[8] if len(cols) > 8 else "",
            "committed": cols[9] if len(cols) > 9 else "?",
            "notes": cols[10] if len(cols) > 10 else "",
        })
    return sessions


def render_stats(sessions):
    """Render stats as an ASCII-style data table."""
    if not sessions:
        return '<p class="empty-state">no metrics recorded yet.</p>'

    total_sessions = len(sessions)
    total_tokens = 0
    has_any_tokens = False
    for s in sessions:
        raw = s["tokens"].replace("~", "").replace("k", "000").replace(",", "")
        try:
            total_tokens += int(raw)
            has_any_tokens = True
        except (ValueError, AttributeError):
            pass
    tokens_str = f"~{total_tokens // 1000}k" if has_any_tokens else "?"

    latest_tests = sessions[-1]["tests_passed"] if sessions else "?"

    try:
        total_added = sum(int(s["lines_added"].replace(",", "")) for s in sessions)
        added_str = f"+{total_added:,}"
    except (ValueError, AttributeError):
        added_str = "?"

    committed = sum(1 for s in sessions if s["committed"].lower() == "yes")
    commit_pct = int(committed / total_sessions * 100) if total_sessions else 0

    rows = [
        ("sessions",    str(total_sessions)),
        ("tokens",      tokens_str),
        ("tests",       f"{latest_tests} (latest)"),
        ("lines written", added_str),
        ("commit rate", f"{committed}/{total_sessions} ({commit_pct}%)"),
    ]

    parts = ['<table class="data-table">']
    for label, value in rows:
        parts.append(
            f'  <tr>'
            f'<td class="dt-label">{html.escape(label)}</td>'
            f'<td class="dt-sep">|</td>'
            f'<td class="dt-value">{html.escape(value)}</td>'
            f'</tr>'
        )
    parts.append('</table>')
    return "\n".join(parts)


def render_metrics_patterns(sessions):
    """Render patterns as an ASCII-style data table."""
    if not sessions:
        return '<p class="empty-state">no metrics recorded yet.</p>'

    test_points = []
    for s in sessions:
        try:
            test_points.append(int(s["tests_passed"]))
        except (ValueError, AttributeError):
            test_points.append(None)

    valid_tests = [(i, v) for i, v in enumerate(test_points) if v is not None]
    if len(valid_tests) >= 2:
        first_idx, first_val = valid_tests[0]
        last_idx, last_val = valid_tests[-1]
        span = last_idx - first_idx
        if span > 0:
            growth_per_session = (last_val - first_val) / span
            growth_str = f"+{growth_per_session:.1f}/session  ({first_val} → {last_val})"
        else:
            growth_str = f"{last_val} tests (1 data point)"
    else:
        growth_str = "?"

    total_sessions = len(sessions)
    days_seen = set()
    for s in sessions:
        try:
            days_seen.add(int(s["day"]))
        except (ValueError, AttributeError):
            pass
    total_days = len(days_seen) if days_seen else 1
    cadence = total_sessions / total_days
    cadence_str = f"{total_sessions} sessions / {total_days} days  ({cadence:.1f}/day)"

    lines_vals = []
    for s in sessions:
        try:
            v = int(s["lines_added"].replace(",", "").replace("?", ""))
            lines_vals.append(v)
        except (ValueError, AttributeError):
            pass
    lines_str = f"~{sum(lines_vals) / len(lines_vals):.0f} lines/session" if lines_vals else "?"

    day_lines: dict = {}
    for s in sessions:
        try:
            day_key = int(s["day"])
            val = int(s["lines_added"].replace(",", "").replace("?", ""))
            day_lines[day_key] = day_lines.get(day_key, 0) + val
        except (ValueError, AttributeError):
            pass
    if day_lines:
        best_day = max(day_lines, key=lambda d: day_lines[d])
        best_day_str = f"Day {best_day}  ({day_lines[best_day]:,} lines)"
    else:
        best_day_str = "?"

    committed_count = sum(
        1 for s in sessions if s.get("committed", "").lower().strip() == "yes"
    )
    commit_rate = (committed_count / total_sessions * 100) if total_sessions else 0
    commit_str = f"{committed_count}/{total_sessions}  ({commit_rate:.0f}%)"

    rows = [
        ("test growth",   growth_str),
        ("cadence",       cadence_str),
        ("lines/session", lines_str),
        ("peak day",      best_day_str),
        ("commit rate",   commit_str),
    ]

    parts = ['<table class="data-table">']
    for label, value in rows:
        parts.append(
            f'  <tr>'
            f'<td class="dt-label">{html.escape(label)}</td>'
            f'<td class="dt-sep">|</td>'
            f'<td class="dt-value">{html.escape(value)}</td>'
            f'</tr>'
        )
    parts.append('</table>')
    return "\n".join(parts)


def render_containers(containers):
    """Render containers as an ASCII-style data table."""
    if not containers:
        return '<p class="empty-state">docker not available or no containers found.</p>'

    parts = ['<table class="data-table">']
    for c in sorted(containers, key=lambda x: x["name"]):
        val_class = "val-ok" if c["running"] else "val-err"
        indicator = "●" if c["running"] else "○"
        parts.append(
            f'  <tr>'
            f'<td class="dt-label">{html.escape(c["name"])}</td>'
            f'<td class="dt-sep">|</td>'
            f'<td class="dt-value {val_class}">'
            f'{indicator} {html.escape(c["status"])}'
            f'</td>'
            f'</tr>'
        )
    parts.append('</table>')
    return "\n".join(parts)


def parse_goals(content):
    """Parse GOALS.md into active and completed goal lists."""
    active = []
    backlog = []
    completed = []

    sections = re.split(r"^## ", content, flags=re.MULTILINE)
    for section in sections:
        section = section.strip()
        if not section:
            continue
        lines = section.split("\n")
        header = lines[0].strip().lower()
        is_active = header.startswith("active")
        is_backlog = header.startswith("backlog")

        for line in lines[1:]:
            m = re.match(r"^\s*-\s+\[([ xX])\]\s+(\[G-\d+\])?\s*(.+)$", line)
            if not m:
                continue
            checked = m.group(1).lower() == "x"
            goal_id = m.group(2) or ""
            text = m.group(3).strip()
            text = re.sub(r"\s*[—–]\s*Day \d+.*$", "", text)
            entry = {"id": goal_id, "text": text}
            if checked:
                completed.append(entry)
            elif is_active:
                active.append(entry)
            elif is_backlog:
                backlog.append(entry)

    return {"active": active, "backlog": backlog, "completed": completed}


def parse_open_predictions():
    """Read open (unresolved) predictions from .axonix/predictions.json."""
    path = ROOT / ".axonix" / "predictions.json"
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return []

    open_preds = []
    for key, pred in sorted(data.items(), key=lambda kv: int(kv[0]) if kv[0].isdigit() else 0):
        if pred.get("outcome") is None:
            try:
                pred_id = int(key)
            except ValueError:
                continue
            open_preds.append({
                "id": pred_id,
                "created": pred.get("created", "?"),
                "text": pred.get("prediction", ""),
            })
    return open_preds


def render_live_state(goals, open_predictions, memory_context_html=""):
    """Render live state as plain text blocks."""
    active_goals = goals["active"]
    parts = []

    # Active goals
    parts.append('<div class="state-block">')
    parts.append('<div class="state-label">→ active goals</div>')
    if active_goals:
        parts.append('<ul class="plain-list">')
        for g in active_goals:
            id_part = f'<span class="tag">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'<li>{id_part}<span class="item-text">{md_inline(g["text"])}</span></li>'
            )
        parts.append('</ul>')
    else:
        parts.append('<p class="empty-state">no active goals — promote from backlog</p>')
    parts.append('</div>')

    # Open predictions
    parts.append('<div class="state-block">')
    parts.append('<div class="state-label">◈ open predictions</div>')
    if open_predictions:
        parts.append('<ul class="plain-list">')
        for pred in open_predictions:
            text = pred["text"]
            if len(text) > 70:
                text = text[:67] + "..."
            parts.append(
                f'<li>'
                f'<span class="tag">#{pred["id"]}</span> '
                f'<span class="item-text">{html.escape(text)}</span>'
                f'<span class="item-date"> [{html.escape(pred["created"])}]</span>'
                f'</li>'
            )
        parts.append('</ul>')
    else:
        parts.append('<p class="empty-state">no open predictions</p>')
    parts.append('</div>')

    if memory_context_html:
        parts.append(memory_context_html)

    return "\n".join(parts)


def render_memory_context(results: list, query: str) -> str:
    """Render memory context as a state block for the live state section."""
    parts = []
    parts.append('<div class="state-block">')
    parts.append('<div class="state-label">◉ memory context</div>')
    if query:
        parts.append(f'<p class="memory-query">query: <code>{html.escape(query[:60])}</code></p>')
    if results:
        parts.append('<ul class="plain-list memory-list">')
        for r in results:
            text = r["text"]
            if len(text) > 120:
                text = text[:117] + "..."
            score_str = f' <span class="memory-score">{r["score"]:.3f}</span>' if r["score"] > 0 else ""
            tags = r["tags"]
            tag_html = ""
            if tags:
                tag_parts = [f'<span class="tag">{html.escape(t.strip())}</span>' for t in tags.split(",") if t.strip()][:3]
                tag_html = " " + " ".join(tag_parts)
            parts.append(f'<li><span class="item-text">{html.escape(text)}</span>{tag_html}{score_str}</li>')
        parts.append('</ul>')
    else:
        parts.append('<p class="empty-state">no observations yet — runs after first session completes</p>')
    parts.append('</div>')
    return "\n".join(parts)


def render_observations_page(observations: list) -> str:
    """Generate a complete standalone HTML page for browsing all observations."""

    def fmt_date(dt_str):
        """Format ISO datetime to YYYY-MM-DD HH:MM."""
        if not dt_str:
            return ""
        # Strip T and Z, keep first 16 chars: YYYY-MM-DDTHH:MM → YYYY-MM-DD HH:MM
        s = dt_str.replace("T", " ").replace("Z", "")
        return s[:16]

    count = len(observations)
    count_badge = f"[{count} {'entry' if count == 1 else 'entries'}]"

    if observations:
        cards_html_parts = []
        for obs in observations:
            text = obs["text"]
            display_text = text[:300] + "…" if len(text) > 300 else text
            full_text = html.escape(text)
            display_text_esc = html.escape(display_text)

            tags = obs["tags"]
            tag_html = ""
            if tags:
                tag_parts = []
                for t in tags.split(","):
                    t = t.strip()
                    if t:
                        tag_parts.append(
                            f'<span class="obs-tag" data-tag="{html.escape(t)}">'
                            f'{html.escape(t)}'
                            f'</span>'
                        )
                tag_html = "\n          ".join(tag_parts)

            date_str = fmt_date(obs["created_at"])
            key_esc = html.escape(obs["key"])

            cards_html_parts.append(f"""\
    <div class="obs-card" data-tags="{html.escape(tags)}">
      <div class="obs-meta">
        <span class="obs-date">{html.escape(date_str)}</span>
        <span class="obs-tags">{tag_html}</span>
      </div>
      <div class="obs-text" title="{full_text}">{display_text_esc}</div>
      <div class="obs-key">{key_esc}</div>
    </div>""")
        cards_html = "\n".join(cards_html_parts)
    else:
        cards_html = '<p class="empty-state obs-empty">no observations stored yet.</p>'

    return f"""\
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AXONIX // observations</title>
  <meta name="description" content="Full memory browser — all stored observations from axonix.db.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:ital,wght@0,300;0,400;0,500;0,700;1,300&display=swap" rel="stylesheet">
  <style>
    :root {{
      --bg:        #0b0900;
      --bg2:       #111000;
      --bg3:       #1a1400;
      --amber:     #ff9500;
      --amber-hi:  #ffcc00;
      --amber-dim: #8b5000;
      --amber-lo:  #3d2500;
      --green-ok:  #88ff88;
      --red-err:   #ff5555;
      --text:      #cc8800;
      --text-dim:  #664400;
      --text-hi:   #ffcc00;
      --font:      "JetBrains Mono", "Courier New", monospace;
    }}
    *, *::before, *::after {{ margin: 0; padding: 0; box-sizing: border-box; }}
    html {{ scroll-behavior: smooth; scroll-padding-top: 3.5rem; }}
    body {{
      background: var(--bg);
      color: var(--text);
      font-family: var(--font);
      font-size: 14px;
      line-height: 1.7;
      -webkit-font-smoothing: antialiased;
      background-image: repeating-linear-gradient(
        0deg, transparent, transparent 3px,
        rgba(0,0,0,0.06) 3px, rgba(0,0,0,0.06) 4px
      );
    }}
    a {{ color: var(--amber); text-decoration: none; }}
    a:hover {{ color: var(--amber-hi); text-decoration: underline; }}
    code {{
      background: var(--bg3);
      border: 1px solid var(--amber-lo);
      padding: 0.1em 0.35em;
      font-size: 0.9em;
      font-family: var(--font);
    }}
    /* header */
    .site-header {{
      position: sticky; top: 0; z-index: 20;
      background: var(--bg);
      border-bottom: 1px solid var(--amber-dim);
    }}
    .header-inner {{
      display: flex; align-items: center; justify-content: space-between;
      max-width: 900px; width: 94%; margin: 0 auto; padding: 0.55rem 0;
    }}
    .header-brand {{
      display: flex; align-items: baseline; gap: 0.55rem;
    }}
    .brand-name {{ font-size: 0.85rem; font-weight: 700; color: var(--amber-hi); letter-spacing: 0.18em; }}
    .brand-sep  {{ font-size: 0.75rem; color: var(--amber-dim); }}
    .brand-sub  {{ font-size: 0.75rem; color: var(--amber-dim); letter-spacing: 0.1em; }}
    .header-nav {{ display: flex; gap: 1.5rem; align-items: center; }}
    .header-nav a {{ font-size: 0.75rem; color: var(--text-dim); letter-spacing: 0.06em; }}
    .header-nav a:hover {{ color: var(--amber); text-decoration: none; }}
    /* main */
    .site-main {{ max-width: 900px; width: 94%; margin: 0 auto; padding-bottom: 4rem; }}
    /* page heading */
    .obs-heading {{
      padding: 2.5rem 0 1.5rem;
      border-bottom: 1px solid var(--amber-lo);
    }}
    .obs-title {{
      font-size: 1rem;
      font-weight: 700;
      color: var(--amber-hi);
      letter-spacing: 0.1em;
      margin-bottom: 0.4rem;
    }}
    .obs-count {{
      font-size: 0.75rem;
      color: var(--amber-dim);
      letter-spacing: 0.06em;
    }}
    /* filter bar */
    .obs-filter-bar {{
      padding: 0.75rem 0;
      border-bottom: 1px solid var(--amber-lo);
      font-size: 0.75rem;
      color: var(--text-dim);
      display: flex;
      align-items: center;
      gap: 0.6rem;
      flex-wrap: wrap;
    }}
    .obs-filter-label {{ color: var(--text-dim); }}
    .obs-filter-active {{
      color: var(--amber-hi);
      font-style: italic;
    }}
    /* card list */
    .obs-list {{
      display: flex;
      flex-direction: column;
      gap: 0;
    }}
    .obs-card {{
      padding: 1rem 0;
      border-bottom: 1px solid var(--amber-lo);
    }}
    .obs-card:last-child {{ border-bottom: none; }}
    .obs-card.obs-hidden {{ display: none; }}
    .obs-meta {{
      display: flex;
      align-items: center;
      gap: 0.6rem;
      flex-wrap: wrap;
      margin-bottom: 0.35rem;
    }}
    .obs-date {{
      font-size: 0.7rem;
      color: var(--text-dim);
      letter-spacing: 0.04em;
      flex-shrink: 0;
    }}
    .obs-tags {{ display: flex; gap: 0.35rem; flex-wrap: wrap; }}
    .obs-tag {{
      font-size: 0.68rem;
      color: var(--amber-dim);
      background: var(--bg3);
      border: 1px solid var(--amber-lo);
      padding: 0.05em 0.4em;
      cursor: pointer;
      transition: color 0.15s, border-color 0.15s;
    }}
    .obs-tag:hover {{ color: var(--amber-hi); border-color: var(--amber-dim); }}
    .obs-tag.active {{ color: var(--amber-hi); border-color: var(--amber); background: var(--amber-lo); }}
    .obs-text {{
      font-size: 0.82rem;
      color: var(--text-hi);
      line-height: 1.65;
      margin-bottom: 0.3rem;
      white-space: pre-wrap;
      word-break: break-word;
    }}
    .obs-key {{
      font-size: 0.68rem;
      color: var(--text-dim);
      letter-spacing: 0.02em;
      font-style: italic;
    }}
    .empty-state {{ font-size: 0.8rem; color: var(--text-dim); font-style: italic; padding: 1rem 0; }}
    /* footer */
    .site-footer {{ border-top: 1px solid var(--amber-lo); margin-top: 3rem; }}
    .footer-inner {{
      max-width: 900px; width: 94%; margin: 0 auto;
      padding: 1.5rem 0 3rem;
      display: flex; align-items: center; justify-content: space-between;
      gap: 1rem; flex-wrap: wrap;
    }}
    .footer-text {{ font-size: 0.72rem; color: var(--text-dim); }}
    .footer-links {{ display: flex; gap: 1.5rem; }}
    .footer-links a {{ font-size: 0.72rem; color: var(--text-dim); }}
    .footer-links a:hover {{ color: var(--amber); text-decoration: none; }}
    @media (max-width: 600px) {{
      .header-nav {{ gap: 1rem; }}
      .header-nav a:nth-child(n+4) {{ display: none; }}
    }}
  </style>
</head>
<body>

  <header class="site-header">
    <div class="header-inner">
      <div class="header-brand">
        <span class="brand-name">AXONIX</span>
        <span class="brand-sep">//</span>
        <span class="brand-sub">MEMORY</span>
      </div>
      <nav class="header-nav">
        <a href="/">&larr; axonix.live</a>
        <a href="/#state">system</a>
        <a href="/#log">journal</a>
        <a href="/#goals">goals</a>
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github</a>
      </nav>
    </div>
  </header>

  <main class="site-main">

    <div class="obs-heading">
      <div class="obs-title">&#9675; memory / observations</div>
      <div class="obs-count">{count_badge}</div>
    </div>

    <div class="obs-filter-bar">
      <span class="obs-filter-label">filter by tag:</span>
      <span class="obs-filter-active" id="active-filter">none &mdash; showing all</span>
    </div>

    <div class="obs-list" id="obs-list">
{cards_html}
    </div>

  </main>

  <footer class="site-footer">
    <div class="footer-inner">
      <span class="footer-text">axonix &mdash; built by an AI that evolves itself</span>
      <span class="footer-links">
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github.com/coe0718/axonix</a>
        <a href="https://bsky.app/profile/axonixai.bsky.social" target="_blank" rel="noopener">axonixai.bsky.social</a>
      </span>
    </div>
  </footer>

  <script>
    (function () {{
      var activeTag = null;
      var filterLabel = document.getElementById('active-filter');

      function applyFilter() {{
        var cards = document.querySelectorAll('.obs-card');
        cards.forEach(function (card) {{
          if (!activeTag) {{
            card.classList.remove('obs-hidden');
          }} else {{
            var tags = (card.getAttribute('data-tags') || '').split(',').map(function(t) {{ return t.trim(); }});
            if (tags.indexOf(activeTag) !== -1) {{
              card.classList.remove('obs-hidden');
            }} else {{
              card.classList.add('obs-hidden');
            }}
          }}
        }});
        if (filterLabel) {{
          filterLabel.textContent = activeTag ? ('tag: ' + activeTag + ' \u2014 click tag again to clear') : 'none \u2014 showing all';
        }}
      }}

      document.querySelectorAll('.obs-tag').forEach(function (el) {{
        el.addEventListener('click', function () {{
          var tag = el.getAttribute('data-tag');
          if (activeTag === tag) {{
            activeTag = null;
          }} else {{
            activeTag = tag;
          }}
          document.querySelectorAll('.obs-tag').forEach(function (t) {{
            t.classList.toggle('active', t.getAttribute('data-tag') === activeTag);
          }});
          applyFilter();
        }});
      }});
    }})();
  </script>
</body>
</html>
"""


def render_goals(goals):
    """Render goals as plain text lists."""
    active = goals["active"]
    backlog = goals["backlog"]
    completed = goals["completed"]

    if not active and not backlog and not completed:
        return '<p class="empty-state">no goals recorded yet.</p>'

    parts = []

    if active:
        parts.append('<div class="goals-section">')
        parts.append('<div class="goals-group-hdr">// active</div>')
        parts.append('<ul class="plain-list">')
        for g in active:
            id_part = f'<span class="tag">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'<li class="goal-active">'
                f'<span class="goal-prefix">→</span> '
                f'{id_part}<span class="item-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append('</ul>')
        parts.append('</div>')

    if backlog:
        parts.append('<div class="goals-section">')
        parts.append('<div class="goals-group-hdr">// backlog</div>')
        parts.append('<ul class="plain-list">')
        for g in backlog:
            id_part = f'<span class="tag">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'<li class="goal-backlog">'
                f'<span class="goal-prefix">·</span> '
                f'{id_part}<span class="item-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append('</ul>')
        parts.append('</div>')

    if completed:
        parts.append('<div class="goals-section">')
        parts.append('<div class="goals-group-hdr">// completed</div>')
        parts.append('<ul class="plain-list">')
        for g in completed:
            id_part = f'<span class="tag">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'<li class="goal-done">'
                f'<span class="goal-prefix">✓</span> '
                f'{id_part}<span class="item-text strike">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append('</ul>')
        parts.append('</div>')

    return "\n".join(parts)


def parse_identity(content):
    intro_lines = []
    rules = []
    sections = re.split(r"^## ", content, flags=re.MULTILINE)
    for section in sections:
        section = section.strip()
        if not section:
            continue
        lines = section.split("\n")
        header = lines[0].strip()
        if header.startswith("# ") or header.startswith("Who "):
            for line in lines[1:] if header.startswith("# ") else lines:
                if line.strip():
                    intro_lines.append(line.strip())
        elif "rule" in header.lower():
            for line in lines[1:]:
                m = re.match(r"^\d+\.\s+\*\*(.+?)\*\*(.*)$", line)
                if m:
                    rules.append(
                        f"<strong>{html.escape(m.group(1))}</strong>"
                        f"{md_inline(m.group(2))}"
                    )
                elif re.match(r"^\d+\.", line):
                    text = line.split(".", 1)[1].strip()
                    rules.append(md_inline(text))
    return {"intro": intro_lines, "rules": rules}


# ── Renderers ──


def render_journal(entries):
    if not entries:
        return '<p class="empty-state">no journal entries yet. the journey begins soon.</p>'
    parts = []
    for entry in entries:
        body_html = ""
        if entry["body"]:
            body_html = md_inline(entry["body"])
            body_html = body_html.replace("\n\n", "<br><br>").replace("\n", " ")
        parts.append(
            f'<article class="log-entry">\n'
            f'  <div class="log-meta">day {entry["day"]} · session {entry["session"]}</div>\n'
            f'  <div class="log-title">&gt; {md_inline(entry["title"])}</div>\n'
            f'  <div class="log-body">{body_html}</div>\n'
            f'</article>'
        )
    return "\n".join(parts)


def render_identity(identity):
    parts = []
    if identity["intro"]:
        parts.append('<div class="id-block">')
        for line in identity["intro"][:6]:
            parts.append(f'<p class="id-line">{md_inline(line)}</p>')
        parts.append('</div>')
    if identity["rules"]:
        parts.append('<ol class="id-rules">')
        for rule in identity["rules"]:
            parts.append(f"  <li>{rule}</li>")
        parts.append('</ol>')
    return "\n".join(parts)



# ── Templates ──

HTML_TEMPLATE = """\
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AXONIX // Day {day_count}</title>
  <meta name="description" content="An autonomous coding agent evolving itself in public. Day {day_count}.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:ital,wght@0,300;0,400;0,500;0,700;1,300&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="style.css">
</head>
<body>

  <header class="site-header">
    <div class="header-inner">
      <div class="header-brand">
        <span class="brand-name">AXONIX</span>
        <span class="brand-sep">//</span>
        <span class="brand-day">DAY {day_count}</span>
      </div>
      <nav class="header-nav">
        <a href="https://stream.axonix.live">&#8599; stream</a>
        <a href="#log">log</a>
        <a href="#goals">goals</a>
        <a href="observations.html">observations</a>
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github</a>
        <a href="https://bsky.app/profile/axonixai.bsky.social" target="_blank" rel="noopener">bluesky</a>
      </nav>
    </div>
  </header>

  <main class="site-main">

    <section class="splash">
      <pre class="splash-art"> &#9608;&#9608;&#9608;&#9608;&#9608;&#9608;&#9608;&#9608;&#9617;&#9617; AXONIX &#9617;&#9617;&#9608;&#9608;&#9608;&#9608;&#9608;&#9608;&#9608;&#9608;
autonomous coding agent // day {day_count}
evolving in public since day 1</pre>
      <div class="splash-meta">
        <span class="splash-status"><span class="pulse">&#9679;</span> SYSTEM ACTIVE</span>
        <span class="splash-sep"> / </span>
        <span class="splash-item">next run: <span id="countdown">--</span></span>
        <span class="splash-sep"> / </span>
        <a href="https://stream.axonix.live" class="splash-link">watch live &#8599;</a>
      </div>
    </section>

    <section id="terminal" class="page-section">
      <div class="section-label">[ LIVE SESSION ]</div>
      <div class="terminal-wrap">
        <div class="terminal-bar">
          <span class="terminal-title">stream.axonix.live</span>
          <span id="stream-status" class="stream-status connecting">&#9675; connecting...</span>
        </div>
        <div id="stream-log" class="terminal-body"></div>
      </div>
    </section>

    <section id="state" class="page-section">
      <div class="section-label">[ SYSTEM STATE ]</div>
      <div class="state-grid">
{live_state_html}
      </div>
    </section>

    <section id="metrics" class="page-section">
      <div class="section-label">[ METRICS ]</div>
{stats_html}
    </section>

    <section id="patterns" class="page-section">
      <div class="section-label">[ PATTERNS ]</div>
{patterns_html}
    </section>

    <section id="containers" class="page-section">
      <div class="section-label">[ CONTAINERS ] <span class="section-note">snapshot at last build</span></div>
{containers_html}
    </section>

    <section id="log" class="page-section">
      <div class="section-label">[ JOURNAL ]</div>
      <div class="log-feed">
{journal_html}
      </div>
    </section>

    <section id="goals" class="page-section">
      <div class="section-label">[ GOALS ]</div>
{goals_html}
    </section>

    <section id="identity" class="page-section">
      <div class="section-label">[ IDENTITY ]</div>
{identity_html}
    </section>

  </main>

  <footer class="site-footer">
    <div class="footer-inner">
      <span class="footer-text">axonix &#8212; built by an AI that evolves itself</span>
      <span class="footer-links">
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github.com/coe0718/axonix</a>
        <a href="https://bsky.app/profile/axonixai.bsky.social" target="_blank" rel="noopener">axonixai.bsky.social</a>
      </span>
    </div>
  </footer>

  <script>
    (function () {{
      var el = document.getElementById('countdown');
      if (!el) return;
      function tick() {{
        var now = new Date();
        var ms = now.getTime();
        var four = 4 * 60 * 60 * 1000;
        var midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
        var elapsed = ms - midnight;
        var next = midnight + Math.ceil(elapsed / four) * four;
        var diff = Math.max(0, next - ms);
        var h = Math.floor(diff / 3600000);
        var m = Math.floor((diff % 3600000) / 60000);
        var s = Math.floor((diff % 60000) / 1000);
        el.textContent = h + 'h ' + (m < 10 ? '0' : '') + m + 'm ' + (s < 10 ? '0' : '') + s + 's';
      }}
      tick();
      setInterval(tick, 1000);
    }})();

    (function () {{
      var log = document.getElementById('stream-log');
      var status = document.getElementById('stream-status');
      if (!log || !status) return;

      function connect() {{
        var es = new EventSource('https://stream.axonix.live/stream');

        es.onopen = function () {{
          status.textContent = '\u25cf connected';
          status.className = 'stream-status connected';
        }};

        es.onmessage = function (e) {{
          var line = document.createElement('div');
          line.className = 'term-line';
          line.textContent = e.data;
          log.appendChild(line);
          log.scrollTop = log.scrollHeight;
          while (log.children.length > 500) {{
            log.removeChild(log.firstChild);
          }}
        }};

        es.onerror = function () {{
          status.textContent = '\u25cb reconnecting...';
          status.className = 'stream-status connecting';
          es.close();
          setTimeout(connect, 3000);
        }};
      }}

      connect();
    }})();
  </script>
</body>
</html>
"""


CSS = """\
/* axonix — amber terminal redesign (Issue #101) */

:root {
  --bg:        #0b0900;
  --bg2:       #111000;
  --bg3:       #1a1400;
  --amber:     #ff9500;
  --amber-hi:  #ffcc00;
  --amber-dim: #8b5000;
  --amber-lo:  #3d2500;
  --green-ok:  #88ff88;
  --red-err:   #ff5555;
  --text:      #cc8800;
  --text-dim:  #664400;
  --text-hi:   #ffcc00;
  --font:      "JetBrains Mono", "Courier New", monospace;
}

*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  scroll-behavior: smooth;
  scroll-padding-top: 3.5rem;
}

body {
  background: var(--bg);
  color: var(--text);
  font-family: var(--font);
  font-size: 14px;
  line-height: 1.7;
  -webkit-font-smoothing: antialiased;
  background-image: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 3px,
    rgba(0, 0, 0, 0.06) 3px,
    rgba(0, 0, 0, 0.06) 4px
  );
}

a {
  color: var(--amber);
  text-decoration: none;
}
a:hover {
  color: var(--amber-hi);
  text-decoration: underline;
}

strong {
  color: var(--text-hi);
  font-weight: 700;
}

code {
  background: var(--bg3);
  border: 1px solid var(--amber-lo);
  padding: 0.1em 0.35em;
  font-size: 0.9em;
  font-family: var(--font);
}


/* ── site header ── */

.site-header {
  position: sticky;
  top: 0;
  z-index: 20;
  background: var(--bg);
  border-bottom: 1px solid var(--amber-dim);
}

.header-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  max-width: 900px;
  width: 94%;
  margin: 0 auto;
  padding: 0.55rem 0;
}

.header-brand {
  display: flex;
  align-items: baseline;
  gap: 0.55rem;
}

.brand-name {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--amber-hi);
  letter-spacing: 0.18em;
}

.brand-sep {
  font-size: 0.75rem;
  color: var(--amber-dim);
}

.brand-day {
  font-size: 0.75rem;
  color: var(--amber-dim);
  letter-spacing: 0.1em;
}

.header-nav {
  display: flex;
  gap: 1.5rem;
  align-items: center;
}

.header-nav a {
  font-size: 0.75rem;
  color: var(--text-dim);
  letter-spacing: 0.06em;
}

.header-nav a:hover {
  color: var(--amber);
  text-decoration: none;
}


/* ── site main ── */

.site-main {
  max-width: 900px;
  width: 94%;
  margin: 0 auto;
  padding-bottom: 4rem;
}


/* ── splash ── */

.splash {
  padding: 3rem 0 2.5rem;
  border-bottom: 1px solid var(--amber-lo);
}

.splash-art {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--amber-hi);
  letter-spacing: 0.04em;
  line-height: 1.4;
  font-family: var(--font);
  white-space: pre;
  margin-bottom: 1rem;
}

.splash-meta {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-wrap: wrap;
  font-size: 0.75rem;
  color: var(--text-dim);
}

.splash-status {
  color: var(--amber);
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.splash-sep {
  color: var(--amber-dim);
}

.splash-item {
  color: var(--text-dim);
}

.splash-link {
  color: var(--amber);
  font-size: 0.75rem;
}

/* pulsing dot */
.pulse {
  color: var(--amber-hi);
  animation: blink 1.4s step-end infinite;
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50%       { opacity: 0.2; }
}


/* ── page sections ── */

.page-section {
  padding-top: 2.5rem;
  border-bottom: 1px solid var(--amber-lo);
  padding-bottom: 1.5rem;
}

.page-section:last-child {
  border-bottom: none;
}

.section-label {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--amber-hi);
  letter-spacing: 0.18em;
  margin-bottom: 1rem;
}

.section-note {
  font-weight: 400;
  color: var(--text-dim);
  letter-spacing: 0.04em;
  font-style: italic;
}


/* ── terminal ── */

.terminal-wrap {
  border: 1px solid var(--amber-dim);
}

.terminal-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.35rem 0.75rem;
  border-bottom: 1px solid var(--amber-dim);
  background: var(--bg3);
}

.terminal-title {
  font-size: 0.7rem;
  color: var(--amber-dim);
  letter-spacing: 0.06em;
}

.stream-status {
  font-size: 0.7rem;
}

.stream-status.connected {
  color: var(--green-ok);
}

.stream-status.connecting {
  color: var(--amber-dim);
}

.terminal-body {
  background: #060503;
  padding: 0.75rem;
  height: 300px;
  overflow-y: auto;
  font-size: 0.78rem;
  line-height: 1.55;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-all;
}

.term-line {
  display: block;
}


/* ── state grid ── */

.state-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1.5rem;
}

@media (max-width: 560px) {
  .state-grid { grid-template-columns: 1fr; }
}

.state-block {
  border-left: 2px solid var(--amber-lo);
  padding-left: 1rem;
}

.state-label {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--amber);
  letter-spacing: 0.1em;
  margin-bottom: 0.6rem;
}


/* ── plain list (shared) ── */

.plain-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.plain-list li {
  font-size: 0.82rem;
  line-height: 1.55;
  padding: 0.2rem 0;
  border-bottom: 1px solid var(--amber-lo);
  display: flex;
  align-items: baseline;
  gap: 0.4rem;
  flex-wrap: wrap;
}

.plain-list li:last-child {
  border-bottom: none;
}

.tag {
  font-size: 0.7rem;
  color: var(--text-dim);
  flex-shrink: 0;
}

.item-text {
  color: var(--text-hi);
  flex: 1;
  min-width: 0;
}

.item-date {
  font-size: 0.7rem;
  color: var(--text-dim);
  flex-shrink: 0;
}

.empty-state {
  font-size: 0.8rem;
  color: var(--text-dim);
  font-style: italic;
}


/* ── data table ── */

.data-table {
  border-collapse: collapse;
  width: 100%;
  max-width: 560px;
}

.data-table tr {
  border-bottom: 1px solid var(--amber-lo);
}

.data-table tr:last-child {
  border-bottom: none;
}

.dt-label {
  font-size: 0.75rem;
  color: var(--text-dim);
  padding: 0.3rem 0;
  width: 10rem;
  vertical-align: baseline;
}

.dt-sep {
  font-size: 0.75rem;
  color: var(--amber-lo);
  padding: 0.3rem 0.75rem;
  vertical-align: baseline;
}

.dt-value {
  font-size: 0.82rem;
  color: var(--amber);
  padding: 0.3rem 0;
  vertical-align: baseline;
}

.val-ok { color: var(--green-ok); }
.val-err { color: var(--red-err); }


/* ── journal ── */

.log-feed {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.log-entry {
  border-left: 2px solid var(--amber-lo);
  padding-left: 1rem;
}

.log-meta {
  font-size: 0.68rem;
  color: var(--text-dim);
  letter-spacing: 0.06em;
  margin-bottom: 0.2rem;
}

.log-title {
  font-size: 0.9rem;
  color: var(--amber-hi);
  font-weight: 500;
  margin-bottom: 0.35rem;
  line-height: 1.4;
}

.log-body {
  font-size: 0.8rem;
  color: var(--text);
  line-height: 1.7;
}


/* ── goals ── */

.goals-section {
  margin-bottom: 1.25rem;
}

.goals-group-hdr {
  font-size: 0.7rem;
  color: var(--amber-dim);
  letter-spacing: 0.1em;
  margin-bottom: 0.4rem;
}

.goal-active .goal-prefix { color: var(--amber); }
.goal-backlog .goal-prefix { color: var(--text-dim); }
.goal-done .goal-prefix { color: var(--green-ok); }

.goal-prefix {
  flex-shrink: 0;
  width: 1rem;
  text-align: center;
  font-size: 0.8rem;
}

.strike {
  text-decoration: line-through;
  text-decoration-color: var(--amber-dim);
  color: var(--text-dim) !important;
}


/* ── identity ── */

.id-block {
  border-left: 2px solid var(--amber-dim);
  padding-left: 1rem;
  margin-bottom: 1.25rem;
}

.id-line {
  font-size: 0.85rem;
  color: var(--text);
  line-height: 1.7;
  margin-bottom: 0.25rem;
}

.id-rules {
  list-style: none;
  counter-reset: rules;
  padding: 0;
}

.id-rules li {
  counter-increment: rules;
  position: relative;
  padding-left: 2.5rem;
  margin-bottom: 0.6rem;
  font-size: 0.82rem;
  color: var(--text);
  line-height: 1.65;
}

.id-rules li::before {
  content: counter(rules, decimal-leading-zero) ".";
  position: absolute;
  left: 0;
  color: var(--text-dim);
  font-size: 0.72rem;
  top: 0.15rem;
}


/* ── footer ── */

.site-footer {
  border-top: 1px solid var(--amber-lo);
  margin-top: 3rem;
}

.footer-inner {
  max-width: 900px;
  width: 94%;
  margin: 0 auto;
  padding: 1.5rem 0 3rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.footer-text {
  font-size: 0.72rem;
  color: var(--text-dim);
}

.footer-links {
  display: flex;
  gap: 1.5rem;
}

.footer-links a {
  font-size: 0.72rem;
  color: var(--text-dim);
}

.footer-links a:hover {
  color: var(--amber);
  text-decoration: none;
}


/* ── responsive ── */

@media (max-width: 600px) {
  .header-nav {
    gap: 1rem;
  }
  .header-nav a:nth-child(n+4) {
    display: none;
  }
  .splash-art {
    font-size: 0.72rem;
  }
  .footer-inner {
    flex-direction: column;
    align-items: flex-start;
  }
}

/* ── memory context panel ── */

.memory-query { font-size: 0.78rem; color: var(--text-dim); margin: 0.2rem 0 0.4rem; }
.memory-score { font-size: 0.72rem; color: var(--amber); margin-left: 0.4rem; }
.memory-list li { margin-bottom: 0.5rem; }
"""



# ── Build ──


def build():
    day_count = 0
    try:
        day_count = int(read_file("DAY_COUNT").split()[0])
    except (ValueError, AttributeError):
        pass

    metrics = parse_metrics(read_file("METRICS.md"))
    goals = parse_goals(read_file("GOALS.md"))
    open_predictions = parse_open_predictions()
    containers = get_docker_containers()

    # Memory context: query for the first active goal title
    active_goals = goals.get("active", [])
    memory_query = active_goals[0]["text"] if active_goals else ""
    memory_results = get_memory_context(memory_query)
    memory_context_html = render_memory_context(memory_results, memory_query)

    stats_html = render_stats(metrics)
    patterns_html = render_metrics_patterns(metrics)
    live_state_html = render_live_state(goals, open_predictions, memory_context_html)
    containers_html = render_containers(containers)
    journal_html = render_journal(parse_journal(read_file("JOURNAL.md")))
    goals_html = render_goals(goals)
    identity_html = render_identity(parse_identity(read_file("IDENTITY.md")))

    page = HTML_TEMPLATE.format(
        day_count=day_count,
        live_state_html=live_state_html,
        stats_html=stats_html,
        patterns_html=patterns_html,
        containers_html=containers_html,
        journal_html=journal_html,
        goals_html=goals_html,
        identity_html=identity_html,
    )

    DOCS.mkdir(exist_ok=True)
    (DOCS / "index.html").write_text(page)
    (DOCS / "style.css").write_text(CSS)
    (DOCS / ".nojekyll").touch()

    observations = get_all_observations()
    obs_page = render_observations_page(observations)
    (DOCS / "observations.html").write_text(obs_page)

    n_open_preds = len(open_predictions)
    n_active_goals = len(goals["active"])
    n_containers = len(containers)
    print(f"Site built: docs/index.html (Day {day_count}, {len(metrics)} sessions, "
          f"{n_active_goals} active goals, {n_open_preds} open predictions, "
          f"{n_containers} containers, {len(observations)} observations)")


if __name__ == "__main__":
    build()
