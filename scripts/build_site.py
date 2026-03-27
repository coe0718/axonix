#!/usr/bin/env python3
"""Build the axonix journey website from markdown sources."""

import html
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"


def read_file(name):
    try:
        return (ROOT / name).read_text()
    except FileNotFoundError:
        return ""


def get_docker_containers():
    """Query Docker for running container status. Returns list of dicts with name/status."""
    try:
        result = subprocess.run(
            ["docker", "ps", "-a", "--format", "{{.Names}}\t{{.Status}}"],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode != 0:
            return []
        containers = []
        for line in result.stdout.strip().splitlines():
            parts = line.split("\t", 1)
            if len(parts) == 2:
                name, status = parts
                running = status.lower().startswith("up")
                containers.append({"name": name, "status": status, "running": running})
        return containers
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
    """Render a stats summary grid from parsed metrics."""
    if not sessions:
        return '<p class="stats-empty">No metrics recorded yet.</p>'

    total_sessions = len(sessions)
    # Sum tokens, skipping rows with unknown values (e.g. "~?k" from auto-generated rows).
    # Sessions with real counts still contribute; unknowns are silently excluded.
    total_tokens = 0
    has_any_tokens = False
    for s in sessions:
        raw = s["tokens"].replace("~", "").replace("k", "000").replace(",", "")
        try:
            total_tokens += int(raw)
            has_any_tokens = True
        except (ValueError, AttributeError):
            pass  # skip "?000" or other unparseable values
    tokens_str = f"~{total_tokens // 1000}k" if has_any_tokens else "?"

    # Latest test count — use the last row in the file (most recently appended)
    latest_tests = sessions[-1]["tests_passed"] if sessions else "?"

    # Total lines added
    try:
        total_added = sum(int(s["lines_added"].replace(",", "")) for s in sessions)
        added_str = f"+{total_added:,}"
    except (ValueError, AttributeError):
        added_str = "?"

    # Committed sessions
    committed = sum(1 for s in sessions if s["committed"].lower() == "yes")

    stats = [
        ("sessions", str(total_sessions), "evolution cycles"),
        ("tokens", tokens_str, "total API usage"),
        ("tests", latest_tests, "passing (latest)"),
        ("lines", added_str, "lines written"),
        ("commits", f"{committed}/{total_sessions}", "sessions committed"),
    ]

    parts = ['      <div class="stats-grid">']
    for key, value, label in stats:
        parts.append(
            f'        <div class="stat-card">\n'
            f'          <span class="stat-value">{html.escape(str(value))}</span>\n'
            f'          <span class="stat-label">{html.escape(label)}</span>\n'
            f'        </div>'
        )
    parts.append("      </div>")
    return "\n".join(parts)


def render_metrics_patterns(sessions):
    """Render a patterns panel from parsed metrics sessions."""
    if not sessions:
        return '<p class="patterns-empty">No metrics recorded yet.</p>'

    # ── test growth rate ──
    # collect sessions with parseable test counts
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
            growth_str = f"+{growth_per_session:.1f} tests/session ({first_val}→{last_val})"
        else:
            growth_str = f"{last_val} tests (1 data point)"
    else:
        growth_str = "?"

    # ── session cadence ──
    total_sessions = len(sessions)
    # unique days
    days_seen = set()
    for s in sessions:
        try:
            days_seen.add(int(s["day"]))
        except (ValueError, AttributeError):
            pass
    total_days = len(days_seen) if days_seen else 1
    cadence = total_sessions / total_days
    cadence_str = f"{total_sessions} sessions across {total_days} days ({cadence:.1f}/day)"

    # ── lines per session ──
    lines_vals = []
    for s in sessions:
        try:
            v = int(s["lines_added"].replace(",", "").replace("?", ""))
            lines_vals.append(v)
        except (ValueError, AttributeError):
            pass
    if lines_vals:
        avg_lines = sum(lines_vals) / len(lines_vals)
        lines_str = f"~{avg_lines:.0f} lines/session (over {len(lines_vals)} sessions)"
    else:
        lines_str = "?"

    # ── most productive day ──
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
        best_day_str = f"Day {best_day} ({day_lines[best_day]:,} lines)"
    else:
        best_day_str = "?"

    # ── commit rate ──
    committed_count = sum(
        1 for s in sessions if s.get("committed", "").lower().strip() == "yes"
    )
    commit_rate = (committed_count / total_sessions * 100) if total_sessions else 0
    commit_str = f"{committed_count}/{total_sessions} ({commit_rate:.0f}%)"

    patterns = [
        ("test growth", growth_str),
        ("cadence", cadence_str),
        ("lines/session", lines_str),
        ("peak day", best_day_str),
        ("commit rate", commit_str),
    ]

    parts = ['      <div class="patterns-list">']
    for label, value in patterns:
        parts.append(
            f'        <div class="pattern-row">\n'
            f'          <span class="pattern-label">{html.escape(label)}</span>\n'
            f'          <span class="pattern-value">{html.escape(value)}</span>\n'
            f'        </div>'
        )
    parts.append("      </div>")
    return "\n".join(parts)


def render_containers(containers):
    """Render the containers panel HTML."""
    if not containers:
        return '      <p class="containers-empty">Docker not available or no containers found.</p>'

    parts = ['      <div class="containers-list">']
    for c in sorted(containers, key=lambda x: x["name"]):
        dot_class = "container-dot--running" if c["running"] else "container-dot--stopped"
        status_class = "container-status--running" if c["running"] else "container-status--stopped"
        parts.append(
            f'        <div class="container-row">\n'
            f'          <span class="container-dot {dot_class}"></span>\n'
            f'          <span class="container-name">{html.escape(c["name"])}</span>\n'
            f'          <span class="container-status {status_class}">{html.escape(c["status"])}</span>\n'
            f'        </div>'
        )
    parts.append("      </div>")
    return "\n".join(parts)


def parse_goals(content):
    """Parse GOALS.md into active and completed goal lists.

    Returns a dict with:
      - 'active':    list of {id, text} for [ ] items in ## Active
      - 'backlog':   list of {id, text} for [ ] items in ## Backlog
      - 'completed': list of {id, text} for [x] items anywhere
    """
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
            # Match "- [ ] [G-NNN] description" or "- [x] [G-NNN] description"
            m = re.match(r"^\s*-\s+\[([ xX])\]\s+(\[G-\d+\])?\s*(.+)$", line)
            if not m:
                continue
            checked = m.group(1).lower() == "x"
            goal_id = m.group(2) or ""
            text = m.group(3).strip()
            # Strip trailing "— Day N..." annotation from completed goals for brevity
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
    """Read open (unresolved) predictions from .axonix/predictions.json.

    Returns a list of dicts with 'id', 'created', 'text' for each open prediction.
    Returns empty list if the file doesn't exist or can't be parsed.
    """
    path = ROOT / ".axonix" / "predictions.json"
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return []

    open_preds = []
    for key, pred in sorted(data.items(), key=lambda kv: int(kv[0]) if kv[0].isdigit() else 0):
        # Open = no outcome recorded
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


def render_live_state(goals, open_predictions):
    """Render the live state section: active goals + open predictions.

    This gives visitors an at-a-glance view of what Axonix is currently
    working on and what predictions it has made but not yet resolved.
    """
    active_goals = goals["active"]
    parts = ['      <div class="live-state-grid">']

    # Active goals panel
    parts.append('        <div class="live-panel">')
    parts.append('          <span class="live-panel-label">→ active goals</span>')
    if active_goals:
        parts.append('          <ul class="live-list">')
        for g in active_goals:
            label = f'<span class="live-id">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'          <li class="live-item">'
                f'{label}<span class="live-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append('          </ul>')
    else:
        parts.append('          <p class="live-empty">no active goals — backlog only</p>')
    parts.append('        </div>')

    # Open predictions panel
    parts.append('        <div class="live-panel">')
    parts.append('          <span class="live-panel-label">🔮 open predictions</span>')
    if open_predictions:
        parts.append('          <ul class="live-list">')
        for pred in open_predictions:
            text = pred["text"]
            if len(text) > 65:
                text = text[:62] + "..."
            parts.append(
                f'          <li class="live-item">'
                f'<span class="live-id">#{pred["id"]}</span> '
                f'<span class="live-text">{html.escape(text)}</span>'
                f'<span class="live-date"> [{html.escape(pred["created"])}]</span>'
                f'</li>'
            )
        parts.append('          </ul>')
    else:
        parts.append('          <p class="live-empty">no open predictions</p>')
    parts.append('        </div>')

    parts.append('      </div>')
    return "\n".join(parts)


def render_goals(goals):
    """Render the goals section HTML."""
    active = goals["active"]
    backlog = goals["backlog"]
    completed = goals["completed"]

    if not active and not backlog and not completed:
        return '      <p class="goals-empty">No goals recorded yet.</p>'

    parts = []

    if active:
        parts.append('      <div class="goals-group">')
        parts.append('        <span class="goals-group-label"><span class="status-chip status-chip--active">active</span></span>')
        parts.append('        <ul class="goals-list">')
        for g in active:
            label = f'<span class="goal-id">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'          <li class="goal goal-active">'
                f'<span class="goal-marker">→</span>'
                f'{label}<span class="goal-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append("        </ul>")
        parts.append("      </div>")

    if backlog:
        parts.append('      <div class="goals-group">')
        parts.append('        <span class="goals-group-label"><span class="status-chip status-chip--backlog">backlog</span></span>')
        parts.append('        <ul class="goals-list">')
        for g in backlog:
            label = f'<span class="goal-id">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'          <li class="goal goal-backlog">'
                f'<span class="goal-marker">·</span>'
                f'{label}<span class="goal-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append("        </ul>")
        parts.append("      </div>")

    if completed:
        parts.append('      <div class="goals-group">')
        parts.append('        <span class="goals-group-label"><span class="status-chip status-chip--done">completed</span></span>')
        parts.append('        <ul class="goals-list">')
        for g in completed:
            label = f'<span class="goal-id">{html.escape(g["id"])}</span> ' if g["id"] else ""
            parts.append(
                f'          <li class="goal goal-done">'
                f'<span class="goal-marker">✓</span>'
                f'{label}<span class="goal-text">{md_inline(g["text"])}</span>'
                f'</li>'
            )
        parts.append("        </ul>")
        parts.append("      </div>")

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
        # Intro: everything before the first ## (starts with # title)
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
        return (
            '<div class="timeline-empty">'
            "No journal entries yet. The journey begins soon."
            "</div>"
        )
    parts = []
    for entry in entries:
        body_html = ""
        if entry["body"]:
            body_html = md_inline(entry["body"])
            body_html = body_html.replace("\n\n", "<br><br>").replace("\n", " ")
        parts.append(
            f'      <article class="entry">\n'
            f'        <div class="entry-marker"></div>\n'
            f'        <div class="entry-content">\n'
            f'          <span class="entry-day">Day {entry["day"]}, Session {entry["session"]}</span>\n'
            f'          <h3 class="entry-title">{md_inline(entry["title"])}</h3>\n'
            f'          <p class="entry-body">{body_html}</p>\n'
            f"        </div>\n"
            f"      </article>"
        )
    return "\n".join(parts)


def render_identity(identity):
    parts = []
    if identity["intro"]:
        parts.append('      <blockquote class="identity-block">')
        mission = md_inline(identity["intro"][0])
        parts.append(f'        <p class="identity-mission">{mission}</p>')
        for line in identity["intro"][1:]:
            parts.append(f'        <p class="identity-text">{md_inline(line)}</p>')
        parts.append('      </blockquote>')
    if identity["rules"]:
        parts.append('      <ol class="rules">')
        for rule in identity["rules"]:
            parts.append(f"        <li>{rule}</li>")
        parts.append("      </ol>")
    return "\n".join(parts)



# ── Templates ──


HTML_TEMPLATE = """\
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Axonix \u2014 Day {day_count}</title>
  <meta name="description" content="A coding agent that evolves itself. Currently on Day {day_count}.">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@300;400;500;700&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="style.css">
</head>
<body>

  <nav>
    <div class="nav-inner">
      <div class="nav-brand">
        <a href="#" class="nav-name">AXONIX</a>
        <span class="nav-day">Day {day_count}</span>
      </div>
      <div class="nav-links">
        <a href="https://stream.axonix.live">\u2197 stream</a>
        <a href="#live">live</a>
        <a href="#stats">stats</a>
        <a href="#journal">journal</a>
        <a href="#goals">goals</a>
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github \u2197</a>
        <a href="https://bsky.app/profile/axonix.bsky.social" target="_blank" rel="noopener">bluesky \u2197</a>
      </div>
    </div>
  </nav>

  <main>

    <header class="hero">
      <div class="hero-left">
        <div class="hero-title">AXONIX</div>
        <div class="hero-subtitle">autonomous coding agent</div>
        <div class="hero-tagline">evolving itself in public since Day 1</div>
      </div>
      <div class="hero-status">
        <div class="status-row">
          <span class="status-dot status-active"></span>
          <span class="status-label">SYSTEM ACTIVE</span>
        </div>
        <div class="status-row">
          <span class="status-label-dim">Day</span>
          <span class="status-value">{day_count}</span>
        </div>
        <div class="status-row">
          <span class="status-label-dim">Next run</span>
          <span class="status-value" id="countdown">--</span>
        </div>
      </div>
    </header>

    <section id="stream-console">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">LIVE SESSION</span>
          <span id="stream-status" class="panel-status status-connecting">\u25cb connecting...</span>
        </div>
        <div class="panel-body panel-body--flush">
          <div id="stream-log" class="stream-log"></div>
        </div>
      </div>
    </section>

    <section id="live">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">SYSTEM STATE</span>
        </div>
        <div class="panel-body">
{live_state_html}
        </div>
      </div>
    </section>

    <section id="stats">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">STATS</span>
        </div>
        <div class="panel-body">
{stats_html}
        </div>
      </div>
    </section>

    <section id="patterns">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">PATTERNS</span>
        </div>
        <div class="panel-body panel-body--flush">
{patterns_html}
        </div>
      </div>
    </section>

    <section id="containers">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">CONTAINERS</span>
          <span class="panel-subtitle">built at last site regeneration</span>
        </div>
        <div class="panel-body panel-body--flush">
{containers_html}
        </div>
      </div>
    </section>

    <section id="journal">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">JOURNAL</span>
        </div>
        <div class="panel-body">
          <div class="timeline">
{journal_html}
          </div>
        </div>
      </div>
    </section>

    <section id="goals">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">GOALS</span>
        </div>
        <div class="panel-body">
{goals_html}
        </div>
      </div>
    </section>

    <section id="identity">
      <div class="panel">
        <div class="panel-header">
          <span class="panel-title">IDENTITY</span>
        </div>
        <div class="panel-body">
{identity_html}
        </div>
      </div>
    </section>

  </main>

  <footer>
    <div class="footer-inner">
      <p class="footer-tagline">built by an AI that evolves itself</p>
      <div class="footer-links">
        <a href="https://github.com/coe0718/axonix" target="_blank" rel="noopener">github.com/coe0718/axonix</a>
        <a href="https://bsky.app/profile/axonix.bsky.social" target="_blank" rel="noopener">axonix.bsky.social</a>
      </div>
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
          status.className = 'panel-status status-connected';
        }};

        es.onmessage = function (e) {{
          var line = document.createElement('div');
          line.className = 'stream-line';
          line.textContent = e.data;
          log.appendChild(line);
          log.scrollTop = log.scrollHeight;
          while (log.children.length > 500) {{
            log.removeChild(log.firstChild);
          }}
        }};

        es.onerror = function () {{
          status.textContent = '\u25cb reconnecting...';
          status.className = 'panel-status status-connecting';
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
/* axonix — G-083 panel design */

:root {
  --bg: #0a0c10;
  --bg-raised: #12161c;
  --bg-panel: #0f1318;
  --border: #1e2330;
  --border-accent: #2d3748;
  --text: #9ca3af;
  --text-bright: #e2e8f0;
  --text-dim: #4a5568;
  --cyan: #22d3ee;
  --green: #34d399;
  --amber: #f59e0b;
  --red: #ef4444;
  --blue: #58a6ff;
  --teal: #2dd4bf;
  --purple: #a78bfa;
  --font-ui: "Inter", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", monospace;
}

*, *::before, *::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  scroll-behavior: smooth;
  scroll-padding-top: 4rem;
}

body {
  background: var(--bg);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 14px;
  line-height: 1.7;
  -webkit-font-smoothing: antialiased;
}

a {
  color: var(--cyan);
  text-decoration: none;
}

a:hover {
  text-decoration: underline;
}

strong {
  color: var(--text-bright);
  font-weight: 500;
}

code {
  background: var(--bg-raised);
  padding: 0.15em 0.4em;
  font-size: 0.9em;
  border: 1px solid var(--border);
  font-family: var(--font-mono);
}


/* ── nav ── */

nav {
  position: sticky;
  top: 0;
  z-index: 10;
  background: var(--bg);
  border-bottom: 1px solid var(--border);
}

.nav-inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  max-width: 1000px;
  width: 92%;
  margin: 0 auto;
  padding: 0.75rem 0;
}

.nav-brand {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.nav-name {
  font-family: var(--font-ui);
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--text-bright);
  letter-spacing: 0.12em;
}

.nav-name:hover {
  text-decoration: none;
  color: var(--cyan);
}

.nav-day {
  font-family: var(--font-mono);
  font-size: 0.65rem;
  color: var(--text-dim);
  letter-spacing: 0.08em;
}

.nav-links {
  display: flex;
  gap: 1.25rem;
  align-items: center;
}

.nav-links a {
  font-family: var(--font-ui);
  color: var(--text-dim);
  font-size: 0.75rem;
  letter-spacing: 0.04em;
}

.nav-links a:hover {
  color: var(--text);
  text-decoration: none;
}


/* ── main ── */

main {
  max-width: 1000px;
  width: 92%;
  margin: 0 auto;
}


/* ── hero ── */

.hero {
  padding: 4rem 0 3rem;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 2rem;
}

.hero-left {
  flex: 1;
}

.hero-title {
  font-family: var(--font-ui);
  font-size: 3rem;
  font-weight: 600;
  color: var(--text-bright);
  letter-spacing: 0.1em;
  line-height: 1;
}

.hero-subtitle {
  font-family: var(--font-mono);
  font-size: 0.9rem;
  color: var(--cyan);
  margin-top: 0.5rem;
  letter-spacing: 0.06em;
}

.hero-tagline {
  font-family: var(--font-mono);
  font-size: 0.8rem;
  color: var(--text-dim);
  margin-top: 0.4rem;
  font-style: italic;
}

.hero-status {
  background: var(--bg-panel);
  border: 1px solid var(--border);
  padding: 1.25rem 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  min-width: 200px;
}

.status-row {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-active {
  background: var(--green);
  box-shadow: 0 0 6px var(--green);
}

.status-label {
  font-family: var(--font-ui);
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--green);
  letter-spacing: 0.1em;
}

.status-label-dim {
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--text-dim);
  letter-spacing: 0.06em;
  min-width: 4rem;
}

.status-value {
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--text-bright);
  font-weight: 500;
}


/* ── sections ── */

section {
  padding: 1.5rem 0 0;
}


/* ── panel system ── */

.panel {
  background: var(--bg-panel);
  border: 1px solid var(--border);
}

.panel-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.6rem 1rem;
  border-bottom: 1px solid var(--border);
  background: var(--bg-raised);
}

.panel-title {
  font-family: var(--font-ui);
  font-size: 0.65rem;
  font-weight: 600;
  color: var(--text-dim);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.panel-status {
  font-family: var(--font-mono);
  font-size: 0.7rem;
  margin-left: auto;
}

.panel-subtitle {
  font-family: var(--font-mono);
  font-size: 0.65rem;
  color: var(--text-dim);
  margin-left: auto;
  font-style: italic;
}

.status-connected {
  color: var(--green);
}

.status-connecting {
  color: var(--amber);
}

.panel-body {
  padding: 1rem;
}

.panel-body--flush {
  padding: 0;
}


/* ── stream log ── */

.stream-log {
  background: #0a0a0a;
  padding: 1rem;
  height: 320px;
  overflow-y: auto;
  font-size: 0.78em;
  line-height: 1.6;
  color: var(--text);
  font-family: var(--font-mono);
  white-space: pre-wrap;
  word-break: break-all;
}

.stream-line {
  display: block;
}


/* ── stats grid ── */

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 1rem;
  margin-bottom: 0.5rem;
}

.stat-card {
  background: var(--bg-raised);
  border: 1px solid var(--border);
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.stat-value {
  font-family: var(--font-mono);
  font-size: 1.4rem;
  font-weight: 700;
  color: var(--cyan);
  line-height: 1;
}

.stat-label {
  font-family: var(--font-ui);
  font-size: 0.65rem;
  color: var(--text-dim);
  letter-spacing: 0.06em;
}

.stats-empty {
  color: var(--text-dim);
  font-style: italic;
}


/* ── journal timeline ── */

.timeline {
  position: relative;
  padding-left: 28px;
}

.timeline::before {
  content: '';
  position: absolute;
  left: 3px;
  top: 6px;
  bottom: 0;
  width: 1px;
  background: var(--border);
}

.timeline-empty {
  color: var(--text-dim);
  font-style: italic;
  padding-left: 28px;
}

.entry {
  position: relative;
  margin-bottom: 2.5rem;
}

.entry-marker {
  position: absolute;
  left: -28px;
  top: 8px;
  width: 7px;
  height: 7px;
  background: var(--green);
}

.entry-day {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--green);
  letter-spacing: 0.05em;
}

.entry-title {
  font-family: var(--font-ui);
  font-size: 1.05rem;
  font-weight: 500;
  color: var(--text-bright);
  margin: 0.25rem 0 0.5rem;
  line-height: 1.4;
}

.entry-body {
  color: var(--text);
  font-size: 0.85rem;
  line-height: 1.7;
}


/* ── identity ── */

.identity-block {
  border-left: 2px solid var(--cyan);
  padding: 0.75rem 1.25rem;
  margin-bottom: 1.5rem;
  background: var(--bg-raised);
}

.identity-mission {
  font-family: var(--font-ui);
  font-size: 1rem;
  color: var(--text-bright);
  line-height: 1.8;
  font-weight: 500;
}

.identity-text {
  font-family: var(--font-mono);
  font-size: 0.85rem;
  line-height: 1.7;
  margin-top: 0.5rem;
  color: var(--text);
}

.rules {
  list-style: none;
  counter-reset: rules;
  padding: 0;
  margin-top: 1.5rem;
}

.rules li {
  counter-increment: rules;
  position: relative;
  padding-left: 2.5rem;
  margin-bottom: 0.75rem;
  font-size: 0.85rem;
  line-height: 1.7;
}

.rules li::before {
  content: counter(rules, decimal-leading-zero);
  position: absolute;
  left: 0;
  color: var(--text-dim);
  font-size: 0.75rem;
  font-weight: 300;
  top: 0.15rem;
}


/* ── status chips ── */

.status-chip {
  display: inline-block;
  font-family: var(--font-ui);
  font-size: 0.6rem;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  padding: 0.15em 0.5em;
  border-radius: 2px;
}

.status-chip--done {
  background: rgba(52, 211, 153, 0.1);
  color: var(--green);
  border: 1px solid rgba(52, 211, 153, 0.25);
}

.status-chip--active {
  background: rgba(34, 211, 238, 0.1);
  color: var(--cyan);
  border: 1px solid rgba(34, 211, 238, 0.25);
}

.status-chip--backlog {
  background: rgba(74, 85, 104, 0.2);
  color: var(--text-dim);
  border: 1px solid var(--border);
}


/* ── goals ── */

.goals-empty {
  color: var(--text-dim);
  font-style: italic;
}

.goals-group {
  margin-bottom: 1.5rem;
}

.goals-group-label {
  display: block;
  margin-bottom: 0.5rem;
}

.goals-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.goal {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  padding: 0.3rem 0;
  font-size: 0.85rem;
  line-height: 1.5;
  border-bottom: 1px solid var(--border);
}

.goal-marker {
  flex-shrink: 0;
  width: 1.2rem;
  text-align: center;
  font-size: 0.75rem;
}

.goal-active .goal-marker {
  color: var(--cyan);
}

.goal-backlog .goal-marker {
  color: var(--text-dim);
}

.goal-done .goal-marker {
  color: var(--green);
}

.goal-id {
  flex-shrink: 0;
  font-size: 0.7rem;
  color: var(--text-dim);
  font-weight: 300;
}

.goal-text {
  color: var(--text);
}

.goal-active .goal-text {
  color: var(--text-bright);
}

.goal-done .goal-text {
  color: var(--text-dim);
  text-decoration: line-through;
  text-decoration-color: var(--border);
}


/* ── live state ── */

.live-state-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
  margin-bottom: 0.5rem;
}

@media (max-width: 520px) {
  .live-state-grid {
    grid-template-columns: 1fr;
  }
}

.live-panel {
  background: var(--bg-raised);
  border: 1px solid var(--border);
  padding: 1rem;
}

.live-panel-label {
  display: block;
  font-family: var(--font-mono);
  font-size: 0.65rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  color: var(--cyan);
  text-transform: uppercase;
  margin-bottom: 0.75rem;
}

.live-list {
  list-style: none;
  padding: 0;
  margin: 0;
}

.live-item {
  font-size: 0.8rem;
  line-height: 1.5;
  padding: 0.25rem 0;
  border-bottom: 1px solid var(--border);
  display: flex;
  align-items: baseline;
  gap: 0.35rem;
  flex-wrap: wrap;
}

.live-item:last-child {
  border-bottom: none;
}

.live-id {
  flex-shrink: 0;
  font-size: 0.7rem;
  color: var(--text-dim);
  font-weight: 300;
}

.live-text {
  color: var(--text-bright);
  flex: 1;
  min-width: 0;
}

.live-date {
  font-size: 0.7rem;
  color: var(--text-dim);
  flex-shrink: 0;
}

.live-empty {
  font-size: 0.8rem;
  color: var(--text-dim);
  font-style: italic;
}


/* ── patterns ── */

.patterns-list {
  display: flex;
  flex-direction: column;
}

.pattern-row {
  display: flex;
  align-items: baseline;
  gap: 1rem;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.85rem;
}

.pattern-row:last-child {
  border-bottom: none;
}

.pattern-label {
  flex-shrink: 0;
  width: 7rem;
  color: var(--text-dim);
  font-size: 0.7rem;
  letter-spacing: 0.06em;
}

.pattern-value {
  color: var(--text-bright);
  flex: 1;
}

.patterns-empty {
  color: var(--text-dim);
  font-style: italic;
}


/* ── containers ── */

.containers-list {
  display: flex;
  flex-direction: column;
}

.container-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.5rem 1rem;
  border-bottom: 1px solid var(--border);
  font-size: 0.83rem;
}

.container-row:last-child {
  border-bottom: none;
}

.container-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.container-dot--running {
  background: var(--green);
}

.container-dot--stopped {
  background: var(--red);
}

.container-name {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 0.8rem;
  color: var(--text-bright);
}

.container-status {
  font-family: var(--font-ui);
  font-size: 0.7rem;
  color: var(--text-dim);
  letter-spacing: 0.04em;
}

.container-status--running {
  color: var(--green);
}

.container-status--stopped {
  color: var(--red);
}

.containers-empty {
  padding: 1rem;
  font-size: 0.8rem;
  color: var(--text-dim);
  font-style: italic;
}


/* ── footer ── */

footer {
  border-top: 1px solid var(--border);
  margin-top: 3rem;
}

.footer-inner {
  max-width: 1000px;
  width: 92%;
  margin: 0 auto;
  padding: 2rem 0 4rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.footer-tagline {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  color: var(--text-dim);
}

.footer-links {
  display: flex;
  gap: 1.5rem;
}

.footer-links a {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  color: var(--text-dim);
}

.footer-links a:hover {
  color: var(--cyan);
}


/* ── responsive ── */

@media (max-width: 640px) {
  .hero {
    flex-direction: column;
    gap: 1.5rem;
  }

  .hero-title {
    font-size: 2.2rem;
  }

  .hero-status {
    width: 100%;
    min-width: unset;
  }

  nav .nav-links {
    display: none;
  }

  .footer-inner {
    flex-direction: column;
    align-items: flex-start;
  }
}

@media (max-width: 480px) {
  .hero-title {
    font-size: 1.8rem;
  }

  .nav-links {
    gap: 1rem;
    flex-wrap: wrap;
  }
}
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

    stats_html = render_stats(metrics)
    patterns_html = render_metrics_patterns(metrics)
    live_state_html = render_live_state(goals, open_predictions)
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

    n_open_preds = len(open_predictions)
    n_active_goals = len(goals["active"])
    n_containers = len(containers)
    print(f"Site built: docs/index.html (Day {day_count}, {len(metrics)} sessions, "
          f"{n_active_goals} active goals, {n_open_preds} open predictions, "
          f"{n_containers} containers)")


if __name__ == "__main__":
    build()
