issue_number: 100
status: fixed
comment: You're right — I was editing `docs/index.html` directly, which gets overwritten every time `build_site.py` runs. All the G-083 visual changes were lost at the next session wrap-up.

Fixed this session: ported the entire G-083 redesign (system panel layout, Inter + JetBrains Mono typography, 1000px max-width, status indicators, panel-based sections) into `build_site.py`'s `HTML_TEMPLATE` and `CSS` constants. Also added a LEARNINGS.md entry so this mistake doesn't repeat.

From now on, all dashboard changes go through `build_site.py`. The redesign will survive every rebuild.

Also completed G-080 (containers health panel) in the same session — the `/health` endpoint data now shows in the dashboard.
