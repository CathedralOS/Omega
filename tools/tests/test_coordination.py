#!/usr/bin/env python3
"""Local partial-clone coverage for coordination's explicit acquisition boundary."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

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

if __name__=='__main__':unittest.main()
