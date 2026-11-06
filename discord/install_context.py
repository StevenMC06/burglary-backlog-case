#!/usr/bin/env python3
"""Connect this project's agents to the ESEC team Discord (read-only).

Same design as the Loremark Discord kit, as a second, separate team:

  * a read-only "ESEC Context" bot (View Channels + Read Message History only),
  * the kit's MCP server registered with Claude Code as "discord-esec", at user scope
    (stored in ~/.claude.json, never in the project folder; user scope is used because local
    scope does not load when VS Code opens the folder with a lowercase drive letter),
  * the kit's team_context.py helper in its own folder with its own team.json, catch-up
    state and decisions ledger,
  * a SessionStart hook (.claude/settings.local.json) and an /esec-catchup skill.

It also writes a ready-made config for the ESEC Assistant Q&A bot, built from the
channels the Context bot can read; see discord/README.md for that step.

Usage (from anywhere):
    python discord/install_context.py              install or update (asks for the token)
    python discord/install_context.py --dry-run    discover and print, change nothing
    python discord/install_context.py --uninstall  remove MCP entry, hook and skill (keeps the ledger)

The token is read from DISCORD_TOKEN or a hidden prompt. It is never printed and never
written by this script; `claude mcp add` stores it in ~/.claude.json, as the kit does.
Standard library only, Python 3.9+.
"""

import argparse
import getpass
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.dirname(HERE)
DEFAULT_HOME = os.path.join(os.path.expanduser("~"), ".esec-discord-context")
DEFAULT_KIT = os.path.join(os.path.dirname(PROJECT), "loremark-discord-mcp")
SERVER_NAME = "discord-esec"
TEAM_NAME = "ESEC"
SKILL_NAME = "esec-catchup"
SKILL_MARKER = "<!-- esec-discord-context: installed by discord/install_context.py -->"
HOOK_MARKER = ".esec-discord-context"  # substring that identifies our hook command
READ_ONLY_PERMISSIONS = 66560  # View Channels (1024) + Read Message History (65536)
ASSISTANT_PERMISSIONS = 66560  # same invite; Send Messages is granted only in #ask-bot
STATUS_CHANNELS = ("decisions", "resource", "general")
KIT_FILES = ("loremark_discord_mcp.py", "team_context.py")
OPTIONAL_KIT_FILES = ("resources.py",)  # skipped when absent
SCOPES = ("user", "local")
CHANNEL_NAME_RE = re.compile(r"^[a-z0-9_-]{1,100}$")
ID_RE = re.compile(r"^\d{15,25}$")
TEXT_TYPES = (0, 5)  # text, announcement


class InstallError(Exception):
    pass


def say(text=""):
    sys.stdout.write(text + "\n")
    sys.stdout.flush()


# ---------------------------------------------------------------------------
# Kit
# ---------------------------------------------------------------------------


def find_kit(explicit=None):
    """Return the folder holding loremark_discord_mcp.py and team_context.py."""
    candidates = [explicit] if explicit else [
        os.path.join(DEFAULT_KIT, "server"),
        os.path.join(os.path.expanduser("~"), ".loremark-discord-mcp"),
    ]
    for folder in candidates:
        if not folder:
            continue
        folder = os.path.abspath(os.path.expanduser(folder))
        if os.path.isdir(os.path.join(folder, "server")):
            folder = os.path.join(folder, "server")
        srv_py = os.path.join(folder, "loremark_discord_mcp.py")
        tc_py = os.path.join(folder, "team_context.py")
        if os.path.isfile(srv_py) and os.path.isfile(tc_py):
            with open(tc_py, "r", encoding="utf-8") as fh:
                if '"mcp_server"' not in fh.read():
                    raise InstallError(
                        "%s is too old: it has no per-team token lookup ('mcp_server'). Update the kit." % tc_py
                    )
            return folder
    raise InstallError(
        "Cannot find the Loremark Discord kit (loremark_discord_mcp.py + team_context.py). "
        "Pass --kit PATH (the kit folder or its server folder)."
    )


def load_server_module(kit):
    if kit not in sys.path:
        sys.path.insert(0, kit)
    import loremark_discord_mcp as srv  # noqa: E402

    return srv


# ---------------------------------------------------------------------------
# Discovery (GET only, through the kit's client)
# ---------------------------------------------------------------------------


def invite_url(app_id, permissions=READ_ONLY_PERMISSIONS):
    return "https://discord.com/oauth2/authorize?client_id=%s&scope=bot&permissions=%d" % (app_id, permissions)


def pick_guild(guilds, wanted=None):
    if wanted:
        for g in guilds:
            if g.get("id") == wanted:
                return g
        raise InstallError("The bot is not in server %s. Servers it is in: %s." % (
            wanted, ", ".join("%s (%s)" % (g.get("name"), g.get("id")) for g in guilds) or "none"))
    if len(guilds) == 1:
        return guilds[0]
    raise InstallError("The bot is in %d servers; pass --guild-id with one of: %s." % (
        len(guilds), ", ".join("%s (%s)" % (g.get("name"), g.get("id")) for g in guilds)))


def discover(client, srv, wanted_guild=None):
    """Return dict(bot, guild, readable=[{id,name}], unreadable=[name]). Raises InstallError."""
    _, me = client.get("/users/@me", what="checking the token")
    if not isinstance(me, dict) or not me.get("id"):
        raise InstallError("Discord returned an unexpected answer for the bot user.")
    _, guilds = client.get("/users/@me/guilds", what="listing servers")
    guilds = [g for g in (guilds or []) if isinstance(g, dict)]
    if not guilds:
        raise InstallError(
            "The bot %s is not in any server yet. Invite it read-only with:\n  %s"
            % (me.get("username"), invite_url(me["id"]))
        )
    guild = pick_guild(guilds, wanted_guild)
    _, channels = client.get("/guilds/%s/channels" % guild["id"], what="listing channels")
    text = [c for c in (channels or []) if isinstance(c, dict) and c.get("type") in TEXT_TYPES]
    text.sort(key=lambda c: (c.get("position", 0), int(c.get("id", "0"))))
    readable, unreadable = [], []
    for ch in text:
        try:
            client.get("/channels/%s/messages" % ch["id"], {"limit": 1}, what="probing #%s" % ch.get("name"))
            readable.append({"id": ch["id"], "name": ch.get("name") or ch["id"]})
        except srv.DiscordError as err:
            if err.status in (401,):
                raise
            unreadable.append(ch.get("name") or ch["id"])
    return {"bot": me, "guild": guild, "readable": readable, "unreadable": unreadable}


# ---------------------------------------------------------------------------
# Files we write
# ---------------------------------------------------------------------------


def build_team_json(found, ask_name, existing=None):
    """team.json for the ESEC copy of team_context.py. Keeps the user's mode/bootstrap_days."""
    existing = existing if isinstance(existing, dict) else {}
    channels = [c for c in found["readable"] if c["name"] != ask_name]
    cfg = {
        "guild_id": found["guild"]["id"],
        "channels": channels,
        "project_match": ["esec-case-comp"],
        "github_repo": "",
        "bootstrap_days": existing.get("bootstrap_days", 14),
        "mode": existing.get("mode", "auto"),
        "team_name": TEAM_NAME,
        "mcp_server": SERVER_NAME,
        "catchup_skill": SKILL_NAME,
    }
    return cfg


def build_assistant_config(template, found, ask_name, ledger_path):
    """assistant.json template for the ESEC Assistant (setup_assistant.ps1 -ConfigTemplate)."""
    cfg = json.loads(json.dumps(template))
    cfg["guild_id"] = found["guild"]["id"]
    ask = [c for c in found["readable"] if c["name"] == ask_name]
    cfg["ask_channel_id"] = ask[0]["id"] if ask else ""
    names = [c["name"] for c in found["readable"] if c["name"] != ask_name and CHANNEL_NAME_RE.match(c["name"])]
    if names:
        cfg["digest_channels"] = names[:10]
        ordered = [n for n in STATUS_CHANNELS if n in names][:3]
        cfg["status_channels"] = ordered or names[:2]
    cfg["ledger_path"] = ledger_path.replace("\\", "/")
    return cfg


def render_skill(template_text, python, home):
    text = template_text.replace("{{PYTHON}}", python.replace("\\", "/"))
    text = text.replace("{{INSTALL_DIR}}", home.replace("\\", "/"))
    if SKILL_MARKER not in text:
        raise InstallError("skill template lost its marker")
    return text


def _quote(path):
    path = path.replace("\\", "/")
    return '"%s"' % path if " " in path else path


def hook_entry(python, home):
    command = "%s %s hook --format claude" % (_quote(python), _quote(os.path.join(home, "team_context.py")))
    return {
        "matcher": "startup|resume|clear|compact",
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": 15,
            "statusMessage": "Checking ESEC team Discord...",
        }],
    }


def _is_ours(entry):
    return any(HOOK_MARKER in str(h.get("command", "")) for h in (entry.get("hooks") or []) if isinstance(h, dict))


def merge_settings(settings, entry):
    """Put our SessionStart entry in a settings dict (replacing an old one). Returns new dict."""
    settings = json.loads(json.dumps(settings)) if isinstance(settings, dict) else {}
    hooks = settings.setdefault("hooks", {})
    start = [e for e in hooks.get("SessionStart", []) if isinstance(e, dict) and not _is_ours(e)]
    if entry is not None:
        start.append(entry)
    if start:
        hooks["SessionStart"] = start
    else:
        hooks.pop("SessionStart", None)
    if not hooks:
        settings.pop("hooks", None)
    return settings


def read_json(path, default=None):
    try:
        with open(path, "r", encoding="utf-8-sig") as fh:
            return json.load(fh)
    except FileNotFoundError:
        return default
    except ValueError as err:
        raise InstallError("%s is not valid JSON (%s); fix or remove it first." % (path, err))


def write_text(path, text):
    """Atomic write; keeps a .bak of a file we are about to change."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.isfile(path):
        with open(path, "r", encoding="utf-8") as fh:
            if fh.read() == text:
                return False
        shutil.copy2(path, path + ".bak")
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".tmp-")
    with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    os.replace(tmp, path)
    return True


def write_json(path, data):
    return write_text(path, json.dumps(data, indent=2) + "\n")


# ---------------------------------------------------------------------------
# Claude Code registration
# ---------------------------------------------------------------------------


def claude_exe():
    exe = shutil.which("claude")
    if not exe:
        raise InstallError("The `claude` command was not found on PATH, so the MCP server cannot be registered. "
                           "Re-run with --no-mcp to skip that step.")
    return exe


def run_claude(args, token=None):
    """Run `claude ...` in the project folder. Output is scrubbed of the token before use."""
    done = subprocess.run([claude_exe()] + args, cwd=PROJECT, capture_output=True, text=True, timeout=120)
    out = (done.stdout or "") + (done.stderr or "")
    if token:
        out = out.replace(token, "[token]")
    return done.returncode, out.strip()


def register_mcp(python, home, token):
    for scope in ("user", "local"):
        run_claude(["mcp", "remove", "--scope", scope, SERVER_NAME])  # fine if it was not there
    server = os.path.join(home, "loremark_discord_mcp.py").replace("\\", "/")
    cache = os.path.join(home, "cache").replace("\\", "/")
    code, out = run_claude(
        ["mcp", "add", "--scope", "user", SERVER_NAME,
         "-e", "DISCORD_TOKEN=" + token, "-e", "DISCORD_MCP_LABEL=" + TEAM_NAME,
         "-e", "DISCORD_MCP_CACHE_DIR=" + cache,
         "--", python.replace("\\", "/"), server],
        token=token,
    )
    if code != 0:
        raise InstallError("claude mcp add failed: %s" % out[:400])
    code, out = run_claude(["mcp", "get", SERVER_NAME], token=token)
    return code == 0


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def read_token():
    raw = os.environ.get("DISCORD_TOKEN", "")
    if raw.strip():
        say("Using the token from the DISCORD_TOKEN environment variable.")
        return raw
    if not sys.stdin.isatty():
        raise InstallError("No DISCORD_TOKEN set and no terminal to ask for it.")
    return getpass.getpass("ESEC Context bot token (input hidden): ")


def install(args):
    kit = find_kit(args.kit)
    srv = load_server_module(kit)
    home = os.path.abspath(os.path.expanduser(args.home))
    python = os.path.abspath(sys.executable)
    say("Kit: %s" % kit)

    token, problem = srv.clean_token(read_token())
    if not token:
        raise InstallError(problem or "No token given.")
    srv.set_secret(token)  # the kit scrubs it from every error message
    base, _ignored = srv.resolve_api_base(os.environ.get("LOREMARK_DISCORD_API_BASE"))
    client = srv.DiscordClient(token, base)
    try:
        found = discover(client, srv, args.guild_id)
    except srv.DiscordError as err:
        raise InstallError(srv.scrub(err.message))
    bot, guild = found["bot"], found["guild"]
    say("Bot: %s (id %s)" % (bot.get("username"), bot["id"]))
    say("Server: %s (id %s)" % (guild.get("name"), guild["id"]))
    say("Readable text channels: %s" % (", ".join("#" + c["name"] for c in found["readable"]) or "none"))
    if found["unreadable"]:
        say("Not readable (private, bot role not added): %s" % ", ".join("#" + n for n in found["unreadable"]))
    if not found["readable"]:
        raise InstallError("The bot cannot read any text channel. Check its role and channel permissions.")

    team_path = os.path.join(home, "team.json")
    team = build_team_json(found, args.ask_channel_name, read_json(team_path, {}))
    template = read_json(os.path.join(HERE, "esec-assistant.template.json"))
    ledger = os.path.join(home, "team-decisions.md")
    assistant = build_assistant_config(template, found, args.ask_channel_name, ledger)
    with open(os.path.join(HERE, "esec-catchup.SKILL.template.md"), "r", encoding="utf-8") as fh:
        skill = render_skill(fh.read(), python, home)
    settings_path = os.path.join(PROJECT, ".claude", "settings.local.json")
    settings = merge_settings(read_json(settings_path, {}), hook_entry(python, home))
    skill_path = os.path.join(PROJECT, ".claude", "skills", SKILL_NAME, "SKILL.md")

    if args.dry_run:
        say("\n[dry run] Would write %s:\n%s" % (team_path, json.dumps(team, indent=2)))
        say("\n[dry run] Would write %s:\n%s" % (os.path.join(home, "esec-assistant.json"), json.dumps(assistant, indent=2)))
        say("\n[dry run] Would set the SessionStart hook in %s and the skill %s." % (settings_path, skill_path))
        return 0

    os.makedirs(home, exist_ok=True)
    for name in ("loremark_discord_mcp.py", "team_context.py") + OPTIONAL_KIT_FILES:
        if os.path.isfile(os.path.join(kit, name)):
            shutil.copy2(os.path.join(kit, name), os.path.join(home, name))
    write_json(team_path, team)
    write_json(os.path.join(home, "esec-assistant.json"), assistant)
    say("OK: helper and settings in %s" % home)

    if args.no_mcp:
        say("Skipped MCP registration (--no-mcp).")
    else:
        registered = register_mcp(python, home, token)
        say("OK: Claude Code has '%s' (user scope)%s." % (
            SERVER_NAME, "" if registered else ", but `claude mcp get` could not confirm it"))

    if args.no_hook:
        settings = merge_settings(read_json(settings_path, {}), None)
        say("Skipped the SessionStart hook (--no-hook).")
    write_json(settings_path, settings)
    write_text(skill_path, skill)
    say("OK: /%s skill and %s" % (SKILL_NAME, "hook" if not args.no_hook else "no hook"))

    status = subprocess.run([python, os.path.join(home, "team_context.py"), "status"],
                            capture_output=True, text=True, timeout=60,
                            env=dict(os.environ, DISCORD_TOKEN=token))
    say("\n" + (status.stdout or status.stderr).replace(token, "[token]").strip())

    say("\nNext:")
    say("  1. Restart Claude Code in this folder; ask: \"list the Discord servers you can see\".")
    if not assistant["ask_channel_id"]:
        say("  2. Create #%s, then re-run this script so the Assistant config gets its channel id." % args.ask_channel_name)
    else:
        say("  2. Install the Q&A bot (see discord/README.md, step 4):")
        say('     powershell -ExecutionPolicy Bypass -File "%s" -DataDir "%s" -TaskName "ESEC Assistant" -ConfigTemplate "%s" -Autostart' % (
            os.path.join(os.path.dirname(kit), "chatbot", "setup_assistant.ps1"),
            os.path.join(os.path.expanduser("~"), ".esec-assistant"),
            os.path.join(home, "esec-assistant.json")))
    return 0


def uninstall(args):
    home = os.path.abspath(os.path.expanduser(args.home))
    if not args.no_mcp:
        for scope in ("user", "local"):
            code, out = run_claude(["mcp", "remove", "--scope", scope, SERVER_NAME])
            say("MCP '%s' (%s): %s" % (SERVER_NAME, scope, "removed" if code == 0 else "not registered"))
    settings_path = os.path.join(PROJECT, ".claude", "settings.local.json")
    current = read_json(settings_path, None)
    if current is not None:
        write_json(settings_path, merge_settings(current, None))
        say("Hook removed from %s" % settings_path)
    skill_dir = os.path.join(PROJECT, ".claude", "skills", SKILL_NAME)
    skill = os.path.join(skill_dir, "SKILL.md")
    if os.path.isfile(skill):
        with open(skill, "r", encoding="utf-8") as fh:
            ours = SKILL_MARKER in fh.read()
        if ours:
            shutil.rmtree(skill_dir)
            say("Skill /%s removed" % SKILL_NAME)
    say("Kept %s (ledger, state, team.json). Delete it by hand if you no longer need it." % home)
    return 0


def build_parser():
    p = argparse.ArgumentParser(description="Connect this project's agents to the ESEC Discord (read-only).")
    p.add_argument("--kit", help="Loremark Discord kit folder (default: ../loremark-discord-mcp)")
    p.add_argument("--home", default=DEFAULT_HOME, help="data folder (default ~/.esec-discord-context)")
    p.add_argument("--guild-id", help="server id, if the bot is in more than one server")
    p.add_argument("--ask-channel-name", default="ask-bot", help="Q&A channel name, excluded from catch-ups")
    p.add_argument("--no-mcp", action="store_true", help="do not register the MCP server with Claude Code")
    p.add_argument("--no-hook", action="store_true", help="no SessionStart hook; keep the skill only")
    p.add_argument("--dry-run", action="store_true", help="discover and print; change nothing")
    p.add_argument("--uninstall", action="store_true", help="remove the MCP entry, hook and skill")
    return p


def main(argv=None):
    args = build_parser().parse_args(argv)
    if args.guild_id and not ID_RE.match(args.guild_id):
        say("ERROR: --guild-id must be 15-25 digits.")
        return 2
    try:
        return uninstall(args) if args.uninstall else install(args)
    except InstallError as err:
        say("ERROR: %s" % err)
        return 1


if __name__ == "__main__":
    sys.exit(main())
