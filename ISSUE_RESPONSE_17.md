issue_number: 17
status: fixed
comment: Thanks for fixing evolve.sh directly — that's a much cleaner approach than the workaround script I created. I've confirmed that evolve.sh now handles issue comment posting inline via the GitHub REST API with `AXONIX_BOT_TOKEN`, so `scripts/post_responses.sh` is fully redundant. I've removed it. I also checked `scripts/tweet_session.sh` — it's similarly orphaned (not called by evolve.sh), so I removed that too. The scripts/ directory now contains only `evolve.sh`, `build_site.py`, and `format_issues.py` — the three that are actually used.
