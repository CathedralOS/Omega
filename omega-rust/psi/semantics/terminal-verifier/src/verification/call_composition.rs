//! Contract composition for the three exact terminal call policies.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, MachineId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralCaseSubject, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralArgument, TerminalMachine, TerminalModule,
};
use terminal_semantics::{CallResultRule, call_composition_semantic_row};

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
                arguments,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated unit-call target exists");
            let structural_substitutions =
                structural_contract_substitutions(module, machine, callee, structural_arguments)?;
            let value_substitutions = callee
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                .collect::<BTreeMap<_, _>>();
            let substitute = |proposition: &Proposition| {
                substitute_proposition_values(
                    &substitute_proposition_structural_places(
                        proposition,
                        &structural_substitutions,
                    ),
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
            invalidate_mutated_arguments(machine, axioms, structural_arguments);
            for guarantee in &callee.contract.ensures {
                push_unique(axioms, substitute(&guarantee.proposition));
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
            )?;
        }
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::CallDynamicScalar {
                descriptor_ordinal,
                requirement_obligations,
                ..
            },
        ) => {
            let indirect = module
                .dynamic_dispatch
                .indirect_dispatches
                .iter()
                .find(|dispatch| {
                    dispatch.owner == machine.id
                        && dispatch.operation == operation.id
                        && dispatch.descriptor_ordinal == *descriptor_ordinal
                });
            let stored = module
                .dynamic_dispatch
                .stored_dispatches
                .iter()
                .find(|dispatch| {
                    dispatch.owner == machine.id
                        && dispatch.operation == operation.id
                        && dispatch.descriptor_ordinal == *descriptor_ordinal
                });
            let (realization, selection_ordinal) = match (indirect, stored) {
                (Some(dispatch), None) => {
                    let descriptor = module
                        .dynamic_dispatch
                        .rebound_descriptors
                        .iter()
                        .find(|descriptor| {
                            descriptor.owner == machine.id
                                && descriptor.ordinal == *descriptor_ordinal
                        })
                        .expect("validated dynamic call has one rebound descriptor row");
                    (dispatch.realization, descriptor.rebound_selection_ordinal)
                }
                (None, Some(dispatch)) => {
                    let descriptor = module
                        .dynamic_dispatch
                        .stored_descriptors
                        .iter()
                        .find(|descriptor| {
                            descriptor.owner == machine.id
                                && descriptor.ordinal == *descriptor_ordinal
                        })
                        .expect("validated dynamic call has one stored descriptor row");
                    (dispatch.realization, descriptor.selection_ordinal)
                }
                _ => unreachable!("validated dynamic call has one dispatch lane"),
            };
            let selection = module
                .dynamic_dispatch
                .selections
                .iter()
                .find(|selection| {
                    selection.owner == machine.id && selection.ordinal == selection_ordinal
                })
                .expect("validated dynamic descriptor has one selected source");
            let callee = machines
                .get(&realization)
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
            )?;
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
            CallResultRule::UnitCalleeResult,
            OperationKind::CallDynamicUnit {
                requirement_obligations,
                crash_continuations,
                ..
            },
        )
        | (
            CallResultRule::UnitCalleeResult,
            OperationKind::CallDynamicParameterUnit {
                requirement_obligations,
                crash_continuations,
                ..
            },
        ) => {
            debug_assert!(requirement_obligations.is_empty());
            debug_assert!(crash_continuations.is_empty());
            // The admitted Unit descriptor rows have empty contracts. The
            // dynamic catalog independently validates the selected table slot.
        }
        (
            CallResultRule::StructuralCalleeResult,
            OperationKind::CallStructuralWithScalarArguments {
                callee,
                structural_arguments,
                requirement_obligations,
                ..
            },
        )
        | (
            CallResultRule::StructuralCalleeResult,
            OperationKind::CallStructural {
                callee,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let (arguments, selected_evidence) = match &operation.kind {
                OperationKind::CallStructuralWithScalarArguments { arguments, .. } => {
                    (arguments.as_slice(), &[][..])
                }
                OperationKind::CallStructural {
                    selected_evidence, ..
                } => (&[][..], selected_evidence.as_slice()),
                _ => return Err(ModuleError::ScalarCaseResultMismatch(operation.id)),
            };
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
            let mut substitutions =
                structural_contract_substitutions(module, machine, callee, structural_arguments)?;
            substitutions.insert(callee_result.place, (call_result.place, Vec::new()));
            let scalar_substitutions = callee
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                .collect::<BTreeMap<_, _>>();
            let instantiate = |proposition: &Proposition| {
                substitute_proposition_values(
                    &substitute_proposition_structural_places(proposition, &substitutions),
                    &scalar_substitutions,
                )
            };
            // This operation establishes a fresh result. Prior observations
            // of its storage cannot justify this invocation's requirements.
            // Versioned content identities retain their independent meaning.
            axioms.retain(|proposition| {
                !crate::validation::proposition_observes_unversioned_places(
                    proposition,
                    &[call_result.place],
                )
            });
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
                        proposition: instantiate(required),
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: axioms.clone(),
                    canonical_certificate: false,
                });
            }
            invalidate_mutated_arguments(machine, axioms, structural_arguments);
            for guarantee in &callee.contract.ensures {
                push_unique(axioms, instantiate(&guarantee.proposition));
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
                    .unwrap_or_else(|| instantiate(&guarantee.proposition));
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
        (
            CallResultRule::BoundaryDeclaredResult,
            OperationKind::BoundaryCall {
                structural_arguments,
                ..
            },
        ) => {
            invalidate_mutated_arguments(machine, axioms, structural_arguments);
        }
        _ => {
            return Err(ModuleError::OperationSemanticSchema(
                terminal_semantics::OperationSemanticError::CallCompositionSchemaMismatch(
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
    structural_arguments: &[terminal_psi::StructuralArgument],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
    value_types: &BTreeMap<ValueId, ScalarType>,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    let structural_substitutions =
        structural_contract_substitutions(module, machine, callee, structural_arguments)?;
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
    invalidate_mutated_arguments(machine, axioms, structural_arguments);
    for guarantee in &callee.contract.ensures {
        push_unique(axioms, substitute(&guarantee.proposition));
    }
    Ok(())
}

/// Borrowed referents have no owned-field spelling in the proposition algebra.
/// Omit an unused binder only after checking the complete published contract;
/// a contract that actually needs the missing projection remains unsupported.
fn structural_contract_substitutions(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
) -> Result<BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>, ModuleError> {
    let mut substitutions = BTreeMap::new();
    for (parameter, argument) in callee.structural_parameters.iter().zip(arguments) {
        if let Some(prefix) =
            crate::validation::structural_argument_canonical_prefix(module, caller, argument)
        {
            substitutions.insert(parameter.place, (argument.place, prefix));
            continue;
        }
        let observed = callee
            .contract
            .requires
            .iter()
            .chain(
                callee
                    .contract
                    .ensures
                    .iter()
                    .map(|clause| &clause.proposition),
            )
            .chain(
                callee
                    .contract
                    .outcome_specific_ensures
                    .iter()
                    .map(|clause| &clause.proposition),
            )
            .chain(
                callee
                    .contract
                    .crash_routes
                    .iter()
                    .flat_map(|bucket| &bucket.alternatives)
                    .filter_map(|guard| match guard {
                        terminal_psi::CrashRouteGuard::Truth => None,
                        terminal_psi::CrashRouteGuard::Predicate(predicate) => {
                            Some(predicate.proposition())
                        }
                    }),
            )
            .any(|proposition| {
                crate::validation::proposition_observes_places(proposition, &[parameter.place])
            });
        if !crate::validation::is_reference_projection(module, caller, argument) || observed {
            return Err(ModuleError::InvalidReferenceCustody {
                machine: caller.id,
                reason: "callee contract requires an unsupported reference projection",
            });
        }
    }
    Ok(substitutions)
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

/// Requirements use the pre-call state; only guarantees may describe a
/// mutable argument after completion. No callee write-frame summary is assumed.
fn invalidate_mutated_arguments(
    machine: &TerminalMachine,
    axioms: &mut Vec<Proposition>,
    arguments: &[StructuralArgument],
) {
    let mut written = arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
        })
        .map(|argument| argument.place)
        .collect::<Vec<_>>();
    if arguments.iter().any(|argument| {
        argument
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
            && matches!(
                argument.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
    }) {
        // The carrier ID is not its referent. Until this proof consumer uses
        // the reconstructed loan frontier, retain no possibly aliased storage
        // observation. Immutable scalar snapshot equalities remain unchanged.
        written.extend(machine.structural_places.iter().map(|place| place.id));
    }
    // An owned argument may be changed by its recipient too. Forget current
    // field observations without erasing separately versioned content evidence.
    let consumed = arguments
        .iter()
        .filter(|argument| argument.access == StructuralAccess::Owned)
        .map(|argument| argument.place)
        .collect::<Vec<_>>();
    if !written.is_empty() || !consumed.is_empty() {
        axioms.retain(|proposition| {
            (written.is_empty()
                || !crate::validation::proposition_observes_places(proposition, &written))
                && (consumed.is_empty()
                    || !crate::validation::proposition_observes_unversioned_places(
                        proposition,
                        &consumed,
                    ))
        });
    }
}
