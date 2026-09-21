//! Concrete premise discharge for a zero-argument `machine()` invocation.
//!
//! An authored `requires` premise has no meaning outside an invocation, so
//! the common floor fences every call closure carrying one until a checked
//! probe re-runs ordinary contract checking on the exact call. Positions
//! that own an authored call node — constant initializers — build that
//! probe around it. A const-position leg such as a fixed-array length's
//! `machine()` owns no call node, but the invocation is just as concrete:
//! the exact call is `machine()` at the leg's own source span. Admission
//! therefore builds the private probe itself, appending a generated machine
//! whose body is the synthesized call, and treats a clean checked lowering
//! with no retained crash routes as the discharge evidence. Every other
//! floor axis — service reach, suspension, blocking, termination, linear
//! carriers, declaration-selection authority — is enforced unchanged.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableTransition, TransitionTargetNode};

/// Whether a private checked probe discharges `machine`'s authored
/// `requires` premises for the concrete zero-argument call `machine()`.
/// Returns `true` only when the synthesized call survives ordinary checked
/// lowering and the probe machine retains no unhandled crash routes; a
/// premise that ordinary checking cannot prove at that call keeps the
/// conservative closure fence.
pub(super) fn zero_argument_invocation_discharges(
    program: &TypedTrees,
    machine: &Machine,
    source_span: Option<source::SourceSpan>,
) -> bool {
    let mut probe = program.clone();
    // The probe runs inside the preliminary package checkpoint: deferred
    // range-endpoint marks name the outer const-eval continuation, not this
    // private program, so they must not survive as a tolerated lost fold.
    probe.pending_const_range_endpoints.clear();

    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let Some((entry_symbol, entry_name, entry_return)) = probe
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine.symbol)
        .and_then(|candidate| {
            let states = probe.machine_states(candidate);
            states
                .iter()
                .find(|state| state.name.as_str() == leaf)
                .or_else(|| states.first())
        })
        .filter(|entry| probe.state_parameters(entry).is_empty())
        .map(|entry| (entry.symbol, entry.name.clone(), entry.return_type))
    else {
        return false;
    };

    let call = TableCallExpression {
        receiver: ExpressionHandle::invalid(),
        target_symbol: entry_symbol,
        static_machine_parameter: SymbolHandle::invalid(),
        target: entry_name,
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        arguments: probe.expression_table.insert_expression_handles([]),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_semantics::CallOperationalAcknowledgement::default(),
    };
    let concrete = probe.expression_table.insert(ExpressionNode::Call(call));
    let call_span = source_span
        .or_else(|| probe.symbols.symbol_source_span(machine.symbol))
        .unwrap_or_default();
    probe.expression_table.set_source_span(concrete, call_span);
    let owner = append_invocation_probe(
        &mut probe,
        machine.symbol,
        "@const-length",
        concrete,
        entry_return,
    );
    // The const-eval window sits inside preliminary package checking: the
    // owning program has not reached settlement, so the probe lowers in the
    // same mode. Contract checking is not gated on the mode, so authored
    // `requires` premises are still decided at the synthesized call.
    let Ok(checked) = typed_trees_to_checked_trees::lower_typed_trees(
        probe,
        &typed_trees_to_checked_trees::CheckingRequest::preliminary(),
    ) else {
        return false;
    };
    matches!(
        typed_trees_to_checked_trees::infer_checked_machine_crash_causes(
            &checked.typed,
            &checked.facts,
            owner,
        ),
        Some(causes) if causes.is_empty()
    )
}

/// Append the generated probe machine whose single state returns the
/// synthesized call: `machine @const-length() -> <entry return> { machine() }`.
fn append_invocation_probe(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    expression: ExpressionHandle,
    destination: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    let symbol = probe
        .symbols
        .insert_generated_root_from(owner, SymbolKind::Machine, name);
    let children = probe
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name)]);
    let target = probe
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let mut state = State {
        symbol: children.start(),
        name: typed_trees::name::Identifier::generated(name),
        return_type: destination,
        ..Default::default()
    };
    let source_span = probe.expression_table.source_span(expression);
    probe.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    let mut probe_machine = Machine {
        symbol,
        name: typed_trees::name::Identifier::generated(name),
        ..Default::default()
    };
    probe.push_machine_state(&mut probe_machine, state);
    probe.push_machine(probe_machine);
    symbol
}

#[cfg(test)]
mod tests {
    use super::zero_argument_invocation_discharges;
    use crate::BuildTimeAdmissionPlan;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn typed(source: &str) -> typed_trees::TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    fn admission(program: &typed_trees::TypedTrees) -> BuildTimeAdmissionPlan {
        BuildTimeAdmissionPlan::infer(program, None)
    }

    fn machine<'a>(
        program: &'a typed_trees::TypedTrees,
        name: &str,
    ) -> &'a typed_trees::machine::Machine {
        program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("machine exists")
    }

    #[test]
    fn zero_argument_probe_discharges_a_provable_machine_requires() {
        let program = typed(
            r#"
machine length() -> u64
requires true;
{
    4
}
data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let plan = admission(&program);
        let target = machine(&program, "length");
        assert!(plan.closure_includes_authored_requires(&program, target));
        assert!(zero_argument_invocation_discharges(
            &program,
            target,
            Some(source::SourceSpan::default()),
        ));
    }

    #[test]
    fn zero_argument_probe_keeps_the_fence_for_an_unprovable_requires() {
        let program = typed(
            r#"
machine length() -> u64
requires false;
{
    4
}
data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let plan = admission(&program);
        let target = machine(&program, "length");
        assert!(plan.closure_includes_authored_requires(&program, target));
        assert!(!zero_argument_invocation_discharges(
            &program,
            target,
            Some(source::SourceSpan::default()),
        ));
    }

    #[test]
    fn zero_argument_probe_discharges_state_and_callee_premises() {
        let program = typed(
            r#"
machine nonzero() -> u64
requires true;
{
    7
}

machine length() -> u64 {
    nonzero()
}
data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let plan = admission(&program);
        let target = machine(&program, "length");
        assert!(plan.closure_includes_authored_requires(&program, target));
        assert!(zero_argument_invocation_discharges(
            &program,
            target,
            Some(source::SourceSpan::default()),
        ));
    }
}
