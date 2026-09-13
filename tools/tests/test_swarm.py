#!/usr/bin/env python3
"""Swarm launcher tests; stdlib only, no network (urlopen is mocked)."""

import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import urllib.error


LAUNCH = Path(__file__).resolve().parents[1] / "swarm" / "launch.py"
TEMPLATE = Path(__file__).resolve().parents[1] / "swarm" / "prompt_template.md"


def load_launch():
    specification = importlib.util.spec_from_file_location("swarm_under_test", LAUNCH)
    module = importlib.util.module_from_spec(specification)
    previous = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        specification.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module


class Response:
    def __init__(self, record):
        self.record = record

    def read(self):
        return json.dumps(self.record).encode("utf-8")

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


def http_error(code):
    return urllib.error.HTTPError("https://api.devin.ai/v3/x", code,
                                  "error", {}, io.BytesIO(b"{}"))


def manifest(session_overrides=None, **overrides):
    session = {"name": "alpha", "board": "TASKS.md", "item": "ITEM-ONE",
               "host": "linux", "owning_paths": ["src/one"]}
    session.update(session_overrides or {})
    record = {"wave": "w9", "max_acu_limit": 10, "devin_mode": None,
              "bypass_approval": True, "resumable": True,
              "tags": ["omega-swarm", "w9"], "owner_label_prefix": "Tester / w9",
              "exclusions": ["ITEM-EXCLUDED"], "sessions": [session]}
    record.update(overrides)
    return record


class SwarmTests(unittest.TestCase):
    def setUp(self):
        self.module = load_launch()
        self.temporary = tempfile.TemporaryDirectory(prefix="omega swarm test ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repo"
        self.repository.mkdir()
        self.environment = {key: value for key, value in os.environ.items()
                            if not key.startswith("GIT_")}
        self.environment.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                                GIT_TERMINAL_PROMPT="0")
        self.git(self.repository, "init", "-b", "main")
        for name, value in (("user.name", "Swarm Test"),
                            ("user.email", "test@example.invalid"),
                            ("commit.gpgSign", "false")):
            self.git(self.repository, "config", name, value)
        (self.repository / "src" / "one").mkdir(parents=True)
        (self.repository / "src" / "one" / "code.rs").write_text(
            "// fixture\n", encoding="utf-8")
        for board in self.module.BOARDS:
            (self.repository / board).write_text(
                f"- **ITEM-ONE.** First item.\n- **ITEM-TWO.** Second item.\n",
                encoding="utf-8")
        self.git(self.repository, "add", "-A")
        self.git(self.repository, "commit", "-m", "initial")
        self.patch_environ = mock.patch.dict(
            os.environ, {"DEVIN_API_KEY": "cog_testkey", "DEVIN_ORG_ID": "org-test"})
        self.patch_environ.start()
        self.addCleanup(self.patch_environ.stop)

    def git(self, directory, *arguments):
        result = subprocess.run(["git", "-C", str(directory), *arguments],
                                capture_output=True, text=True, encoding="utf-8",
                                env=self.environment, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def validate(self, record):
        return self.module.validate_manifest(record, self.repository)

    def test_manifest_rejects_excluded_item(self):
        record = manifest({"item": "ITEM-EXCLUDED"})
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_rejects_duplicate_owning_path(self):
        record = manifest(sessions=[
            {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
             "host": "linux", "owning_paths": ["src/one"]},
            {"name": "b", "board": "TASKS.md", "item": "ITEM-TWO",
             "host": "linux", "owning_paths": ["src/one"]}])
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_rejects_non_linux_host(self):
        record = manifest({"host": "windows"})
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_rejects_unknown_board(self):
        record = manifest({"board": "TASKS_ELSEWHERE.md"})
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_rejects_item_absent_from_board(self):
        record = manifest({"item": "ITEM-NOWHERE"})
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_rejects_missing_fields(self):
        record = manifest()
        del record["sessions"][0]["item"]
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)
        with self.assertRaises(self.module.SwarmError):
            self.validate({"wave": "w9"})

    def test_manifest_rejects_bad_host_gates_and_probe_only(self):
        for value in ([], [""], ["true", 3]):
            with self.subTest(host_gates=value):
                record = manifest({"host_gates": value})
                with self.assertRaisesRegex(self.module.SwarmError, "alpha"):
                    self.validate(record)
        for value in ("true", 1, None):
            with self.subTest(probe_only=value):
                record = manifest({"probe_only": value})
                with self.assertRaisesRegex(self.module.SwarmError, "alpha"):
                    self.validate(record)

    def test_plan_refuses_failing_host_gate_unless_probe_only(self):
        manifest_path = self.root / "wave.json"
        failing = manifest({"host_gates": ["exit 3"]})
        manifest_path.write_text(json.dumps(failing), encoding="utf-8")
        with self.assertRaisesRegex(self.module.SwarmError, "exit 3"):
            self.module.command_plan(
                mock.Mock(manifest=str(manifest_path), skip_host_gates=False),
                self.repository)
        prompt_path = (self.module.build_directory(self.repository, "w9")
                       / "prompts" / "alpha.md")
        self.assertFalse(prompt_path.exists())

        probe = manifest({"host_gates": ["exit 3"], "probe_only": True})
        manifest_path.write_text(json.dumps(probe), encoding="utf-8")
        with mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path), skip_host_gates=False),
                self.repository), 0)
            planned = emitted.call_args.args[0]["sessions"][0]
        self.assertEqual(planned["host_gates"], [{"command": "exit 3", "exit": 3}])
        self.assertTrue(planned["probe_only"])
        self.assertIn(
            "Probe-only slot",
            prompt_path.read_text(encoding="utf-8"))

        passing = manifest({"host_gates": ["true"]})
        manifest_path.write_text(json.dumps(passing), encoding="utf-8")
        with mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path), skip_host_gates=False),
                self.repository), 0)
            planned = emitted.call_args.args[0]["sessions"][0]
        self.assertEqual(planned["host_gates"], [{"command": "true", "exit": 0}])
        self.assertFalse(planned["probe_only"])
        self.assertNotIn(
            "Probe-only slot",
            prompt_path.read_text(encoding="utf-8"))

    def test_route_crates_marks_reachable_and_unreachable(self):
        metadata = {
            "workspace_members": ["omega-id", "reachable-id", "unreachable-id"],
            "packages": [
                {"id": "omega-id", "name": "omega",
                 "manifest_path": str(self.repository / "omega" / "Cargo.toml")},
                {"id": "reachable-id", "name": "reachable",
                 "manifest_path": str(self.repository / "reachable" / "Cargo.toml")},
                {"id": "unreachable-id", "name": "unreachable",
                 "manifest_path": str(self.repository / "unreachable" / "Cargo.toml")},
            ],
            "resolve": {
                "nodes": [
                    {"id": "omega-id", "deps": [{"pkg": "reachable-id"}]},
                    {"id": "reachable-id", "deps": []},
                    {"id": "unreachable-id", "deps": []},
                ],
            },
        }
        result = mock.Mock(returncode=0, stdout=json.dumps(metadata))
        with mock.patch.object(self.module.subprocess, "run", return_value=result):
            crates = self.module.route_crates(self.repository)
        self.assertEqual(crates["omega"],
                         {"name": "omega", "on_route": True})
        self.assertEqual(crates["reachable"],
                         {"name": "reachable", "on_route": True})
        self.assertEqual(crates["unreachable"],
                         {"name": "unreachable", "on_route": False})

    def test_plan_refuses_off_route_crate_unless_probe_only(self):
        (self.repository / "src" / "one" / "Cargo.toml").write_text(
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n", encoding="utf-8")
        manifest_path = self.root / "wave.json"
        record = manifest()
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        route = {"src/one": {"name": "one", "on_route": False}}
        with mock.patch.object(self.module, "route_crates", return_value=route):
            with self.assertRaisesRegex(self.module.SwarmError, "not on the omega route"):
                self.module.command_plan(
                    mock.Mock(manifest=str(manifest_path),
                              skip_host_gates=True, skip_route_check=False),
                    self.repository)
        prompt_path = (self.module.build_directory(self.repository, "w9")
                       / "prompts" / "alpha.md")
        self.assertFalse(prompt_path.exists())

        record["sessions"][0]["probe_only"] = True
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        with mock.patch.object(self.module, "route_crates", return_value=route), \
                mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path),
                          skip_host_gates=True, skip_route_check=False),
                self.repository), 0)
            planned = emitted.call_args.args[0]["sessions"][0]
        freshness = {line["path"]: line for line in planned["freshness"]}
        self.assertFalse(freshness["src/one"]["on_route"])
        self.assertEqual(freshness["src/one"]["crate_name"], "one")

    def test_plan_skip_route_check_records_skipped(self):
        (self.repository / "src" / "one" / "Cargo.toml").write_text(
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n", encoding="utf-8")
        manifest_path = self.root / "wave.json"
        manifest_path.write_text(json.dumps(manifest()), encoding="utf-8")
        with mock.patch.object(self.module, "route_crates") as route, \
                mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path),
                          skip_host_gates=True, skip_route_check=True),
                self.repository), 0)
            route.assert_not_called()
            planned = emitted.call_args.args[0]["sessions"][0]
        freshness = {line["path"]: line for line in planned["freshness"]}
        self.assertEqual(freshness["src/one"]["on_route"], "skipped")
        self.assertEqual(freshness["TASKS.md"]["on_route"], "skipped")

    def test_plan_with_unavailable_cargo_records_unknown(self):
        (self.repository / "src" / "one" / "Cargo.toml").write_text(
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n", encoding="utf-8")
        with mock.patch.object(self.module.subprocess, "run",
                               side_effect=FileNotFoundError("cargo")):
            self.assertIsNone(self.module.route_crates(self.repository))
        manifest_path = self.root / "wave.json"
        manifest_path.write_text(json.dumps(manifest()), encoding="utf-8")
        with mock.patch.object(self.module, "route_crates",
                               return_value=None):
            with mock.patch.object(self.module, "emit") as emitted:
                self.assertEqual(self.module.command_plan(
                    mock.Mock(manifest=str(manifest_path),
                              skip_host_gates=True, skip_route_check=False),
                    self.repository), 0)
                planned = emitted.call_args.args[0]["sessions"][0]
        freshness = {line["path"]: line for line in planned["freshness"]}
        self.assertEqual(freshness["src/one"]["on_route"], "unknown")

    def test_launch_dry_run_skips_gates_and_skip_flag(self):
        record = manifest({"host_gates": ["exit 3"]})
        manifest_path = self.root / "wave.json"
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        self.assertEqual(self.module.main(
            ["--repository", str(self.repository), "launch",
             "--manifest", str(manifest_path), "--dry-run"]), 0)
        with mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "plan",
                 "--manifest", str(manifest_path), "--skip-host-gates"]), 0)
            planned = emitted.call_args.args[0]["sessions"][0]
        self.assertEqual(planned["host_gates"], "skipped")

    def test_prompt_rendering_has_no_unresolved_placeholders(self):
        record = manifest()
        self.validate(record)
        template = TEMPLATE.read_text(encoding="utf-8")
        prompt = self.module.render_prompt(template, record, record["sessions"][0])
        for field in self.module.PROMPT_FIELDS:
            self.assertNotIn("{" + field + "}", prompt)
        self.assertIn("ITEM-ONE", prompt)

    def test_prompt_owner_label_and_exclusions(self):
        record = manifest()
        self.validate(record)
        template = TEMPLATE.read_text(encoding="utf-8")
        prompt = self.module.render_prompt(template, record, record["sessions"][0])
        self.assertIn('Tester / w9-alpha', prompt)
        self.assertIn("ITEM-EXCLUDED", prompt)

    def test_prompt_suggestion_labelled_and_omitted_when_absent(self):
        template = TEMPLATE.read_text(encoding="utf-8")
        with_slice = manifest({"suggested_first_slice": "try the small pair"})
        self.validate(with_slice)
        prompt = self.module.render_prompt(template, with_slice,
                                         with_slice["sessions"][0])
        self.assertIn("try the small pair", prompt)
        self.assertIn("suggestion", prompt.lower())
        without_slice = manifest()
        prompt = self.module.render_prompt(template, without_slice,
                                           without_slice["sessions"][0])
        self.assertNotIn("suggested first slice", prompt.lower())
        for field in self.module.PROMPT_FIELDS:
            self.assertNotIn("{" + field + "}", prompt)

    def test_request_body_fields_and_no_key_material(self):
        record = manifest()
        self.validate(record)
        body = self.module.request_body(record, record["sessions"][0], "the prompt")
        self.assertEqual(body["max_acu_limit"], 10)
        self.assertEqual(body["tags"], ["omega-swarm", "w9"])
        self.assertEqual(body["title"], "swarm w9 alpha: ITEM-ONE")
        self.assertTrue(body["bypass_approval"])
        self.assertTrue(body["structured_output_required"])
        self.assertIn("result", body["structured_output_schema"]["properties"])
        self.assertNotIn("devin_mode", body)
        self.assertNotIn("cog_testkey", json.dumps(body))

    def test_wave1_body_carries_fusion_mode(self):
        wave = json.loads((LAUNCH.parent / "waves" / "wave-1.json").read_text(
            encoding="utf-8"))
        body = self.module.request_body(wave, wave["sessions"][0], "the prompt")
        self.assertEqual(body["devin_mode"], "fusion")

    def test_launch_idempotency_requires_relaunch(self):
        record = manifest()
        manifest_path = self.root / "wave.json"
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        with mock.patch.object(self.module.urllib.request, "urlopen",
                               return_value=Response({"session_id": "s1",
                                                      "url": "https://app.devin.ai/sessions/s1"})) as opened:
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "launch",
                 "--manifest", str(manifest_path)]), 0)
            self.assertEqual(opened.call_count, 1)
        with mock.patch.object(self.module.urllib.request, "urlopen") as opened:
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "launch",
                 "--manifest", str(manifest_path)]), 0)
            opened.assert_not_called()
        with mock.patch.object(self.module.urllib.request, "urlopen",
                               return_value=Response({"session_id": "s2"})):
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "launch",
                 "--manifest", str(manifest_path), "--relaunch", "alpha"]), 0)
        receipts = self.module.read_receipts(
            self.module.build_directory(self.repository, "w9"))
        self.assertEqual(receipts["alpha"]["session_id"], "s2")

    def test_status_and_report_with_and_without_structured_output(self):
        directory = self.module.build_directory(self.repository, "w9")
        self.module.write_receipts(directory, {
            "alpha": {"name": "alpha", "session_id": "s1",
                      "url": "https://app.devin.ai/sessions/s1"},
            "beta": {"name": "beta", "session_id": "s2",
                     "url": "https://app.devin.ai/sessions/s2"},
        })
        structured = {"result": "blocked",
                      "commits": [{"sha": "abc123def", "subject": "wip"}],
                      "checks": [{"command": "cargo check", "host": "linux",
                                  "exit": 1}],
                      "remaining_dependency": "needs path owned by beta",
                      "unrelated_failures": ["flaky corpus entry"],
                      "lease_expiries": 2, "first_build_seconds": 400}
        responses = {"s1": {"status": "blocked", "status_detail": "waiting",
                            "acus_consumed": 7.5,
                            "structured_output": structured},
                     "s2": {"status": "expired", "status_detail": "stopped",
                            "acus_consumed": 10.0}}

        def fake_urlopen(request, timeout=60):
            session_id = request.full_url.rsplit("/", 1)[1]
            return Response(responses[session_id])

        with mock.patch.object(self.module.urllib.request, "urlopen", fake_urlopen):
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "status",
                 "--wave", "w9"]), 0)
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "report",
                 "--wave", "w9"]), 0)
        report = (directory / "report.md").read_text(encoding="utf-8")
        self.assertIn("blocked", report)
        self.assertIn("beta", report)
        self.assertIn("no structured output", report)
        self.assertIn("needs path owned by beta", report)

    def test_retry_on_429_then_success_and_fail_fast_on_401(self):
        calls = []

        def flaky(request, timeout=60):
            calls.append(1)
            if len(calls) == 1:
                raise http_error(429)
            return Response({"ok": True})

        with mock.patch.object(self.module.urllib.request, "urlopen", flaky), \
                mock.patch.object(self.module.time, "sleep"):
            self.assertEqual(self.module.api_request("GET", "https://x"), {"ok": True})
        self.assertEqual(len(calls), 2)

        with mock.patch.object(self.module.urllib.request, "urlopen",
                               side_effect=http_error(401)), \
                mock.patch.object(self.module.time, "sleep") as slept:
            with self.assertRaises(self.module.SwarmError):
                self.module.api_request("GET", "https://x")
            slept.assert_not_called()

    def test_freshness_probe_reports_paths(self):
        record = manifest()
        self.validate(record)
        probe = self.module.freshness_probe(self.repository,
                                            record["sessions"][0])
        paths = {line["path"] for line in probe}
        self.assertEqual(paths, {"src/one", "TASKS.md"})
        for line in probe:
            self.assertEqual(line["commits_7d"], 1)
            self.assertIn("Swarm Test", line["last_commit"])
            self.assertIsNone(line["crate"])

    def test_freshness_probe_reports_crate(self):
        (self.repository / "src" / "one" / "Cargo.toml").write_text(
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n", encoding="utf-8")
        (self.repository / "src" / "two").mkdir()
        record = manifest({"owning_paths": ["src/one", "src/two"]})
        self.validate(record)
        probe = self.module.freshness_probe(self.repository,
                                            record["sessions"][0])
        crates = {line["path"]: line["crate"] for line in probe}
        self.assertEqual(crates["src/one"], "src/one")
        self.assertIsNone(crates["src/two"])
        self.assertIsNone(crates["TASKS.md"])


if __name__ == "__main__":
    unittest.main()
