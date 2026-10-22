import hashlib,json,subprocess
from pathlib import Path
root=Path.cwd(); scratch=root/'build/lowering-landing/publication';scratch.mkdir(exist_ok=True);binary=root/'build/lowering-landing/omega-publication'
subjects=['samples/cli/collections/segmented_packet_checksum/main.omg','samples/cli/facts/case_payload_exclusive_loan/call_result/main.omg','samples/cli/facts/case_payload_exclusive_loan/target_body/main.omg','samples/cli/facts/body_bearing_comparison/call_result/main.omg']
controls={
'samples/cli/facts/case_payload_exclusive_loan/call_result/invalid/main.omg':['while local borrow `selected` is still active'],
'samples/cli/facts/case_payload_exclusive_loan/target_body/invalid/main.omg':['while local borrow `selected` is still active'],
'samples/cli/facts/body_bearing_comparison/call_result/invalid/main.omg':['cannot prove requires contract for call hold','direct == false'],
 'tests/omega/fail/slices/scalar_copy_missing_domain/main.omg':['cannot prove scalar result domain'],
 'tests/omega/fail/borrows/returned_reference_invalidated_membership/main.omg':['cannot prove default-domain field requirement for return from borrow_invalidated','resource requires Resource::Open']}
records=[]
for subject in subjects+list(controls):
    invalid=subject in controls;command=[str(binary),*(['--check'] if invalid else ['run','--both']),subject]
    result=subprocess.run(command,capture_output=True,text=True,timeout=90);row=dict(subject=subject,command=command,exit=result.returncode,stdout=result.stdout,stderr=result.stderr);records.append(row)
    (scratch/'validation.json').write_text(json.dumps(dict(records=records,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest()),indent=2)+'\n')
    assert result.returncode==(1 if invalid else 0),row
    if invalid: assert all(fragment in result.stderr for fragment in controls[subject]),row
    else: assert 'native exit 0' in result.stderr and 'interp exit: 0' in result.stderr,row
print('Four native/Terminal agreements and five fact-sensitive rejections passed.')
