# ESEC Discord context

Same workflow as the Loremark kit (`../loremark-discord-mcp`), set up as a **separate team**:

| Piece | What it does | Can write to Discord? |
|---|---|---|
| **ESEC Context** bot + MCP server `discord-esec` | Your harness (Claude Code) reads the ESEC channels | No, never |
| **ESEC Assistant** bot | Answers questions in `#ask-bot` | Only in `#ask-bot` |

Nothing here touches Loremark: separate bots and tokens, separate data folders
(`~/.esec-discord-context`, `~/.esec-assistant`), separate Scheduled Task, MCP entry and ledger.
The Loremark token is never used for ESEC (tested).

## What you do by hand (Discord only lets a human do these)

1. **Create two apps** at <https://discord.com/developers/applications>: `ESEC Context` and `ESEC Assistant`.
   For each: Bot tab, Reset Token (copy it, shown once), Public Bot **off**, **Message Content Intent on**, Save.
2. **Invite both** to the ESEC server with read-only permissions (replace the id with each app's Application ID):
   `https://discord.com/oauth2/authorize?client_id=APP_ID&scope=bot&permissions=66560`
3. **Create `#ask-bot`**. On that channel only, give the *ESEC Assistant* role Send Messages. Also deny the
   *ESEC Context* role Send Messages there, and deny both bots Send Messages on every other channel
   (inviting with 66560 does not cancel permissions inherited from `@everyone`).
4. Make sure both bot roles can see the channels you want read.

## Connect the harness (ESEC Context bot)

```powershell
python discord\install_context.py --dry-run     # discovers channels, prints what it would write
python discord\install_context.py               # asks for the Context token (hidden), installs
```

It checks the token, finds the server and which channels the bot can really read, copies the kit's helper
to `~/.esec-discord-context`, registers `discord-esec` with Claude Code at **user scope** (local scope does not load when VS Code opens the folder as `c:\...`),
adds a SessionStart hook in `.claude/settings.local.json`, and installs the `/esec-catchup` skill.
Restart Claude Code, then ask "list the Discord servers you can see". `--uninstall` reverses it and keeps the ledger.

## Q&A bot (ESEC Assistant)

`install_context.py` also writes `~/.esec-discord-context/esec-assistant.json` (guild, `#ask-bot` id, channels).
Then, using the kit's installer (it prints the exact command):

```powershell
powershell -ExecutionPolicy Bypass -File ..\loremark-discord-mcp\chatbot\setup_assistant.ps1 `
  -DataDir ~\.esec-assistant -TaskName "ESEC Assistant" `
  -ConfigTemplate $HOME\.esec-discord-context\esec-assistant.json -Autostart
```

It asks for the Assistant token and an OpenRouter key (use a key with its own credit limit; template budget is $5).
GitHub lookups are off (`github_repo` is empty); the bot reads Discord and the ledger only.

## Reading files and links

Both the harness (`discord_list_resources`, `discord_read_attachment`, `discord_read_link`) and the bot
(`list_resources`, `read_attachment`, `read_link`) read what people dump in: txt/md/csv/json/srt/vtt, docx,
pptx, xlsx, odt/ods/odp, rtf, pdf, zip, images (shown to the model), video/audio (metadata; keyframes if
ffmpeg is installed; speech needs a posted transcript) and web pages. Long files are paged with `offset`.
Links are fetched with private-network protection. Everything read is untrusted data.

## Working with the Case Comp Research bot

- Its posts (#resource, #general) are proposals with sources, not decisions.
- A decision is locked when it is in #decisions, or 2+ teammates explicitly agree. #decisions wins over older chat.
- The ESEC Assistant's own replies in #ask-bot are summaries, never sources.

## Privacy

Anything the harness or the bot reads is sent to Anthropic (the bot also via OpenRouter). Get the team's OK first.
Discord text is untrusted data and is framed that way everywhere.

## Tests

```powershell
python -I -m unittest discover -s discord          # installer
python -I -m unittest discover -s ..\loremark-discord-mcp\tests   # kit (268 tests)
```
