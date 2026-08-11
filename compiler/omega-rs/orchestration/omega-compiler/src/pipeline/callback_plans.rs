//! Omega-owned plans for contextual static callback selection.
//!
//! Psi proves that a static machine argument refines its authored callable
//! contract. This pass adds the native-only fact that one such argument names
//! an explicitly satisfied boundary requirement with an evaluated inbound
//! calling plan. It deliberately retains symbols and plans, never a numeric
//! entry address.

use crate::pipeline::calling_policy_plans::BoundaryCallingPlanRealization;
use omega_calling_conventions::BoundaryEntryPlan;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCallbackBindingPlan {
    pub registration_requirement: SymbolHandle,
    pub callback_trait: SymbolHandle,
    pub callback_requirement: SymbolHandle,
    pub callback_machine: SymbolHandle,
    pub callback_entry: SymbolHandle,
    pub calling_plan_fingerprint: u64,
    pub specialization_fingerprint: u64,
    pub boundary_entry_plan: BoundaryEntryPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticCallbackBindingPlanSet {
    bindings: Vec<StaticCallbackBindingPlan>,
}

impl StaticCallbackBindingPlanSet {
    pub fn bindings(&self) -> &[StaticCallbackBindingPlan] {
        &self.bindings
    }
}

#[derive(Debug, Clone, Copy)]
struct CallbackCandidate<'plan> {
    callback_trait: SymbolHandle,
    callback_requirement: SymbolHandle,
    callback_machine: SymbolHandle,
    callback_entry: SymbolHandle,
    realization: &'plan BoundaryCallingPlanRealization,
}

pub(super) fn elaborate_static_callback_binding_plans(
    program: &CheckedTrees,
    realizations: &[BoundaryCallingPlanRealization],
) -> Result<StaticCallbackBindingPlanSet, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    let mut diagnostics = Vec::new();

    for (_, expression) in program.expression_table.iter_expressions() {
        let psi_checked_trees::expression::ExpressionNode::Call(call) = expression else {
            continue;
        };
        append_call_binding(
            program,
            realizations,
            call.target_symbol,
            call.target.as_str(),
            &call.machine_arguments,
            &mut bindings,
            &mut diagnostics,
        );
    }
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let psi_checked_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                append_call_binding(
                    program,
                    realizations,
                    call.target_symbol,
                    call.target.as_str(),
                    &call.machine_arguments,
                    &mut bindings,
                    &mut diagnostics,
                );
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    bindings.sort_by_key(|binding| binding.specialization_fingerprint);
    bindings.dedup();
    Ok(StaticCallbackBindingPlanSet { bindings })
}

fn append_call_binding(
    program: &CheckedTrees,
    realizations: &[BoundaryCallingPlanRealization],
    registration_requirement: SymbolHandle,
    target_name: &str,
    machine_arguments: &[psi_checked_trees::expression::StaticMachineArgument],
    bindings: &mut Vec<StaticCallbackBindingPlan>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if machine_arguments.is_empty() {
        return;
    }
    let Some((registration_trait, registration_signature)) =
        program.traits().iter().find_map(|definition| {
            definition.is_boundary.then(|| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == registration_requirement)
                    .map(|signature| (definition, signature))
            })?
        })
    else {
        return;
    };

    let mut candidates = Vec::new();
    for argument in machine_arguments {
        let Some((callback_machine, callback_entry)) =
            program.machines().iter().find_map(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == argument.symbol)
                    .map(|entry| (machine, entry))
            })
        else {
            // A generic wrapper still carrying its own machine parameter is
            // not a concrete callback selection. Its concrete specialization
            // contributes the eventual binding row.
            continue;
        };
        if callback_machine.supply_mode != psi_language_semantics::MachineSupplyMode::Boundary {
            continue;
        }

        let mut argument_candidates = Vec::new();
        for conformance in program.machine_trait_conformances(callback_machine) {
            let Some(requirement_name) = conformance.requirement.as_ref() else {
                continue;
            };
            let Some(callback_trait) = program.traits().iter().find(|definition| {
                definition.is_boundary && definition.symbol == conformance.symbol
            }) else {
                continue;
            };
            let requirement_leaf = requirement_name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(requirement_name.as_str());
            let matching_requirements = program
                .trait_machine_signatures(callback_trait)
                .iter()
                .filter(|signature| signature.name.as_str() == requirement_leaf)
                .collect::<Vec<_>>();
            let conformance_arguments = program
                .type_reference_table
                .type_reference_handles(conformance.arguments);
            for callback_requirement in matching_requirements {
                for realization in realizations.iter().filter(|realization| {
                    realization.boundary_trait == callback_trait.symbol
                        && realization.boundary_arguments == conformance_arguments
                        && realization.requirement_machine == callback_requirement.symbol
                }) {
                    argument_candidates.push(CallbackCandidate {
                        callback_trait: callback_trait.symbol,
                        callback_requirement: callback_requirement.symbol,
                        callback_machine: callback_machine.symbol,
                        callback_entry: callback_entry.symbol,
                        realization,
                    });
                }
            }
        }

        match argument_candidates.as_slice() {
            [] => {}
            [candidate] => candidates.push(*candidate),
            ambiguous => diagnostics.push(Diagnostic::error(format!(
                "boundary operation `{target_name}` selects static boundary machine `{}` with {} evaluated callback conformances; callback selection must name one exact requirement",
                callback_machine.name,
                ambiguous.len(),
            ))),
        }
    }

    let [candidate] = candidates.as_slice() else {
        if candidates.len() > 1 {
            diagnostics.push(Diagnostic::error(format!(
                "boundary operation `{target_name}` selects {} static callback machines; the current registered-callback lowering requires exactly one",
                candidates.len(),
            )));
        }
        return;
    };

    let registration_identity = program
        .normalized_trait_requirement_overload_identity(registration_trait, registration_signature)
        .identity();
    let callback_identity = program.traits().iter().find_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == candidate.callback_requirement)
            .map(|signature| {
                program
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity()
                    .to_owned()
            })
    });
    let callback_machine_identity = program.machines().iter().find_map(|machine| {
        (machine.symbol == candidate.callback_machine)
            .then(|| program.normalized_machine_overload_identity(machine))?
            .map(|identity| identity.identity().to_owned())
    });
    let (Some(callback_identity), Some(callback_machine_identity)) =
        (callback_identity, callback_machine_identity)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "boundary operation `{target_name}` selected a callback without a normalized requirement or machine identity",
        )));
        return;
    };

    let mut hash = StableHash::new();
    hash.string("static-callback-binding-v1");
    hash.string(registration_identity.as_str());
    hash.string(&callback_identity);
    hash.string(&callback_machine_identity);
    hash.u64(candidate.realization.fingerprint);
    let binding = StaticCallbackBindingPlan {
        registration_requirement,
        callback_trait: candidate.callback_trait,
        callback_requirement: candidate.callback_requirement,
        callback_machine: candidate.callback_machine,
        callback_entry: candidate.callback_entry,
        calling_plan_fingerprint: candidate.realization.fingerprint,
        specialization_fingerprint: hash.finish(),
        boundary_entry_plan: candidate.realization.boundary_entry_plan.clone(),
    };
    if !bindings.contains(&binding) {
        bindings.push(binding);
    }
}

struct StableHash(u64);

impl StableHash {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn string(&mut self, value: &str) {
        for byte in value.as_bytes() {
            self.byte(*byte);
        }
        self.byte(0);
    }

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn finish(self) -> u64 {
        self.0.max(1)
    }
}
