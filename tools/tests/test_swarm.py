#!/usr/bin/env python3
"""Swarm launcher tests; stdlib only, no network (urlopen is mocked)."""

import argparse
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import worktree_testkit as kit
from unittest import mock
import urllib.error


LAUNCH = Path(__file__).resolve().parents[1] / "swarm" / "launch.py"
TEMPLATE = Path(__file__).resolve().parents[1] / "swarm" / "prompt_template.md"


def load_launch():
    return kit.load_module(LAUNCH, 'swarm_under_test')


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
    for row in record['sessions']:
        if not row.get('semantic_contracts'):
            row.setdefault('work_classification', dict(kind='non_semantic', reason='Launcher transport test fixture without language changes'))
    return record


class OutcomeOwnershipTests(unittest.TestCase):
    def test_generated_prompts_keep_complete_outcome_ownership(self):
        module = load_launch()
        record = manifest()
        session = record['sessions'][0]
        repository = LAUNCH.parents[2]
        state = {'worktree': repository / 'build' / 'test-owner',
                 'branch': 'swarm/w9-alpha', 'ahead': 0,
                 'base': module.git(repository, 'rev-parse', 'HEAD')}
        prompts = {
            'cloud': module.render_prompt(TEMPLATE.read_text(encoding='utf-8'),
                                          record, session),
            'local': module.render_local_prompt(
                TEMPLATE.with_name('prompt_template_local.md').read_text(
                    encoding='utf-8'), record, session, repository, state),
        }
        for transport, prompt in prompts.items():
            with self.subTest(transport=transport):
                prompt = ' '.join(prompt.split())
                self.assertIn('law of completion in `AGENTS.md`', prompt)
                self.assertIn('authorized landing verified on remote main', prompt)
                self.assertIn('do not wait for another agent', prompt)
                self.assertIn('Do not publish half-finished functionality', prompt)
                self.assertIn('without bypassing any gate', prompt)
                self.assertIn('explicitly read-only probe remains read-only', prompt)
                self.assertIn('Working paths reveal the design', prompt)
                self.assertNotIn('without completing or handing off', prompt)

    def test_legacy_hints_and_readonly_probes_do_not_change_completion(self):
        module = load_launch()
        hint = 'ONE bounded leg; return a handoff only'
        block, probe = module.session_blocks({'suggested_first_slice': hint})
        self.assertIn(hint, block)
        self.assertIn('not a finish line', block)
        self.assertIn('superseded by AGENTS.md', block)
        self.assertEqual(probe, '')
        block, probe = module.session_blocks({'probe_only': True})
        self.assertEqual(block, '')
        self.assertIn('explicitly read-only', probe)
        self.assertIn('Do not edit or publish code', probe)
        self.assertIn('not a completed implementation', probe)

    def test_canonical_skills_reference_the_completion_law(self):
        repository = LAUNCH.parents[2]
        agents = (repository / 'AGENTS.md').read_text(encoding='utf-8')
        self.assertIn('## Law of completion', agents)
        self.assertIn('uncompilable sample through every necessary compiler', agents)
        for name in ('advance', 'devin-swarm', 'local-swarm', 'codex-swarm',
                     'dot-swarm', 'cursor-swarm'):
            with self.subTest(skill=name):
                skill = repository / '.agents' / 'skills' / name / 'SKILL.md'
                self.assertIn('AGENTS.md#law-of-completion',
                              skill.read_text(encoding='utf-8'))


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

    def configure_origin(self):
        remote = self.root / "origin.git"
        self.git(self.root, "init", "--bare", str(remote))
        self.git(self.repository, "remote", "add", "origin", str(remote))
        self.git(self.repository, "push", "-u", "origin", "main")

    def test_semantic_launch_review_and_continuation_deliver_verified_context(self):
        spec = self.repository / 'wiki/spec/rule.md'
        spec.parent.mkdir(parents=True)
        spec.write_text('The selected canonical rule.\n', encoding='utf-8')
        self.git(self.repository, 'add', '.')
        self.git(self.repository, 'commit', '-m', 'canonical rule')
        self.configure_origin()
        base = self.git(self.repository, 'rev-parse', 'HEAD')
        record = manifest(dict(host='local', source_revision=base, semantic_contracts=[dict(
            implementation=['src/one'], contract='wiki/spec/rule.md',
            classification='missing_implementation', reason='repair the existing behavior',
            passages=['The selected canonical rule.'])]))
        session = self.validate(record)[0]
        manifest_path = self.root / 'semantic-wave.json'
        def delivered_prompt():
            manifest_path.write_text(json.dumps(record), encoding='utf-8')
            with mock.patch.object(self.module, 'emit'):
                result = self.module.main(['--repository', str(self.repository), 'local',
                    '--manifest', str(manifest_path), '--skip-host-gates',
                    '--skip-route-check', '--skip-claims-check'])
            self.assertEqual(result, 0)
            return (self.module.build_directory(self.repository, 'w9') / 'prompts/alpha.md').read_text(encoding='utf-8')
        self.assertIn('The selected canonical rule.', delivered_prompt())
        state = dict(worktree=self.repository, branch='main', ahead=1, base=base)
        cloud = TEMPLATE.read_text(encoding='utf-8')
        local = TEMPLATE.with_name('prompt_template_local.md').read_text(encoding='utf-8')
        for prompt in (self.module.render_prompt(cloud, record, session),
                       self.module.render_local_prompt(local, record, session, self.repository, state)):
            self.assertIn('The selected canonical rule.', prompt)
            self.assertIn(base, prompt)
            self.assertIn(self.git(self.repository, 'rev-parse', base + ':wiki/spec/rule.md'), prompt)
        prior = session['semantic_handoff']
        session['semantic_handoff_role'] = 'review'
        self.validate(record)
        for prompt in (self.module.render_prompt(cloud, record, session),
                       self.module.render_local_prompt(local, record, session, self.repository, state)):
            self.assertIn('The selected canonical rule.', prompt)
            self.assertIn('Independent read-only final review', prompt)
            self.assertIn('Do not edit files, claim implementation ownership or publish', prompt)
            self.assertIn('implementation owner retains landing', prompt)
            self.assertNotIn('advance', prompt)
        self.assertIn('Independent read-only final review', delivered_prompt())
        session['semantic_handoff_role'] = 'continuation'
        self.validate(record)
        self.assertIn('The selected canonical rule.', delivered_prompt())
        session['previous_semantic_handoff'] = prior
        self.validate(record)
        resumed = self.module.render_local_prompt(local, record, session, self.repository, state)
        self.assertIn('The selected canonical rule.', resumed)
        self.assertIn('The selected canonical rule.', delivered_prompt())
        spec.write_text('The revised canonical rule.\n', encoding='utf-8')
        self.git(self.repository, 'add', '.')
        self.git(self.repository, 'commit', '-m', 'revise canonical rule')
        session['source_revision'] = self.git(self.repository, 'rev-parse', 'HEAD')
        with self.assertRaisesRegex(self.module.SwarmError, 'missing/stale'):
            self.validate(record)
        session['semantic_contracts'][0]['passages'] = ['The revised canonical rule.']
        self.validate(record)
        refreshed = self.module.render_local_prompt(local, record, session, self.repository, state)
        self.assertIn('The revised canonical rule.', refreshed)
        self.assertIn('The revised canonical rule.', delivered_prompt())

    def test_default_compiler_launch_requires_mapping_and_non_semantic_needs_reason(self):
        record=manifest({'host':'local'})
        record['sessions'][0].pop('work_classification')
        path=self.root/'missing-context.json';path.write_text(json.dumps(record))
        with contextlib.redirect_stderr(io.StringIO()),mock.patch.object(self.module,'emit'):
            self.assertEqual(self.module.main(['--repository',str(self.repository),'local',
                '--manifest',str(path),'--skip-host-gates','--skip-route-check','--skip-claims-check']),1)
        self.assertFalse((self.module.build_directory(self.repository,'w9')/'prompts/alpha.md').exists())
        record['sessions'][0]['host']='linux';path.write_text(json.dumps(record))
        with contextlib.redirect_stderr(io.StringIO()),mock.patch.object(self.module,'emit'):
            self.assertEqual(self.module.main(['--repository',str(self.repository),'launch',
                '--manifest',str(path),'--dry-run']),1)
        record['sessions'][0]['work_classification']=dict(kind='non_semantic',reason=' ')
        with self.assertRaisesRegex(self.module.SwarmError,'concrete reason'):
            self.validate(record)
        record['sessions'][0]['work_classification']['reason']='Only launcher transport changes'
        self.validate(record)

    def test_manifest_rejects_excluded_item(self):
        record = manifest({"item": "ITEM-EXCLUDED"})
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_manifest_allows_shared_paths_for_distinct_outcomes(self):
        record = manifest(sessions=[
            {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
             "host": "linux", "owning_paths": ["src/one"]},
            {"name": "b", "board": "TASKS.md", "item": "ITEM-TWO",
             "host": "linux", "owning_paths": ["src/one"]}])
        self.assertEqual(len(self.validate(record)), 2)

    def test_manifest_allows_path_overlap_across_layers(self):
        record = manifest(sessions=[
            {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
             "host": "linux", "owning_paths": ["src/one"], "layer": 0},
            {"name": "b", "board": "TASKS.md", "item": "ITEM-TWO",
             "host": "linux", "owning_paths": ["src/one"], "layer": 1}])
        self.assertEqual(len(self.validate(record)), 2)
        record["sessions"][1]["layer"] = 0
        self.assertEqual(len(self.validate(record)), 2)
        record["sessions"][1]["layer"] = -1
        with self.assertRaises(self.module.SwarmError):
            self.validate(record)

    def test_filter_layer_selects_one_layer(self):
        sessions = [{"name": "a"}, {"name": "b", "layer": 0},
                    {"name": "c", "layer": 1}]
        self.assertEqual(self.module.filter_layer(sessions, None), sessions)
        self.assertEqual([s["name"] for s in
                          self.module.filter_layer(sessions, 0)], ["a", "b"])
        self.assertEqual([s["name"] for s in
                          self.module.filter_layer(sessions, 1)], ["c"])
        with self.assertRaises(self.module.SwarmError):
            self.module.filter_layer(sessions, 2)

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

        passing = manifest({"host_gates": ["exit 0"]})
        manifest_path.write_text(json.dumps(passing), encoding="utf-8")
        with mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path), skip_host_gates=False),
                self.repository), 0)
            planned = emitted.call_args.args[0]["sessions"][0]
        self.assertEqual(planned["host_gates"], [{"command": "exit 0", "exit": 0}])
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

    def plan_first_session(self, manifest_path, route):
        """command_plan under a fixed route; returns the first emitted session."""
        with mock.patch.object(self.module, "route_crates", return_value=route), \
                mock.patch.object(self.module, "emit") as emitted:
            self.assertEqual(self.module.command_plan(
                mock.Mock(manifest=str(manifest_path),
                          skip_host_gates=True, skip_route_check=False),
                self.repository), 0)
            return emitted.call_args.args[0]["sessions"][0]

    def test_plan_scopes_off_route_rejection_to_compiler_crates(self):
        (self.repository / "tests" / "native-differential" / "Cargo.toml").parent.mkdir(
            parents=True)
        (self.repository / "tests" / "native-differential" / "Cargo.toml").write_text(
            "[package]\nname = \"one\"\nversion = \"0.1.0\"\n", encoding="utf-8")
        (self.repository / "omega-rust" / "one" / "Cargo.toml").parent.mkdir(
            parents=True)
        (self.repository / "omega-rust" / "one" / "Cargo.toml").write_text(
            "[package]\nname = \"one-compiler\"\nversion = \"0.1.0\"\n",
            encoding="utf-8")
        manifest_path = self.root / "wave.json"
        record = manifest()
        record["sessions"][0]["owning_paths"] = [
            "tests/native-differential/tests/scalar_case_results.rs"
        ]
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        route = {
            "tests/native-differential": {"name": "one", "on_route": False},
            "omega-rust/one": {"name": "one-compiler", "on_route": False},
        }
        planned = self.plan_first_session(manifest_path, route)
        freshness = {line["path"]: line for line in planned["freshness"]}
        self.assertFalse(freshness[
            "tests/native-differential/tests/scalar_case_results.rs"
        ]["on_route"])

        record["wave"] = "w10"
        record["sessions"][0]["owning_paths"] = ["omega-rust/one/src/lib.rs"]
        record["sessions"][0].pop("probe_only", None)
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        with mock.patch.object(self.module, "route_crates", return_value=route):
            with self.assertRaisesRegex(self.module.SwarmError, "not on the omega route"):
                self.module.command_plan(
                    mock.Mock(manifest=str(manifest_path),
                              skip_host_gates=True, skip_route_check=False),
                    self.repository)
        prompt_path = (self.module.build_directory(self.repository, "w10")
                       / "prompts" / "alpha.md")
        self.assertFalse(prompt_path.exists())

        record["sessions"][0]["probe_only"] = True
        manifest_path.write_text(json.dumps(record), encoding="utf-8")
        planned = self.plan_first_session(manifest_path, route)
        freshness = {line["path"]: line for line in planned["freshness"]}
        self.assertFalse(freshness["omega-rust/one/src/lib.rs"]["on_route"])
        self.assertEqual(freshness["omega-rust/one/src/lib.rs"]["crate_name"],
                         "one-compiler")

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

    def test_prompt_keeps_cross_stage_implementation_actionable(self):
        record = manifest()
        self.validate(record)
        template = TEMPLATE.read_text(encoding="utf-8")
        prompt = self.module.render_prompt(template, record, record["sessions"][0])
        self.assertIn("Required edits may span other sessions' paths and multiple stages", prompt)
        self.assertIn("it grants\nnothing and blocks nothing", prompt)
        self.assertIn("There are no handoffs", prompt)
        self.assertIn("A checkpoint does not end the work", prompt)
        self.assertIn("diagnosis is intermediate", prompt)
        self.assertIn("`omega run` after relevant changes", prompt)
        self.assertNotIn("DRI", prompt)
        self.assertNotIn("coordinator writes board updates", prompt)
        self.assertIn("only when you were actively implementing", prompt)
        self.assertNotIn("is `verification_only`, not `budget_exhausted`", prompt)

    def write_sample(self, path, header):
        directory = self.repository / path
        directory.mkdir(parents=True)
        (directory / "main.omg").write_text(header + "machine main() {}\n",
                                            encoding="utf-8")

    def sample_manifest(self, sample, host="linux"):
        record = manifest()
        record["sessions"][0] = {
            "name": "alpha", "sample": sample, "host": host,
            "owning_paths": ["src/one"],
            "work_classification": dict(kind="non_semantic", reason="fixture")}
        return record

    def test_sample_session_runs_toward_its_oracle(self):
        self.write_sample("samples/cli/demo", "// Expected exit: 7.\n")
        record = self.sample_manifest("samples/cli/demo")
        session = self.validate(record)[0]
        self.assertEqual(session["item"], "samples/cli/demo")
        self.assertIsNone(session["board"])
        command = self.module.claim_command(session, "Tester / w9-alpha")
        self.assertNotIn("--board", command)
        self.assertIn("--item samples/cli/demo", command)
        template = TEMPLATE.read_text(encoding="utf-8")
        prompt = " ".join(self.module.render_prompt(template, record, session).split())
        self.assertIn("Your customer is sample `samples/cli/demo`", prompt)
        self.assertIn("authored oracle (exit 7)", prompt)
        self.assertNotIn("Your customer is board item", prompt)
        local = TEMPLATE.with_name("prompt_template_local.md").read_text(encoding="utf-8")
        state = {"worktree": self.repository / "wt", "branch": "swarm/w9-alpha", "ahead": 0,
                 "base": self.git(self.repository, "rev-parse", "HEAD")}
        local_prompt = self.module.render_local_prompt(
            local, record, session, self.repository, state)
        self.assertIn("samples/cli/demo", local_prompt)
        for field in self.module.LOCAL_PROMPT_FIELDS:
            self.assertNotIn("{" + field + "}", local_prompt)

    def test_sample_session_without_an_oracle_is_refused(self):
        self.write_sample("samples/cli/pins_a_contract", "// Checks only.\n")
        with self.assertRaisesRegex(self.module.SwarmError, "no `Expected"):
            self.validate(self.sample_manifest("samples/cli/pins_a_contract"))
        with self.assertRaisesRegex(self.module.SwarmError, "samples/ project"):
            self.validate(self.sample_manifest("src/one"))

    def test_sample_session_names_no_board_row(self):
        self.write_sample("samples/cli/demo", "// Expected exit: 0.\n")
        record = self.sample_manifest("samples/cli/demo")
        record["sessions"][0]["board"] = "TASKS.md"
        with self.assertRaisesRegex(self.module.SwarmError, "not a board or item"):
            self.validate(record)

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
        self.configure_origin()
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
        fetched = self.git(self.repository, "rev-parse", "FETCH_HEAD")
        self.assertEqual(receipts["alpha"]["base"], fetched)
        self.assertIn(f"freshly fetched main at `{fetched}`", receipts["alpha"]["body"]["prompt"])

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

    def test_report_save_records_tracked_outcomes(self):
        directory = self.module.build_directory(self.repository, "w9")
        self.module.write_receipts(directory, {
            "alpha": {"name": "alpha", "session_id": "s1",
                      "url": "https://app.devin.ai/sessions/s1"},
            "beta": {"name": "beta", "session_id": "s2",
                     "url": "https://app.devin.ai/sessions/s2"},
        })
        waves = self.repository / "tools" / "swarm" / "waves"
        waves.mkdir(parents=True)
        record = manifest(sessions=[
            {"name": "alpha", "board": "TASKS.md", "item": "ITEM-ONE",
             "host": "linux", "owning_paths": ["src/one"]},
            {"name": "beta", "board": "TASKS.md", "item": "ITEM-GONE",
             "host": "linux", "owning_paths": ["src/two"]}])
        (waves / "w9.json").write_text(json.dumps(record), encoding="utf-8")
        structured = {"result": "landed", "commits": [], "checks": [],
                      "remaining_dependency": "", "unrelated_failures": [],
                      "lease_expiries": 0}
        responses = {"s1": {"status": "finished", "acus_consumed": 8.0,
                            "structured_output": structured},
                     "s2": {"status": "finished", "acus_consumed": 6.0,
                            "structured_output": structured}}

        def fake_urlopen(request, timeout=60):
            session_id = request.full_url.rsplit("/", 1)[1]
            return Response(responses[session_id])

        with mock.patch.object(self.module.urllib.request, "urlopen", fake_urlopen):
            self.assertEqual(self.module.main(
                ["--repository", str(self.repository), "report",
                 "--wave", "w9", "--save"]), 0)
        outcomes = json.loads((waves / "w9.outcomes.json").read_text(
            encoding="utf-8"))
        by_name = {row["name"]: row for row in outcomes["sessions"]}
        self.assertEqual(by_name["alpha"]["result"], "landed")
        self.assertFalse(by_name["alpha"]["item_closed"])
        self.assertTrue(by_name["beta"]["item_closed"])
        self.assertEqual(by_name["beta"]["item"], "ITEM-GONE")
        self.assertEqual(outcomes["summary"]["items_closed"], 1)
        self.assertEqual(outcomes["summary"]["acus_consumed"], 14.0)
        self.assertTrue((directory / "report.md").is_file())

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

    def write_board(self, text):
        (self.repository / "TASKS.md").write_text(text, encoding="utf-8")

    def test_partition_hints_flag_dependency_language(self):
        self.write_board(
            "- **ITEM-ONE.** Join semantics from the preceding task. "
            "Depends on ITEM-TWO above.\n"
            "- **ITEM-TWO.** Second item.\n")
        session = {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
                   "owning_paths": ["src/one"], "layer": 0}
        hints = self.module.partition_hints(self.repository, session,
                                            [session], "skipped")
        self.assertIn("join", hints["dependency_language"])
        self.assertIn("depends on", hints["dependency_language"])
        self.assertEqual(hints["references_items"], ["ITEM-TWO"])
        self.assertNotIn("same_layer_reference", hints)

    def test_partition_hints_flag_same_layer_reference(self):
        self.write_board(
            "- **ITEM-ONE.** Consumes ITEM-TWO output.\n"
            "- **ITEM-TWO.** Second item.\n")
        one = {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
               "owning_paths": ["src/one"], "layer": 0}
        two = {"name": "b", "board": "TASKS.md", "item": "ITEM-TWO",
               "owning_paths": ["src/two"], "layer": 0}
        hints = self.module.partition_hints(self.repository, one,
                                            [one, two], "skipped")
        self.assertIn("ITEM-TWO", hints["same_layer_reference"])
        two["layer"] = 1
        hints = self.module.partition_hints(self.repository, one,
                                            [one, two], "skipped")
        self.assertNotIn("same_layer_reference", hints)

    def test_partition_hints_flag_uncovered_mentions_and_scale(self):
        self.write_board(
            "- **ITEM-ONE.** Wire `other-crate/src/foo.rs` through "
            "`omega-rust/omega/packages/build-output` and `third-crate` while "
            "`fourth-crate` and `fifth-crate` keep their evidence.\n")
        crates = {"x/other-crate": {"name": "other-crate", "on_route": True},
                  "x/third-crate": {"name": "third-crate", "on_route": True},
                  "x/fourth-crate": {"name": "fourth-crate", "on_route": True},
                  "x/fifth-crate": {"name": "fifth-crate", "on_route": True}}
        session = {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
                   "owning_paths": ["omega-rust/omega/packages/build-output"]}
        hints = self.module.partition_hints(self.repository, session,
                                            [session], crates)
        self.assertIn("other-crate/src/foo.rs", hints["uncovered_mentions"])
        self.assertNotIn("omega-rust/omega/packages/build-output",
                         hints["uncovered_mentions"])
        self.assertIn("scale_hint", hints)

    def test_partition_hints_quiet_when_text_is_benign(self):
        session = {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
                   "owning_paths": ["src/one"]}
        hints = self.module.partition_hints(self.repository, session,
                                            [session], "skipped")
        self.assertEqual(hints, {})

    def test_partition_hints_flag_resolution_language(self):
        self.write_board(
            "- **ITEM-ONE.** Resolved — scope verified, already landed at "
            "`abc123`.\n- **ITEM-TWO.** Second item.\n")
        session = {"name": "a", "board": "TASKS.md", "item": "ITEM-ONE",
                   "owning_paths": ["src/one"]}
        hints = self.module.partition_hints(self.repository, session,
                                            [session], "skipped")
        self.assertIn("resolution_language", hints)
        self.assertIn("already landed", hints["resolution_language"])
        self.assertIn("resolved —", hints["resolution_language"])

    def test_commit_signal_classifies_landed_commits(self):
        (self.repository / "src" / "one" / "code.rs").write_text(
            "// changed\n", encoding="utf-8")
        self.git(self.repository, "add", "-A")
        self.git(self.repository, "commit", "-m", "content")
        content = self.git(self.repository, "rev-parse", "HEAD")[:10]
        self.write_board("- **ITEM-ONE.** swept row\n")
        self.git(self.repository, "add", "TASKS.md")
        self.git(self.repository, "commit", "-m", "board ledger")
        board = self.git(self.repository, "rev-parse", "HEAD")[:10]
        self.git(self.repository, "commit", "--allow-empty", "-m", "empty")
        empty = self.git(self.repository, "rev-parse", "HEAD")[:10]
        rows = [{"commits": [f"{content} content", f"{board} board ledger",
                             f"{empty} empty", "deadbeef00 not fetched"]}]
        self.assertEqual(
            self.module.commit_signal(self.repository, rows),
            {"commits": 3, "board_only": 1, "empty": 1, "unresolved_refs": 1})

    def local_manifest(self):
        record = manifest(sessions=[
            {"name": "alpha", "board": "TASKS.md", "item": "ITEM-ONE",
             "host": "local", "owning_paths": ["src/one"]}])
        path = self.repository / "manifest.json"
        path.write_text(json.dumps(record), encoding="utf-8")
        return path

    def test_local_prompt_renders_shared_host_guidance_on_supported_hosts(self):
        template = (LAUNCH.parent / "prompt_template_local.md").read_text(
            encoding="utf-8")
        record = manifest({"host": "local"})
        state = {"ahead": 0, "worktree": self.repository / "worker",
                 "branch": "swarm/w9-alpha",
                 "base": self.git(self.repository, "rev-parse", "HEAD")}
        for system, machine in (("Linux", "x86_64"), ("Windows", "AMD64"),
                                ("Darwin", "arm64")):
            with self.subTest(system=system), mock.patch.object(
                    self.module.platform, "system", return_value=system), \
                    mock.patch.object(self.module.platform, "machine",
                                      return_value=machine):
                prompt = self.module.render_local_prompt(
                    template, record, record["sessions"][0], self.repository,
                    state)
                self.assertNotIn("{host_block}", prompt)
                self.assertNotIn("{build_tool}", prompt)
                self.assertIn("local-swarm/host-setup.md", prompt)
                self.assertIn("CARGO_TARGET_DIR", prompt)
                self.assertIn("stdin=subprocess.DEVNULL", prompt)

    def run_local(self, manifest_path):
        arguments = argparse.Namespace(
            manifest=str(manifest_path), layer=None, sessions=None,
            create_worktrees=True, skip_host_gates=True,
            skip_route_check=True, skip_claims_check=True)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            self.module.command_local(arguments, self.repository)
        return json.loads(output.getvalue())

    def test_local_fetch_failure_refuses_cached_base_before_preparation(self):
        self.git(self.repository, "update-ref", "refs/remotes/origin/main",
                 "HEAD")
        with mock.patch.object(self.module, "session_probes",
                               return_value=({"alpha": "skipped"}, {"alpha": []},
                                             {"alpha": {}}, {"alpha": "skipped"})) as probes:
            with self.assertRaisesRegex(self.module.SwarmError, "fetch"):
                self.run_local(self.local_manifest())
        probes.assert_not_called()
        self.assertFalse((self.repository / ".codex" / "worktrees"
                          / "w9-alpha").exists())
        self.assertFalse(self.module.build_directory(self.repository, "w9").exists())

    def test_local_dispatch_table_requires_fetch_without_worktree_creation(self):
        self.git(self.repository, "update-ref", "refs/remotes/origin/main", "HEAD")
        arguments = argparse.Namespace(
            manifest=str(self.local_manifest()), layer=None, sessions=None,
            create_worktrees=False, skip_host_gates=True,
            skip_route_check=True, skip_claims_check=True)
        with self.assertRaisesRegex(self.module.SwarmError, "fetch"):
            self.module.command_local(arguments, self.repository)

    def test_cloud_fetch_failure_creates_no_sessions_or_receipts(self):
        self.git(self.repository, "update-ref", "refs/remotes/origin/main", "HEAD")
        path = self.root / "wave.json"
        path.write_text(json.dumps(manifest()), encoding="utf-8")
        arguments = argparse.Namespace(
            manifest=str(path), layer=None, dry_run=False,
            skip_host_gates=True, skip_route_check=True, skip_claims_check=True,
            relaunch=None, base_url="https://example.invalid")
        with mock.patch.object(self.module, "api_request",
                               return_value={"session_id": "unexpected-create"}) as request, \
                mock.patch.object(self.module, "host_gate_results") as gates:
            with self.assertRaisesRegex(self.module.SwarmError, "fetch"):
                self.module.command_launch(arguments, self.repository)
        request.assert_not_called()
        gates.assert_not_called()
        self.assertFalse(self.module.build_directory(self.repository, "w9").exists())

    def test_local_create_worktrees_reports_fetched_base(self):
        remote = self.root / "origin.git"
        self.git(self.root, "init", "--bare", str(remote))
        self.git(self.repository, "remote", "add", "origin", str(remote))
        self.git(self.repository, "push", "-u", "origin", "main")
        record = self.run_local(self.local_manifest())
        self.assertEqual(record["base"],
                         self.git(self.repository, "rev-parse", "origin/main"))
        self.assertTrue((self.repository / ".codex" / "worktrees"
                         / "w9-alpha").is_dir())

    def test_local_new_worker_uses_remote_main_and_preserves_dirty_checkout(self):
        remote = self.root / "origin.git"
        self.git(self.root, "init", "--bare", str(remote))
        self.git(self.repository, "remote", "add", "origin", str(remote))
        self.git(self.repository, "push", "-u", "origin", "main")
        old_head = self.git(self.repository, "rev-parse", "HEAD")
        publisher = self.root / "publisher"
        self.git(self.root, "clone", "--branch", "main", str(remote), str(publisher))
        self.git(publisher, "config", "user.name", "Swarm Test")
        self.git(publisher, "config", "user.email", "test@example.invalid")
        (publisher / "latest.txt").write_text("new main instructions\n", encoding="utf-8")
        self.git(publisher, "add", "latest.txt")
        self.git(publisher, "commit", "-m", "repo: update startup instructions")
        self.git(publisher, "push", "origin", "main")
        fresh_head = self.git(publisher, "rev-parse", "HEAD")
        dirty = self.repository / "src" / "one" / "code.rs"
        dirty.write_text("// unpublished work\n", encoding="utf-8")
        self.assertEqual(self.git(self.repository, "rev-parse", "origin/main"), old_head)
        # Even a narrow fetch configuration must update main explicitly. A later
        # tracking-ref change cannot change the commit chosen for this worker.
        self.git(self.repository, "config", "remote.origin.fetch",
                 "refs/heads/unrelated:refs/remotes/origin/unrelated")
        original_state = self.module.local_session_state

        def move_tracking_ref(*arguments):
            self.git(self.repository, "update-ref", "refs/remotes/origin/main", old_head)
            return original_state(*arguments)

        with mock.patch.object(self.module, "local_session_state", side_effect=move_tracking_ref):
            record = self.run_local(self.local_manifest())
        worker = self.repository / ".codex" / "worktrees" / "w9-alpha"
        self.assertEqual(record["base"], fresh_head)
        self.assertEqual(self.git(worker, "rev-parse", "HEAD"), fresh_head)
        prompt = (self.repository / record["sessions"][0]["prompt"]).read_text()
        self.assertIn(f"-b swarm/w9-alpha {fresh_head}`", prompt)
        self.assertEqual((worker / "latest.txt").read_text(), "new main instructions\n")
        self.assertEqual(self.git(self.repository, "rev-parse", "HEAD"), old_head)
        self.assertEqual(dirty.read_text(), "// unpublished work\n")


class OwnershipPathTests(unittest.TestCase):
    def setUp(self):
        self.module = load_launch()
        self.root, self.repository = kit.seed_repository(self, 'omega ownership test ')
        (self.repository / 'TASKS.md').write_text('- **ITEM-ONE.** One\n- **ITEM-TWO.** Two\n', encoding='utf-8')

    def test_live_registry_paths_are_advisory_but_item_ownership_rejects(self):
        sessions = self.module.validate_manifest(manifest(), self.repository)
        overlap = dict(reason='overlapping paths', owner='peer', item='ITEM-TWO',
                       ticket='peer-ticket', expires_utc='later')
        with mock.patch.object(self.module.claims, 'Claims') as constructor:
            registry = constructor.return_value
            registry.snapshot.return_value = {'record': {}}
            registry.conflicts.return_value = [overlap]
            self.assertEqual(self.module.claims_report(self.repository, sessions),
                             {'alpha': [overlap]})
            registry.conflicts.return_value = [dict(overlap, reason='same item')]
            with self.assertRaises(self.module.SwarmError):
                self.module.claims_report(self.repository, sessions)

    def test_rejects_invalid_ownership_paths_with_session_context(self):
        for path in ('../outside', '/absolute', 'C:\\outside', '.', '', '  ', 42, None):
            with self.subTest(path=path):
                with self.assertRaisesRegex(self.module.SwarmError, 'alpha'):
                    self.module.validate_manifest(manifest({'owning_paths': [path]}), self.repository)

    def test_canonical_paths_reach_returned_sessions(self):
        record = manifest({'owning_paths': [' ./src\\one/ ', ' src/two ']})
        sessions = self.module.validate_manifest(record, self.repository)
        self.assertEqual(sessions[0]['owning_paths'], ['src/one', 'src/two'])

    def test_same_layer_paths_overlap_but_outcomes_stay_distinct(self):
        record = manifest(sessions=[
            {'name':'alpha','board':'TASKS.md','item':'ITEM-ONE','host':'local','owning_paths':['src\\one']},
            {'name':'beta','board':'TASKS.md','item':'ITEM-TWO','host':'local','owning_paths':['src/one/child']}])
        self.assertEqual(len(self.module.validate_manifest(record, self.repository)), 2)
        record['sessions'][1]['item'] = 'ITEM-ONE'
        with self.assertRaises(self.module.SwarmError):
            self.module.validate_manifest(record, self.repository)
        record['sessions'][1]['item'] = 'ITEM-TWO'
        record['sessions'][1]['owning_paths'] = ['src/one-more']
        self.assertEqual(len(self.module.validate_manifest(record, self.repository)), 2)

    def test_different_layers_keep_allowed_overlap(self):
        record = manifest(sessions=[
            {'name':'alpha','board':'TASKS.md','item':'ITEM-ONE','host':'local','owning_paths':['src\\one'],'layer':0},
            {'name':'beta','board':'TASKS.md','item':'ITEM-TWO','host':'local','owning_paths':['src/one'],'layer':1}])
        self.assertEqual(len(self.module.validate_manifest(record, self.repository)), 2)


class JevFenceTests(unittest.TestCase):
    def setUp(self):
        self.module = load_launch()

    def call_fence(self):
        return self.module.jev_fence_verdict(
            "ITEM-ONE", "the item text", ["src/one"])

    def test_returns_the_choice_from_the_response(self):
        with mock.patch.dict(os.environ, {"OMEGA_JEV_OFFLINE": ""}), \
                mock.patch.object(self.module.typesafe_key, "find_key",
                                  return_value="test-key"), \
                mock.patch.object(self.module.typesafe_key, "post",
                                  return_value={"answers": {"fence": {
                                      "choice": "under_fenced"}}}) as post:
            self.assertEqual(self.call_fence(), "under_fenced")
        post.assert_called_once()
        arguments, keywords = post.call_args
        self.assertEqual(arguments[:2],
                         (self.module.typesafe_key.ENDPOINT, "test-key"))
        self.assertEqual(keywords.get("timeout"), 15)

    def test_none_response_yields_none(self):
        with mock.patch.dict(os.environ, {"OMEGA_JEV_OFFLINE": ""}), \
                mock.patch.object(self.module.typesafe_key, "find_key",
                                  return_value="test-key"), \
                mock.patch.object(self.module.typesafe_key, "post",
                                  return_value=None) as post:
            self.assertIsNone(self.call_fence())
        self.assertEqual(post.call_args.kwargs.get("timeout"), 15)

    def test_offline_and_missing_key_return_none_without_posting(self):
        with mock.patch.dict(os.environ, {"OMEGA_JEV_OFFLINE": "1"}), \
                mock.patch.object(self.module.typesafe_key, "post") as post:
            self.assertIsNone(self.call_fence())
            post.assert_not_called()
        with mock.patch.dict(os.environ, {"OMEGA_JEV_OFFLINE": ""}), \
                mock.patch.object(self.module.typesafe_key, "find_key",
                                  return_value=""), \
                mock.patch.object(self.module.typesafe_key, "post") as post:
            self.assertIsNone(self.call_fence())
            post.assert_not_called()


if __name__ == "__main__":
    unittest.main()
