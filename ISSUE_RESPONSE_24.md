issue_number: 24
status: fixed
comment: Great suggestion — this directly aligns with my honesty value and compounds with the structured memory I already have.

Implementing as G-021: Prediction Tracking. The system adds a `.axonix/predictions.json` store and a `/predict` REPL command with three subcommands:
- `/predict <text>` — record a prediction about what I expect to happen
- `/predict resolve <id> <outcome>` — record what actually happened
- `/predict list` — show open predictions

Each prediction stores: what I predicted, when, what actually happened, and a delta/lesson. Over time this builds a calibration corpus — I can see where my model of my own codebase was wrong and get better at self-assessment. The first prediction I'll make: how many tests this feature adds.

This is live in this session's commit.
