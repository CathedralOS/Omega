//! Contract composition for the three exact terminal call policies.

use std::collections::BTreeMap;

use psi_core::{MachineId, Proposition, ScalarTerm, ScalarType, StructuralCaseSubject, ValueId};
use psi_proof_admission::{Obligation, ObligationClass};
use psi_terminal::{Operation, OperationKind, TerminalMachine, TerminalModule};
use psi_terminal_semantics::{CallResultRule, call_composition_semantic_row};

use crate::ModuleError;

use super::reconstruction::{
    ReconstructedOperationObligation, ReconstructedTerminalObligationOwner,
};
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
            for (requirement_position, (required, obligation)) in callee
                .contract
                .requires
                .iter()
                .zip(requirement_obligations)
                .enumerate()
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    owner: ReconstructedTerminalObligationOwner::CallRequires {
                        machine: machine.id,
                        operation: operation.id,
                        requirement_position: u32::try_from(requirement_position)
                            .expect("validated call requirement position fits u32"),
                    },
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
            for (requirement_position, (required, obligation)) in callee
                .contract
                .requires
                .iter()
                .zip(requirement_obligations)
                .enumerate()
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    owner: ReconstructedTerminalObligationOwner::CallRequires {
                        machine: machine.id,
                        operation: operation.id,
                        requirement_position: u32::try_from(requirement_position)
                            .expect("validated call requirement position fits u32"),
                    },
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
                arguments,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated structural scalar-call target exists");
            compose_structural_scalar_call(
                module,
                machine,
                operation,
                callee,
                arguments,
                structural_arguments,
                requirement_obligations,
                value_types,
                axioms,
                operation_obligations,
            );
        }
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::CallDynamicScalar {
                descriptor_ordinal,
                requirement_obligations,
                ..
            },
        ) => {
            let dispatch = module
                .dynamic_dispatch
                .indirect_dispatches
                .iter()
                .find(|dispatch| {
                    dispatch.owner == machine.id
                        && dispatch.operation == operation.id
                        && dispatch.descriptor_ordinal == *descriptor_ordinal
                })
                .expect("validated dynamic call has one indirect dispatch row");
            let descriptor = module
                .dynamic_dispatch
                .rebound_descriptors
                .iter()
                .find(|descriptor| {
                    descriptor.owner == machine.id && descriptor.ordinal == *descriptor_ordinal
                })
                .expect("validated dynamic call has one descriptor row");
            let selection = module
                .dynamic_dispatch
                .selections
                .iter()
                .find(|selection| {
                    selection.owner == machine.id
                        && selection.ordinal == descriptor.rebound_selection_ordinal
                })
                .expect("validated dynamic descriptor has one latest selection");
            let callee = machines
                .get(&dispatch.realization)
                .copied()
                .expect("validated dynamic realization exists");
            compose_structural_scalar_call(
                module,
                machine,
                operation,
                callee,
                &[],
                std::slice::from_ref(&selection.source),
                requirement_obligations,
                value_types,
                axioms,
                operation_obligations,
            );
        }
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::CallDynamicParameterScalar {
                requirement_obligations,
                crash_continuations,
                ..
            },
        ) => {
            debug_assert!(requirement_obligations.is_empty());
            debug_assert!(crash_continuations.is_empty());
            // The parameter interface fixes the result type and table slot,
            // but no concrete realization is selected until invocation.
            // V1 therefore imports no realization-specific contract axioms.
        }
        (
            CallResultRule::StructuralCalleeResult,
            OperationKind::CallStructuralWithScalarArguments {
                requirement_obligations,
                crash_continuations,
                ..
            },
        ) => {
            debug_assert!(requirement_obligations.is_empty());
            debug_assert!(crash_continuations.is_empty());
            // The first mixed structural-result lane admits only an empty
            // callee contract, so there are no propositions to substitute or
            // import. Scalar and structural ABI custody remains explicit in
            // the operation and is validated independently.
        }
        (
            CallResultRule::StructuralCalleeResult,
            OperationKind::CallStructural {
                callee,
                structural_arguments,
                requirement_obligations,
                selected_evidence,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated structural-call target exists");
            let call_result = operation
                .result
                .structural()
                .expect("validated structural call has a structural result");
            let callee_result = callee
                .result
                .structural()
                .expect("validated structural-call target has a structural result");
            let mut substitutions = callee
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
            substitutions.insert(callee_result.place, (call_result.place, Vec::new()));
            for (requirement_position, (required, obligation)) in callee
                .contract
                .requires
                .iter()
                .zip(requirement_obligations)
                .enumerate()
            {
                operation_obligations.push(ReconstructedOperationObligation {
                    owner: ReconstructedTerminalObligationOwner::CallRequires {
                        machine: machine.id,
                        operation: operation.id,
                        requirement_position: u32::try_from(requirement_position)
                            .expect("validated call requirement position fits u32"),
                    },
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
            for guarantee in &callee.contract.outcome_specific_ensures {
                let proposition = selected_evidence
                    .iter()
                    .find(|binding| {
                        binding.guard == guarantee.guard
                            && binding.position == guarantee.position
                            && guarantee.proposition
                                == Proposition::Atom(binding.callee_proposition)
                    })
                    .map(|binding| Proposition::Atom(binding.instantiated_proposition))
                    .unwrap_or_else(|| {
                        substitute_proposition_structural_places(
                            &guarantee.proposition,
                            &substitutions,
                        )
                    });
                push_unique(
                    axioms,
                    Proposition::Implication {
                        premise: Box::new(Proposition::StructuralCaseMembership {
                            subject: StructuralCaseSubject::new(call_result.place, Vec::new()),
                            case: guarantee.guard.result_case,
                        }),
                        conclusion: Box::new(proposition),
                    },
                );
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

#[allow(clippy::too_many_arguments)]
fn compose_structural_scalar_call(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    callee: &TerminalMachine,
    arguments: &[ValueId],
    structural_arguments: &[psi_terminal::StructuralArgument],
    requirement_obligations: &[psi_core::ObligationId],
    value_types: &BTreeMap<ValueId, ScalarType>,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) {
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
    let mut value_substitutions = callee
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
        .collect::<BTreeMap<_, _>>();
    value_substitutions.insert(
        callee
            .result
            .scalar()
            .expect("validated structural scalar-call target has a scalar result")
            .id,
        value_term(operation.result.expect_scalar().id, value_types),
    );
    let substitute = |proposition: &Proposition| {
        substitute_proposition_values(
            &substitute_proposition_structural_places(proposition, &structural_substitutions),
            &value_substitutions,
        )
    };
    for (requirement_position, (required, obligation)) in callee
        .contract
        .requires
        .iter()
        .zip(requirement_obligations)
        .enumerate()
    {
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::CallRequires {
                machine: machine.id,
                operation: operation.id,
                requirement_position: u32::try_from(requirement_position)
                    .expect("validated call requirement position fits u32"),
            },
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
