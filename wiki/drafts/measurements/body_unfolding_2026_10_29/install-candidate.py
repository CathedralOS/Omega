import subprocess
from pathlib import Path
out=Path('build/body-unfolding')
patch=(out/'guards.patch').read_bytes()
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
subprocess.run(['git','apply','--reverse',str(out/'guards.patch')],check=True)
subprocess.run(['git','diff','--exit-code','--','omega-rust'],check=True)
p=Path('omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/structural_judgment/integer_predicates.rs')
s=p.read_text()
s=s.replace("    // Only one immutable judge's goal batch may share resolved operand payloads.","    // Answers belong to one immutable judge's query or goal batch.")
start=s.index('    fn integer_predicate_observations(');end=s.index('\n    fn observed_integer_premises(',start)
part=s[start:end]
assert part.count('operand_resolutions: None,')==1
part=part.replace('            operand_resolutions: None,','''            // Goal and premise projection use the same immutable judge. Reuse
            // only exact operand payloads within this query; formation and
            // operation registration still run for every occurrence.
            operand_resolutions: Some(Vec::new()),''')
s=s[:start]+part+s[end:];p.write_text(s)
