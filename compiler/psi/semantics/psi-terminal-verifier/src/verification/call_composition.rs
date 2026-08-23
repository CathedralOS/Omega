//! Contract composition for the three exact terminal call policies.

use std::collections::BTreeMap;

use psi_core::{MachineId, Proposition, ScalarTerm, ScalarType, ValueId};
use psi_proof_kernel::{Obligation, ObligationClass};
use psi_terminal::{Operation, OperationKind, TerminalMachine, TerminalModule};
use psi_terminal_semantics::{CallResultRule, call_composition_semantic_row};

use crate::ModuleError;

use super::reconstruction::ReconstructedOperationObligation;
use super::substitution::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

pub(super) fn compose_call_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<bool, ModuleError> {
    let Some(row) = call_composition_semantic_row(&operation.kind)
        .map_err(ModuleError::OperationSemanticSchema)?
    else {
        return Ok(false);
    };
    match (row.schema().result(), &operation.kind) {
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::Call {
                callee,
                arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated scalar-call target exists");
            let mut substitutions = callee
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                .collect::<BTreeMap<_, _>>();
            substitutions.insert(
                callee
                    .result
                    .scalar()
                    .expect("validated scalar-call target has a scalar result")
                    .id,
                value_term(operation.result.expect_scalar().id, value_types),
            );
            for (required, obligation) in
                callee.contract.requires.iter().zip(requirement_obligations)
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    obligation: Obligation {
                        id: *obligation,
                        proposition: substitute_proposition_values(required, &substitutions),
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: axioms.clone(),
                    canonical_certificate: false,
                });
            }
            for guarantee in &callee.contract.ensures {
                push_unique(
                    axioms,
                    substitute_proposition_values(&guarantee.proposition, &substitutions),
                );
            }
        }
        (
            CallResultRule::UnitCalleeResult,
            OperationKind::CallUnit {
                callee,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated unit-call target exists");
            let substitutions = callee
                .structural_parameters
                .iter()
                .zip(structural_arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.place,
                        (
                            argument.place,
                            crate::validation::structural_argument_canonical_prefix(
                                module, machine, argument,
                            )
                            .expect("validated structural argument has a canonical path"),
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            for (required, obligation) in
                callee.contract.requires.iter().zip(requirement_obligations)
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    obligation: Obligation {
                        id: *obligation,
                        proposition: substitute_proposition_structural_places(
                            required,
                            &substitutions,
                        ),
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: axioms.clone(),
                    canonical_certificate: false,
                });
            }
            for guarantee in &callee.contract.ensures {
                push_unique(
                    axioms,
                    substitute_proposition_structural_places(
                        &guarantee.proposition,
                        &substitutions,
                    ),
                );
            }
        }
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::CallStructuralScalar {
                callee,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated structural scalar-call target exists");
            let structural_substitutions = callee
                .structural_parameters
                .iter()
                .zip(structural_arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.place,
                        (
                            argument.place,
                            crate::validation::structural_argument_canonical_prefix(
                                module, machine, argument,
                            )
                            .expect("validated structural argument has a canonical path"),
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let result_substitution = BTreeMap::from([(
                callee
                    .result
                    .scalar()
                    .expect("validated structural scalar-call target has a scalar result")
                    .id,
                value_term(operation.result.expect_scalar().id, value_types),
            )]);
            let substitute = |proposition: &Proposition| {
                substitute_proposition_values(
                    &substitute_proposition_structural_places(
                        proposition,
                        &structural_substitutions,
                    ),
                    &result_substitution,
                )
            };
            for (required, obligation) in
                callee.contract.requires.iter().zip(requirement_obligations)
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    obligation: Obligation {
                        id: *obligation,
                        proposition: substitute(required),
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: axioms.clone(),
                    canonical_certificate: false,
                });
            }
            for guarantee in &callee.contract.ensures {
                push_unique(axioms, substitute(&guarantee.proposition));
            }
        }
        (CallResultRule::BoundaryDeclaredResult, OperationKind::BoundaryCall { .. }) => {}
        _ => {
            return Err(ModuleError::OperationSemanticSchema(
                psi_terminal_semantics::OperationSemanticError::CallCompositionSchemaMismatch(
                    row.tag(),
                ),
            ));
        }
    }
    Ok(true)
}

fn value_term(value: ValueId, value_types: &BTreeMap<ValueId, ScalarType>) -> ScalarTerm {
    ScalarTerm::value(
        value,
        *value_types
            .get(&value)
            .expect("validated call value has a scalar type"),
    )
}

fn push_unique(propositions: &mut Vec<Proposition>, proposition: Proposition) {
    if !propositions.contains(&proposition) {
        propositions.push(proposition);
    }
}
