//! Checked discharge of authored `requires` premises in a call closure.
//!
//! An authored `requires` premise has no meaning outside an invocation, so
//! the common floor fences every call closure carrying one until ordinary
//! contract checking proves it. Two routes produce that proof here.
//!
//! A premise on the invoked machine or its entry state depends on the
//! invocation's arguments. Positions that own an authored call node —
//! constant initializers — build a checked probe around it. A const-position
//! leg such as a fixed-array length's `machine()` owns no call node, but the
//! invocation is just as concrete: the exact call is `machine()` at the leg's
//! own source span. Admission therefore builds the private probe itself,
//! appending a generated machine whose body is the synthesized call, and
//! treats a clean checked lowering with no retained crash routes as the
//! discharge evidence.
//!
//! Every other premise — on a callee, a later state, or a callable target —
//! sits at a source call or transition site whose proof reads only that
//! caller's own facts. When the entry carries no premise, a clean checked
//! lowering of the program proves the whole closure for any arguments, so an
//! argument-carrying policy or layout invocation needs no synthesized call.
//!
//! Every other floor axis — service reach, suspension, blocking,
//! termination, linear carriers, declaration-selection authority — is
//! enforced unchanged.

use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
    StatementNode, TableTransition, TransitionTargetNode,
};
use symbols::{SymbolHandle, SymbolKind};

use super::closure_validation::has_authored_requires;

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

    let Some((entry_symbol, entry_name, entry_return)) = entry_state(program, machine)
        .filter(|entry| program.state_parameters(entry).is_empty())
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

/// Whether ordinary checking alone discharges every authored `requires`
/// premise in `machine`'s call closure, whatever the invocation's arguments.
/// Holds only when neither the machine nor its entry state carries an
/// authored premise and the program survives ordinary checked lowering;
/// an entry premise keeps its concrete routes or the conservative fence.
pub(super) fn argument_independent_closure_discharges(
    program: &TypedTrees,
    machine: &Machine,
) -> bool {
    let entry_is_unconditional = !has_authored_requires(program.machine_contracts(machine))
        && entry_state(program, machine)
            .is_some_and(|entry| !has_authored_requires(program.state_contracts(entry)));
    if !entry_is_unconditional {
        return false;
    }
    let mut checked = program.clone();
    // Same preliminary-package window as the zero-argument probe above.
    checked.pending_const_range_endpoints.clear();
    typed_trees_to_checked_trees::lower_typed_trees(
        checked,
        &typed_trees_to_checked_trees::CheckingRequest::preliminary(),
    )
    .is_ok()
}

/// The state an invocation of `machine` enters: the one named by the
/// machine's leaf, else its first state.
fn entry_state<'a>(program: &'a TypedTrees, machine: &Machine) -> Option<&'a State> {
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == leaf)
        .or_else(|| states.first())
}

/// Append the generated probe machine whose single state returns the
/// synthesized call: `machine @const-length() -> <entry return> { machine() }`.
fn append_invocation_probe(
    probe: &mut TypedTrees,
    owner: SymbolHandle,
    name: &str,
    expression: ExpressionHandle,
    destination: symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle,
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
        name: symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(name),
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
        name: symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(name),
        ..Default::default()
    };
    probe.push_machine_state(&mut probe_machine, state);
    probe.push_machine(probe_machine);
    symbol
}

#[cfg(test)]
mod tests {
    use super::{argument_independent_closure_discharges, zero_argument_invocation_discharges};
    use crate::build_time_evaluation::BuildTimeAdmissionPlan;

    fn admission(
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    ) -> BuildTimeAdmissionPlan {
        BuildTimeAdmissionPlan::infer(program, None)
    }

    fn machine<'a>(
        program: &'a symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        name: &str,
    ) -> &'a symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine {
        program
            .realized_machine_named(name)
            .expect("machine exists")
    }

    #[test]
    fn zero_argument_probe_discharges_a_provable_machine_requires() {
        let program = crate::build_time_evaluation::front_end::typed_program(
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
        let program = crate::build_time_evaluation::front_end::typed_program(
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
        let program = crate::build_time_evaluation::front_end::typed_program(
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

    #[test]
    fn checking_discharges_callee_premises_past_an_unconditional_entry() {
        let program = crate::build_time_evaluation::front_end::typed_program(
            r#"
machine bounded(value: u64) -> u64
requires value <= 512;
{
    value
}

machine policy(value: u64) -> u64 {
    bounded(256)
}
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let plan = admission(&program);
        let target = machine(&program, "policy");
        assert!(plan.closure_includes_authored_requires(&program, target));
        assert!(argument_independent_closure_discharges(&program, target));
        plan.require_common_floor(&program, target)
            .expect("the callee premise is proved at its call site");
    }

    #[test]
    fn an_unproved_callee_premise_keeps_the_fence() {
        let program = crate::build_time_evaluation::front_end::typed_program(
            r#"
machine bounded(value: u64) -> u64
requires value <= 512;
{
    value
}

machine policy(value: u64) -> u64 {
    bounded(value)
}
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let plan = admission(&program);
        let target = machine(&program, "policy");
        assert!(!argument_independent_closure_discharges(&program, target));
        let error = plan
            .require_common_floor(&program, target)
            .expect_err("ordinary checking cannot prove `value <= 512`")
            .reason;
        assert!(
            error.contains("has an authored `requires` premise"),
            "{error}"
        );
    }

    #[test]
    fn an_entry_premise_depends_on_the_invocation_arguments() {
        let program = crate::build_time_evaluation::front_end::typed_program(
            r#"
machine policy(value: u64) -> u64
requires value <= 512;
{
    value
}
data Main { }
machine Main::main(&mut self) { }
"#,
        );
        let target = machine(&program, "policy");
        assert!(!argument_independent_closure_discharges(&program, target));
    }
}
