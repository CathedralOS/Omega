#!/usr/bin/env python3
"""Local partial-clone coverage for coordination's explicit acquisition boundary."""
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

import worktree_testkit as kit

SPEC = importlib.util.spec_from_file_location('coordination_under_test', Path(__file__).resolve().parents[1] / 'coordination.py')
coordination = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(coordination)

class PartialCloneTests(unittest.TestCase):
    def test_missing_tip_probe_does_not_fetch_from_promisor(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); origin=root/'origin'; clone=root/'clone'
            def git(repository,*arguments):
                return subprocess.run(['git','-C',str(repository),*arguments],check=True,capture_output=True,text=True).stdout.strip()
            origin.mkdir();git(origin,'init','-q');git(origin,'config','user.name','Fixture');git(origin,'config','user.email','fixture@example.test')
            git(origin,'config','uploadpack.allowFilter','true');git(origin,'config','uploadpack.allowAnySHA1InWant','true')
            (origin/'file').write_text('first');git(origin,'add','file');git(origin,'commit','-qm','first')
            subprocess.run(['git','clone','-q','--filter=blob:none','--no-checkout',origin.as_uri(),str(clone)],check=True,capture_output=True)
            self.assertEqual(git(clone,'config','--get','remote.origin.promisor'),'true')
            # Golden-loss checks and intermediate checkout still need promised
            # content. Suppressing acquisition globally would hide lost rows.
            self.assertEqual(coordination.git(clone,'show','HEAD:file').stdout,'first')
            (origin/'file').write_text('second');git(origin,'add','file');git(origin,'commit','-qm','second');tip=git(origin,'rev-parse','HEAD')
            # The remote could satisfy this probe. The coordination wrapper must
            # report local absence, then acquire explicitly through its chosen URL.
            probe=coordination.git(clone,'cat-file','-e',tip,allow_failure=True)
            self.assertNotEqual(probe.returncode,0)
            coordination.fetch_object(clone,origin.as_uri(),tip)
            self.assertEqual(coordination.git(clone,'cat-file','-e',tip).returncode,0)

class SubmoduleFetchTests(unittest.TestCase):
    def test_registry_acquisition_does_not_contact_unavailable_submodules(self):
        root, origin = kit.seed_repository(self, "coordination submodules ")
        child = root / "child"
        kit.git(self, root, "init", "-b", "main", str(child))
        kit.configure(self, child, "Child Fixture")
        kit.git(self, child, "commit", "--allow-empty", "-m", "child")
        kit.git(self, origin, "-c", "protocol.file.allow=always", "submodule",
                "add", str(child), "application")
        kit.git(self, origin, "commit", "-am", "application submodule")
        reader = root / "reader"
        kit.git(self, root, "-c", "protocol.file.allow=always", "clone",
                "--recurse-submodules", str(origin), str(reader))
        missing = str(root / "unavailable-application")
        kit.git(self, reader / "application", "remote", "set-url", "origin", missing)
        kit.git(self, reader, "config", "fetch.recurseSubmodules", "true")

        # Model the real independent, empty-tree registry history. The ordinary
        # fetch control must try the unavailable submodule, proving this fixture
        # can distinguish object acquisition from inherited recursive fetching.
        with mock.patch.dict(os.environ, self.environment, clear=True):
            tree = coordination.git(origin, "mktree").stdout
            parent = coordination.git(origin, "commit-tree", tree,
                                      input_text="initial registry").stdout
            ordinary = coordination.git(reader, "fetch", "--no-tags",
                "--no-write-fetch-head", str(origin), parent, allow_failure=True)
            self.assertNotEqual(ordinary.returncode, 0)
            self.assertIn("application", ordinary.stderr)

            tools = Path(__file__).resolve().parents[1]
            for name, class_name, reference in (
                ("claims", "Claims", "refs/coordination/omega-claims/main"),
                ("landing", "Landing", "refs/coordination/omega-landing/main"),
            ):
                module = kit.load_module(tools / f"{name}.py", f"{name}_submodule_test")
                original_fetch = module.coordination.fetch_object
                for fallback in (False, True):
                    with self.subTest(protocol=name, fallback=fallback):
                        tip = coordination.git(origin, "commit-tree", tree, "-p", parent,
                            input_text=f"{name} fallback={fallback}").stdout
                        coordination.git(origin, "update-ref", reference, tip)
                        parent = tip
                        client = object.__new__(getattr(module, class_name))
                        client.repository, client.push_url = reader, str(origin)
                        client.deadline = time.monotonic() + 15

                        def acquire(repository, url, identity, **keywords):
                            if fallback and identity == tip:
                                raise module.coordination.CoordinationError("raw ID refused")
                            return original_fetch(repository, url, identity, **keywords)

                        cached = module.coordination.git(reader, "rev-parse", "--verify",
                            "--quiet", module.coordination.cache_ref(str(origin), reference),
                            allow_failure=True)
                        with mock.patch.object(module.coordination, "fetch_object",
                                               side_effect=acquire) as fetch, \
                                mock.patch.object(module.coordination, "git",
                                                  wraps=module.coordination.git) as commands:
                            client.fetch_version(tip)
                        self.assertEqual(fetch.call_count, 2 if fallback else 1)
                        acquisition = next(call.args for call in commands.call_args_list
                                           if call.args[1] == "fetch")
                        hints = [value for value in acquisition
                                 if isinstance(value, str) and value.startswith("--negotiation-tip=")]
                        self.assertEqual(hints, [f"--negotiation-tip={cached.stdout}"]
                                         if cached.returncode == 0 else [])
                        self.assertEqual(coordination.git(reader, "cat-file", "-e", tip).returncode, 0)
                        self.assertEqual(coordination.git(reader, "config", "--get",
                            "fetch.recurseSubmodules").stdout, "true")
                        self.assertEqual(coordination.git(reader / "application", "remote",
                            "get-url", "origin").stdout, missing)


class HistorySearchTests(unittest.TestCase):
    def test_old_ticket_and_merge_parent_are_searched(self):
        root, repository = kit.seed_repository(self, "history search ")
        with mock.patch.dict(os.environ, self.environment, clear=True):
            tree = coordination.git(repository, "mktree").stdout
            def commit(message, *parents):
                arguments = ["commit-tree", tree]
                for parent in parents:
                    arguments += ["-p", parent]
                return coordination.git(repository, *arguments, input_text=message).stdout
            retired = "a" * 32
            first = commit("retired " + retired)
            left = commit("left", first)
            right_ticket = "b" * 32
            right = commit("side history " + right_ticket, first)
            tip = commit("merge", left, right)
            for needle in (retired, right_ticket):
                self.assertTrue(coordination.history_contains_message(repository, tip, needle,
                                                                       deadline=time.monotonic() + 10))
            self.assertFalse(coordination.history_contains_message(repository, tip, "c" * 32,
                                                                    deadline=time.monotonic() + 10))

    def test_large_message_chunk_boundary_and_header_exclusion(self):
        needle = b"b" * 32
        header = b"tree " + b"a" * 40 + b"\nauthor " + needle + b"\n\n"
        for body in (b"clean", b"x" * (65536 - len(header) - 5) + needle + b"tail"):
            raw = header + body
            self.assertEqual(coordination._message_matches(io.BytesIO(raw), len(raw), needle, None),
                             needle in body)
        with self.assertRaises(coordination.CoordinationError):
            coordination._message_matches(io.BytesIO(b"tree without separator"), 22, needle, None)
        with self.assertRaises(coordination.CoordinationError):
            coordination._message_matches(io.BytesIO(header + needle), len(header) + 100, needle, None)

    def fake_history(self, payload, deadline=None, exit_code=0, delay=0, child_marker=None):
        # Keep real process ownership and pipes while replacing only the Git
        # producer, so malformed output and deadline tests exercise cleanup.
        from contextlib import contextmanager
        identity = b"d" * 40
        original = coordination._history_process
        processes = []
        real_popen = subprocess.Popen

        @contextmanager
        def producer(repository, arguments, standard_input, output, end):
            data = identity + b"\n" if arguments[0] == "rev-list" else payload
            pause = 0 if arguments[0] == "rev-list" else delay
            def launch(*args, **keywords):
                script = ("import sys,time,subprocess,pathlib;"
                          "sys.stdin.buffer.readline() if sys.argv[5]=='1' else None;"
                          "marker=sys.argv[4];"
                          "child=subprocess.Popen([sys.executable,'-c','import time;time.sleep(30)']) if marker else None;"
                          "pathlib.Path(marker).write_text(str(child.pid)) if marker else None;"
                          "sys.stdout.buffer.write(bytes.fromhex(sys.argv[1]));sys.stdout.flush();"
                          "time.sleep(float(sys.argv[2]));sys.exit(int(sys.argv[3]))")
                process = real_popen([sys.executable, "-c", script, data.hex(), str(pause),
                                      str(exit_code if arguments[0] == "cat-file" else 0),
                                      str(child_marker) if child_marker and arguments[0] == "cat-file" else "",
                                      "1" if arguments[0] == "cat-file" else "0"],
                                     **keywords)
                processes.append(process)
                return process
            with mock.patch.object(coordination.subprocess, "Popen", side_effect=launch):
                with original(repository, arguments, standard_input, output, end) as result:
                    yield result

        try:
            with mock.patch.object(coordination, "_history_process", side_effect=producer):
                return coordination.history_contains_message(".", "d" * 40, "e" * 32,
                                                             deadline=deadline)
        finally:
            self.assertTrue(processes)
            self.assertTrue(all(process.poll() is not None for process in processes))

    def test_malformed_truncated_missing_and_extra_output_fail_closed(self):
        identity = b"d" * 40
        raw = b"tree " + b"a" * 40 + b"\n\nclean"
        header = identity + b" commit " + str(len(raw)).encode() + b"\n"
        for payload in (b"", identity + b" missing\n", b"bad\n",
                        b"f" * 40 + b" commit 0\n", header + raw[:-1],
                        header + raw, header + raw + b"!", header + raw + b"\nextra"):
            with self.subTest(payload=payload), self.assertRaises(coordination.CoordinationError):
                self.fake_history(payload, time.monotonic() + 5)

    def test_failed_git_exit_cannot_establish_absence(self):
        raw = b"tree " + b"a" * 40 + b"\n\nclean"
        payload = b"d" * 40 + b" commit " + str(len(raw)).encode() + b"\n" + raw + b"\n"
        with self.assertRaises(coordination.CoordinationError):
            self.fake_history(payload, time.monotonic() + 5, exit_code=3)

    def test_blocked_pipe_timeout_settles_process(self):
        with self.assertRaises(coordination.CoordinationTimeout):
            self.fake_history(b"", time.monotonic() + 0.3, delay=30)

    def test_parse_error_settles_live_process(self):
        with self.assertRaises(coordination.CoordinationError):
            self.fake_history(b"malformed\n", time.monotonic() + 5, delay=30)

    def test_final_write_between_read_and_exit_poll_is_not_lost(self):
        stream = mock.Mock()
        stream.read.side_effect = [b"", b"final"]
        process = mock.Mock()
        process.poll.return_value = 0
        self.assertEqual(coordination._HistoryOutput(stream, process, None).read(5), b"final")

    @unittest.skipUnless(os.name == "posix" and Path("/proc").is_dir(), "Linux process observation")
    def test_timeout_and_parse_failure_settle_inherited_pipe_descendants(self):
        raw = b"tree " + b"a" * 40 + b"\n\n" + b"e" * 32
        valid = b"d" * 40 + b" commit " + str(len(raw)).encode() + b"\n" + raw + b"\n"
        for payload, delay in ((b"", 30), (b"malformed\n", 0), (valid, 30)):
            with tempfile.TemporaryDirectory() as directory:
                marker = Path(directory) / "child"
                if payload == valid:
                    self.assertTrue(self.fake_history(payload, time.monotonic() + 5,
                                                      delay=delay, child_marker=marker))
                else:
                    with self.assertRaises(coordination.CoordinationError):
                        self.fake_history(payload, time.monotonic() + 0.5,
                                          delay=delay, child_marker=marker)
                self.assertTrue(marker.is_file())
                status = Path("/proc") / marker.read_text() / "stat"
                settled = time.monotonic() + 0.5
                while status.exists() and status.read_text().split(")", 1)[1].split()[0] != "Z" and time.monotonic() < settled:
                    time.sleep(0.005)
                self.assertTrue(not status.exists() or status.read_text().split(")", 1)[1].split()[0] == "Z")


if __name__=='__main__':unittest.main()
