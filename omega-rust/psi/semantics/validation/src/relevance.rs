//! Runtime noninterference and fail-closed fences for occurrence-level
//! `[erased]` data fields.
//!
//! The executable slice supports transparent records and sums, plus closed
//! synthesized generic-record instances at explicitly typed local
//! initializers, and closed plain data whose attached machines are ordinary
//! checked bodies. The full semantic tree remains intact for proofs and
//! ownership; native lowering later strips erased literal fields from its
//! private runtime expression graph and attached-machine storage/topology.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    Runtime,
    ErasedInitializer,
    Proof,
}

pub(crate) fn validate_relevance(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let proof_only = typed_trees::proof_only::classify(program);
    validate_supported_shapes(program, diagnostics);

    for machine in program.machines() {
        let machine_context = if proof_only.is_proof_machine(program, machine)
            || crate::content_projections::is_content_projection_machine(program, machine)
        {
            Context::Proof
        } else {
            Context::Runtime
        };
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::AssemblyFact(fact) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        fact.expression,
                        Context::Proof,
                        diagnostics,
                    ),
                    StatementNode::Assignment(assignment) => {
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            assignment.target,
                            machine_context,
                            diagnostics,
                        );
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            assignment.value,
                            machine_context,
                            diagnostics,
                        );
                    }
                    StatementNode::Call(call) => {
                        let argument_context = if machine_context == Context::Proof
                            || call_targets_proof_machine(program, &proof_only, call.target_symbol)
                        {
                            Context::Proof
                        } else {
                            Context::Runtime
                        };
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            validate_expression(
                                program,
                                &proof_only,
                                machine,
                                state,
                                *argument,
                                argument_context,
                                diagnostics,
                            );
                        }
                    }
                    StatementNode::Expression(expression) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        *expression,
                        machine_context,
                        diagnostics,
                    ),
                    StatementNode::LocalData(local) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        local.initial_value,
                        machine_context,
                        diagnostics,
                    ),
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            validate_expression(
                                program,
                                &proof_only,
                                machine,
                                state,
                                guard,
                                machine_context,
                                diagnostics,
                            );
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named { arguments, .. } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        validate_expression(
                                            program,
                                            &proof_only,
                                            machine,
                                            state,
                                            *argument,
                                            machine_context,
                                            diagnostics,
                                        );
                                    }
                                }
                                TransitionTargetNode::Value(value) => validate_expression(
                                    program,
                                    &proof_only,
                                    machine,
                                    state,
                                    *value,
                                    machine_context,
                                    diagnostics,
                                ),
                                TransitionTargetNode::SelfTarget
                                | TransitionTargetNode::Terminal => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

mod shape_admission;
pub(super) use shape_admission::erased_fields;
use shape_admission::validate_supported_shapes;

mod runtime_uses;
use runtime_uses::validate_expression;

pub(super) fn call_targets_proof_machine(
    program: &TypedTrees,
    proof_only: &typed_trees::proof_only::ProofOnlyClassification,
    target: SymbolHandle,
) -> bool {
    program.machines().iter().any(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
            && proof_only.is_proof_machine(program, machine)
    })
}
