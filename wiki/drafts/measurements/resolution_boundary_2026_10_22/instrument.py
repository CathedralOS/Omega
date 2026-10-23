"""Install temporary, semantics-preserving diagnostic probes on the recorded base.

Run from the checkout root after freezing its unmodified compiler. The resulting
patch is evidence, not a production compiler change. Restore its touched paths
after freezing the instrumented compiler. No allocator or semantic cache changes.
"""
from pathlib import Path
import json

root = Path('omega-rust/psi/compilation')
touched = []

def edit(path, transform):
    path = root / path
    original = path.read_text()
    changed = transform(original)
    assert changed != original, path
    path.write_text(changed)
    if str(path) not in touched:
        touched.append(str(path))

helper = r'''
/// Temporary diagnostic instrumentation; removed from the retained compiler.
pub mod measurement_probe {
    pub struct Scope {
        label: &'static str,
        start: Option<std::time::Instant>,
    }
    impl Scope {
        pub fn begin(label: &'static str, detail: impl FnOnce() -> String) -> Self {
            let start = if std::env::var_os("OMEGA_RESOLUTION_MEASURE").is_some() {
                eprintln!("RESMEASURE\t{:?}\tbegin\t{}\t{}", std::thread::current().id(), label, detail());
                Some(std::time::Instant::now())
            } else { None };
            Self { label, start }
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            if let Some(start) = self.start {
                let elapsed = start.elapsed().as_nanos();
                eprintln!("RESMEASURE\t{:?}\tend\t{}\t{}", std::thread::current().id(), self.label, elapsed);
            }
        }
    }
}
'''
for carrier in ('syntax-trees', 'symbol-resolved-trees'):
    edit(f'representations/{carrier}/src/lib.rs', lambda text: text + helper)

def scope(path, function, label, details='String::new()', helper_path='symbol_resolved_trees::measurement_probe'):
    def transform(text):
        start = text.index('fn ' + function + '(')
        body = text.index('{\n', start) + 2
        line = f'    let _measurement_scope = {helper_path}::Scope::begin("{label}", || {details});\n'
        return text[:body] + line + text[body:]
    edit(path, transform)

resolution = 'pipeline/02_syntax-trees-to-symbol-resolved-trees/src/'
for function in ('resolve', 'resolve_deferred', 'resolve_deferred_numeric_probe',
                 'resolve_const_argument_selection', 'resolve_numeric_probe',
                 'resolve_deferred_initializer_probe', 'prepare_const_initializer_selection',
                 'resolve_extension'):
    scope(resolution + 'resolution.rs', function, 'resolver.' + function,
          'format!("roots={}", request.syntax.root_item_count())')
scope(resolution + 'lowering/mod.rs', 'lower_items', 'resolver.lower_items',
      'format!("roots={}", syntax.root_item_count())')
scope(resolution + 'preparation/mod.rs', 'prepare', 'resolver.prepare',
      'format!("roots={}", syntax.root_item_count())')
scope(resolution + 'resolution.rs', 'finish_resolution', 'resolver.finish')
scope(resolution + 'resolution.rs', 'finish_resolved_tables', 'resolver.finish_tables')
scope(resolution + 'resolution/body_completion.rs', 'complete_machine_bodies', 'resolver.complete_bodies',
      'format!("selected={}", selected.len())')
scope(resolution + 'resolution/body_completion.rs', 'seed_body_lowerer', 'resolver.clone_body_base',
      'format!("expressions={}", self.trees.tables.bodies.expressions.iter_expressions().count())')

compile_time = 'compile-time/src/'
scope(compile_time + 'build_time_evaluation.rs', 'evaluate_pre_resolution', 'pre_resolution',
      'format!("roots={}", request.syntax_trees.root_item_count())')
for file, function, label in (
    ('const_initializers', 'evaluate', 'initializers'),
    ('const_generic_expressions', 'evaluate', 'generic_expressions'),
    ('const_generic_calls', 'evaluate_const_generic_calls', 'generic_calls'),
    ('const_generic_calls', 'evaluate_application_group', 'generic_call_group'),
    ('range_arguments', 'evaluate', 'range_arguments'),
    ('const_qualifications', 'prove_ready', 'qualifications'),
):
    scope(compile_time + f'const_evaluation/{file}.rs', function, 'pre.' + label)
scope(compile_time + 'syntax_probes.rs', 'normalize_generic_data', 'pre.normalize_generic')
scope('driver/src/source_compilation.rs', 'prepare', 'driver.prepare',
      helper_path='syntax_trees::measurement_probe')
scope('driver/src/source_compilation.rs', 'complete_authored', 'driver.complete_authored',
      helper_path='syntax_trees::measurement_probe')

def syntax_clone(text):
    text = text.replace('#[derive(Debug, Clone)]\npub struct SyntaxTrees',
                        '#[derive(Debug)]\npub struct SyntaxTrees', 1)
    text += '''
impl Clone for SyntaxTrees {
    fn clone(&self) -> Self {
        let _measurement_scope = crate::measurement_probe::Scope::begin("syntax.clone", ||
            format!("roots={} expressions={}", self.root_item_count(), self.expressions.iter_expressions().count()));
        Self { source_id: self.source_id, roots: self.roots.clone(), tables: self.tables.clone() }
    }
}
'''
    return text
edit('representations/syntax-trees/src/syntax_trees.rs', syntax_clone)

def expressions_clone(text):
    text = text.replace('#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ExpressionTable',
                        '#[derive(Debug, PartialEq, Eq)]\npub struct ExpressionTable', 1)
    text += '''
impl Clone for ExpressionTable {
    fn clone(&self) -> Self {
        let _measurement_scope = crate::measurement_probe::Scope::begin("resolved.expressions.clone", ||
            format!("expressions={}", self.nodes.expressions.len()));
        Self { nodes: self.nodes.clone(), spans: self.spans.clone() }
    }
}
'''
    return text
edit('representations/symbol-resolved-trees/src/symbol_resolved_trees/values/expression.rs', expressions_clone)
scope('representations/symbol-resolved-trees/src/symbol_resolved_trees.rs',
      'rebuild_tables', 'resolved.rebuild_tables', helper_path='crate::measurement_probe')
scope('representations/symbol-resolved-trees/src/symbol_resolved_trees.rs',
      'append_completed_body_tables', 'resolved.append_body_tables', helper_path='crate::measurement_probe')
Path('build/resolution-measurement/instrumented-paths.json').write_text(json.dumps(touched, indent=2)+'\n')
print(f'Instrumented {len(touched)} files')
