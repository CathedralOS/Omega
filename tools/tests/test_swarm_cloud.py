#!/usr/bin/env python3
"""Deterministic cloud adapter faults; no cloud sessions or remote publication."""

import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/swarm'))
import cloud
import completion
from recovery_state import Store, RecoveryError, atomic_json


def requirements():
    rows = [dict(id=kind, kind=kind, command='fixture ' + kind, inputs=['src'], exit=0,
                 verdict='rejected' if kind == 'negative' else 'passed')
            for kind in ('native', 'interpreter', 'tests', 'negative')]
    for row in rows:
        if row['kind'] in {'native','interpreter'}: row['expected_result'] = dict(exit=0,stdout_sha256=hashlib.sha256(b'').hexdigest())
    return rows


def assignment(name, base='a' * 40):
    return dict(id=name, base=base, item='item-'+name, owner='owner-' + name, outcome='program-' + name,
                writes=['src/' + name + '.py'], reads=[], retry_budget=1, max_continuations=6,
                dependency_review='producer/consumer, digest and golden closure reviewed', acceptance=requirements())


def manifest(size=2, prefix='', previous=None):
    rows = [assignment(prefix + str(i)) for i in range(size)]
    rows[1]['reads'] = rows[0]['writes'][:]
    return dict(id='batch-' + prefix + str(size), size=size, previous=previous,
                dependency_review='complete shared-path review', publication='serialized-landing', assignments=rows,
                dependencies=[dict(id='digest-' + prefix, producer=rows[0]['id'], consumer=rows[1]['id'],
                                   paths=rows[0]['writes'][:], kind='digest', verification='recompute after producer integration')])


def snapshot():
    return dict(observed_utc=cloud.now(), evidence='actual read-only registry result', state='claimed', claims=[])


def checkpoint(phase='preflight'):
    stamp = cloud.now()
    return dict(phase=phase, observed_utc=stamp, started_utc=stamp, progress_utc=stamp,
                active_command=None, session=None, evidence='checkpoint artifact or task result', next_gate='focused checks',
                reason='exact shared dependency or approval reason')


def preflight_receipt(assignment='0', base='a' * 40, thread='thread-0', environment='env-0', generation='gen-0'):
    return dict(assignment=assignment, base=base, thread=thread, environment=environment, generation=generation,
                repository='CathedralOS/Omega', source_revision=base, toolchain='pinned nightly',
                execution_route='exposed approved host route', evidence='recorded preflight results', doctor_exit=0,
                identity=dict(name='Jarod Castillo',email='43973221+Smotto@users.noreply.github.com',scope='repository-local'),
                resources=dict(cpus=4,memory_bytes=1024,disk_bytes=1024,build_jobs=4),
                smoke=dict(native_exit=0,interpreter_exit=0,command='omega run --both fixture',preparation='omega update fixture',evidence='smoke log'),
                backup=dict(status='confirmed-upload',file_id='fixture-file',version=0,route='fixture-approved-artifact-route',sha256='a'*64,bytes=1))


class ControllerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.store = Store(self.root / 'state.json'); self.store.initialize('parent-executor', 'instance-1')
        self.update(lambda r: cloud.initialize(r, dict(evidence='inspected real runtime schemas',
                 tools=dict(create='exposed.create', read='exposed.read', follow_up='exposed.follow_up'),
                 reporting=dict(mode='bounded-turn-results', max_turn_seconds=900, evidence='read exposes recorded task results'))))
        self.update(lambda r: cloud.admit(r, manifest(), snapshot()))
        self.update(lambda r: cloud.checkpoint(r, '0', checkpoint()))

    def update(self, change):
        return self.store.update(self.store.load()['revision'], change)

    def request(self, action='create', name='dispatch-1', assignment='0'):
        result = []
        self.update(lambda r: result.append(cloud.intent(r, assignment, action, name,
                                           dict(model='gpt-6.1-sol', reasoning='medium', phase='preflight' if action=='create' else 'implementation'))))
        return result[0]

    def reply(self, request, effect='applied', result=None):
        return dict(schema=1, operation=request['operation'], execution_id=request['execution_id'],
                    assignment=request['assignment'], effect=effect, evidence='actual tool result',
                    observed_utc=cloud.now(), revision=self.store.load()['revision'],
                    result=result or dict(thread='thread-0', environment='env-0', generation='gen-0'))

    def ingest(self, value):
        self.update(lambda r: cloud.ingest(r, value))

    def read(self, effects=None, runtime='completed', acknowledge=True, include_preflight=True, **extra):
        if self.state()['thread'] and include_preflight:
            value = snapshot(); value['ticket'] = 'ticket-0'
            value['claims'] = [dict(ticket='ticket-0',item='item-0',owner='owner-0',paths=['src/0.py'])]
            self.update(lambda r:cloud.ownership(r,'0',value))
            extra.setdefault('preflight',preflight_receipt())
        if acknowledge:
            extra['acknowledgments'] = [dict(operation=name,execution_id=operation['request']['execution_id'],status='completed',evidence='actual child request acknowledgment')
                for name,operation in self.store.load()['cloud']['operations'].items()
                if operation['assignment']=='0' and operation['request']['action'] in {'create','follow_up'}
                and (operation['effect']=='applied' or (effects or {}).get(name,{}).get('effect')=='applied')]
        request = self.request('read', 'read-' + str(self.store.load()['revision']))
        reply = self.reply(request, result=dict(thread='thread-0', environment='env-0', generation='gen-0',
                          runtime=runtime, processes_stopped=True, recovery=dict(assignment='0',environment='env-0',generation='gen-0',unknown_operations=[],build_slot_clear=True,evidence='fresh executor recovery read'), effects=effects or {}, **extra))
        self.ingest(reply)
        return reply

    def state(self):
        return self.store.load()['cloud']['assignments']['0']

    def test_interrupted_dispatch_loss_never_creates_twice(self):
        request = self.request()
        with self.assertRaisesRegex(RecoveryError, 'unknown'):
            self.request(name='duplicate')
        self.assertEqual(cloud.plan(self.store.load())['assignments'][0]['action'], 'read_reconcile')
        self.read({'dispatch-1':dict(execution_id=request['execution_id'], effect='applied', evidence='exact runtime request found',
                   thread='thread-0', environment='env-0', generation='gen-0')}, runtime='running')
        self.assertEqual(self.state()['thread'], 'thread-0')
        with self.assertRaisesRegex(RecoveryError, 'existing thread'):
            self.request(name='another')
        self.read(runtime='running')
        with self.assertRaisesRegex(RecoveryError, 'running'):
            self.request('follow_up', 'continue')

    def test_result_saved_before_failed_merge_can_be_ingested_without_dispatch(self):
        request = self.request(); reply = self.reply(request)
        path = self.root / 'returned.json'; atomic_json(path, reply)
        with self.assertRaises(RecoveryError):
            self.store.update(0, lambda r: cloud.ingest(r, reply))
        self.ingest(json.loads(path.read_text()))
        revision = self.store.load()['revision']; self.ingest(reply)
        self.assertEqual(self.store.load()['revision'], revision)
        changed = copy.deepcopy(reply); changed['result']['thread'] = 'wrong'
        with self.assertRaisesRegex(RecoveryError, 'duplicate'):
            self.ingest(changed)

    def test_unknown_mutation_effect_and_not_applied_retry_budget(self):
        first = self.request(); self.ingest(self.reply(first, 'unknown'))
        self.read(runtime='unknown')
        with self.assertRaisesRegex(RecoveryError, 'unknown'):
            self.request(name='retry')
        self.read({first['operation']:dict(execution_id=first['execution_id'], effect='not_applied', evidence='authoritative exact request absent')})
        second = self.request(name='retry-2'); self.ingest(self.reply(second, 'not_applied'))
        with self.assertRaisesRegex(RecoveryError, 'budget'):
            self.request(name='retry-3')

    def test_stale_runtime_observation_rejected(self):
        request = self.request(); self.ingest(self.reply(request))
        read = self.request('read', 'inspect'); reply = self.reply(read, result=dict(runtime='completed',thread='thread-0',environment='env-0',generation='gen-0'))
        for field, value in [('revision', 0), ('observed_utc', (datetime.now(timezone.utc) - timedelta(seconds=121)).isoformat())]:
            bad = dict(reply, **{field:value})
            with self.assertRaises(RecoveryError): self.ingest(bad)
        reply['result']['generation'] = 'other'
        with self.assertRaisesRegex(RecoveryError, 'mapping'): self.ingest(reply)

    def test_completed_child_with_dependency_checkpoint_is_waiting(self):
        request = self.request(); self.ingest(self.reply(request))
        report = dict(assignment='0', thread='thread-0', environment='env-0', sequence=1,
                      evidence='cloud read recorded result', checkpoint=checkpoint('waiting_dependency'))
        self.read(report=report)
        self.assertEqual(cloud.plan(self.store.load())['assignments'][0]['action'], 'waiting_dependency')
        self.assertIsNone(self.state()['accepted'])
        with self.assertRaisesRegex(RecoveryError, 'stale'):
            self.read(report=report)

    def test_actual_approval_denial_stops_retry(self):
        request = self.request(); reply = self.reply(request, 'not_applied', dict(action='execute',error='automatic approval rejected'))
        reply['approval_denied'] = True; self.ingest(reply)
        self.assertEqual(cloud.plan(self.store.load())['assignments'][0]['action'], 'required_approval')
        with self.assertRaisesRegex(RecoveryError, 'approval'): self.request(name='bypass')
        self.update(lambda r:cloud.checkpoint(r,'0',checkpoint()))
        with self.assertRaisesRegex(RecoveryError,'approval'):self.request(name='checkpoint-bypass')
        receipt=dict(operation=request['operation'],execution_id=request['execution_id'],action='execute',status='approved',evidence='actual approval result',route='exposed approval route',observed_utc=cloud.now())
        self.update(lambda r:cloud.approval(r,'0',receipt))
        self.request(name='authorized-retry')

    def test_unsupported_lifecycle_not_invented(self):
        with self.assertRaisesRegex(RecoveryError, 'expose'): self.request('lifecycle')

    def test_dependency_requires_bilateral_exact_claim_handoff(self):
        self.update(lambda r: cloud.checkpoint(r, '1', checkpoint('waiting_dependency')))
        with self.assertRaisesRegex(RecoveryError, 'dependency'): self.request('follow_up',assignment='1')
        for name in ('0','1'):
            value = snapshot(); value['ticket'] = 'ticket-' + name
            value['claims'] = [dict(ticket=value['ticket'], item='item-'+name, owner='owner-' + name, paths=['src/' + name + '.py'])]
            self.update(lambda r, n=name, v=value: cloud.ownership(r, n, v))
        handoff = dict(dependency='digest-', producer='0', consumer='1', candidate='b' * 40, paths=['src/0.py'],
                       verification='recompute after producer integration', producer_ticket='ticket-0', consumer_ticket='ticket-1',
                       producer_receipt='owner release message', consumer_receipt='integration acknowledgment',
                       verification_evidence='candidate checks', claim_evidence='single writer registry', ownership='single-writer', observed_utc=cloud.now())
        for field in ('producer_receipt','consumer_receipt','claim_evidence','producer_ticket'):
            bad = dict(handoff, **{field:None})
            with self.assertRaises(RecoveryError): self.update(lambda r: cloud.handoff(r, bad))
        self.update(lambda r: cloud.handoff(r, handoff))
        self.update(lambda r: cloud.checkpoint(r, '1', checkpoint()))
        self.request(assignment='1')

    def test_live_global_claim_rejected_without_weakening_fence(self):
        m = manifest(prefix='next'); s = snapshot()
        s['claims'] = [dict(ticket='peer', owner='peer', paths=['src'])]
        with self.assertRaisesRegex(RecoveryError, 'live claim'): self.update(lambda r: cloud.admit(r,m,s))

    def test_existing_global_item_claim_is_a_fence_even_with_disjoint_paths(self):
        m=manifest(prefix='next');snapshot_value=snapshot()
        snapshot_value['claims']=[dict(ticket='peer',item='item-next0',board=None,owner='peer',paths=['unrelated/file.py'])]
        with self.assertRaisesRegex(RecoveryError,'live claim'):self.update(lambda r:cloud.admit(r,m,snapshot_value))

    def test_independent_canary_needs_no_invented_dependency(self):
        value = manifest(prefix='independent')
        value['assignments'][1]['reads'] = []
        value['dependencies'] = []
        record = self.store.load()
        record['cloud']['batches'].clear()
        record['cloud']['assignments'].clear()
        cloud.admit(record, value, snapshot())
        self.assertEqual(record['cloud']['batches'][value['id']]['manifest']['dependencies'], [])

    def test_canary_accepts_a_real_dependency_without_requiring_a_digest(self):
        value = manifest(prefix='shared')
        value['dependencies'][0]['kind'] = 'producer-consumer'
        record = self.store.load()
        record['cloud']['batches'].clear()
        record['cloud']['assignments'].clear()
        cloud.admit(record, value, snapshot())
        self.assertEqual(record['cloud']['batches'][value['id']]['manifest']['dependencies'][0]['kind'],
                         'producer-consumer')

    def test_unsafe_overlap_undeclared_dependency_cycle(self):
        for mutate, reason in [
            (lambda m:m['assignments'][1].update(writes=m['assignments'][0]['writes']), 'unsafe overlap'),
            (lambda m:m.update(dependencies=[]), 'undeclared'),
            (lambda m:m['assignments'][1].update(outcome=m['assignments'][0]['outcome']), 'disjoint')]:
            m = manifest(prefix='new'); mutate(m)
            with self.assertRaisesRegex(RecoveryError, reason): self.update(lambda r: cloud.admit(r,m,snapshot()))
        m = manifest(prefix='cycle'); m['assignments'][0]['reads'] = m['assignments'][1]['writes']
        m['dependencies'].append(dict(id='reverse', producer='cycle1',consumer='cycle0', paths=['src/cycle1.py'],kind='shared',verification='check'))
        with self.assertRaisesRegex(RecoveryError, 'cycle'): self.update(lambda r:cloud.admit(r,m,snapshot()))

    def test_batch_admission_requires_previous_complete_stage(self):
        for size in (5,10):
            m = manifest(size, 'stage'+str(size), 'batch-2')
            with self.assertRaises(RecoveryError): self.update(lambda r:cloud.admit(r,m,snapshot()))
        # Only verified acceptance normally writes this field; mock the verifier's
        # terminal contract to isolate deterministic admission rules.
        self.update(lambda r:[row.update(accepted=dict(result='landed',cleanup='completed',user_rescue=False))
                              for row in r['cloud']['assignments'].values()])
        m = manifest(5,'five','batch-2'); self.update(lambda r:cloud.admit(r,m,snapshot()))
        self.assertEqual(len(self.store.load()['cloud']['assignments']),7)
        with self.assertRaises(RecoveryError):self.update(lambda r:cloud.admit(r,manifest(10,'ten',m['id']),snapshot()))

    def test_successful_report_turns_do_not_consume_retry_budget(self):
        request=self.request();self.ingest(self.reply(request))
        for index in range(6):
            self.read()
            followup=self.request('follow_up','leg-'+str(index));self.ingest(self.reply(followup))
        self.read()
        with self.assertRaisesRegex(RecoveryError,'continuation'):self.request('follow_up','overflow')

    def test_dependency_wait_allows_only_bounded_preflight_create(self):
        self.update(lambda r:cloud.checkpoint(r,'1',checkpoint('waiting_dependency')))
        request=self.request(assignment='1')
        self.assertEqual(request['parameters']['phase'],'preflight')
        self.ingest(self.reply(request))
        with self.assertRaisesRegex(RecoveryError,'dependency'):self.request('follow_up','implement',assignment='1')

    def test_dependency_ids_cannot_reuse_old_handoff_in_new_batch(self):
        m=manifest(prefix='next');m['dependencies'][0]['id']='digest-'
        with self.assertRaisesRegex(RecoveryError,'duplicate dependency'):self.update(lambda r:cloud.admit(r,m,snapshot()))

    def test_unfinished_canary_cannot_be_hidden_by_another_batch(self):
        with self.assertRaisesRegex(RecoveryError,'unfinished batch'):
            self.update(lambda r:cloud.admit(r,manifest(prefix='parallel'),snapshot()))

    def test_preflight_and_fresh_claim_receipts_gate_implementation(self):
        request=self.request();self.ingest(self.reply(request))
        self.read(include_preflight=False)
        with self.assertRaisesRegex(RecoveryError,'preflight'):self.request('follow_up','unprepared')
        receipt=preflight_receipt();bad=copy.deepcopy(receipt);bad['smoke']['native_exit']=1
        with self.assertRaisesRegex(RecoveryError,'smoke'):self.update(lambda r:cloud.preflight(r,'0',bad))
        self.update(lambda r:cloud.preflight(r,'0',receipt))
        with self.assertRaisesRegex(RecoveryError,'claims'):self.request('follow_up','unclaimed')
        self.read();self.request('follow_up','prepared')

    def test_message_enqueue_is_not_child_acknowledgment(self):
        request=self.request();self.ingest(self.reply(request));self.read()
        continuation=self.request('follow_up','send-1');self.ingest(self.reply(continuation))
        self.read(acknowledge=False)
        with self.assertRaisesRegex(RecoveryError,'acknowledgment'):self.request('follow_up','send-2')
        self.assertEqual(cloud.plan(self.store.load())['assignments'][0]['action'],'read_reconcile')
        self.read();self.request('follow_up','send-2')

    def test_recovery_unknown_mutations_and_build_slot_also_gate_continuation(self):
        request=self.request();self.ingest(self.reply(request));self.read()
        for field,value in [('unknown_operations',['push']),('build_slot_clear',False)]:
            self.update(lambda r:r['cloud']['assignments']['0']['observation']['recovery'].update({field:value}))
            with self.assertRaisesRegex(RecoveryError,'recovery'):self.request('follow_up','unsafe-'+field)
            self.read()
        self.update(lambda r:r.update(lanes={'same':dict(id='same',assignment='0',operations={'claim':dict(outcome='unknown')})}))
        with self.assertRaisesRegex(RecoveryError,'local recovery'):self.request('follow_up','unsafe-local')

    def test_unavailable_read_preserves_work_and_can_be_read_again(self):
        request=self.request();self.ingest(self.reply(request))
        read=self.request('read','unavailable');self.ingest(self.reply(read,'unknown',dict(error='disconnected')))
        self.assertIsNone(self.state()['observation'])
        self.read();self.request('follow_up','healthy-again')

    def test_preflight_rejects_incomplete_and_wrong_executor_receipts(self):
        request=self.request();self.ingest(self.reply(request))
        good=preflight_receipt()
        for field,value in [('base','b'*40),('generation','other'),('doctor_exit',1),('backup',dict(status='local-only')),('identity',dict(name='Wrong'))]:
            bad=copy.deepcopy(good);bad[field]=value
            with self.assertRaises(RecoveryError):self.update(lambda r:cloud.preflight(r,'0',bad))
        bad=copy.deepcopy(good);bad['resources']['build_jobs']=8
        with self.assertRaisesRegex(RecoveryError,'CPU'):self.update(lambda r:cloud.preflight(r,'0',bad))

    def test_dispatch_carries_exact_board_and_existing_ticket(self):
        self.update(lambda r:r['cloud']['assignments']['0'].update(board='TASKS.md',claim_ticket='exact-existing-ticket'))
        request=self.request()
        self.assertEqual(request['contract']['board'],'TASKS.md')
        self.assertEqual(request['contract']['item'],'item-0')
        self.assertEqual(request['contract']['claim_ticket'],'exact-existing-ticket')

    def test_permission_bypass_and_credentials_not_adapter_parameters(self):
        for field in ('skip_permissions','bypass_approval','api_key'):
            with self.assertRaisesRegex(RecoveryError,'normalized'):
                self.update(lambda r:cloud.intent(r,'0','create','forbidden-'+field,dict(model='gpt-6.1-sol',reasoning='medium',phase='preflight',**{field:'forbidden'})))

    def test_failed_preflight_can_continue_preflight_without_implementation_authority(self):
        request=self.request();self.ingest(self.reply(request));self.read(include_preflight=False)
        self.update(lambda r:cloud.intent(r,'0','follow_up','retry-preflight',dict(phase='preflight',message='repair bounded preflight only')))
        with self.assertRaisesRegex(RecoveryError,'unknown'):self.request('follow_up','premature-implementation')

    def test_controller_cli_emits_request_after_durable_intent(self):
        value = self.root / 'intent.json'; atomic_json(value, dict(assignment='0',operation='cli-create',action='create',parameters=dict(model='gpt-6.1-sol',reasoning='medium',phase='preflight')))
        args = [sys.executable,str(ROOT/'tools/swarm/cloud.py'),'--state',str(self.store.path),'--revision',str(self.store.load()['revision']),'intent','--input',str(value)]
        result = subprocess.run(args, capture_output=True, text=True)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)
        request = json.loads(result.stdout)['request']
        self.assertEqual(self.store.load()['cloud']['operations']['cli-create']['request'],request)


class AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name); self.repo=self.root/'repo'; self.repo.mkdir()
        self.git('init'); self.git('config','user.name','Fixture'); self.git('config','user.email','fixture@example.invalid')
        (self.repo/'src').mkdir(); (self.repo/'src/program').write_text('before')
        self.git('add','.'); self.git('commit','-m','tools: before'); self.base=self.git('rev-parse','HEAD')
        (self.repo/'src/program').write_text('after'); self.git('commit','-am','tools: after'); self.candidate=self.git('rev-parse','HEAD')
        self.git('update-ref','refs/remotes/origin/main',self.candidate)
        self.assignment=assignment('program', self.base)
        self.receipt=dict(schema='omega-landing-receipt-v1',candidate=self.candidate,base=self.base,exit_code=0,packages=[])
        self.manifest=dict(schema=1,assignment='program',candidate=self.candidate,base=self.base,outcome='program-program',
             remote_main=self.candidate,user_rescue=False,environment='host/toolchain/flags/input-hash',final_receipt=self.receipt,
             receipt_artifact=self.artifact('receipt.json',json.dumps(self.receipt)),historical=[],
             before=dict(revision=self.base,outcome='program-program',kind='native',failure_kind='compile',environment='host/toolchain/flags/input-hash',verdict='failed',exit=1,command='fixture native',artifact=self.artifact('before.log','failed')),
             final_evidence=[dict(r,program_result=r.get('expected_result'),revision=self.candidate,environment='host/toolchain/flags/input-hash',count=2,artifact=self.artifact(r['id']+'.log','real recorded result')) for r in requirements()])
        self.observation=dict(workspace=dict(head=self.candidate,clean=True,primary=False,owned=True,evidence='Git inventory before removal'),landed_head=self.candidate,main=self.candidate,remote_evidence='fresh push-destination fetch',claims=[],landing_tickets=[],registry_evidence='fresh claims and landing reads',processes_stopped=True,cleanup='completed',cleanup_evidence='owned clean worktree removed')

    def git(self,*arguments):
        p=subprocess.run(['git','-C',str(self.repo),*arguments],capture_output=True,text=True)
        self.assertEqual(p.returncode,0,p.stderr);return p.stdout.strip()

    def artifact(self,name,text):
        (self.root/name).write_text(text)
        return dict(path=name,sha256=hashlib.sha256(text.encode()).hexdigest())

    def validate(self): return completion.validate(self.repo,self.root,self.assignment,self.manifest,self.observation)


    def controller_record(self, phase='landed'):
        row=dict(self.assignment,thread='thread',environment='env',generation='generation',checkpoint=checkpoint(phase),preflight=preflight_receipt('program',self.base,'thread','env','generation'),accepted=None,batch='batch')
        return dict(revision=7,cloud=dict(assignments={'program':row},operations={},batches={'batch':dict(manifest=dict(dependencies=[]))},handoffs={}))

    def final_observation(self):
        return dict(self.observation,revision=7,observed_utc=cloud.now(),thread='thread',environment='env',generation='generation',runtime='completed',recovery=dict(assignment='program',environment='env',generation='generation',unknown_operations=[],build_slot_clear=True,evidence='fresh child recovery journal'))

    def test_blocked_completed_child_cannot_be_accepted(self):
        for phase in ('blocked','waiting_dependency','candidate_ready','required_approval'):
            with self.assertRaisesRegex(RecoveryError,'checkpoint'):
                cloud.accept(self.controller_record(phase),'program',self.manifest,self.final_observation(),self.repo,self.root)

    def test_duplicate_acceptance_idempotent_wrong_assignment_and_uncertain_effect_rejected(self):
        record=self.controller_record();observed=self.final_observation()
        cloud.accept(record,'program',self.manifest,observed,self.repo,self.root)
        accepted=copy.deepcopy(record['cloud']['assignments']['program']['accepted'])
        cloud.accept(record,'program',self.manifest,observed,self.repo,self.root)
        self.assertEqual(record['cloud']['assignments']['program']['accepted'],accepted)
        observed['generation']='wrong'
        with self.assertRaisesRegex(RecoveryError,'mapping'):cloud.accept(record,'program',self.manifest,observed,self.repo,self.root)
        record=self.controller_record();record['cloud']['operations']['publish'] = dict(assignment='program',effect='unknown',request=dict(action='follow_up'))
        with self.assertRaisesRegex(RecoveryError,'mutation'):cloud.accept(record,'program',self.manifest,self.final_observation(),self.repo,self.root)

    def test_cleanup_review_precedes_removal_and_never_accepts_partial_inventory(self):
        record=self.controller_record();observed=self.final_observation();observed['cleanup']='pending'
        cloud.accept(record,'program',self.manifest,observed,self.repo,self.root,cleanup_review=True)
        self.assertIsNone(record['cloud']['assignments']['program']['accepted'])
        self.assertEqual(record['cloud']['assignments']['program']['cleanup_review']['candidate'],self.candidate)
        for field,value in [('clean',False),('primary',True),('owned',False),('head',self.base)]:
            bad=copy.deepcopy(observed);bad['workspace'][field]=value
            with self.assertRaises(RecoveryError):cloud.accept(record,'program',self.manifest,bad,self.repo,self.root,cleanup_review=True)

    def test_child_and_matching_local_recovery_uncertainty_block_acceptance(self):
        for field,value in [('unknown_operations',['publish']),('build_slot_clear',False)]:
            observed=self.final_observation();observed['recovery'][field]=value
            with self.assertRaisesRegex(RecoveryError,'recovery'):cloud.accept(self.controller_record(),'program',self.manifest,observed,self.repo,self.root)
        record=self.controller_record();record['lanes']={'lane':dict(id='lane',assignment='program',operations={'push':dict(outcome='unknown')})}
        with self.assertRaisesRegex(RecoveryError,'local recovery'):cloud.accept(record,'program',self.manifest,self.final_observation(),self.repo,self.root)
        record['lanes']['lane']['operations']['push']['outcome']='applied';record['build_slot']=dict(lane='lane')
        with self.assertRaisesRegex(RecoveryError,'build slot'):cloud.accept(record,'program',self.manifest,self.final_observation(),self.repo,self.root)

    def test_exact_landed_contract_passes(self): self.assertEqual(self.validate()['candidate'],self.candidate)

    def test_wrong_final_sha_rejected(self):
        self.manifest['final_receipt']['candidate']=self.base
        with self.assertRaisesRegex(RecoveryError,'exact final'):self.validate()

    def test_historical_cannot_fill_final_manifest_without_explicit_reuse(self):
        self.manifest['historical']=self.manifest['final_evidence'];self.manifest['final_evidence']=[]
        with self.assertRaisesRegex(RecoveryError,'missing'):self.validate()

    def test_changed_inputs_reuse_rejected_and_unchanged_reuse_accepted(self):
        row=self.manifest['final_evidence'][0]; history=copy.deepcopy(row);history['revision']=self.base
        self.manifest['historical']=[history];row['reuse']=dict(historical_id=history['id'],reason='unchanged closure')
        with self.assertRaisesRegex(RecoveryError,'inputs changed'):self.validate()
        history['revision']=self.candidate
        (self.repo/'unrelated').write_text('board only');self.git('add','.');self.git('commit','-m','docs: unrelated')
        final=self.git('rev-parse','HEAD');self.git('update-ref','refs/remotes/origin/main',final)
        self.manifest.update(candidate=final,remote_main=final);self.observation.update(landed_head=final,main=final);self.observation['workspace']['head']=final
        self.receipt['candidate']=final;self.manifest['receipt_artifact']=self.artifact('receipt.json',json.dumps(self.receipt))
        for value in self.manifest['final_evidence']: value['revision']=final
        self.assertEqual(self.validate()['candidate'],final)
        history['environment']='other'
        with self.assertRaisesRegex(RecoveryError,'host/toolchain'):self.validate()

    def test_safe_cleanup_and_registry_require_full_inventory(self):
        for field,value in [('claims',['live']),('landing_tickets',['uncertain']),('processes_stopped',False),('cleanup','pending'),('cleanup_evidence',''),('remote_evidence','')]:
            original=self.observation[field];self.observation[field]=value
            with self.assertRaises(RecoveryError):self.validate()
            self.observation[field]=original

    def test_wrong_authored_program_result_rejected(self):
        self.manifest['final_evidence'][0]['program_result']=dict(exit=1)
        with self.assertRaisesRegex(RecoveryError,'oracle'):self.validate()

    def test_empty_tests_bad_program_negative_and_tampered_artifact(self):
        for index,field,value in [(0,'verdict','failed'),(1,'exit',1),(2,'count',0),(3,'verdict','accepted')]:
            row=self.manifest['final_evidence'][index];original=row[field];row[field]=value
            with self.assertRaises(RecoveryError):self.validate()
            row[field]=original
        (self.root/'native.log').write_text('tampered')
        with self.assertRaisesRegex(RecoveryError,'digest'):self.validate()

    def test_user_rescue_prevents_scale(self):
        self.manifest['user_rescue']=True
        with self.assertRaisesRegex(RecoveryError,'rescue'):self.validate()

    def test_fetched_ancestry_not_client_success(self):
        self.git('update-ref','refs/remotes/origin/main',self.base)
        with self.assertRaisesRegex(RecoveryError,'fetched'):self.validate()


class DocumentationTests(unittest.TestCase):
    def test_new_document_links_resolve_and_cli_surface_matches_contract(self):
        for path in (ROOT/'tools/swarm/cloud.md', ROOT/'.agents/skills/dot-swarm/references/cloud-coordination.md'):
            for target in re.findall(r'\]\(([^)]+)\)',path.read_text()):
                if '://' not in target and not target.startswith('#'):
                    self.assertTrue((path.parent/target.split('#')[0]).exists(),(path,target))
        result=subprocess.run([sys.executable,str(ROOT/'tools/swarm/cloud.py'),'--help'],capture_output=True,text=True,timeout=5)
        self.assertEqual(result.returncode,0,result.stderr)
        for command in ('status','init','admit','handoff','checkpoint','ownership','preflight','approval','intent','ingest','cleanup-review','accept'):
            self.assertIn(command,result.stdout)
        self.assertIn('no routine parent sign-off', (ROOT/'tools/swarm/cloud.md').read_text())


if __name__=='__main__': unittest.main()
