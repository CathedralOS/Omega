//! Runtime noninterference and fail-closed fences for occurrence-level
//! `[erased]` bindings: data fields, case payload fields, signature
//! parameters, and `let` locals.
//!
//! The executable slice supports transparent records and sums, plus closed
//! synthesized generic-record instances at explicitly typed local
//! initializers, and closed plain data whose attached machines are ordinary
//! checked bodies. The full semantic tree remains intact for proofs and
//! ownership; native lowering later strips erased literal fields from its
//! private runtime expression graph and attached-machine storage/topology.
//!
//! Every binding occurrence shares one contract
//! (wiki/spec/proofs/contracts.md#explicit-erased-bindings): the binding may
//! supply proof computation but cannot determine runtime data or control.
//! Contracts, invariants, and proof machines are `Context::Proof` and never
//! walk here as runtime reads. An erased binding's own initializer -- a
//! field initializer in a struct literal, a `let x [erased]` initializer, or
//! the argument supplied to an erased parameter position -- is
//! `Context::ErasedInitializer`: it may read erased bindings but cannot call
//! a runtime machine or perform an atomic operation, because nothing at
//! runtime will ever observe the result. Runtime reads of erased parameters
//! and locals are resolved by symbol against the current state's own
//! parameters and `let` statements; parameter positions align arguments with
//! the callee's non-`self` parameters. A statement call's receiver is its
//! callee's `self` operand -- a runtime place, not a proof position -- so a
//! runtime call through an erased binding or erased field projection rejects
//! with the same diagnostic as a direct read.

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
            || crate::value_custody::content_projections::is_content_projection_machine(
                program, machine,
            ) {
            Context::Proof
        } else {
            Context::Runtime
        };
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::RootBinding(binding) => {
                        // A root binding is a Build declaration, not a
                        // runtime call, but its two expression operands are
                        // read at build evaluation exactly like runtime
                        // places: the receiver supplies the `&mut Build`
                        // cell and a delegated operand supplies the
                        // `ProductEntryRef` description of the installed
                        // entry. Neither is reachable through a call's
                        // argument list, so the walker must visit them here.
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            binding.receiver,
                            machine_context,
                            diagnostics,
                        );
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            binding.implementation_operand,
                            machine_context,
                            diagnostics,
                        );
                    }
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
                        // The receiver is the callee's `self` operand: a
                        // runtime place read or address of storage. It is a
                        // name path with resolved root/leaf symbols, not an
                        // expression child, so the expression walker never
                        // sees it -- a runtime call must not receive through
                        // an erased binding or erased field projection.
                        if argument_context == Context::Runtime {
                            runtime_uses::validate_statement_call_receiver(
                                program,
                                machine,
                                state,
                                call,
                                diagnostics,
                            );
                        }
                        let callee_parameters = callee_parameters(program, call.target_symbol);
                        for (position, argument) in program
                            .statement_table
                            .expression_handles(call.arguments)
                            .iter()
                            .enumerate()
                        {
                            validate_expression(
                                program,
                                &proof_only,
                                machine,
                                state,
                                *argument,
                                argument_position_context(
                                    argument_context,
                                    callee_parameters,
                                    position,
                                ),
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
                        if local.relevance.is_erased() && machine_context == Context::Runtime {
                            Context::ErasedInitializer
                        } else {
                            machine_context
                        },
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
                                TransitionTargetNode::Named {
                                    path, arguments, ..
                                } => {
                                    // A named transition binds the target
                                    // state's parameters exactly like a call
                                    // binds a callee's: an argument at an
                                    // erased position is that binding's
                                    // initializer, not a runtime transfer.
                                    let target_parameters =
                                        transition_target_parameters(program, state, path);
                                    for (position, argument) in program
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .iter()
                                        .enumerate()
                                    {
                                        validate_expression(
                                            program,
                                            &proof_only,
                                            machine,
                                            state,
                                            *argument,
                                            argument_position_context(
                                                machine_context,
                                                target_parameters,
                                                position,
                                            ),
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

/// The callee state's authored parameters, when `target` names a state.
/// Positional call arguments align with the non-`self` parameters.
pub(super) fn callee_parameters(
    program: &TypedTrees,
    target: SymbolHandle,
) -> Option<&[typed_trees::signature::StateParameter]> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target)
            .map(|state| program.state_parameters(state))
    })
}

/// The parameters a named transition target binds: the sibling state (or
/// machine entry) `path` resolves to, or the current state itself for a
/// bare `self` re-entry. Positional transition arguments align with the
/// target's non-`self` parameters, the same rule as call arguments.
fn transition_target_parameters<'program>(
    program: &'program TypedTrees,
    state: &'program typed_trees::state::State,
    path: &typed_trees::statement::TableNamePath,
) -> Option<&'program [typed_trees::signature::StateParameter]> {
    callee_parameters(program, path.symbol).or_else(|| {
        let members = program.statement_table.name_path_members(path.members);
        matches!(members, [member] if member.is_self_receiver())
            .then(|| program.state_parameters(state))
    })
}

/// A runtime argument supplied to an erased parameter position is the erased
/// binding's initializer, not a runtime transfer.
pub(super) fn argument_position_context(
    context: Context,
    callee_parameters: Option<&[typed_trees::signature::StateParameter]>,
    position: usize,
) -> Context {
    if context != Context::Runtime {
        return context;
    }
    let erased_position = callee_parameters.is_some_and(|parameters| {
        parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .nth(position)
            .is_some_and(|parameter| parameter.relevance.is_erased())
    });
    if erased_position {
        Context::ErasedInitializer
    } else {
        Context::Runtime
    }
}

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
