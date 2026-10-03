//! Contract composition for the three exact terminal call policies.
//!
//! Requires use the caller's pre-call facts. Before importing ensures, expire
//! observations of the validated argument's writable subtree, using the same
//! canonical path overlap as field stores. No knowledge of a callee's body or
//! a producer-supplied write summary can preserve facts inside that subtree.
//! Dynamic selectors widen to their containing subtree; unknown paths and
//! reference-bearing transfers keep the existing conservative alias fences.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId,
    Proposition, ScalarTerm, ScalarType, StructuralCaseSubject, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, Operation, OperationKind, StructuralAccess, StructuralArgument,
    StructuralFieldType, StructuralTypeShape, TerminalMachine, TerminalModule,
};
use terminal_semantics::{
    CallResultRule, SharedMutation, call_composition_semantic_row, shared_mutation,
};

use crate::ModuleError;

use super::reconstruction::{
    ReconstructedOperationObligation, ReconstructedTerminalObligationOwner,
};
use super::substitution::{
    substitute_proposition_structural_places, substitute_proposition_values,
};

#[cfg(test)]
mod tests;

pub(super) fn compose_call_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    storage: &super::storage_observations::StorageObservations<'_>,
    capture_snapshots: bool,
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
                erased_arguments,
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
            for (formal, argument) in callee
                .contract
                .erased_scalar_formals
                .iter()
                .zip(erased_arguments)
            {
                substitutions.insert(formal.id, argument.clone());
            }
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
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                &[],
                storage,
                capture_snapshots,
            );
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
                erased_arguments,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            let callee = machines
                .get(callee)
                .copied()
                .expect("validated unit-call target exists");
            let structural_substitutions = structural_contract_substitutions(
                module,
                machine,
                callee,
                structural_arguments,
                axioms,
            )?;
            let mut value_substitutions = callee
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                .collect::<BTreeMap<_, _>>();
            for (formal, argument) in callee
                .contract
                .erased_scalar_formals
                .iter()
                .zip(erased_arguments)
            {
                value_substitutions.insert(formal.id, argument.clone());
            }
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
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                structural_arguments,
                storage,
                capture_snapshots,
            );
            for guarantee in &callee.contract.ensures {
                push_unique(axioms, substitute(&guarantee.proposition));
            }
        }
        (
            CallResultRule::ScalarCalleeResult,
            OperationKind::CallStructuralScalar {
                callee,
                arguments,
                erased_arguments,
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
                erased_arguments,
                structural_arguments,
                requirement_obligations,
                value_types,
                storage,
                capture_snapshots,
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
            let (realization, source) =
                selected_dynamic_call(module, machine, operation, *descriptor_ordinal);
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
                &[],
                std::slice::from_ref(source),
                requirement_obligations,
                value_types,
                storage,
                capture_snapshots,
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
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                &[],
                storage,
                capture_snapshots,
            );
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
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                &[],
                storage,
                capture_snapshots,
            );
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
            let erased_arguments: &[ScalarTerm] = match &operation.kind {
                OperationKind::CallStructuralWithScalarArguments {
                    erased_arguments, ..
                } => erased_arguments.as_slice(),
                _ => &[],
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
            let mut substitutions = structural_contract_substitutions(
                module,
                machine,
                callee,
                structural_arguments,
                axioms,
            )?;
            substitutions.insert(callee_result.place, (call_result.place, Vec::new()));
            let mut scalar_substitutions = callee
                .parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                .collect::<BTreeMap<_, _>>();
            scalar_substitutions.extend(
                callee
                    .contract
                    .erased_scalar_formals
                    .iter()
                    .zip(erased_arguments)
                    .map(|(formal, argument)| (formal.id, argument.clone())),
            );
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
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                structural_arguments,
                storage,
                capture_snapshots,
            );
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
                boundary,
                arguments,
                structural_arguments,
                requirement_obligations,
                ..
            },
        ) => {
            // The declaration's scalar `requires` rows live in its positional
            // formal telescope; one simultaneous substitution of this call's
            // actuals yields each obligation, proven from the pre-call facts
            // exactly as an ordinary callee's requires are.
            let boundary = module
                .boundary_machines
                .iter()
                .find(|candidate| candidate.id == *boundary)
                .expect("validated boundary call target exists");
            if !boundary.scalar_requires.is_empty() {
                let substitutions = boundary
                    .scalar_contract_parameters()
                    .ok_or(ModuleError::InvalidBoundaryCrashParameters(boundary.id))?
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
                    .collect::<BTreeMap<_, _>>();
                for (requirement_position, (required, obligation)) in boundary
                    .scalar_requires
                    .iter()
                    .zip(requirement_obligations)
                    .enumerate()
                {
                    operation_obligations.push(ReconstructedOperationObligation {
                        owner: ReconstructedTerminalObligationOwner::CallRequires {
                            machine: machine.id,
                            operation: operation.id,
                            requirement_position: u32::try_from(requirement_position)
                                .expect("validated boundary requirement position fits u32"),
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
            }
            invalidate_mutated_arguments(
                module,
                machine,
                operation,
                axioms,
                structural_arguments,
                storage,
                capture_snapshots,
            );
            if !boundary.scalar_ensures.is_empty() {
                let mut substitutions = boundary
                    .scalar_contract_parameters()
                    .ok_or(ModuleError::InvalidBoundaryCrashParameters(boundary.id))?
                    .iter()
                    .zip(arguments)
                    .map(|(formal, actual)| (formal.id, value_term(*actual, value_types)))
                    .collect::<BTreeMap<_, _>>();
                if let Some(formal) = boundary.scalar_contract_result()
                    && let Some(result) = operation.result.scalar_ref()
                {
                    substitutions.insert(formal.id, value_term(result.id, value_types));
                }
                axioms.extend(
                    boundary
                        .scalar_ensures
                        .iter()
                        .map(|row| substitute_proposition_values(row, &substitutions)),
                );
            }
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
    erased_arguments: &[ScalarTerm],
    structural_arguments: &[terminal_psi::StructuralArgument],
    requirement_obligations: &[semantic_vocabulary::ObligationId],
    value_types: &BTreeMap<ValueId, ScalarType>,
    storage: &super::storage_observations::StorageObservations<'_>,
    capture_snapshots: bool,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    let structural_substitutions =
        structural_contract_substitutions(module, machine, callee, structural_arguments, axioms)?;
    let mut value_substitutions = callee
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.id, value_term(*argument, value_types)))
        .collect::<BTreeMap<_, _>>();
    value_substitutions.extend(
        callee
            .contract
            .erased_scalar_formals
            .iter()
            .zip(erased_arguments)
            .map(|(formal, argument)| (formal.id, argument.clone())),
    );
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
    invalidate_mutated_arguments(
        module,
        machine,
        operation,
        axioms,
        structural_arguments,
        storage,
        capture_snapshots,
    );
    for guarantee in &callee.contract.ensures {
        push_unique(axioms, substitute(&guarantee.proposition));
    }
    Ok(())
}

/// The structural type reached by a canonical argument prefix. `Field` steps
/// resolve inside records, `FixedIndex` steps inside a fixed array's extent,
/// and `Case` steps supply no owned structural namespace here.
fn canonical_path_end_type(
    module: &TerminalModule,
    mut current: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        current = match (segment, &declaration.shape) {
            (
                CanonicalStructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. },
            ) => {
                let field = fields.iter().find(|field| field.id == *identity)?;
                if field.relevance != BindingRelevance::Relevant {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (
                CanonicalStructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(current)
}

/// A `ViewExtent` term whose root and path name a fixed array has a declared
/// static extent: supply it as a caller-side fact so a callee's substituted
/// requires discharges against the argument's real storage.
fn mint_static_extent_axiom(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
    prefix: &[CanonicalStructuralPathSegment],
    axioms: &mut Vec<Proposition>,
) {
    let Some(root) = super::storage_observations::place_root_type(caller, argument.place) else {
        return;
    };
    let Some(end) = canonical_path_end_type(module, root, prefix) else {
        return;
    };
    let Some(length) =
        module
            .structural_types
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::FixedArray { length, .. } if declaration.id == end => {
                    Some(length)
                }
                _ => None,
            })
    else {
        return;
    };
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let extent = ScalarTerm::view_extent(argument.place, prefix.to_vec(), integer_type);
    let Ok(count) = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(u128::from(length)))
    else {
        return;
    };
    push_unique(axioms, Proposition::Equal(extent, count));
}

/// Borrowed referents and fixed byte windows have no owned-field spelling in
/// the proposition algebra. A window must never substitute its entire backing.
/// Omit an unused binder only after checking the complete published contract;
/// a contract that actually needs the missing projection remains unsupported.
fn structural_contract_substitutions(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    axioms: &mut Vec<Proposition>,
) -> Result<BTreeMap<PlaceId, (PlaceId, Vec<CanonicalStructuralPathSegment>)>, ModuleError> {
    let mut substitutions = BTreeMap::new();
    for (parameter, argument) in callee.structural_parameters.iter().zip(arguments) {
        if let Some(prefix) =
            crate::validation::structural_argument_canonical_prefix(module, caller, argument)
        {
            mint_static_extent_axiom(module, caller, argument, &prefix, axioms);
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
        let borrowed_presentation = caller.structural_parameters.iter().any(|actual| {
            terminal_semantics::fixed_byte_array_window(
                module.structural_types.iter(),
                actual,
                argument,
                parameter,
            )
            .is_some()
        }) || crate::validation::structural::operations::is_record_loan(
            module, caller, parameter, argument,
        );
        // Completed local backing has the same lent presentation as a
        // parameter. Reuse call validation's geometry rather than inventing
        // an owned-field prefix for a window. An observed binder still fails
        // below: omission cannot discharge a callee's place contract.
        // A runtime-indexed borrow likewise has no canonical owned-field
        // prefix: it lends the dynamically selected element, and an unobserved
        // binder is omitted rather than substituted with the whole backing.
        // The exclusive spelling keeps the same dynamic element loan.
        let runtime_indexed_borrow =
            matches!(
                argument.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && crate::validation::is_runtime_indexed_borrow_path(&argument.path);
        // The same omission applies to a literal-indexed borrow rooted at a
        // live element-view binding: the descriptor's stored extent means the
        // FixedIndex hop has no canonical FixedArray spelling either. Runtime
        // index hops into a view already ride runtime_indexed_borrow above.
        let element_view_borrow = !argument.path.is_empty()
            && argument.access != terminal_psi::StructuralAccess::Owned
            && crate::validation::is_static_borrow_path(&argument.path)
            && crate::validation::element_view::view_loan_result_in_module(
                module,
                caller,
                argument.place,
            )
            .is_some();
        // Static stored-view fields were substituted above. A remaining
        // noncanonical path (for example an indexed view parent) may still
        // lend a valid descriptor, but cannot rename an observed binder to
        // its whole backing. Only an unobserved binder can be omitted.
        let stored_view_field =
            super::storage_observations::place_root_type(caller, argument.place).is_some_and(
                |root| {
                    terminal_semantics::stored_view_field_presentation(
                        module.structural_types.iter(),
                        root,
                        argument,
                        parameter,
                    )
                    .is_some()
                },
            );
        if observed
            || (!crate::validation::is_reference_projection(module, caller, argument)
                && !borrowed_presentation
                && !runtime_indexed_borrow
                && !element_view_borrow
                && !stored_view_field)
        {
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
    super::substitution::push_unique_fact(propositions, proposition);
}

/// Reuse the independently validated catalog join for both Unit and scalar
/// dispatch. The latest selected source, not an initial descriptor value,
/// carries the receiver's writable storage permission.
fn selected_dynamic_call<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    descriptor_ordinal: u32,
) -> (MachineId, &'a StructuralArgument) {
    let indirect = module
        .dynamic_dispatch
        .indirect_dispatches
        .iter()
        .find(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.descriptor_ordinal == descriptor_ordinal
        });
    let stored = module
        .dynamic_dispatch
        .stored_dispatches
        .iter()
        .find(|dispatch| {
            dispatch.owner == machine.id
                && dispatch.operation == operation.id
                && dispatch.descriptor_ordinal == descriptor_ordinal
        });
    let (realization, selection_ordinal) = match (indirect, stored) {
        (Some(dispatch), None) => {
            let descriptor = module
                .dynamic_dispatch
                .rebound_descriptors
                .iter()
                .find(|descriptor| {
                    descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
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
                    descriptor.owner == machine.id && descriptor.ordinal == descriptor_ordinal
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
        .find(|selection| selection.owner == machine.id && selection.ordinal == selection_ordinal)
        .expect("validated dynamic descriptor has one selected source");
    (realization, &selection.source)
}

/// Requirements use the pre-call state; only guarantees may describe a
/// mutable argument after completion. The validated argument path bounds the
/// callee's writable subtree without assuming a callee write-frame summary.
fn invalidate_mutated_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    axioms: &mut Vec<Proposition>,
    arguments: &[StructuralArgument],
    storage: &super::storage_observations::StorageObservations<'_>,
    capture_snapshots: bool,
) {
    // Descriptor parameters erase their concrete receiver type and backing.
    // Shared interfaces can reach atomics, but never authorize ordinary writes.
    let parameter_access = |ordinal| {
        module
            .dynamic_dispatch
            .parameters
            .iter()
            .find(|parameter| parameter.owner == machine.id && parameter.ordinal == ordinal)
            .expect("validated dynamic parameter exists")
            .access
    };
    let selected_source = |ordinal| {
        &module
            .dynamic_dispatch
            .selections
            .iter()
            .find(|selection| selection.owner == machine.id && selection.ordinal == ordinal)
            .expect("validated dynamic source selection exists")
            .source
    };
    let mut arguments = arguments.iter().collect::<Vec<_>>();
    let mut unknown_write = false;
    let mut unknown_shared_write = false;
    match &operation.kind {
        OperationKind::CallDynamicScalar {
            descriptor_ordinal, ..
        }
        | OperationKind::CallDynamicUnit {
            descriptor_ordinal, ..
        } => {
            let (_, source) =
                selected_dynamic_call(module, machine, operation, *descriptor_ordinal);
            if !arguments.contains(&source) {
                arguments.push(source);
            }
        }
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal, ..
        }
        | OperationKind::CallDynamicParameterUnit {
            parameter_ordinal, ..
        } => {
            let access = parameter_access(*parameter_ordinal);
            unknown_write |= access != StructuralAccess::SharedBorrow;
            unknown_shared_write |= access == StructuralAccess::SharedBorrow;
        }
        _ => {}
    }
    // Ordinary calls can forward a descriptor without a structural argument.
    // Apply the same fence to that authenticated hidden receiver channel.
    for argument in module
        .dynamic_dispatch
        .arguments
        .iter()
        .filter(|argument| argument.owner == machine.id && argument.operation == operation.id)
    {
        match argument.source {
            terminal_psi::TerminalDynamicDescriptorSource::Selection { ordinal } => {
                arguments.push(selected_source(ordinal));
            }
            terminal_psi::TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
                let descriptor = module
                    .dynamic_dispatch
                    .rebound_descriptors
                    .iter()
                    .find(|descriptor| {
                        descriptor.owner == machine.id && descriptor.ordinal == ordinal
                    })
                    .expect("validated forwarded rebound descriptor exists");
                arguments.push(selected_source(descriptor.rebound_selection_ordinal));
            }
            terminal_psi::TerminalDynamicDescriptorSource::Parameter { ordinal } => {
                let access = parameter_access(ordinal);
                unknown_write |= access != StructuralAccess::SharedBorrow;
                unknown_shared_write |= access == StructuralAccess::SharedBorrow;
            }
        }
    }
    let mut written = arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.access,
                StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
                    | StructuralAccess::SharedBorrow
            )
        })
        .filter_map(|argument| {
            // A dynamic element or byte window may write anywhere under its
            // containing carrier, but cannot reach that carrier's siblings.
            let prefix_end = argument
                .path
                .iter()
                .position(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
                            | terminal_psi::StructuralPathSegment::FixedByteRange { .. }
                            | terminal_psi::StructuralPathSegment::Referent
                    )
                })
                .unwrap_or(argument.path.len());
            let path = crate::validation::canonical_field_path(
                module,
                machine,
                argument.place,
                &argument.path[..prefix_end],
            );
            let shared = argument.access == StructuralAccess::SharedBorrow;
            if shared {
                let mutation =
                    super::storage_observations::place_root_type(machine, argument.place)
                        .zip(path.as_deref())
                        .map(|(root, path)| {
                            shared_mutation(module.structural_types.iter(), root, path)
                        })
                        .unwrap_or(SharedMutation::Unbounded);
                // Two incoming shared roots may denote the same cell. Their
                // distinct PlaceIds are not alias-disjoint evidence. Expire
                // only unstable observations, never ordinary field siblings.
                unknown_shared_write |= mutation != SharedMutation::None;
                return None;
            }
            Some((argument.place, path))
        })
        .collect::<Vec<_>>();
    if unknown_write
        || arguments.iter().any(|argument| {
            crate::validation::argument_may_write_unbounded_storage(module, machine, argument)
        })
    {
        // The same current-call custody judgment used for descriptor loans
        // determines whether the selected slot bounds all writable backing.
        // Unknown nested descriptors/references cannot establish disjointness
        // from another observed root. Scalar snapshots remain immutable.
        written.extend(
            machine
                .structural_places
                .iter()
                .map(|place| (place.id, None)),
        );
    }
    // An owned argument may be changed by its recipient too. Forget current
    // field observations without erasing separately versioned content evidence.
    let consumed = arguments
        .iter()
        .filter(|argument| argument.access == StructuralAccess::Owned)
        .map(|argument| argument.place)
        .collect::<Vec<_>>();
    if !written.is_empty() || !consumed.is_empty() || unknown_shared_write {
        super::field_snapshots::retain(axioms, capture_snapshots, |proposition| {
            let observes_shared_mutation = storage.observes_shared_mutation(proposition);
            !(unknown_shared_write && observes_shared_mutation)
                && !written.iter().any(|(root, path)| match path {
                    Some(path) => storage.observes_write(proposition, *root, path),
                    None => storage.observes_places(proposition, &[*root]),
                })
                && (consumed.is_empty()
                    || !storage.observes_unversioned_places(proposition, &consumed))
        });
    }
}
