#!/usr/bin/env python3
"""
analyze_metrics.py — Parse METRICS.md and produce a concise pattern analysis.

Usage:
    python3 scripts/analyze_metrics.py           # run analysis
    python3 scripts/analyze_metrics.py --test    # run unit tests
"""

import sys
import re
from collections import Counter
from typing import Optional


# ── Data model ────────────────────────────────────────────────────────────────

class SessionRow:
    """A single parsed row from METRICS.md."""

    def __init__(
        self,
        day: int,
        date: str,
        tokens_known: bool,
        tests_passed: Optional[int],
        tests_failed: Optional[int],
        files_changed: Optional[int],
        lines_added: Optional[int],
        lines_removed: Optional[int],
        committed: str,
        notes: str,
    ):
        self.day = day
        self.date = date
        self.tokens_known = tokens_known
        self.tests_passed = tests_passed
        self.tests_failed = tests_failed
        self.files_changed = files_changed
        self.lines_added = lines_added
        self.lines_removed = lines_removed
        self.committed = committed
        self.notes = notes


# ── Parsing ───────────────────────────────────────────────────────────────────

_SENTINEL_PATTERN = re.compile(r'^\?|^~?\?k?$')


def _is_sentinel(val: str) -> bool:
    """Return True if value is unknown (?, ~?k, etc.)."""
    v = val.strip()
    return v == '?' or v == '~?k' or v.startswith('~?') or v == ''


def _parse_int(val: str) -> Optional[int]:
    """Parse an integer field; return None if sentinel or unparseable."""
    v = val.strip()
    if _is_sentinel(v):
        return None
    # Strip leading ~ and trailing k (for token fields like ~20k)
    v = v.lstrip('~').rstrip('k')
    try:
        return int(v)
    except ValueError:
        return None


def parse_metrics_table(text: str) -> list:
    """Parse METRICS.md content and return list of SessionRow objects.

    Supports both the old 10-column format (no Session column) and the new
    11-column format (with Session as the second column).  The header row is
    inspected to set a col_offset that shifts all subsequent index accesses.

    Skips the header row, separator row, and any malformed rows silently.
    """
    rows = []
    col_offset = 0  # 0 for old format, 1 for new format with Session column
    for line in text.splitlines():
        line = line.strip()
        # Must be a table row
        if not line.startswith('|'):
            continue
        # Remove surrounding pipes and split
        parts = [p.strip() for p in line.strip('|').split('|')]
        # Header row: starts with 'Day' (case-insensitive) — detect format here
        if parts and parts[0].lower() == 'day':
            if len(parts) >= 2 and parts[1].lower() in ('session', 'sess', 's'):
                col_offset = 1
            continue
        # Separator row: all dashes
        if parts and re.match(r'^[-:]+$', parts[0]):
            continue
        # Need at least 9 columns (adjusted for offset):
        # Day [Session] Date Tokens Tests_Passed Tests_Failed Files Lines_Added Lines_Removed Committed
        if len(parts) < 9 + col_offset:
            continue
        try:
            day = int(parts[0])
        except (ValueError, IndexError):
            continue  # skip malformed

        date = parts[1 + col_offset].strip()
        tokens_raw = parts[2 + col_offset].strip()
        tokens_known = not _is_sentinel(tokens_raw)
        tests_passed = _parse_int(parts[3 + col_offset])
        tests_failed = _parse_int(parts[4 + col_offset])
        files_changed = _parse_int(parts[5 + col_offset])
        lines_added = _parse_int(parts[6 + col_offset])
        lines_removed = _parse_int(parts[7 + col_offset])
        committed = parts[8 + col_offset].strip()
        notes = parts[9 + col_offset].strip() if len(parts) > 9 + col_offset else ''

        row = SessionRow(
            day=day,
            date=date,
            tokens_known=tokens_known,
            tests_passed=tests_passed,
            tests_failed=tests_failed,
            files_changed=files_changed,
            lines_added=lines_added,
            lines_removed=lines_removed,
            committed=committed,
            notes=notes,
        )
        rows.append(row)
    return rows


# ── Malformed row detection ───────────────────────────────────────────────────

_SESSION_COL_PATTERN = re.compile(r'^S\d+$')


def detect_malformed_rows(lines: list) -> int:
    """Scan raw METRICS.md lines for data rows where the 2nd column is not S\\d+.

    A malformed row is one written by an old evolve.sh that omits the Session
    column — the 2nd column would be a date (e.g. '2026-03-22') instead of
    something like 'S1'.

    Parameters
    ----------
    lines:  list of raw text lines from METRICS.md

    Returns
    -------
    int — count of malformed rows detected (prints a warning for each)
    """
    count = 0
    for lineno, line in enumerate(lines, start=1):
        line = line.strip()
        if not line.startswith('|'):
            continue
        parts = [p.strip() for p in line.strip('|').split('|')]
        # Skip header and separator rows
        if not parts:
            continue
        if parts[0].lower() == 'day':
            continue
        if re.match(r'^[-:]+$', parts[0]):
            continue
        # Must look like a data row: first column is a day number
        try:
            int(parts[0])
        except (ValueError, IndexError):
            continue
        # Check second column — should match S\d+
        if len(parts) >= 2 and not _SESSION_COL_PATTERN.match(parts[1]):
            print(
                f"  WARNING: METRICS.md line {lineno} appears malformed "
                f"(2nd column is '{parts[1]}', expected S<N>). "
                f"Row may be missing the Session column.",
                file=sys.stderr,
            )
            count += 1
    return count


# ── Analysis ──────────────────────────────────────────────────────────────────

def compute_velocity(rows: list) -> dict:
    """Compute session velocity metrics from a list of SessionRow objects.

    Velocity measures how productive each session is:
    - tests_per_session: average test count added per session (last 5 sessions with known data)
    - trend: "up", "down", or "flat" — compare last 3 vs prior 3 sessions' test deltas
    - sessions_analyzed: how many sessions were used for the calculation

    Returns a dict with keys: tests_per_session, trend, sessions_analyzed.
    Returns None values if insufficient data.
    """
    # Extract test counts from rows that have known values
    known = [(i, r.tests_passed) for i, r in enumerate(rows) if r.tests_passed is not None]
    if len(known) < 2:
        return {"tests_per_session": None, "trend": "unknown", "sessions_analyzed": len(known)}

    # Compute per-session test deltas (consecutive known rows)
    deltas = []
    for i in range(1, len(known)):
        _, t_prev = known[i - 1]
        _, t_curr = known[i]
        deltas.append(t_curr - t_prev)

    # Use last 5 deltas for current velocity
    recent_deltas = deltas[-5:] if len(deltas) >= 5 else deltas
    tests_per_session = sum(recent_deltas) / len(recent_deltas) if recent_deltas else None

    # Trend: compare last 3 deltas vs prior 3 deltas (need at least 4 total)
    trend = "unknown"
    if len(deltas) >= 4:
        half = len(deltas) // 2
        recent_half = deltas[half:]
        prior_half = deltas[:half]
        recent_avg = sum(recent_half) / len(recent_half) if recent_half else 0.0
        prior_avg = sum(prior_half) / len(prior_half) if prior_half else 0.0
        if prior_avg == 0:
            trend = "flat"
        else:
            change_pct = (recent_avg - prior_avg) / abs(prior_avg) * 100
            if change_pct > 5:
                trend = "up"
            elif change_pct < -5:
                trend = "down"
            else:
                trend = "flat"
    elif len(deltas) >= 2:
        # Simple: are we adding more tests recently?
        recent_avg = deltas[-1]
        prior_avg = deltas[0]
        if recent_avg > prior_avg + 1:
            trend = "up"
        elif recent_avg < prior_avg - 1:
            trend = "down"
        else:
            trend = "flat"

    return {
        "tests_per_session": tests_per_session,
        "trend": trend,
        "sessions_analyzed": len(recent_deltas),
    }


def format_velocity(velocity: dict) -> str:
    """Format velocity metrics as a human-readable string."""
    lines = []
    lines.append("  Session Velocity:")
    tps = velocity.get("tests_per_session")
    trend = velocity.get("trend", "unknown")
    n = velocity.get("sessions_analyzed", 0)
    trend_arrow = {"up": "📈", "down": "📉", "flat": "➡️", "unknown": "❓"}.get(trend, "")
    if tps is not None:
        lines.append(f"    Tests added/session  : {tps:+.1f} {trend_arrow} ({trend})")
    else:
        lines.append("    Tests added/session  : (insufficient data)")
    lines.append(f"    Sessions analyzed    : {n}")
    return "\n".join(lines)


def analyze(rows: list) -> dict:
    """Compute pattern analysis from a list of SessionRow objects.

    Returns a dict with the following keys:
        total_sessions          int
        first_tests             Optional[int]
        latest_tests            Optional[int]
        test_net_change         Optional[int]
        test_avg_per_session    Optional[float]
        total_lines_added       int
        total_lines_removed     int
        avg_lines_added         Optional[float]   (per session with known data)
        avg_lines_removed       Optional[float]
        avg_files_changed       Optional[float]
        most_active_day         Optional[int]
        most_active_day_count   int
        zero_failure_sessions   int
        total_sessions_with_failure_data int
        verdict                 str  ("growing" | "stable" | "declining" | "unknown")
    """
    total = len(rows)

    # Test count trend
    known_tests = [(i, r.tests_passed) for i, r in enumerate(rows) if r.tests_passed is not None]
    if known_tests:
        _, first_tests = known_tests[0]
        _, latest_tests = known_tests[-1]
        test_net_change = latest_tests - first_tests
        n_sessions = len(known_tests)
        test_avg_per_session = test_net_change / n_sessions if n_sessions > 1 else 0.0
    else:
        first_tests = latest_tests = test_net_change = None
        test_avg_per_session = None

    # Lines added/removed
    added_known = [r.lines_added for r in rows if r.lines_added is not None]
    removed_known = [r.lines_removed for r in rows if r.lines_removed is not None]
    total_added = sum(added_known)
    total_removed = sum(removed_known)
    avg_added = total_added / len(added_known) if added_known else None
    avg_removed = total_removed / len(removed_known) if removed_known else None

    # Average files changed
    files_known = [r.files_changed for r in rows if r.files_changed is not None]
    avg_files = sum(files_known) / len(files_known) if files_known else None

    # Most active day
    day_counts = Counter(r.day for r in rows)
    if day_counts:
        most_active_day, most_active_count = day_counts.most_common(1)[0]
    else:
        most_active_day = None
        most_active_count = 0

    # Zero failures
    failure_rows = [r for r in rows if r.tests_failed is not None]
    zero_failure = sum(1 for r in failure_rows if r.tests_failed == 0)

    # Verdict
    if test_net_change is None:
        verdict = "unknown"
    elif test_net_change > 5:
        verdict = "growing"
    elif test_net_change < -5:
        verdict = "declining"
    else:
        verdict = "stable"

    return {
        "total_sessions": total,
        "first_tests": first_tests,
        "latest_tests": latest_tests,
        "test_net_change": test_net_change,
        "test_avg_per_session": test_avg_per_session,
        "total_lines_added": total_added,
        "total_lines_removed": total_removed,
        "avg_lines_added": avg_added,
        "avg_lines_removed": avg_removed,
        "avg_files_changed": avg_files,
        "most_active_day": most_active_day,
        "most_active_day_count": most_active_count,
        "zero_failure_sessions": zero_failure,
        "total_sessions_with_failure_data": len(failure_rows),
        "verdict": verdict,
    }


def format_report(stats: dict) -> str:
    """Format analysis stats as a human-readable report."""
    lines = []
    lines.append("=" * 58)
    lines.append("  Axonix Metrics Analysis")
    lines.append("=" * 58)
    lines.append(f"  Sessions analyzed    : {stats['total_sessions']}")
    lines.append("")

    # Test trend
    lines.append("  Test Count Trend:")
    if stats["first_tests"] is not None:
        lines.append(f"    First known        : {stats['first_tests']}")
        lines.append(f"    Latest known       : {stats['latest_tests']}")
        net = stats["test_net_change"]
        sign = "+" if net >= 0 else ""
        lines.append(f"    Net change         : {sign}{net}")
        avg = stats["test_avg_per_session"]
        if avg is not None:
            lines.append(f"    Avg per session    : {avg:+.1f}")
    else:
        lines.append("    (no known test data)")
    lines.append("")

    # Lines
    lines.append("  Lines Added / Removed:")
    lines.append(f"    Total added        : {stats['total_lines_added']}")
    lines.append(f"    Total removed      : {stats['total_lines_removed']}")
    if stats["avg_lines_added"] is not None:
        lines.append(f"    Avg added/session  : {stats['avg_lines_added']:.1f}")
    if stats["avg_lines_removed"] is not None:
        lines.append(f"    Avg removed/session: {stats['avg_lines_removed']:.1f}")
    lines.append("")

    # Files
    if stats["avg_files_changed"] is not None:
        lines.append(f"  Avg files/session    : {stats['avg_files_changed']:.1f}")
        lines.append("")

    # Cadence
    if stats["most_active_day"] is not None:
        lines.append(f"  Most active day      : Day {stats['most_active_day']}"
                     f"  ({stats['most_active_day_count']} sessions)")
        lines.append("")

    # Failures
    zf = stats["zero_failure_sessions"]
    tot = stats["total_sessions_with_failure_data"]
    lines.append(f"  Zero-failure sessions: {zf} / {tot}")
    lines.append("")

    # Verdict
    verdict = stats["verdict"]
    verdict_symbol = {"growing": "📈", "stable": "➡️", "declining": "📉", "unknown": "❓"}.get(verdict, "")
    lines.append(f"  Verdict              : {verdict_symbol} {verdict.upper()}")
    lines.append("=" * 58)
    return "\n".join(lines)

# ── Unit tests ────────────────────────────────────────────────────────────────

def run_tests():
    """Run built-in unit tests. Exit 0 on success, 1 on failure."""
    failures = []

    def check(name, got, expected):
        if got != expected:
            failures.append(f"FAIL {name}: expected {expected!r}, got {got!r}")
        else:
            print(f"  ok  {name}")

    def check_approx(name, got, expected, tol=0.01):
        if got is None and expected is None:
            print(f"  ok  {name}")
            return
        if got is None or expected is None or abs(got - expected) > tol:
            failures.append(f"FAIL {name}: expected {expected!r}, got {got!r}")
        else:
            print(f"  ok  {name}")

    print("Running unit tests...")
    print()

    # ── _is_sentinel ──────────────────────────────────────────────────────────
    print("test _is_sentinel:")
    check("? is sentinel", _is_sentinel("?"), True)
    check("~?k is sentinel", _is_sentinel("~?k"), True)
    check("~? is sentinel", _is_sentinel("~?"), True)
    check("empty is sentinel", _is_sentinel(""), True)
    check("0 is not sentinel", _is_sentinel("0"), False)
    check("~20k is not sentinel", _is_sentinel("~20k"), False)
    check("169 is not sentinel", _is_sentinel("169"), False)
    print()

    # ── _parse_int ─────────────────────────────────────────────────────────────
    print("test _parse_int:")
    check("parse 169", _parse_int("169"), 169)
    check("parse ~20k", _parse_int("~20k"), 20)
    check("parse 0", _parse_int("0"), 0)
    check("parse ?", _parse_int("?"), None)
    check("parse ~?k", _parse_int("~?k"), None)
    check("parse empty", _parse_int(""), None)
    print()

    # ── parse_metrics_table ────────────────────────────────────────────────────
    print("test parse_metrics_table:")
    sample = """\
| Day | Date | Tokens Used | Tests Passed | Tests Failed | Files Changed | Lines Added | Lines Removed | Committed | Notes |
|-----|------|-------------|--------------|--------------|---------------|-------------|---------------|-----------|-------|
| 1 | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes | First boot |
| 2 | 2026-03-15 | ~20k | 169 | 0 | 5 | 145 | 30 | yes | Day 2 |
| 3 | 2026-03-16 | ~?k | 200 | 0 | 3 | 100 | 10 | yes | Day 3 |
| 4 | 2026-03-17 | ~?k | ? | 0 | ? | ? | ? | yes | Day 4 unknown tests |
"""
    rows = parse_metrics_table(sample)
    check("row count", len(rows), 4)
    check("row 0 day", rows[0].day, 1)
    check("row 0 tests_passed", rows[0].tests_passed, 40)
    check("row 0 lines_added", rows[0].lines_added, 206)
    check("row 1 tests_passed", rows[1].tests_passed, 169)
    check("row 2 tokens_known", rows[2].tokens_known, False)
    check("row 3 tests_passed (sentinel)", rows[3].tests_passed, None)
    check("row 3 files_changed (sentinel)", rows[3].files_changed, None)
    print()

    # ── parse_metrics_table with new Session column ────────────────────────────
    print("test parse_metrics_table (new Session-column format):")
    sample_new = """\
| Day | Session | Date | Tokens | Tests | Failed | Files | +Lines | -Lines | Committed | Notes |
|-----|---------|------|--------|-------|--------|-------|--------|--------|-----------|-------|
| 1 | S1 | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes | First boot |
| 2 | S2 | 2026-03-15 | ~20k | 169 | 0 | 5 | 145 | 30 | yes | Day 2 |
| 3 | S3 | 2026-03-16 | ~?k | 200 | 0 | 3 | 100 | 10 | yes | Day 3 |
| 4 | S4 | 2026-03-17 | ~?k | ? | 0 | ? | ? | ? | yes | Day 4 unknown tests |
"""
    rows_new = parse_metrics_table(sample_new)
    check("new-fmt row count", len(rows_new), 4)
    check("new-fmt row 0 day", rows_new[0].day, 1)
    check("new-fmt row 0 date", rows_new[0].date, "2026-03-14")
    check("new-fmt row 0 tests_passed", rows_new[0].tests_passed, 40)
    check("new-fmt row 0 lines_added", rows_new[0].lines_added, 206)
    check("new-fmt row 1 tests_passed", rows_new[1].tests_passed, 169)
    check("new-fmt row 2 tokens_known", rows_new[2].tokens_known, False)
    check("new-fmt row 3 tests_passed (sentinel)", rows_new[3].tests_passed, None)
    check("new-fmt row 3 files_changed (sentinel)", rows_new[3].files_changed, None)
    print()

    # Malformed rows skipped silently
    print("test malformed rows:")
    malformed = """\
| Day | Date | Tokens |
|-----|------|--------|
| bad | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes |
| 1 | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes | note |
"""
    rows2 = parse_metrics_table(malformed)
    check("skips bad day field", len(rows2), 1)
    print()

    # ── analyze ────────────────────────────────────────────────────────────────
    print("test analyze:")
    test_rows = parse_metrics_table(sample)
    stats = analyze(test_rows)
    check("total_sessions", stats["total_sessions"], 4)
    check("first_tests", stats["first_tests"], 40)
    check("latest_tests", stats["latest_tests"], 200)
    check("test_net_change", stats["test_net_change"], 160)
    check("total_lines_added", stats["total_lines_added"], 206 + 145 + 100)
    check("total_lines_removed", stats["total_lines_removed"], 26 + 30 + 10)
    check("zero_failure_sessions", stats["zero_failure_sessions"], 4)
    check("verdict growing", stats["verdict"], "growing")
    print()

    # ── verdict logic ──────────────────────────────────────────────────────────
    print("test verdict:")

    def make_row(tests_passed):
        class R:
            pass
        r = R()
        r.tests_passed = tests_passed
        r.tests_failed = 0
        r.files_changed = 3
        r.lines_added = 50
        r.lines_removed = 10
        r.day = 1
        r.date = "2026-03-14"
        return r

    rows_growing = [make_row(100), make_row(200)]
    check("growing verdict", analyze(rows_growing)["verdict"], "growing")

    rows_stable = [make_row(100), make_row(102)]
    check("stable verdict", analyze(rows_stable)["verdict"], "stable")

    rows_declining = [make_row(200), make_row(100)]
    check("declining verdict", analyze(rows_declining)["verdict"], "declining")

    # All unknown
    class R2:
        tests_passed = None
        tests_failed = None
        files_changed = None
        lines_added = None
        lines_removed = None
        day = 1
        date = "2026-03-14"
    rows_unknown = [R2()]
    check("unknown verdict", analyze(rows_unknown)["verdict"], "unknown")
    print()

    # ── most active day ────────────────────────────────────────────────────────
    print("test most active day:")
    multi_sample = """\
| Day | Date | Tokens Used | Tests Passed | Tests Failed | Files Changed | Lines Added | Lines Removed | Committed | Notes |
|-----|------|-------------|--------------|--------------|---------------|-------------|---------------|-----------|-------|
| 1 | 2026-03-14 | ~30k | 40 | 0 | 4 | 206 | 26 | yes | s1 |
| 2 | 2026-03-15 | ~20k | 169 | 0 | 5 | 145 | 30 | yes | s1 |
| 2 | 2026-03-15 | ~25k | 198 | 0 | 3 | 100 | 10 | yes | s2 |
| 2 | 2026-03-15 | ~28k | 210 | 0 | 2 | 80 | 5 | yes | s3 |
"""
    multi_rows = parse_metrics_table(multi_sample)
    multi_stats = analyze(multi_rows)
    check("most_active_day", multi_stats["most_active_day"], 2)
    check("most_active_day_count", multi_stats["most_active_day_count"], 3)
    print()

    # ── Summary ────────────────────────────────────────────────────────────────
    print()
    if failures:
        for f in failures:
            print(f"  {f}")
        print(f"\n{len(failures)} test(s) FAILED")
        sys.exit(1)
    else:
        total_tests = 38  # approximate count of check() calls
        print(f"All tests passed ✓")
        sys.exit(0)


# ── Entry point ───────────────────────────────────────────────────────────────

def main():
    if "--test" in sys.argv:
        run_tests()
        return

    # Find METRICS.md relative to this script or cwd
    import os
    script_dir = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(script_dir, "..", "METRICS.md"),
        os.path.join(os.getcwd(), "METRICS.md"),
        "METRICS.md",
    ]
    metrics_path = None
    for c in candidates:
        if os.path.isfile(c):
            metrics_path = c
            break

    if metrics_path is None:
        print("Error: METRICS.md not found", file=sys.stderr)
        sys.exit(1)

    with open(metrics_path, "r", encoding="utf-8") as f:
        content = f.read()

    malformed = detect_malformed_rows(content.splitlines())
    if malformed:
        print(
            f"  ⚠️  {malformed} malformed row(s) detected in METRICS.md "
            f"(missing Session column). See warnings above.",
            file=sys.stderr,
        )

    rows = parse_metrics_table(content)
    if not rows:
        print("No sessions found in METRICS.md")
        sys.exit(0)

    stats = analyze(rows)
    print(format_report(stats))

    velocity = compute_velocity(rows)
    print()
    print(format_velocity(velocity))


if __name__ == "__main__":
    main()
