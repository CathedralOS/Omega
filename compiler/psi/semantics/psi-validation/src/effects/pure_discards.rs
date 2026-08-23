use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::statement::StatementNode;

/// DECISION 12 -- DISCARD ADMITS EFFECTS: `_ = call();` exists to drop a
/// result the caller does not need while keeping the callee's observable work.
/// When the resolved callee provably has no observable channel at all -- an
/// empty service/operational reach AND no `&mut`/out parameters -- the statement is dead
/// runtime code. AMENDED (owner, 2026-07-12) from reject to WARN, for
/// uniform compilation: the statement always compiles, and the deadness
/// surfaces as a warning ONLY outside proof use. Proof use is silent by
/// design -- a statement call whose callee is a PROOF MACHINE is a CITATION
/// (ch10 "Citing Proofs": invoked for its ensures, erased at codegen), and
/// inside a proof machine's own body every discard is compile-time
/// material. Service reach and the independent operational summaries come
/// from their recursive plans, never the lowercase compatibility set.
pub(super) fn validate_pure_discards(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let classification = psi_typed_trees::proof_only::classify(program);
    for machine in program.machines() {
        // Inside a proof machine, discards are proof context -- silent.
        if classification.is_proof_machine(program, machine) {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                if !call.discards_result {
                    continue;
                }

                // A proof-machine callee makes the statement a CITATION,
                // not dead code.
                if machine_owning_state(program, call.target_symbol)
                    .is_some_and(|callee| classification.is_proof_machine(program, callee))
                {
                    continue;
                }

                // An unresolved callee cannot be PROVABLY pure; resolution
                // errors are owned by other passes.
                let Some(callee) = resolve_discard_callee(
                    program,
                    operational,
                    service_reaches,
                    call.target_symbol,
                ) else {
                    continue;
                };

                if !callee.has_service_reach
                    && !callee.may_suspend
                    && !callee.may_block
                    && !callee.has_mutable_parameter
                {
                    diagnostics.push(Diagnostic::warning(format!(
                        "`_ = {target}(...);` discards a provably pure call: `{target}` has no service reach, suspension, blocking, or mutable out-parameters, so the discard is dead code; remove the statement or use the result",
                        target = call.target.as_str()
                    )));
                }
            }
        }
    }
}

struct DiscardCallee {
    has_service_reach: bool,
    may_suspend: bool,
    may_block: bool,
    has_mutable_parameter: bool,
}

/// The machine whose states include the called state -- the discard rules
/// reason about the CALLEE MACHINE (purity surface, proof classification),
/// while the call site records only the state symbol.
fn machine_owning_state(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&psi_typed_trees::machine::Machine> {
    if !target_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_symbol)
    })
}

fn resolve_discard_callee(
    program: &TypedTrees,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
    target_symbol: SymbolHandle,
) -> Option<DiscardCallee> {
    if !target_symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
        else {
            continue;
        };

        // The callee machine's transitive set is the conservative surface: a
        // called state may transition through sibling states, so anything the
        // machine can reach counts against purity.
        let operational_may = operational
            .machines()
            .iter()
            .find(|entry| entry.symbol == machine.symbol)
            .map(|entry| (entry.transitive_may_suspend, entry.transitive_may_block))?;

        return Some(DiscardCallee {
            has_service_reach: service_reaches
                .for_machine(machine.symbol)
                .is_some_and(|summary| {
                    !service_reaches
                        .services(summary.inferred_transitive)
                        .is_empty()
                }),
            may_suspend: operational_may.0,
            may_block: operational_may.1,
            has_mutable_parameter: program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_mutable),
        });
    }

    for trait_definition in program.traits() {
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
        {
            return Some(signature_discard_callee(
                program,
                trait_definition,
                signature,
            ));
        }
    }

    None
}

/// Signatures have no body to traverse. Their normalized service row and, for
/// a boundary trait, the enclosing service identity still make the call
/// observable without consulting a lowercase compatibility list.
fn signature_discard_callee(
    program: &TypedTrees,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    signature: &psi_typed_trees::signature::StateSignature,
) -> DiscardCallee {
    DiscardCallee {
        has_service_reach: trait_definition.is_boundary
            || !program
                .service_reach_rows
                .services(signature.service_reach_row)
                .is_empty(),
        may_suspend: signature.suspends,
        may_block: signature.blocks,
        has_mutable_parameter: program
            .state_signature_parameters(signature)
            .iter()
            .any(|parameter| parameter.is_mutable),
    }
}
