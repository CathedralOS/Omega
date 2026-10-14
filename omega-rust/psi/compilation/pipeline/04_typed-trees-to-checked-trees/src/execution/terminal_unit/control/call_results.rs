//! Binding scalar and structural call results to checked unit locals.
//!
//! Ordinary completion transfers an already established value, including byte
//! predicates represented in its field types. Fresh construction does not gain
//! that authority from its declaration: it must still establish each member.
//! Both routes retain the exact carrier for the receiving custody replay.

use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, parameter_qualifications};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

use crate::execution::terminal_unit::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralResultBindingPlan, Multiplicity, SymbolHandle, TypeReferenceHandle,
    is_reference,
};

/// Rebind a Unit call's result to the scalar local its statement declares.
/// A boundary call keeps its completion receipts; an ordinary call gains its
/// target's contract commitment. Boundary targets need `allow_boundary`.
pub(crate) fn bind_scalar_call_result(
    facts: &CheckFacts,
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitScalarResultBindingPlan,
    allow_boundary: bool,
) -> Option<CheckedUnitEffectOperationPlan> {
    let CheckedUnitEffectOperationPlan::Call(call) = operation else {
        return None;
    };
    let mut parts = call.into_parts();
    if parts.result != CheckedUnitCallResult::Unit {
        return None;
    }
    match &mut parts.route {
        CheckedUnitCallRoute::Boundary if !allow_boundary => return None,
        // A selector call's result is the slot's instantiated result; no
        // single implementation's commitment describes it.
        CheckedUnitCallRoute::Boundary | CheckedUnitCallRoute::Dynamic { .. } => {}
        CheckedUnitCallRoute::Module {
            contract_commitment,
            ..
        } => {
            *contract_commitment = Some(
                facts
                    .contract_plans
                    .for_machine(parts.target.machine)?
                    .commitment,
            );
        }
    }
    parts.result = CheckedUnitCallResult::Scalar(result);
    CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
}

/// Keep the checked call's exact target, source coordinate, operands and
/// completion receipts while attaching its independently checked result local.
/// An ordinary structural call already carries its result and custody.
pub(crate) fn bind_structural_call_result(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let CheckedUnitEffectOperationPlan::Call(call) = operation else {
        return None;
    };
    match (call.route(), call.result()) {
        (
            CheckedUnitCallRoute::Module { .. } | CheckedUnitCallRoute::Dynamic { .. },
            CheckedUnitCallResult::Structural { .. },
        ) => Some(CheckedUnitEffectOperationPlan::Call(call)),
        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
            let mut parts = call.into_parts();
            let occurrence = facts.flow.control.call_at(
                caller_state,
                parts.coordinate.statement_index as usize,
                parts.coordinate.call_ordinal as usize,
            )?;
            // A requirement or operator owns a signature, not a Machine body.
            // Resolve its declaration and the same retained specialization the
            // boundary argument/result planner used, never a receiving annotation.
            let return_type = validation::call_target_return_type(program, parts.target.state)?;
            let substitutions = parts
                .source_site
                .and_then(|site| {
                    facts
                        .requirement_call_specializations
                        .for_site(site, occurrence.target_symbol)
                })
                .map(|specialization| {
                    specialization
                        .type_bindings
                        .iter()
                        .map(|binding| (binding.parameter, binding.actual))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let return_type = crate::execution::terminal_unit::types::substituted_formal_type(
                program,
                return_type,
                &substitutions,
            );
            let result_claims = validation::structural_call_result_claims(
                program,
                facts,
                caller_machine,
                caller_state,
                occurrence,
                return_type,
            )
            .ok()?;
            // Boundary ingress remains completion receipts, while receiving
            // custody is the same checked result frontier ordinary calls use.
            // Record it now; lowering must not reconstruct authority from names.
            let checked_trees::CheckedUnitCallTransfers::CompletionReceipts(claim_transfers) =
                parts.transfers
            else {
                return None;
            };
            parts.transfers = checked_trees::CheckedUnitCallTransfers::StructuralCustody(
                checked_trees::CheckedStructuralCallCustodyPlan {
                    claim_transfers,
                    result_claims,
                    ..Default::default()
                },
            );
            parts.result = CheckedUnitCallResult::Structural {
                discard_result_on_return: result.multiplicity == Multiplicity::Affine,
                binding: result,
            };
            CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
        }
        _ => None,
    }
}

/// An independently replayed copy preserves the established leaf's complete
/// represented contents, including bounded byte predicates and live lengths.
/// The shared construction admission below cannot grant that authority: a
/// fresh initializer still owes its member proofs. This binding only
/// classifies storage; the copied value plan must separately rejoin its
/// readable source and exact leaf type.
pub(crate) fn checked_copied_result_type(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    if let Some(result) = checked_structural_result_type(program, shapes, result_type, binders) {
        return Some(result);
    }
    if crate::checks::type_multiplicity(program, result_type) != Multiplicity::Unrestricted
        || !validation::has_contents(
            program,
            result_type,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        )
        || !parameter_qualifications(program, shapes, result_type, binders)?.is_empty()
    {
        return None;
    }
    Some(CheckedUnitStructuralResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        type_identity: shapes.add_type(result_type, binders, &[])?,
        multiplicity: Multiplicity::Unrestricted,
        local: Default::default(),
    })
}

/// Result shape is independent of whether the source binds or discards it,
/// and of whether a literal constructs the value here or a completed producer
/// hands it over. Both answer to one storage judgment: borrow-free owned
/// contents, independent of the cleanup they require. A scalar member under a plain domain whose
/// complete predicate is represented, and a bounded byte member retaining its
/// primitive predicates, are as carriable when a call returns the record as
/// when this body builds it -- the producer discharged the member obligation
/// either way, and classifying storage never establishes one.
///
/// Linear value classification does not establish custody: the ordinary call
/// producer must retain the independently reconstructed transfer/return record.
/// Nominal cleanup still requires its checked settlement plan.
/// Primitive arrays use their complete-shape classifier, including dimensions
/// with no leaves; the older owned-storage classifier excludes empty arrays.
pub(crate) fn checked_structural_result_type(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    if validation::reference_result_custody::parts(program, result_type).is_some() {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_reference_type(result_type, binders)?,
            multiplicity: Multiplicity::Affine,
            local: Default::default(),
        });
    }
    // A `&'a V` named view carries its reference shell in the plan and loans
    // the referent's storage, the same custody shape a `&mut` primitive
    // reference result carries.
    if crate::execution::terminal_unit::types::borrowed_named_view(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_view_type(
                result_type,
                binders,
                language_semantics::ReferenceAccess::Shared,
            )?,
            multiplicity: Multiplicity::Affine,
            local: Default::default(),
        });
    }
    // Shared descriptors are unrestricted and mutable descriptors affine.
    // Descriptor custody owns neither the borrowed backing nor a stored
    // reference carrier; both view families keep their original geometry.
    if crate::execution::terminal_unit::types::borrowed_slice_result(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(result_type, binders, &[])?,
            multiplicity: validation::reference_result_custody::result_multiplicity(
                program,
                result_type,
            ),
            local: Default::default(),
        });
    }
    // A named `&mut R` exclusive structural view is the exclusive-loan
    // analog of the shared views above: affine custody against the callee's
    // storage. Like `parts` and the named views it keeps its reference shell
    // — the `Reference` shape is exactly the custody the result carries.
    // `parts` already covers the primitive-referent case; this arm is what
    // `let cell: &mut Cell = self.pick(bag)` needs to bind.
    if crate::execution::terminal_unit::types::mutable_borrowed_view(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_view_type(
                result_type,
                binders,
                language_semantics::ReferenceAccess::Mutable,
            )?,
            multiplicity: Multiplicity::Affine,
            local: Default::default(),
        });
    }
    let multiplicity = crate::checks::type_multiplicity(program, result_type);
    // Register exact domain identities independently of storage and linear
    // claims. An affine qualified result need not own any linear claim.
    parameter_qualifications(program, shapes, result_type, binders)?;
    // A byte owner transfers its established predicate roster with its live
    // bytes. Signature classification does not establish those predicates;
    // the callee's completion and receiving replay still prove exact custody.
    if multiplicity == Multiplicity::Unrestricted
        && validation::bounded_byte_buffer_capacity(program, result_type).is_some()
        && validation::has_contents(
            program,
            result_type,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        )
    {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(result_type, binders, &[])?,
            multiplicity,
            local: Default::default(),
        });
    }
    if is_unit(program, result_type)
        || program.primitive_type_reference(result_type).is_some()
        || is_reference(program, result_type)
        || (multiplicity != Multiplicity::Linear
            && (!(validation::has_contents(
        program,
        result_type,
        &[],
        validation::ContentsRequirement::BorrowFree
    )
                || validation::is_closed_primitive_array_type(program, result_type)
                || validation::reference_result_custody::is_reference_record(
                    program,
                    result_type,
                )
                // Whole-contents form of the record-level carrier check: a
                // sum whose payloads are plain records or view-carrying
                // records — `DecodeResult<Relayed<M>>` — binds the same
                // result shape the bare record already did.
                || validation::has_contents(
        program,
        result_type,
        &[],
        validation::ContentsRequirement::OwnedOrSharedView
    )))) {
        return None;
    }
    Some(CheckedUnitStructuralResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        type_identity: shapes.add_type(result_type, binders, &[])?,
        // Admission above classifies authored contents. The completed result
        // owns its stored loan carriers, whose retirement is distinct from
        // source copyability (including references nested in fixed arrays).
        multiplicity: validation::reference_result_custody::result_multiplicity(
            program,
            result_type,
        ),
        local: Default::default(),
    })
}
