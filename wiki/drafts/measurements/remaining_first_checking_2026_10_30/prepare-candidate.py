from pathlib import Path
out=Path('build/remaining-first-checking')
source=Path('build/first-checking')
compare=(source/'compare.py').read_text()
compare=compare.replace("out=root/'build/first-checking'", "out=root/'build/remaining-first-checking'").replace('build/preparation-convergence/candidate/omega.exe','build/first-checking/candidate/omega.exe').replace('5a596bcaa6fb117eefd8735c6bb545e0ebbde1cbf1b9b8c3ba79a586a6b16308','1c7645a93c087116c69bb5951cc940709cdadecfeae847d38f9de957d3ad4d60')
compare=compare.replace("'--keep','--target','windows_x86_64',project/'main.omg'", "'--keep',project/'main.omg'").replace("assert code==0 and 'DECLINED' not in text,text", "assert code==0 and 'interp exit: 0' in text and 'native exit 0' in text and 'DECLINED' not in text,text")
(out/'compare.py').write_text(compare,encoding='utf-8',newline='\n')
quality=(source/'quality.py').read_text().split('# Correct the historical')[0]
quality=quality.replace("base=root/'build/first-checking'", "base=root/'build/remaining-first-checking'")
extra=['pass/proofs/constant_boolean_return_join_exit','pass/proofs/normal_return_expression_statement_sequence_exit','pass/calls/constant_result_requires_argument_accepted','fail/core/scoped_integer_call_wrapping_actual_requires_formation','fail/contracts/boolean_result_conflicting_normal_returns','fail/proofs/unknown_boolean_return_is_not_omitted']
quality=quality.replace('for fixture in fixtures:', f'fixtures += {extra!r}\nfor fixture in fixtures:')
(out/'quality.py').write_text(quality,encoding='utf-8',newline='\n')
