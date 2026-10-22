import subprocess,json,re
from pathlib import Path
root=Path.cwd()
result=subprocess.run(['mbx','nextest','run','-p','typed-trees','-p','checked-trees-to-lowered-psi','-p','terminal-semantics'],capture_output=True,text=True)
(root/'build/lowering-landing/candidate-tests.txt').write_text(result.stdout+result.stderr)
baseline=(root/'build/lowering-landing/baseline-tests.txt').read_text()
def failures(text):
    return sorted(set(re.findall(r'^\s*FAIL\s+\[[^\]]+\] (?:\(\d+/\d+\) )?(.+)$',text,re.M)))
assert failures(baseline)==failures(result.stdout+result.stderr), (failures(baseline),failures(result.stdout+result.stderr))
assert all('source_guards::lowering_reads_no_authored_statements_or_expressions' in x or 'source_guards::lowering_refusals_do_not_revalidate_the_plan' in x for x in failures(baseline))
assert result.returncode == (100 if failures(baseline) else 0),result.returncode
print(json.dumps(dict(baseline_failures=failures(baseline),candidate_failures=failures(result.stdout+result.stderr),candidate_exit=result.returncode)))

def violations(text):
    return sorted(re.findall(r'^    lowering .*$', text, re.M))
assert violations(baseline)==violations(result.stdout+result.stderr), (violations(baseline),violations(result.stdout+result.stderr))
print('Exact source-audit violation text unchanged')

assert '49 tests run: 47 passed, 2 failed, 0 skipped' in baseline
assert '49 tests run: 47 passed, 2 failed, 0 skipped' in result.stdout+result.stderr
