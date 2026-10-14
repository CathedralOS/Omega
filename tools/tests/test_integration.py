#!/usr/bin/env python3
"""Real Git/subprocess lifecycle checks; no compiler acceptance is simulated."""
import copy
import io
import json
from pathlib import Path
import sys
import tarfile
import unittest
from unittest import mock
import os

import worktree_testkit as kit
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import integration as integration
q = integration.q


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        self.root, self.worker = kit.seed_repository(self, "omega-integration-")
        (self.worker / "src").mkdir()
        (self.worker / "src/program").write_text("old\n")
        (self.worker / ".gitignore").write_text("build/\n")
        self.git(self.worker, "add", "-A"); self.git(self.worker, "commit", "-m", "fixture base")
        self.base = self.git(self.worker, "rev-parse", "HEAD")
        (self.worker / "src/program").write_text("repaired\n")
        self.git(self.worker, "add", "src"); self.git(self.worker, "commit", "-m", "fixture logical fix")
        self.tip = self.git(self.worker, "rev-parse", "HEAD")
        self.local = self.worker / "build/local.json"
        self.config = dict(context={"host": "fixture"}, work_classification=dict(kind="non_semantic", reason="workflow lifecycle fixture"),
            checks=[dict(id="positive", kind="tests", command=[sys.executable, "-c", "from pathlib import Path; assert Path('src/program').read_text()=='repaired\\n'"],
                         inputs=["src"], artifacts=[], closure_reason="complete fixture", final=False, expected_exit=0, timeout_seconds=10),
                    dict(id="negative", kind="negative", command=[sys.executable, "-c", "import sys; print('intended refusal'); sys.exit(1)"],
                         inputs=["src"], artifacts=[], closure_reason="complete fixture", final=False, expected_exit=1, timeout_seconds=10)])
        r = q.start(self.worker, self.local, self.base, self.config)
        for check in self.config["checks"]:
            r, accepted = q.run_check(self.worker, self.local, r["revision"], check["id"]); self.assertTrue(accepted)
        r = q.assess(self.worker, self.local, r["revision"], self.base)
        self.review = self.worker / "build/review.json"
        q.atomic_json(self.review, dict(approved=True, candidate=self.tip, reviewer="fixture independent reviewer", config_digest=r["config_digest"]))
        self.archive = self.root / "ready.tar.gz"
        integration.submit(self.worker, self.local, self.base, self.review, self.archive, "one", "complete logical fix", [])
        self.repository = self.root / "integrator"
        self.git(self.root, "clone", str(self.worker), str(self.repository)); kit.configure(self, self.repository, "Fixture integrator")
        received = integration.receive(self.repository, self.archive, self.root / "inbox")
        self.manifest = Path(received["manifest"])
        self.selection = [dict(manifest=str(self.manifest), integrated_commits=[self.tip])]
        self.union = copy.deepcopy(self.config)
        for check in self.union["checks"]:
            check["integration_checks"] = [["one", check["id"]]]
        self.union["checks"].append(dict(id="final_review", kind="review", command=[sys.executable, "-c", "print('fixture final review')"],
            inputs=["src"], artifacts=[], closure_reason="exact fixture candidate", final=True, expected_exit=0, timeout_seconds=10))
        self.final = self.repository / "build/qualification.json"
        self.prospective = self.root / "prospective.json"

    def git(self, repository, *arguments):
        return kit.git(self, repository, *arguments)

    def preview(self):
        return integration.seal(self.repository, self.final, self.base, self.union, self.selection, None, preview=True)

    def approve(self):
        value = self.preview(); value.update(approved=True, reviewer="fixture independent union reviewer")
        q.atomic_json(self.prospective, value)

    def freeze(self):
        self.approve()
        return integration.seal(self.repository, self.final, self.base, self.union, self.selection, self.prospective)

    def run_all(self, record):
        for check in self.union["checks"]:
            record, accepted = q.run_check(self.repository, self.final, record["revision"], check["id"])
            self.assertTrue(accepted)
        return q.assess(self.repository, self.final, record["revision"], self.base)

    def rewrite_archive(self, change):
        packet, data = integration.inspect_archive(self.archive); change(packet, data)
        packet["files"] = {n: q.hashlib.sha256(b).hexdigest() for n, b in data.items()}
        packet["digest"] = q.digest({k: v for k, v in packet.items() if k != "digest"})
        other = self.root / "changed.tar.gz"
        with tarfile.open(other, "w:gz") as t:
            for n, b in {"manifest.json": json.dumps(packet).encode(), **data}.items():
                info=tarfile.TarInfo(n); info.size=len(b); t.addfile(info, io.BytesIO(b))
        return other

    def test_round_trip_frozen_union_uses_ordinary_qualification(self):
        r = self.run_all(self.freeze())
        q.validate(self.repository, self.final, self.tip, self.base)
        self.assertEqual(len(r["runs"]), 3)

    def test_new_received_submission_does_not_change_frozen_selection(self):
        r = self.freeze(); seal = copy.deepcopy(r["integration"])
        integration.receive(self.repository, self.archive, self.root / "another-inbox")
        self.assertEqual(q.load(self.final)["integration"], seal)
        self.run_all(r)

    @unittest.skipIf(os.name == "nt", "POSIX fixture launchers; real Windows producer execution requires its host")
    def test_actual_build_process_records_identity_and_preserves_logs(self):
        launchers = self.root / "launchers"; launchers.mkdir()
        cargo = launchers / "cargo"
        cargo.write_text("#!" + sys.executable + "\nimport os\nfrom pathlib import Path\np=Path(os.environ['CARGO_TARGET_DIR'])/'debug/product'\np.parent.mkdir(parents=True,exist_ok=True)\np.write_bytes(b'fixture output')\nprint('actual fixture build')\n")
        cargo.chmod(0o755)
        rustc = launchers / "rustc"
        rustc.write_text("#!" + sys.executable + "\nprint('fixture toolchain identity')\n")
        rustc.chmod(0o755)
        target = self.root / "target"
        plan = dict(configuration="fixture-dev", command=[str(cargo), "build"], inputs=["src"],
                    outputs=[str(target / "debug/product")], environment=dict(CARGO_TARGET_DIR=str(target), CARGO_PROFILE_DEV_OPT_LEVEL="1"))
        receipt = self.root / "producer.json"
        with mock.patch.dict(os.environ, dict(PATH=str(launchers)+os.pathsep+os.environ['PATH'], CARGO_BUILD_JOBS="3")):
            result = integration.build(self.repository, plan, receipt)
            self.assertEqual(result['exit'], 0)
            self.assertEqual(result['actual_environment']['profiles']['CARGO_BUILD_JOBS'], '3')
            self.assertIn('fixture toolchain', result['toolchain'])
            self.assertEqual(result['launcher']['sha256'], q.file_digest(cargo))
            self.assertEqual(result['artifacts'][plan['outputs'][0]], q.file_digest(plan['outputs'][0]))
            before = receipt.with_suffix('.stdout').read_bytes()
            with self.assertRaisesRegex(q.RecoveryError, 'already exists'):
                integration.build(self.repository, plan, receipt)
            self.assertEqual(receipt.with_suffix('.stdout').read_bytes(), before)

    def test_prospective_review_mutation_invalidates_capture(self):
        r = self.freeze()
        q.atomic_json(self.prospective, dict(approved=False))
        with self.assertRaisesRegex(q.RecoveryError, "prospective integration review changed"):
            q.run_check(self.repository, self.final, r["revision"], "positive")

    def test_rehashed_wrong_patch_identity_refused(self):
        archive = self.rewrite_archive(lambda packet, data: packet.update(patch_ids=['f'*40]))
        with self.assertRaisesRegex(q.RecoveryError, 'original logical patch identity'):
            integration.receive(self.repository, archive, self.root / 'bad-patch-inbox')

    def test_duplicate_outcome_refused(self):
        self.selection.append(copy.deepcopy(self.selection[0]))
        with self.assertRaisesRegex(q.RecoveryError, "duplicate outcome"):
            self.preview()

    def test_review_cannot_discharge_positive_negative_labels(self):
        self.union["checks"] = [self.union["checks"][-1]]
        self.union["checks"][0]["integration_checks"] = [["one", "positive"], ["one", "negative"]]
        with self.assertRaisesRegex(q.RecoveryError, "behavioral controls"):
            self.preview()

    def test_omitted_oracle_dependency_refused(self):
        self.union["checks"][0]["inputs"] = ["file.txt"]
        with self.assertRaisesRegex(q.RecoveryError, "oracle/dependency closure"):
            self.preview()

    def test_review_binds_prepared_bytes(self):
        prepared = self.root / "prepared.lock"; prepared.write_text("ready")
        self.union["checks"][0]["prepared_inputs"] = [str(prepared)]
        self.approve(); prepared.write_text("changed")
        with self.assertRaisesRegex(q.RecoveryError, "prospective union"):
            integration.seal(self.repository, self.final, self.base, self.union, self.selection, self.prospective)

    def test_package_mutation_during_command_cannot_be_accepted(self):
        prepared = self.root / "prepared.lock"; prepared.write_text("ready")
        self.union["checks"][0].update(prepared_inputs=[str(prepared)], command=[sys.executable,"-c",f"from pathlib import Path; Path({str(prepared)!r}).write_text('changed')"])
        r = self.freeze(); r, accepted = q.run_check(self.repository, self.final, r["revision"], "positive")
        self.assertFalse(accepted)
        self.assertIn("prepared_inputs_changed_during_run", r["runs"][-1]["invalidations"])
        with self.assertRaisesRegex(q.RecoveryError, "prepared package"):
            q.run_check(self.repository, self.final, r["revision"], "negative")

    def test_payload_mutation_after_receive_refused(self):
        packet = q.read_json(self.manifest); path = self.manifest.parent / next(n for n in packet["files"] if n.startswith("payload/"))
        path.write_bytes(b"replaced")
        with self.assertRaisesRegex(q.RecoveryError, "received evidence changed"):
            self.preview()

    def test_rehashed_manifest_cannot_omit_original_worker_checks(self):
        packet = q.read_json(self.manifest); packet["config"]["checks"].clear(); packet["digest"] = q.digest({k:v for k,v in packet.items() if k!='digest'})
        q.atomic_json(self.manifest,packet)
        with self.assertRaisesRegex(q.RecoveryError, "receive receipt"):
            self.preview()

    def test_rehashed_archive_config_is_checked_against_original_state(self):
        other = self.rewrite_archive(lambda p,d: p['config']['checks'].clear())
        with self.assertRaisesRegex(q.RecoveryError, "qualification definitions"):
            integration.receive(self.repository,other,self.root/'bad-inbox')
        self.assertFalse(any((self.root/'bad-inbox').rglob('received.json')))

    def test_failed_batch_preserves_successful_observation(self):
        self.union['checks'][1]['command']=[sys.executable,'-c','import sys; sys.exit(2)']
        r=self.freeze();r,accepted=q.run_check(self.repository,self.final,r['revision'],'positive');self.assertTrue(accepted)
        r,accepted=q.run_check(self.repository,self.final,r['revision'],'negative');self.assertFalse(accepted)
        self.assertEqual([x['exit'] for x in r['runs']],[0,2])
        r=q.assess(self.repository,self.final,r['revision'],self.base)
        self.assertEqual([x['action'] for x in r['assessments'][-1]['checks']],['reuse','rerun','rerun'])

    def test_external_main_change_requires_explicit_reseal_and_final_review(self):
        r=self.run_all(self.freeze());(self.repository/'README.md').write_text('external writer\n')
        self.git(self.repository,'add','README.md');self.git(self.repository,'commit','-m','external writer')
        tip=self.git(self.repository,'rev-parse','HEAD')
        r=q.assess(self.repository,self.final,r['revision'],tip)
        with self.assertRaisesRegex(q.RecoveryError,'candidate/config moved'):
            q.validate(self.repository,self.final,tip,tip)
        value=integration.seal(self.repository,self.final,tip,self.union,self.selection,None,preview=True);value.update(approved=True,reviewer='fixture reviewer');q.atomic_json(self.prospective,value)
        r=integration.seal(self.repository,self.final,tip,self.union,self.selection,self.prospective,r['revision'])
        self.assertEqual(len(r['integration_history']),1)
        r,accepted=q.run_check(self.repository,self.final,r['revision'],'final_review');self.assertTrue(accepted)
        r=q.assess(self.repository,self.final,r['revision'],tip);q.validate(self.repository,self.final,tip,tip)
        self.assertEqual(len(r['runs']),4)

    def test_moved_source_invalidates_affected_check(self):
        r=self.run_all(self.freeze());(self.repository/'src/program').write_text('changed\n')
        self.git(self.repository,'add','src');self.git(self.repository,'commit','-m','external source change');tip=self.git(self.repository,'rev-parse','HEAD')
        r=q.assess(self.repository,self.final,r['revision'],tip)
        self.assertTrue(all(x['action']=='rerun' for x in r['assessments'][-1]['checks']))

    def test_stale_revision_refused(self):
        r=self.freeze()
        with self.assertRaisesRegex(q.RecoveryError,'stale qualification revision'):
            q.run_check(self.repository,self.final,r['revision']-1,'positive')

    def test_archive_links_refused(self):
        path=self.root/'link.tar.gz'
        with tarfile.open(path,'w:gz') as t:
            x=tarfile.TarInfo('manifest.json');x.type=tarfile.SYMTYPE;x.linkname='/etc/passwd';t.addfile(x)
        with self.assertRaisesRegex(q.RecoveryError,'nonregular'):
            integration.inspect_archive(path)

    def test_duplicate_profile_or_storage_build_overrides_refused(self):
        for flag in ('--release','--profile=release','--target-dir=peer','--config=peer','--manifest-path=peer'):
            with self.subTest(flag=flag),self.assertRaisesRegex(q.RecoveryError,'profile/storage/config'):
                integration.build(self.repository,dict(command=['mbx','build',flag],environment={}),self.root/'receipt.json')


if __name__ == '__main__':
    unittest.main()
