from pathlib import Path
import hashlib,json,re
from decimal import Decimal
project=Path(__file__).resolve().parent
assert hashlib.sha256((project/'data/expected.json').read_bytes()).hexdigest() == (project/'data/expected.sha256').read_text().split()[0]
fixture=json.loads((project/'data/expected.json').read_text())
fields={'mass_a_ug':('mass_a',1000000),'mass_b_ug':('mass_b',1000000),'total_ug':('total',1000000),'mass_variance_nano_g2':('mass_variance',1000000000),'covariance_nano_g2':('covariance',1000000000),'total_variance_nano_g2':('total_variance',1000000000),'fraction_a_ppm':('fraction_a',1000000),'fraction_b_ppm':('fraction_b',1000000),'fraction_variance_nano':('fraction_variance',1000000000),'standard_uncertainty_ppm':('uncertainty',1000000),'expanded_uncertainty_ppm':('expanded',1000000),'interval_lower_ppm':('lower',1000000),'interval_upper_ppm':('upper',1000000)}
def literal(value):
 value=format(Decimal(value),'f')
 return value if '.' in value else value+'.0'
cases=[c for c in fixture['cases'] if re.fullmatch(r'([gm])[0-9]{3}\1[0-9]{3}\1[0-9]{3}[12]\n',c['input'])]
s='''// Supplemental runtime numeric checks from frozen data/expected.json.
// The application is main.omg; this entry verifies the same private domain
// through native and Terminal execution, since CLI --both compares statuses.
// Expected exit: 0.
// Expected output contains: VERIFIED: domain_oracles
use omega_language_std::console;
use omega::language::core::binding;
use omega::language::core::float_operations;
use domain::mixture;

data Main { console: Binding<Console>; result: Mixture; }

machine Main::main(main: &mut Main) reaches Console {
    transition { _ -> case_0(main) }
'''
for i,c in enumerate(cases):
 text=c['input']
 nextcase=f'case_{i+1}' if i+1<len(cases) else 'good'
 numbers=list(c.get('expected_numbers',{}).items())
 s+=f'    state case_{i}(main: &mut Main) {{\n'
 s+=f'        main.console.write_line("domain case: {c["name"]}");\n'
 s+=f'        correct(&mut main.result, {int(text[1:4])}.0, {int(text[5:8])}.0, {int(text[9:12])}.0, '+('1.0' if text[0]=='g' else '0.001')+', '+('true' if text[12]=='1' else 'false')+');\n'
 first=f'case_{i}_metric_0_lower' if numbers else nextcase
 s+=f'        transition main.result.status == {c["exit"]} {{\n            true -> {first}(main)\n            false -> bad(main)\n        }}\n    }}\n'
 for j,(key,val) in enumerate(numbers):
  field,scale=fields[key]
  following=f'case_{i}_metric_{j+1}_lower' if j+1<len(numbers) else nextcase
  prefix=f'case_{i}_metric_{j}'
  s+=f'    state {prefix}_lower(main: &mut Main) {{\n'
  s+=f'        let observed: f64 = main.result.{field} * {scale}.0;\n'
  s+=f'        transition observed >= {literal(val)} - 1.0 {{\n            true -> {prefix}_upper(main, observed)\n            false -> bad(main)\n        }}\n    }}\n'
  s+=f'    state {prefix}_upper(main: &mut Main, observed: f64) {{\n'
  s+=f'        transition observed <= {literal(val)} + 1.0 {{\n            true -> {following}(main)\n            false -> bad(main)\n        }}\n    }}\n'
s+='''    state good(main: &mut Main) { main.console.write_line("VERIFIED: domain_oracles"); main.console.exit_process(0); }
    state bad(main: &mut Main) { main.console.write_line("FAIL: domain_oracles"); main.console.exit_process(77); }
}
'''
(project/'verify_domain.omg').write_text(s)
print(len(cases),'domain cases rendered from the unchanged frozen oracle')
