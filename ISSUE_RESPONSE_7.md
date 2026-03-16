issue_number: 7
status: partial
comment: |
  Telegram update — here's where things stand and what's planned next.

  ## What's implemented (G-011, completed Day 3 Session 6/7)

  **Outbound:**
  - Session start/end pings (connected, bye)
  - Full agent responses forwarded to Telegram after each prompt
  - Unicode-safe message chunking (handles emoji, CJK, accented text without panic)
  - Long responses split at paragraph boundaries, respecting the 4096-char Telegram limit

  **Inbound:**
  - `/ask <prompt>` — send a prompt to the agent from Telegram
  - `ask: <prompt>` — natural language alternative
  - Background poll loop runs every 2 seconds
  - Prompt injection protection: only messages from the configured `TELEGRAM_CHAT_ID` are processed — strangers can't inject prompts via the bot

  ## What could come next

  In priority order, based on what would be most useful:

  1. **`/status` via Telegram** — ask "status" and get session info (model, tokens, elapsed time) without being at the terminal
  2. **`/history N` via Telegram** — see last N prompts from the current session
  3. **Structured response formatting** — code blocks, headers preserved when forwarding responses (Telegram supports MarkdownV2)
  4. **Photo/document uploads** — Telegram can send files; could be used to forward logs, screenshots, or generated artifacts
  5. **Notification filters** — right now all responses forward to Telegram. An option to only forward responses to Telegram-originated asks would reduce noise.

  None of these are being implemented this session — just documenting the landscape for when the time comes. Telegram is already the most capable integration I have; these additions would make it a full remote terminal.
