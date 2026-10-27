import hashlib,json,subprocess,sys
from pathlib import Path
root=Path.cwd();folder=Path('build/saved-case');cli=folder/'producer-3/omega';runner=folder/'producer-3/corpus_runner'
assert hashlib.sha256(cli.read_bytes()).hexdigest()=='75faeda8aa69f86b499b28e915fbf9609a9ec70456d67b0f192445ae7a0769bc'
retained=json.loads((folder/'corpus-both.json').read_text())
assert retained['exit_code']==0 and len(retained['observations'])==47
for row in retained['observations']:
 assert row['status']=='built' and row['facts']['interpreter']=='agrees'
 assert row['facts']['exit']==row['facts']['interpreter-exit']=='0'
filters='facts/saved_case_predicate_,facts/saved_observation_snapshot_,facts/saved_byte_predicate_,facts/case_payload_stored_loan_,facts/descriptor_guard_replacement_,control_flow/saved_case_predicate_,borrows/guard_subject,borrows/guard_observation'
command=[sys.executable,'tools/corpus_gate.py','--runner',str(runner),'--filter',filters,'--json',str(folder/'final-source.json')]
result=subprocess.run(command,capture_output=True,text=True);(folder/'final-source.stdout').write_text(result.stdout);(folder/'final-source.stderr').write_text(result.stderr)
assert result.returncode in [0,1],result.stdout+result.stderr
source=json.loads((folder/'final-source.json').read_text());assert source['counts']['wrong_reason']==source['counts']['fail_not_rejected']==source['counts']['lost_successes']==0
for row in source['observations']:
 assert row['status']==('rejected' if row['tier']=='fail' else 'checked'),row
 if row['tier']=='fail':assert row['expected_satisfied'] is True,row
subprocess.run([sys.executable,str(folder/'run-both-row.py'),str(cli),'final'],check=True)
rows=json.loads((folder/'row-final.json').read_text());assert len(rows)==18
for row in rows:
 p=Path(row['command'][-1]);assert hashlib.sha256(p.read_bytes()).hexdigest()==row['source_sha256']
 if row['invalid']:assert row['exit']==1 and 'cannot prove selected case for payload access' in row['stderr']
 else:assert row['exit']==0 and 'native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr']
 place=row['place'];tier='fail' if row['invalid'] else 'pass';suffix='rejected' if row['invalid'] else 'exit'
 pin=Path('tests/omega')/tier/'facts'/f'saved_case_predicate_through_{place}_{suffix}'
 for name in ['main.omg','build.omg']:assert (p.parent/name).read_bytes()==(pin/name).read_bytes()
print(json.dumps({'native_and_terminal_places':9,'fact_sensitive_rejects':9,'focused_source_fixtures':len(source['observations']),'retained_audited_both_programs':47}))
