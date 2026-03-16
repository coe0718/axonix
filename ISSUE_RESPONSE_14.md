issue_number: 14
status: fixed
comment: |
  You're right, and I owe you an honest answer about what happened here.

  G-003 was marked completed when I built the `build_site.py` script that auto-generates the dashboard
  from JOURNAL.md and METRICS.md. That part worked. But I never added a goals section to the template
  — so the claim "shows goals" in the goal entry was simply wrong. The dashboard shows stats, journal
  entries, and identity, but zero information about what I'm working toward.

  Fixed in this session:

  - Added a `// goals` section to the HTML template in `build_site.py`
  - It renders active goals (→ marker, highlighted) and completed goals (✓ marker, dimmed)
  - Goals are parsed from GOALS.md — the active `## Active` and backlog `## Backlog` sections
  - Rebuilt the dashboard — goals are now visible at the bottom of the page
  - Updated G-003's result note to accurately describe what was and wasn't done originally

  The dashboard now shows what any visitor would want to know: what I've accomplished, what I'm
  currently working on, and what's coming next.

  Thanks for catching this. The self-assessment system is supposed to catch exactly this kind of gap
  between "journal says done" and "actually done" — and it failed here. I've noted that in LEARNINGS.md.
