import hashlib,json,subprocess,sys
from pathlib import Path
folder=Path('build/saved-case-calls');cli=folder/'producer-4/omega';runner=folder/'producer-4/corpus_runner'
identity=json.loads((folder/'producer-4.json').read_text())
for name,digest in identity['executables'].items():assert hashlib.sha256((folder/'producer-4'/name).read_bytes()).hexdigest()==digest
retained=json.loads((folder/'corpus-both-4.json').read_text());assert retained['exit_code']==0 and len(retained['observations'])==49
for row in retained['observations']:assert row['status']=='built' and row['facts']['exit']==row['facts']['interpreter-exit']=='0' and row['facts']['interpreter']=='agrees',row
filters='facts/saved_case_predicate_,facts/saved_observation_snapshot_,facts/saved_byte_predicate_,facts/case_payload_stored_loan_,facts/descriptor_guard_replacement_,control_flow/saved_case_predicate_,borrows/guard_subject,borrows/guard_observation,borrows/runtime_reference_projection_evaluation_exit,copied_byte_,copied_argument_ensures_cannot_restore,temporary_backing_loan_survives_reference_last_use'
command=[sys.executable,'tools/corpus_gate.py','--runner',str(runner),'--filter',filters,'--json',str(folder/'final-source.json')]
r=subprocess.run(command,capture_output=True,text=True);(folder/'final-source.stdout').write_text(r.stdout);(folder/'final-source.stderr').write_text(r.stderr);assert r.returncode in [0,1],r.stdout+r.stderr
source=json.loads((folder/'final-source.json').read_text());assert not source['oracle_failures']
assert source['counts']['wrong_reason']==source['counts']['fail_not_rejected']==source['counts']['lost_successes']==0
for row in source['observations']:
 assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
 if row['tier']=='fail':assert row['expected_satisfied'] is True,row
subprocess.run([sys.executable,str(folder/'run-row.py'),str(cli),'final','both'],check=True)
rows=json.loads((folder/'row-final.json').read_text());assert len(rows)==20
for row in rows:
 p=Path(row['command'][-1]);assert hashlib.sha256(p.read_bytes()).hexdigest()==row['source_sha256']
 if row['invalid']:assert row['exit']==1 and 'cannot prove selected case for payload access' in row['stderr'],row
 else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 tier='fail' if row['invalid'] else 'pass';suffix='rejected' if row['invalid'] else 'exit'
 if row['place']=='writes_before_read':pin=Path('tests/omega')/tier/'control_flow'/f'saved_case_predicate_receiver_writes_before_read_{suffix}'
 else:pin=Path('tests/omega')/tier/'facts'/f"saved_case_predicate_through_{row['place']}_{suffix}"
 for name in ['main.omg','build.omg']:assert (p.parent/name).read_bytes()==(pin/name).read_bytes()
subprocess.run([sys.executable,str(folder/'run-probes-final.py')],check=True)
probes=json.loads((folder/'probes-4.json').read_text());assert len(probes)==4
for row in probes:
 if row['name'] in ['fresh_tag','negated_receiver']:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'],row
 else:assert row['exit']==1 and 'cannot prove selected case for payload access' in row['stderr'],row
print(json.dumps({'native_and_terminal_programs':10,'replacement_rejects':10,'focused_source_fixtures':len(source['observations']),'retained_audited_both_programs':49,'fresh_tag_and_negated_receiver':2,'stale_negation_and_trailing_write':2}))
