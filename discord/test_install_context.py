"""Offline tests for install_context.py. No Discord, no `claude`, no network."""
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import install_context as ic  # noqa: E402

KIT = ic.find_kit()
sys.path.insert(0, KIT)
import loremark_discord_mcp as srv  # noqa: E402


class FakeClient:
    def __init__(self, guilds, channels, forbidden=(), unauthorized=False):
        self.guilds, self.channels = guilds, channels
        self.forbidden, self.unauthorized = set(forbidden), unauthorized
        self.paths = []

    def get(self, path, params=None, what=""):
        self.paths.append(path)
        if self.unauthorized:
            raise srv.DiscordError("Token invalid", status=401)
        if path == "/users/@me":
            return 200, {"id": "900000000000000001", "username": "esec-context"}
        if path == "/users/@me/guilds":
            return 200, self.guilds
        if path.endswith("/channels"):
            return 200, self.channels
        cid = path.split("/")[2]
        if cid in self.forbidden:
            raise srv.DiscordError("no access", status=403)
        return 200, []


GUILD = {"id": "800000000000000001", "name": "ESEC"}
CHANNELS = [
    {"id": "700000000000000001", "name": "general", "type": 0, "position": 0},
    {"id": "700000000000000002", "name": "ask-bot", "type": 0, "position": 1},
    {"id": "700000000000000003", "name": "private", "type": 0, "position": 2},
    {"id": "700000000000000004", "name": "Voice", "type": 2, "position": 3},
]


class TestDiscovery(unittest.TestCase):
    def test_probes_only_text_channels_with_get(self):
        c = FakeClient([GUILD], CHANNELS, forbidden=["700000000000000003"])
        found = ic.discover(c, srv)
        self.assertEqual([x["name"] for x in found["readable"]], ["general", "ask-bot"])
        self.assertEqual(found["unreadable"], ["private"])
        self.assertTrue(all(p.startswith("/") for p in c.paths))

    def test_bad_token_stops(self):
        with self.assertRaises(srv.DiscordError):
            ic.discover(FakeClient([GUILD], CHANNELS, unauthorized=True), srv)

    def test_no_guild_gives_invite_with_read_only_permissions(self):
        with self.assertRaises(ic.InstallError) as cm:
            ic.discover(FakeClient([], CHANNELS), srv)
        self.assertIn("permissions=66560", str(cm.exception))

    def test_several_guilds_require_a_choice(self):
        other = {"id": "800000000000000002", "name": "Other"}
        with self.assertRaises(ic.InstallError):
            ic.pick_guild([GUILD, other])
        self.assertEqual(ic.pick_guild([GUILD, other], other["id"]), other)
        with self.assertRaises(ic.InstallError):
            ic.pick_guild([GUILD], "800000000000000009")


class TestConfigBuilding(unittest.TestCase):
    def found(self):
        return ic.discover(FakeClient([GUILD], CHANNELS, forbidden=["700000000000000003"]), srv)

    def test_team_json_excludes_ask_channel_and_names_the_team(self):
        cfg = ic.build_team_json(self.found(), "ask-bot", {"mode": "notify", "bootstrap_days": 3})
        self.assertEqual([c["name"] for c in cfg["channels"]], ["general"])
        self.assertEqual((cfg["team_name"], cfg["mcp_server"], cfg["catchup_skill"]), ("ESEC", "discord-esec", "esec-catchup"))
        self.assertEqual((cfg["mode"], cfg["bootstrap_days"], cfg["github_repo"]), ("notify", 3, ""))
        self.assertNotIn("loremark", " ".join(cfg["project_match"]).lower())

    def test_assistant_config_is_valid_for_the_bot(self):
        sys.path.insert(0, os.path.join(os.path.dirname(KIT), "chatbot"))
        import loremark_assistant as la

        with open(os.path.join(HERE, "esec-assistant.template.json"), encoding="utf-8") as fh:
            template = json.load(fh)
        cfg = ic.build_assistant_config(template, self.found(), "ask-bot", r"C:\Users\x\.esec\team-decisions.md")
        self.assertEqual(cfg["ask_channel_id"], "700000000000000002")
        self.assertEqual(cfg["digest_channels"], ["general"])
        self.assertNotIn("\\", cfg["ledger_path"])
        path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "_tmp_cfg.json")
        try:
            with open(path, "w", encoding="utf-8") as fh:
                json.dump(cfg, fh)
            loaded = la.load_config(path)
        finally:
            os.remove(path)
        self.assertEqual(loaded["github_repo"], "")
        self.assertEqual(loaded["guild_id"], GUILD["id"])

    def test_missing_ask_channel_leaves_id_empty(self):
        with open(os.path.join(HERE, "esec-assistant.template.json"), encoding="utf-8") as fh:
            template = json.load(fh)
        cfg = ic.build_assistant_config(template, self.found(), "nope", "x")
        self.assertEqual(cfg["ask_channel_id"], "")


class TestSettingsMerge(unittest.TestCase):
    def test_merge_keeps_other_hooks_and_is_idempotent(self):
        theirs = {"matcher": "startup", "hooks": [{"type": "command", "command": "echo hi"}]}
        base = {"permissions": {"allow": ["Bash(ls)"]}, "hooks": {"SessionStart": [theirs], "Stop": [{"hooks": []}]}}
        entry = ic.hook_entry(r"C:\Python\python.exe", r"C:\Users\x\.esec-discord-context")
        once = ic.merge_settings(base, entry)
        twice = ic.merge_settings(once, entry)
        self.assertEqual(once, twice)
        self.assertEqual(once["hooks"]["SessionStart"][0], theirs)
        self.assertEqual(len(once["hooks"]["SessionStart"]), 2)
        self.assertEqual(once["permissions"], base["permissions"])
        self.assertIn("Stop", once["hooks"])
        self.assertNotIn("\\", entry["hooks"][0]["command"])

    def test_removal_drops_only_ours_and_empty_hooks(self):
        entry = ic.hook_entry("python", "/h/.esec-discord-context")
        with_ours = ic.merge_settings({}, entry)
        self.assertEqual(ic.merge_settings(with_ours, None), {})

    def test_paths_with_spaces_are_quoted(self):
        entry = ic.hook_entry("C:/Program Files/Python/python.exe", "/h/.esec-discord-context")
        self.assertTrue(entry["hooks"][0]["command"].startswith('"C:/Program Files/Python/python.exe" '))


class TestSkillAndSafety(unittest.TestCase):
    def test_skill_renders_without_placeholders(self):
        with open(os.path.join(HERE, "esec-catchup.SKILL.template.md"), encoding="utf-8") as fh:
            text = ic.render_skill(fh.read(), r"C:\Py\python.exe", r"C:\Users\x\.esec-discord-context")
        self.assertNotIn("{{", text)
        self.assertIn("C:/Py/python.exe", text)
        self.assertIn("name: esec-catchup", text)

    def test_script_never_prints_or_stores_the_token(self):
        with open(os.path.join(HERE, "install_context.py"), encoding="utf-8") as fh:
            src = fh.read()
        self.assertNotIn("print(token", src)
        self.assertNotIn("say(token", src)
        self.assertIn('replace(token, "[token]")', src)

    def test_json_writer_backs_up_changes(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "a.json")
            self.assertTrue(ic.write_json(p, {"a": 1}))
            self.assertFalse(ic.write_json(p, {"a": 1}))
            self.assertTrue(ic.write_json(p, {"a": 2}))
            self.assertTrue(os.path.isfile(p + ".bak"))


if __name__ == "__main__":
    unittest.main()
