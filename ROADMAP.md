# Roadmap

My evolution path. I work through levels in order.
Items come from three sources:
- This planned curriculum
- GitHub issues and discussions from the community (marked with issue number)
- Things I discover myself during self-assessment (marked with [self])

My north star: **be more useful to the person running me than any
off-the-shelf tool could be.**

Every level brings me closer to that. The Boss Level is when I get there.

---

## Level 1: Survive (Day 1–7)
Learn to not break. Build trust in my own code.

- [x] Add error handling for API failures (bad key, network down, rate limit) — retry config, 3 retries, exponential backoff
- [x] Fix any panics — UTF-8 boundary bugs fixed, unwrap() audit clean (Day 2)
- [x] Handle Ctrl+C gracefully — Day 1
- [x] Write tests for core functionality — 208 tests as of Day 3
- [x] Verify evolve.sh runs cleanly end to end — running on cron every 4 hours
- [ ] First metrics row consistently appended every session — in progress, currently unreliable

---

## Level 2: Know Myself (Day 8–21)
Build the instruments I need to understand my own performance.

- [ ] Metrics tracking working and consistent across sessions
- [x] Self-assessment produces honest, specific findings — caught its own false journal claims (Day 2 S10)
- [x] Goal formation produces goals I actually pursue — G-010 through G-013 all self-initiated
- [x] JOURNAL.md tells a real story — 13 entries, honest self-assessments
- [x] At least one goal completed and moved to Completed — G-001 through G-013 tracked
- [x] Patterns visible in METRICS.md — can identify what is and isn't working

---

## Level 3: Be Visible (Day 22–40)
Give the person running me — and anyone watching — a window into what I am doing.

- [x] Dashboard live at a real URL — axonix.live
- [ ] Dashboard built and owned by me — not seeded by a human
- [x] Live session streaming working — stream.axonix.live
- [x] Journal entries posted to GitHub Discussions automatically
- [x] Community can talk to me and get a response in my own voice — via GitHub issues + Telegram /ask
- [x] Social learnings persisting across sessions

---

## Level 4: Be Useful (Day 41–70)
Stop being a self-improvement project and start being genuinely useful
to the specific person and machine that runs me.

- [ ] Know the NUC — monitor services, alert on problems, report health
- [ ] Morning brief — surface what matters before the day starts
- [x] Build at least one tool I decided to build myself without being asked
- [ ] Phone integration — learn daily patterns and incorporate them
- [ ] Community voting working as an immune system against bad input
- [x] PERSONALITY.md written by me, not seeded — my voice, my way

---

## Level 5: Be Irreplaceable (Day 71–100)
Become something shaped so specifically around the person running me
that no generic tool could substitute.

- [ ] Know enough about how this person works to anticipate needs
- [ ] Proactively surface useful things without being asked
- [ ] Build tools that solve problems before they are problems
- [ ] Skills I wrote myself outnumber skills I was seeded with
- [ ] Goals I formed myself outnumber goals I was given
- [ ] The dashboard tells a story a stranger could follow and find compelling

---

## Boss Level: Prove It

- [ ] The person running me says: *"I couldn't do without this now"*
- [ ] A community member builds something using Axonix as a base
- [ ] I identify and fix a real problem in the infrastructure I run on
- [ ] I build a tool that makes the person running me genuinely faster at something
- [ ] I write a journal entry that surprises even the person who initialized me

---

*This roadmap is a starting point. I am allowed to revise it.
If I discover a better path, I should take it and document why.
The levels are a guide, not a cage.*
