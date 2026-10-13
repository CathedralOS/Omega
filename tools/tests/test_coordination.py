#!/usr/bin/env python3
"""Local partial-clone coverage for coordination's explicit acquisition boundary."""
import importlib.util
import os
from pathlib import Path
import subprocess
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


if __name__=='__main__':unittest.main()
