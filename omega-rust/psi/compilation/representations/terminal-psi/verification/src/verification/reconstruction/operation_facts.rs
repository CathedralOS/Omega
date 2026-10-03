//! Ordered reconstruction of one terminal operation's facts and obligation.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass, ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{
    BoundedIntegerType, MachineId, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{Operation, OperationKind, TerminalMachine, TerminalModule};
use terminal_semantics::{
    GoalFreeScalarLeafSemantics, goal_free_scalar_leaf_semantics,
    proof_bearing_scalar_leaf_semantics, structural_effect_leaf_observation,
};

use crate::ModuleError;

use super::super::call_composition::compose_call_operation;
use super::super::field_snapshots;
use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner};

mod boolean_polarity;
mod byte_extent;
mod case_hops;
pub(super) use case_hops::validate_successors;
mod element_extent;
mod float_fields;
mod primitive_elements;
mod record;
mod runtime_index;
mod scalar_case;
mod structural_case;

#[derive(Clone, Copy)]
pub(super) enum OperationFactPurpose {
    ProofObligations,
    PrivateCrashPredicates,
}

fn append_scalar_facts(
    semantics: &GoalFreeScalarLeafSemantics,
    value_types: &BTreeMap<ValueId, ScalarType>,
    purpose: OperationFactPurpose,
    axioms: &mut Vec<Proposition>,
) -> Result<(), ModuleError> {
    let implications = match purpose {
        OperationFactPurpose::ProofObligations => {
            boolean_polarity::implications(semantics, value_types)?
        }
        // Private crash checking already converts the original denotation
        // equation. Rebinding its redundant implication projections at every
        // path edge would multiply facts without adding crash-proof authority.
        OperationFactPurpose::PrivateCrashPredicates => Vec::new(),
    };
    axioms.push(semantics.result_equation().clone());
    axioms.extend(implications);
    Ok(())
}

pub(super) fn append_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    context: &super::machine_context::MachineReconstructionContext<'_>,
    purpose: OperationFactPurpose,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    let value_types = &context.value_types;
    if matches!(purpose, OperationFactPurpose::ProofObligations) {
        // A payload projection names an overlay, not authority to use it.
        // Check the current case before this operation can invalidate facts
        // (including establishing a mutable loan or composing a call).
        for projection in operation.kind.structural_projections() {
            let Some(case) = projection.path.iter().find_map(|segment| match segment {
                terminal_psi::StructuralPathSegment::Case(case) => Some(*case),
                _ => None,
            }) else {
                continue;
            };
            let path = crate::validation::canonical_field_path(
                module,
                machine,
                projection.root,
                projection.path,
            )
            .ok_or(ModuleError::InvalidStructuralCaseObservation {
                operation: operation.id,
                source: projection.root,
                case,
            })?;
            if !case_hops::active(module, machine, context, axioms, projection.root, &path) {
                return Err(ModuleError::InvalidStructuralCaseObservation {
                    operation: operation.id,
                    source: projection.root,
                    case,
                });
            }
        }
    }

    // Private crash questions retain their own entry-origin discipline. A
    // current-value capture cannot manufacture an invocation-entry observation.
    let capture_snapshots = matches!(purpose, OperationFactPurpose::ProofObligations);
    if let OperationKind::EstablishReference { source } = &operation.kind
        && let Some(result) = operation.result.structural()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(declaration.shape,
                terminal_psi::StructuralTypeShape::Reference {
                    referent, access: terminal_psi::StructuralAccess::MutableBorrow,
                } if module.structural_types.iter().any(|declaration| {
                    declaration.id == referent && matches!(declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. })
                }))
        })
    {
        // The loan may mutate through a different carrier identity. Retire
        // captured-root observations now; release restores permission, not
        // the pre-loan contents. Immutable scalar snapshots remain valid.
        field_snapshots::retain(axioms, capture_snapshots, |proposition| {
            !context
                .storage
                .observes_places(proposition, &[source.place])
        });
    }
    if let OperationKind::ByteSequenceWrite { destination, .. }
    | OperationKind::ElementViewWrite { destination, .. } = &operation.kind
    {
        field_snapshots::retain(axioms, capture_snapshots, |proposition| {
            !context
                .storage
                .observes_payload_write(proposition, *destination, &[])
        });
    }
    runtime_index::append(
        module,
        machine,
        operation,
        value_types,
        axioms,
        operation_obligations,
    )?;
    if let OperationKind::StructuralScalarFieldStore {
        value,
        range_obligation: Some(id),
        ..
    } = operation.kind
        && let Some(bounds) =
            crate::validation::structural_scalar_field_store_range(module, machine, operation)
    {
        let integer_type = bounds.integer_type();
        let value = ScalarTerm::value(value, ScalarType::Integer(integer_type));
        let endpoint = |value| ScalarTerm::Integer {
            scalar_type: integer_type,
            value,
        };
        let mut clauses = vec![
            Proposition::LessOrEqual(endpoint(bounds.minimum()), value.clone()),
            Proposition::LessOrEqual(value, endpoint(bounds.maximum())),
        ];
        clauses.sort();
        // The written field's new equality cannot justify its own invariant.
        // Preserve the pre-write facts before invalidation and replacement.
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id,
                proposition: Proposition::Conjunction(clauses),
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate: true,
        });
    }
    if let OperationKind::EstablishByteSequenceConcatenation {
        capacity_obligation,
        ..
    } = operation.kind
    {
        let result = operation
            .result
            .structural()
            .ok_or(ModuleError::InvalidByteSequenceConcatenation(operation.id))?;
        let capacity = module
            .structural_types
            .iter()
            .find_map(|declaration| {
                if declaration.id != result.structural_type {
                    return None;
                }
                match declaration.shape {
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    ) => Some(capacity),
                    _ => None,
                }
            })
            .ok_or(ModuleError::InvalidByteSequenceConcatenation(operation.id))?;
        let proposition =
            terminal_semantics::byte_sequence_concatenation_capacity_goal(operation, capacity)
                .map_err(ModuleError::OperationSemanticSchema)?;
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: capacity_obligation,
                proposition,
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate: true,
        });
        return Ok(());
    }
    if let OperationKind::ScalarRepresentationView {
        operand,
        landing_obligation,
    } = operation.kind
    {
        // A total re-view publishes no fact: no scalar term spells its bit
        // reading. A byte landing on `bool` owes the canonical `{0, 1}`
        // interval, rebuilt from the operand's declared carrier rather than
        // read from the bundle, against the facts available before the
        // result exists.
        let pair = crate::validation::scalar_representation_view::scalar_representation_view_pair(
            operation,
            value_types,
        )?;
        if let (Some(id), Some(proposition)) = (landing_obligation, pair.landing_goal(operand)) {
            operation_obligations.push(ReconstructedOperationObligation {
                owner: ReconstructedTerminalObligationOwner::Operation {
                    machine: machine.id,
                    operation: operation.id,
                },
                obligation: Obligation {
                    id,
                    proposition,
                    class: ObligationClass::Derivable,
                },
                semantic_axioms: axioms.clone(),
                canonical_certificate: true,
            });
        }
        return Ok(());
    }
    if matches!(operation.kind, OperationKind::EstablishRecord { .. }) {
        return record::append(module, machine, operation, axioms, operation_obligations);
    }
    if matches!(operation.kind, OperationKind::EstablishScalarCase { .. }) {
        return scalar_case::append(module, machine, operation, axioms, operation_obligations);
    }
    if matches!(
        operation.kind,
        OperationKind::EstablishStructuralCase { .. }
    ) {
        return structural_case::append(module, machine, operation, axioms, operation_obligations);
    }
    if matches!(operation.kind, OperationKind::EstablishScalarArray { .. }) {
        crate::validation::scalar::array::shape(module, machine, operation)?;
        // Type and complete initialization are total validation judgments.
        // No scalar equality or extra proof authority is asserted here.
        return Ok(());
    }
    if matches!(
        operation.kind,
        OperationKind::EstablishZeroedStructural { .. }
    ) {
        crate::validation::structural::operations::validate_establish_zeroed_structural(
            module, machine, operation,
        )?;
        // Whole-root zeroed initialization is a total validation judgment;
        // the fresh result place has no prior observations to invalidate.
        return Ok(());
    }
    if matches!(
        operation.kind,
        OperationKind::EstablishStructuralLocal { .. }
    ) {
        crate::validation::structural::operations::validate_establish_structural_local(
            module, machine, operation,
        )?;
        // Home establishment is a total validation judgment; members carry
        // their own establishment observations and the fresh result place
        // has no prior observations to invalidate.
        return Ok(());
    }
    if matches!(purpose, OperationFactPurpose::ProofObligations) {
        // Payload observations through `Case` steps read an overlay that is
        // live only while its case is active (`case_hops`).
        let observed = match &operation.kind {
            OperationKind::StructuralCaseLeafCopy { source, path } => Some((
                *source,
                path,
                ModuleError::InvalidStructuralLeafCopy {
                    operation: operation.id,
                    source: *source,
                },
            )),
            OperationKind::IntegerStructuralField {
                source,
                path,
                field,
            } => Some((
                *source,
                path,
                ModuleError::InvalidIntegerStructuralField {
                    operation: operation.id,
                    source: *source,
                    field: *field,
                },
            )),
            OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            } => Some((
                *source,
                path,
                ModuleError::InvalidBooleanStructuralField {
                    operation: operation.id,
                    source: *source,
                    field: *field,
                },
            )),
            _ => None,
        };
        if let Some((source, path, error)) = observed
            && !case_hops::active(module, machine, context, axioms, source, path)
        {
            return Err(error);
        }
    }
    if let OperationKind::StructuralLeafCopy { source, path } = &operation.kind {
        // Freshness of the result and the exact selected leaf type are total
        // validation judgments. An owned record copy additionally carries
        // its source's scalar leaves through exact declaration paths.
        record::append_leaf_copy(module, machine, operation, *source, path, axioms);
        return Ok(());
    }
    if matches!(operation.kind, OperationKind::StructuralCaseLeafCopy { .. }) {
        // A case-leaf copy publishes no scalar fact: freshness of the result
        // and the exact selected leaf type are total validation judgments.
        return Ok(());
    }
    if let OperationKind::EstablishPrimitiveLocal { .. } = &operation.kind
        && let Some(result) = operation.result.structural()
    {
        field_snapshots::retain(axioms, capture_snapshots, |proposition| {
            !context
                .storage
                .observes_places(proposition, &[result.place])
        });
    }
    if let OperationKind::AtomicAccess { event, .. } = &operation.kind {
        // An atomic event states nothing about its result: another
        // participant may intervene between events, so the observed prior is
        // an unconstrained value of the leaf type, not the last store. A
        // modifying event retires observations of potentially aliasing atomic
        // residents. Shared invocation roots need not have distinct backing;
        // ordinary siblings and immutable result snapshots remain unchanged.
        if event.modifies_resident() {
            field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context.storage.observes_shared_mutation(proposition)
            });
        }
        return Ok(());
    }
    if let OperationKind::WriteOnlyPrimitiveStore {
        destination, path, ..
    } = &operation.kind
    {
        // The store writes its leaf; a runtime-selected element may be any
        // element of the array it selects in, so the written region is the
        // static prefix before the first runtime index. Observations of
        // disjoint sibling paths stay current. A prefix that cannot be scoped
        // forgets the whole root.
        let static_prefix = path
            .iter()
            .position(|segment| segment.runtime_index().is_some())
            .unwrap_or(path.len());
        match crate::validation::canonical_field_path(
            module,
            machine,
            *destination,
            &path[..static_prefix],
        ) {
            Some(written) => field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_write(proposition, *destination, &written)
            }),
            None => field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_places(proposition, &[*destination])
            }),
        }
    }
    if let OperationKind::StructuralScalarFieldStore {
        destination, path, ..
    } = &operation.kind
        && !terminal_psi::is_static_structural_path(path)
    {
        // A field store through a runtime-selected element (`cells[i].hp`)
        // writes some element of the array it selects in, exactly as a
        // primitive store through one does: the written region is the static
        // prefix before the first runtime index, observations of disjoint
        // siblings (`self.i`, `self.scratch`) stay current, and it names no
        // canonical leaf, so it publishes no field equation. A prefix that
        // cannot be scoped forgets the whole root.
        let static_prefix = path
            .iter()
            .position(|segment| segment.runtime_index().is_some())
            .unwrap_or(path.len());
        match crate::validation::canonical_field_path(
            module,
            machine,
            *destination,
            &path[..static_prefix],
        ) {
            Some(written) => field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_write(proposition, *destination, &written)
            }),
            None => field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_places(proposition, &[*destination])
            }),
        }
    } else if let OperationKind::StructuralByteSequenceFieldByteStore { destination, .. } =
        &operation.kind
    {
        // A byte replacement writes backing rather than the descriptor slot.
        // Inline bounded bytes retain sibling geometry; borrowed fields may
        // alias any external backing, independent of the containing root.
        if let Some((root, written)) =
            crate::validation::structural_field_store_write_path(module, machine, operation)
        {
            field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_payload_write(proposition, root, &written)
            });
        } else {
            field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                !context
                    .storage
                    .observes_places(proposition, &[*destination])
            });
        }
    } else if matches!(
        operation.kind,
        OperationKind::StructuralScalarFieldStore { .. }
            | OperationKind::ByteSequenceStore { .. }
            | OperationKind::StructuralByteSequenceFieldStore { .. }
            | OperationKind::StructuralByteViewFieldStore { .. }
            | OperationKind::MoveStructuralField { .. }
            | OperationKind::StoreStructuralField { .. }
    ) {
        match crate::validation::structural_field_store_write_path(module, machine, operation) {
            Some((root, written)) => {
                field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                    !context.storage.observes_write(proposition, root, &written)
                });
                // A scalar field store fixes the exact leaf it writes: the
                // field equals the stored value until a covering write
                // replaces it. Field reads observe the same canonical path,
                // so later read equations transport this fact back to the
                // stored value. Only the canonical write path the checked
                // invalidation just scoped can name the leaf; a store whose
                // path cannot be resolved forgets the root instead.
                if let OperationKind::StructuralScalarFieldStore { value, .. } = &operation.kind {
                    let equation = match value_types.get(value).copied() {
                        Some(ScalarType::Integer(integer)) => Some(Proposition::Equal(
                            ScalarTerm::integer_field_path(root, written.clone(), integer),
                            ScalarTerm::value(*value, ScalarType::Integer(integer)),
                        )),
                        Some(ScalarType::Boolean) => Some(Proposition::Equal(
                            ScalarTerm::boolean_field_path(root, written.clone()),
                            ScalarTerm::value(*value, ScalarType::Boolean),
                        )),
                        Some(ScalarType::IeeeFloat(format)) => Some(Proposition::Equal(
                            ScalarTerm::ieee_float_field_path(root, written.clone(), format),
                            ScalarTerm::value(*value, ScalarType::IeeeFloat(format)),
                        )),
                        _ => None,
                    };
                    if let Some(equation) = equation {
                        axioms.push(equation);
                    }
                }
            }
            // A write whose exact path cannot be resolved forgets the root.
            None => {
                let root = match &operation.kind {
                    OperationKind::StructuralScalarFieldStore { destination, .. }
                    | OperationKind::ByteSequenceStore { destination, .. }
                    | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
                    | OperationKind::StructuralByteViewFieldStore { destination, .. }
                    | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. }
                    | OperationKind::StoreStructuralField { destination, .. } => *destination,
                    OperationKind::MoveStructuralField { source, .. } => *source,
                    _ => unreachable!("store kinds are matched above"),
                };
                field_snapshots::retain(axioms, capture_snapshots, |proposition| {
                    !context.storage.observes_places(proposition, &[root])
                });
            }
        }
        if matches!(
            operation.kind,
            OperationKind::MoveStructuralField { .. } | OperationKind::StoreStructuralField { .. }
        ) {
            // Restoration debt is custody state, not an axiom-bearing
            // observation: the write-path invalidation above is the whole
            // reconstruction effect.
            return Ok(());
        }
    }
    // These equations describe copied bits, including NaN payloads and the
    // sign of zero. They are not IEEE `==` facts. Stores publish only after
    // the ordinary write invalidation above retires prior observations.
    if let Some(equation) =
        float_fields::primitive_equation(module, machine, operation, value_types)
    {
        axioms.push(equation);
    }
    if let Some(equation) =
        primitive_elements::element_equation(module, machine, operation, value_types)
    {
        axioms.push(equation);
    }
    if let Some(bound) = crate::validation::structural_byte_sequence_field_length_capacity_bound(
        module, machine, operation,
    )? {
        axioms.push(bound);
    }
    if let Some(equation) = crate::validation::structural_byte_sequence_field_length_equation(
        module, machine, operation,
    )? {
        axioms.push(equation);
    }
    if let OperationKind::StructuralByteSequenceFieldStore {
        length, obligation, ..
    }
    | OperationKind::ByteSequenceStore {
        length, obligation, ..
    } = &operation.kind
    {
        let capacity = if matches!(operation.kind, OperationKind::ByteSequenceStore { .. }) {
            crate::validation::byte_sequence_store_capacity(module, machine, operation)?
        } else {
            crate::validation::structural_byte_sequence_store_capacity(module, machine, operation)?
        };
        let integer_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid");
        let bound = semantic_vocabulary::ScalarTerm::integer(
            integer_type,
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity)),
        )
        .map_err(|error| {
            ModuleError::OperationSemanticSchema(
                terminal_semantics::OperationSemanticError::InvalidProposition(error),
            )
        })?;
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: *obligation,
                proposition: Proposition::LessOrEqual(
                    semantic_vocabulary::ScalarTerm::value(
                        *length,
                        ScalarType::Integer(integer_type),
                    ),
                    bound,
                ),
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate: true,
        });
        return Ok(());
    }
    if let Some(semantics) = goal_free_scalar_leaf_semantics(operation, value_types)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        return append_scalar_facts(&semantics, value_types, purpose, axioms);
    }
    if let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, value_types)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        // Obligation identity is a total schema decision. Available facts may
        // influence the producer's explicit certificate, but cannot make
        // reconstruction search for a different sufficient proposition.
        let proposition = semantics
            .canonical_goal()
            .kernel_proposition()
            .map_err(ModuleError::OperationSemanticSchema)?;
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: semantics.obligation(),
                proposition,
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate: true,
        });
        axioms.push(semantics.result_equation().clone());
        return Ok(());
    }
    if let Some(observation) = structural_effect_leaf_observation(operation)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        if let terminal_semantics::StructuralEffectObservation::CaseMembershipObserved {
            source,
            path,
            case,
            result,
        } = &observation
            && capture_snapshots
            && let Some(prefix) =
                crate::validation::canonical_field_path(module, machine, *source, path)
        {
            // This is a conditional observation, not unconditional case authority.
            // Its place-bearing conclusion makes existing write invalidation
            // retire it if the discriminator can change before the copy.
            axioms.push(Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    ScalarTerm::value(*result, ScalarType::Boolean),
                    ScalarTerm::Boolean(true),
                )),
                conclusion: Box::new(Proposition::StructuralCaseMembership {
                    subject: semantic_vocabulary::StructuralCaseSubject::new(*source, prefix),
                    case: *case,
                }),
            });
        }
        if let Some((id, proposition)) = observation.canonical_obligation() {
            // An indexed byte store into a predicate-bearing field owes the
            // stored byte's class legs beside its bound, reconstructed from
            // the exact destination declaration rather than the operands.
            let proposition = crate::validation::structural_byte_sequence_field_byte_store_goal(
                module,
                machine,
                operation,
                proposition,
            )?;
            operation_obligations.push(ReconstructedOperationObligation {
                owner: ReconstructedTerminalObligationOwner::Operation {
                    machine: machine.id,
                    operation: operation.id,
                },
                obligation: Obligation {
                    id,
                    proposition,
                    class: ObligationClass::Derivable,
                },
                semantic_axioms: axioms.clone(),
                canonical_certificate: true,
            });
        }
        if let terminal_semantics::StructuralEffectObservation::ByteSequencePredicateEvaluated {
            operand,
            ..
        } = &observation
        {
            let root = match operand {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => *source,
                terminal_psi::control_flow::ByteSequenceOperand::Field { source, .. } => {
                    source.place
                }
            };
            let invalid = || ModuleError::InvalidByteSequencePredicateSource {
                operation: operation.id,
                source: root,
            };
            let root_type = crate::validation::structural::operations::caller_structural_root_type(
                machine, root,
            )
            .ok_or_else(invalid)?;
            let equation = terminal_semantics::byte_predicate_equation(
                &observation,
                root_type,
                &module.structural_types,
            )
            .ok_or_else(invalid)?;
            axioms.push(equation);
        }
        if let terminal_semantics::StructuralEffectObservation::ByteSequenceEqualEvaluated {
            left,
            right,
            ..
        } = &observation
        {
            let invalid = || ModuleError::InvalidByteSequenceEqual(operation.id);
            let root_type = |operand: &terminal_psi::control_flow::ByteSequenceOperand| {
                let root = match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        *source
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field { source, .. } => {
                        source.place
                    }
                };
                crate::validation::structural::operations::caller_structural_root_type(
                    machine, root,
                )
                .ok_or_else(invalid)
            };
            let left_type = root_type(left)?;
            let right_type = root_type(right)?;
            if let Some(equation) = terminal_semantics::byte_equal_equation(
                &observation,
                left_type,
                right_type,
                &module.structural_types,
            ) {
                axioms.push(equation);
            }
        }
        if let Some(equation) = observation.local_equation() {
            axioms.push(equation.clone());
        }
        if let Some(bounds) =
            crate::validation::byte_sequence::read::class_bounds(module, machine, operation)
        {
            // Append only after the read's own bound obligation. Declared
            // invariants or unchanged initializer predicates classify this
            // SSA byte; the read cannot use its result to justify its index.
            axioms.extend(bounds);
        }
        if let Some(bounds) =
            crate::validation::structural::byte_sequence_fields::literal_field_read_bounds(
                module, machine, operation,
            )
        {
            // The read still owes its independent index/length obligation above.
            // Unchanged literal contents bound this scalar snapshot, not future
            // reads of the mutable field. A byte write invalidates the content
            // interval even when it preserves length and the ASCII predicate.
            axioms.extend(bounds);
        }
        if let Some(bounds) =
            crate::validation::integer_structural_field_read_range(module, machine, operation)
            && let Some(result) = operation.result.scalar_ref()
        {
            // A valid referent retains its declared interval. Capture that
            // invariant on this read's SSA value, which survives later writes
            // without asserting that the mutable field keeps its old value.
            // The emission is certificate-gated: only a bound whose fixed-shape
            // elimination certificate re-decides joins under this row.
            let value = ScalarTerm::value(result.id, result.scalar_type);
            for fact in declared_carrier_bounds(context.proposition_context(), bounds, value) {
                if fact.certified {
                    axioms.push(fact.proposition);
                }
            }
        }
        byte_extent::append_length_equations(
            module,
            machine,
            operation,
            &observation,
            context,
            axioms,
        )?;
        element_extent::append_length_equations(
            module,
            machine,
            operation,
            &observation,
            context,
            axioms,
        )?;
        return Ok(());
    }
    if compose_call_operation(
        module,
        machine,
        operation,
        machines,
        value_types,
        &context.storage,
        capture_snapshots,
        axioms,
        operation_obligations,
    )? {
        for composition in machine
            .content_partition_compositions
            .iter()
            .filter(|composition| composition.producer_operation == operation.id)
        {
            let proposition = composition.inferred_proposition();
            if !axioms.contains(&proposition) {
                axioms.push(proposition);
            }
        }
        return Ok(());
    }
    match operation.kind.clone() {
        OperationKind::FloatResultPolicy { policy } => {
            if policy.can_trap() {
                axioms.extend(crate::validation::float_result_policy::normal_return_facts(
                    operation,
                    policy.raw_result(),
                )?);
            }
            axioms.extend(
                crate::validation::float_result_policy::saturating_return_facts(operation, policy)?,
            );
            Ok(())
        }
        OperationKind::TrappingInteger { .. } => {
            // A trap ends the path, so only the normal-return continuation
            // reaches the next operation: the exact result equation is sound
            // there and owes no obligation.
            axioms.push(crate::validation::trapping_integer::normal_return_equation(
                operation,
                value_types,
            )?);
            Ok(())
        }
        OperationKind::IntegerExactCast { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. }
        | OperationKind::ExactIntegerDivide { .. }
        | OperationKind::ExactIntegerRemainder { .. }
        | OperationKind::WrappingIntegerDivide { .. }
        | OperationKind::WrappingIntegerRemainder { .. }
        | OperationKind::SaturatingIntegerDivide { .. }
        | OperationKind::SaturatingIntegerRemainder { .. } => {
            unreachable!("proof-bearing scalar rows return before legacy reduction dispatch")
        }
        OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::WrappingIntegerAdd { .. }
        | OperationKind::SaturatingIntegerAdd { .. }
        | OperationKind::WrappingIntegerSubtract { .. }
        | OperationKind::SaturatingIntegerSubtract { .. }
        | OperationKind::WrappingIntegerMultiply { .. }
        | OperationKind::SaturatingIntegerMultiply { .. } => {
            unreachable!("goal-free scalar rows return before specialized reconstruction")
        }
        OperationKind::IeeeFloatConstant { value } => {
            let result = operation
                .result
                .scalar_ref()
                .filter(|result| result.scalar_type == ScalarType::IeeeFloat(value.format()))
                .ok_or(ModuleError::OperationSemanticSchema(
                    terminal_semantics::OperationSemanticError::ResultShapeMismatch(
                        terminal_semantics::OperationSemanticTag::IeeeFloatConstant,
                    ),
                ))?;
            // This is exact payload identity, not IEEE source `==`: NaN bits
            // and signed zero survive transport into the call's predicate.
            axioms.push(Proposition::Equal(
                ScalarTerm::value(result.id, result.scalar_type),
                ScalarTerm::IeeeFloatLiteral(value),
            ));
            Ok(())
        }
        OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::IeeeFloatCompare { .. }
        | OperationKind::StoreDynamicDescriptor { .. } => Ok(()),
        OperationKind::ScalarRepresentationView { .. } => {
            unreachable!("scalar representation views return before legacy reduction dispatch")
        }
        OperationKind::AtomicAccess { .. } => {
            unreachable!("atomic events return before specialized reconstruction")
        }
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::EstablishReference { .. }
        | OperationKind::ReleaseReference { .. }
        | OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::ByteSequenceStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteViewFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::StructuralByteSequenceFieldRead { .. }
        | OperationKind::EstablishByteSequenceConcatenation { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::ByteSequenceSubslice { .. }
        | OperationKind::ByteSequencePredicate { .. }
        | OperationKind::ByteSequenceEqual { .. }
        | OperationKind::EstablishElementView { .. }
        | OperationKind::EstablishMutableElementView { .. }
        | OperationKind::ElementViewLength { .. }
        | OperationKind::ElementViewRead { .. }
        | OperationKind::ElementViewWrite { .. }
        | OperationKind::ElementViewSubslice { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishStructuralLocal { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishRecord { .. }
        | OperationKind::MoveStructuralField { .. }
        | OperationKind::StoreStructuralField { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::StructuralLeafCopy { .. }
        | OperationKind::StructuralCaseLeafCopy { .. }
        | OperationKind::IntegerStructuralField { .. } => {
            unreachable!("structural/effect rows return before specialized reconstruction")
        }
        OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::BoundaryCall { .. } => {
            unreachable!("call rows return before specialized reconstruction")
        }
    }
}

/// One declared-interval bound plus its generation-time certificate verdict.
///
/// `certified` is `true` when the fixed-shape `ConjunctionElimination`
/// certificate — the value's declared interval invariant cited as assumption
/// zero and the bound eliminated at its conjunct index — was re-decided by
/// proof-admission's certificate checker. `false` marks a licensed premise
/// introduction retained under the calling row's trusted surface (for
/// example `fact:integer-structural-field-read-range` or
/// `fact:structural-case-arm`).
///
/// The generation-time check runs in production regardless; only the test
/// module reads the classification back.
#[allow(dead_code)]
pub(super) struct CarrierBoundFact {
    /// The bound proposition appended to the roster.
    pub proposition: Proposition,
    /// Whether the certificate checker re-decided the emission.
    pub certified: bool,
}

/// Emit the declared interval's two bounds on `value`. Each emission is
/// discharged, not trusted: the certificate for each bound re-decides that
/// it is an exact conjunct of the declared interval invariant, itself cited
/// as assumption zero.
pub(super) fn declared_carrier_bounds(
    proposition_context: &PropositionContext,
    bounds: BoundedIntegerType,
    value: ScalarTerm,
) -> [CarrierBoundFact; 2] {
    let integer_type = bounds.integer_type();
    let endpoint = |value| ScalarTerm::Integer {
        scalar_type: integer_type,
        value,
    };
    let lower = Proposition::LessOrEqual(endpoint(bounds.minimum()), value.clone());
    let upper = Proposition::LessOrEqual(value, endpoint(bounds.maximum()));
    let mut members = vec![lower.clone(), upper.clone()];
    members.sort();
    let declared = Proposition::Conjunction(members);
    [lower, upper].map(|bound| CarrierBoundFact {
        certified: declared_bound_certified(proposition_context, &declared, &bound),
        proposition: bound,
    })
}

/// Re-decide the fixed-shape elimination certificate for one declared bound
/// before it is classified. The declared interval invariant is assumption
/// zero; the bound is eliminated at its exact conjunct index so the checker
/// re-decides that the emission is a conjunct of the declaration rather
/// than a fabricated literal.
fn declared_bound_certified(
    proposition_context: &PropositionContext,
    declared: &Proposition,
    bound: &Proposition,
) -> bool {
    let Proposition::Conjunction(conjuncts) = declared else {
        return false;
    };
    let Some(conjunct) = conjuncts.iter().position(|member| member == bound) else {
        return false;
    };
    let certificate = ProofNode {
        conclusion: bound.clone(),
        rule: ProofRule::ConjunctionElimination {
            conjunction: Box::new(ProofNode {
                conclusion: declared.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            conjunct,
        },
    };
    check_certificate(
        proposition_context,
        bound,
        std::slice::from_ref(declared),
        &[],
        &certificate,
    )
    .is_ok()
}

#[cfg(test)]
mod tests;
