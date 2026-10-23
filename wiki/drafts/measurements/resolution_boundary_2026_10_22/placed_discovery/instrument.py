"""Temporary full-discovery versus bypass experiment; never production code."""
from pathlib import Path
import json
import subprocess

parent = Path(__file__).resolve().parent.parent
output = Path('build/placed-discovery')
assert not subprocess.check_output(['git', 'diff', 'HEAD', '--', 'omega-rust'])
subprocess.run(['git', 'apply', str(parent/'instrumentation.patch')], check=True)
paths = json.loads((parent/'instrumented-paths.json').read_text())

path = Path('omega-rust/psi/compilation/compile-time/src/layouts/placed_views/applied_results.rs')
text = path.read_text()
needle = '    if !syntax.expressions.iter_expressions().any(|(_, node)| {'
assert text.count(needle) == 1
text = text.replace(needle, '''    let measured = std::env::var_os("OMEGA_PLACED_DISCOVERY_MEASURE").is_some();
    if std::env::var("OMEGA_PLACED_DISCOVERY_MODE").as_deref() == Ok("skip") {
        if measured { eprintln!("PLACEDMEASURE\\tskipped"); }
        return Ok(Vec::new());
    }
    let started = measured.then(std::time::Instant::now);
''' + needle)
needle = '    let mut selections = Vec::new();'
assert text.count(needle) == 1
text = text.replace(needle, '''    if measured {
        eprintln!("PLACEDMEASURE\\tinput\\tresolved_expressions={} calls={} machines={}",
            resolved.tables.bodies.expressions.iter_expressions().count(),
            resolved.tables.bodies.expressions.iter_expressions().filter(|(_, node)| matches!(node, symbol_resolved_trees::expression::ExpressionNode::Call(_))).count(),
            resolved.machines.iter().count());
    }
''' + needle)
needle = '    Ok(selections)'
assert text.count(needle) == 1
text = text.replace(needle, '''    if let Some(started) = started {
        eprintln!("PLACEDMEASURE\\toutput\\tselections={} elapsed_ns={}", selections.len(), started.elapsed().as_nanos());
        for selected in &selections {
            eprintln!("PLACEDMEASURE\\tselection\\t{:?}\\t{:?}",
                selected.declaration, syntax.type_references.type_reference(selected.reference));
        }
    }
''' + needle)
path.write_text(text)
paths.append(str(path))

path = Path('omega-rust/psi/compilation/compile-time/src/layouts/placed_views.rs')
text = path.read_text()
needle = '    if applications.is_empty() {'
assert text.count(needle) == 1
text = text.replace(needle, '''    if std::env::var_os("OMEGA_PLACED_DISCOVERY_MEASURE").is_some() {
        eprintln!("PLACEDMEASURE\\tapplications\\tcount={}", applications.len());
        for application in &applications {
            eprintln!("PLACEDMEASURE\\tapplication\\t{:?}\\t{:?}\\t{:?}\\t{:?}",
                application.template_source, application.policy_source,
                application.schema_source, application.invocation_sources);
        }
    }
''' + needle)
path.write_text(text)
paths.append(str(path))
(output/'instrumented-paths.json').write_text(json.dumps(paths, indent=2)+'\n')
(output/'instrumentation.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
print('Installed diagnostic discovery/bypass probes', flush=True)
