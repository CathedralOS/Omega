//! Authored callable results shared by ordinary bodies and composed graphs.
//! Completion plans prove actual returns; they never choose the declaration's
//! scalar/structural carrier or erase it when no path returns normally.
use super::{
    Multiplicity, ShapeCollector, TypeReferenceHandle, is_reference, is_unit,
    parameter_qualifications, projected_result_qualifications,
};
use checked_trees::{CheckedControlResultPlan, CheckedStructuralResultPlan};

/// Retain the authored entry result and its domains independently of body
/// routing, executed result production, and normal-return receiving bindings.
pub(in crate::execution::terminal_unit) fn declared_result_plan(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<checked_trees::CheckedControlResultPlan> {
    let entry = program.machine_states(machine).first()?;
    let result = declared_result_type(program, shapes, entry.return_type)?;
    super::register_authored_result_domains(
        program,
        shapes,
        machine,
        entry,
        &super::machine_binders(program, machine),
    )?;
    // Normal-return contracts use the shared checked guarantee owner for
    // every carrier. Linear results additionally retain exact outgoing claim
    // custody; their contracts do not turn guarded completion into a sequence.
    Some(result)
}

pub(in crate::execution::terminal_unit) fn declared_result_type(
    program: &dyn ::validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    reference: TypeReferenceHandle,
) -> Option<CheckedControlResultPlan> {
    if is_unit(program, reference) {
        return Some(CheckedControlResultPlan::Unit);
    }
    if let Some(primitive_type) = program.primitive_type_reference(reference) {
        // A primitive result is one returned scalar value. An arithmetic
        // policy changes later operation meaning, not the returned payload;
        // value refinements are normal-return guarantees the shared decoder
        // publishes and each exit proves. Semantic atoms retain their separate
        // result custody rather than disappearing behind a scalar carrier.
        return validation::scalar_result_contract_is_represented(program, reference)
            .then_some(CheckedControlResultPlan::Scalar { primitive_type });
    }
    let multiplicity = crate::checks::type_multiplicity(program, reference);
    // A returned pointer transports its exact loan descriptor, independently
    // of shared, mutable or write-only access. Slice descriptors keep their
    // separate geometry; neither family owns the original backing.
    let pointer_result =
        validation::reference_result_custody::has_borrowed_parts(program, reference);
    let view_result = pointer_result
        || crate::execution::terminal_unit::types::borrowed_slice_result(program, reference);
    // A reference carrier transports a loan, not owned referent claims —
    // except a byte view, whose lowered type peels the borrow so `&[u8] in
    // Utf8` registers `Utf8` on the result's own slice carrier. Those
    // inside-borrow domains are the view type's qualifications; other
    // reference results declare none. Owned results keep the root constraint
    // collection.
    let qualifications = if view_result {
        let qualifications =
            validation::view_result_carrier_qualifications(program, reference).ok()?;
        for domain in &qualifications {
            shapes.add_domain(*domain, reference, &[])?;
        }
        qualifications
    } else {
        parameter_qualifications(program, shapes, reference, &[])?
    };
    // Returning owned storage transfers its cleanup obligation with the value.
    // A no-code-disposal classifier would reject that transfer solely because
    // the declaration has a hook. Exact normal-edge plans account separately
    // for owners that remain behind; result custody authenticates the transfer.
    let represented_copy = multiplicity == Multiplicity::Unrestricted
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        );
    let owned_bytes = multiplicity == Multiplicity::Unrestricted
        && validation::bounded_byte_buffer_capacity(program, reference).is_some()
        && validation::has_contents(
            program,
            reference,
            &[],
            validation::ContentsRequirement::RepresentedOwned,
        );
    // An affine owner of linear members returns their claims with it; the
    // caller binds them by path, so their custody needs no storage judgment
    // beyond finite owned contents.
    let claim_bearing = !validation::linear_claim_frontier(program, reference).is_empty()
        && validation::has_contents(
            program,
            reference,
            // The graph retains each member's exact qualification and claim
            // path. This is finite carrier admission, not permission to erase
            // a routed domain or substitute a plain cleanup contract.
            &[],
            validation::ContentsRequirement::StructuralConstruction,
        );
    if (is_reference(program, reference) && !view_result)
        || (multiplicity != Multiplicity::Linear
            && !claim_bearing
            && !owned_bytes
            && (!view_result
                && !represented_copy
                && !validation::reference_result_custody::is_reference_record(program, reference)
                && !validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::BorrowFree,
                )
                && !validation::has_contents(
                    program,
                    reference,
                    &[],
                    validation::ContentsRequirement::OwnedOrSharedView,
                )))
    {
        return None;
    }
    // A returned pointer keeps its reference shell and exact access. Its
    // materialized carrier has affine custody even when shared source use is
    // reusable; it owns no backing. Shape and loan admission remain separate.
    let (type_identity, multiplicity) =
        if crate::execution::terminal_unit::types::borrowed_slice_result(program, reference) {
            // A returned slice transports its descriptor, never a pointer to a
            // callee descriptor home. Access determines its loan custody; the
            // element/byte view declaration remains the same carrier.
            (
                shapes.add_stored_type(reference, &[], &[])?,
                validation::reference_result_custody::binding_multiplicity(program, reference),
            )
        } else if pointer_result {
            (
                shapes.add_reference_type(reference, &[])?,
                Multiplicity::Affine,
            )
        } else {
            // A completed aggregate owns its stored loan carriers even when
            // source observations are reusable. Use the same result custody
            // as an ordinary call binding; source type multiplicity alone
            // misdeclares reference arrays as unrestricted graph results.
            (
                shapes.add_type(reference, &[], &[])?,
                validation::reference_result_custody::result_multiplicity(program, reference),
            )
        };
    // The contents judgments above establish storage, not transfer authority.
    // Reference-record storage may also carry linear claims and nominal
    // cleanup: the graph's result custody rejoins these independent obligations
    // with the retained descriptor loans. ShapeCollector owns finite runtime
    // storage. A graph result carries that same established
    // value, not another record/sum-only vocabulary. Reclassifying fields here
    // sent mixed values and arrays into sequence-only completion, which cannot
    // retain their distinct returning edges and cleanup obligations. Erased
    // members remain semantic fields, never runtime result operands.
    Some(CheckedControlResultPlan::Structural(
        CheckedStructuralResultPlan {
            type_identity,
            multiplicity,
            qualifications,
            projected_qualifications: projected_result_qualifications(
                program,
                shapes,
                reference,
                &[],
            )?,
        },
    ))
}
