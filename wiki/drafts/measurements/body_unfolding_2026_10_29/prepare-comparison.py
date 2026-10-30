from pathlib import Path
out=Path('build/body-unfolding')
old=Path('build/remaining-first-checking')
(out/'compare.py').write_text((old/'compare.py').read_text().replace('build/remaining-first-checking','build/body-unfolding'))
s=(old/'quality.py').read_text().replace('build/remaining-first-checking','build/body-unfolding')
start=s.index('fixtures=');end=s.index('\ntiming_rows=',start)
s=s[:start]+'''fixtures=[
    'pass/calls/selected_contract_equation_reentry_exit',
    'pass/proofs/constant_boolean_return_join_exit',
    'pass/borrows/runtime_reference_call_scalar_snapshot_exit',
    'pass/facts/copied_call_argument_snapshot_through_match_join_exit',
    'pass/contracts/current_computed_scalar_guard_exit',
    'fail/calls/selected_contract_equation_reentry_unproved_requirement',
    'fail/contracts/current_computed_scalar_guard_false_arm',
    'fail/contracts/current_computed_scalar_guard_skipped',
    'fail/contracts/current_computed_scalar_guard_overwritten',
    'fail/core/scoped_integer_call_wrapping_actual_requires_formation',
    'fail/calls/closed_constructor_requires_restricted_nested_call_rejected',
    'fail/borrow/scalar_reference_result_does_not_preserve_mutated_bound',
    'fail/calls/copied_argument_ensures_cannot_restore_callee_mutated_source',
    'fail/contracts/boolean_result_conflicting_normal_returns',
    'fail/proofs/unknown_boolean_return_is_not_omitted',
]
'''+s[end:]
start=s.index('if not screen_passed:');end=s.index('for fixture in fixtures:',start)
s=s[:start]+'''if not screen_passed:
    print('Performance screen failed: retain only selected-contract recursion positive/refusal controls; broader suite skipped.',flush=True)
    fixtures=['pass/calls/selected_contract_equation_reentry_exit','fail/calls/selected_contract_equation_reentry_unproved_requirement']
'''+s[end:]
s=s.replace("('candidate-'+source.name)","('candidate-'+fixture.replace('/','-'))")
s=s.replace("    cache=directory/'cache'\n    cache.mkdir()", "    cache=base/'cases/timing-1-B/cache'\n    assert cache.is_dir()")
s=s.replace("and 'native ' in text", "and f'native exit {expected}' in text")
s=s[:s.index('\nsubprocess.run([sys.executable,str(base/')]
(out/'quality.py').write_text(s.rstrip()+'\n')
chain=(old/'candidate-chain.py').read_text().replace('build/remaining-first-checking','build/body-unfolding').replace("assert b'05.remaining.' not in patch", "assert b'05.unfold.' not in patch and b'05.guard.' not in patch").replace('specification equation early-exit candidate','guard-query operand reuse candidate')
(out/'candidate-chain.py').write_text(chain)
