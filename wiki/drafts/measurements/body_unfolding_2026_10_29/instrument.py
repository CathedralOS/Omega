from pathlib import Path
path=Path('omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/structural_judgment.rs')
text=path.read_text()
start=text.index('    fn unfold_application(')
end=text.index('\n    fn ',start+5)
part=text[start:end]
part=part.replace('        let StructuralTerm::Application {','        let _unfold = phase_profile::guard("05.unfold.total");\n        let setup = phase_profile::guard("05.unfold.setup");\n        let StructuralTerm::Application {',1)
part=part.replace('        let mut environment = environment;','        drop(setup);\n        let mut environment = environment;')
part=part.replace('            if let Some(local) = statement.as_local_data() {','            if let Some(local) = statement.as_local_data() {\n                let _local = phase_profile::guard("05.unfold.local");')
part=part.replace('            if let Some(expression) = statement.as_expression() {','            if let Some(expression) = statement.as_expression() {\n                let _tail = phase_profile::guard("05.unfold.tail");')
part=part.replace('                TransitionGuardNode::When(guard) => {','                TransitionGuardNode::When(guard) => {\n                    let _guard = phase_profile::guard("05.unfold.guard");')
part=part.replace('                        match specification.judge_predicate_in_scope(','                        let _scalar = phase_profile::guard("05.unfold.scalar_guard");\n                        match specification.judge_predicate_in_scope(')
part=part.replace('            return match program.statement_table.transition_target(transition.target) {','            let _target = phase_profile::guard("05.unfold.target");\n            return match program.statement_table.transition_target(transition.target) {')
text=text[:start]+part+text[end:]
start=text.index('    fn callee_term(')
end=text.index('\n    /// Substitute variables',start)
part=text[start:end]
part=part.replace('        if depth >= 32 {','        let _term = phase_profile::guard("05.unfold.callee_term");\n        let preparation = phase_profile::guard("05.unfold.callee_prepare");\n        if depth >= 32 {',1)
part=part.replace('        match program.expression_table.expression(expression) {','        drop(preparation);\n        match program.expression_table.expression(expression) {',1)
text=text[:start]+part+text[end:]
path.write_text(text)
