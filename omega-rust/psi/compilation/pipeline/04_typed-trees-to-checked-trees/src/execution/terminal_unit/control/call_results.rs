//! Binding scalar and structural call results to checked unit locals.

use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, parameter_qualifications, type_graph_requires_nominal_drop,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult, CheckedUnitCallRoute};

use crate::execution::terminal_unit::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, Multiplicity, StatementNode,
    SymbolHandle, TypeReferenceHandle, TypedTrees, is_reference,
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
        CheckedUnitCallRoute::Boundary => {}
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
    operation: CheckedUnitEffectOperationPlan,
    result: CheckedUnitStructuralResultBindingPlan,
) -> Option<CheckedUnitEffectOperationPlan> {
    let CheckedUnitEffectOperationPlan::Call(call) = operation else {
        return None;
    };
    match (call.route(), call.result()) {
        (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Structural { .. }) => {
            Some(CheckedUnitEffectOperationPlan::Call(call))
        }
        (CheckedUnitCallRoute::Boundary, CheckedUnitCallResult::Unit) => {
            let mut parts = call.into_parts();
            parts.result = CheckedUnitCallResult::Structural {
                discard_result_on_return: result.multiplicity == Multiplicity::Affine,
                binding: result,
            };
            CheckedUnitCallPlan::new(parts).map(CheckedUnitEffectOperationPlan::Call)
        }
        _ => None,
    }
}

pub(crate) fn checked_unit_structural_result_local(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    statements: &[StatementNode],
    binders: &[(SymbolHandle, String)],
) -> Option<(CheckedUnitStructuralResultBindingPlan, SymbolHandle)> {
    let StatementNode::LocalData(local) = statements.first()? else {
        return None;
    };
    if !local.initial_value.is_valid()
        || !matches!(
            program.expression_table.expression(local.initial_value),
            ExpressionNode::Call(_)
        )
    {
        return None;
    }
    Some((
        checked_structural_result_type(program, shapes, local.type_reference, binders)?,
        local.symbol,
    ))
}

/// Result shape is independent of whether the source binds or discards it.
/// Linear value classification does not establish custody: the ordinary call
/// producer must retain the independently reconstructed transfer/return record.
/// Nominal cleanup still requires its checked settlement plan.
/// Primitive arrays use their complete-shape classifier, including dimensions
/// with no leaves; the older owned-storage classifier excludes empty arrays.
pub(crate) fn checked_structural_result_type(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    structural_result_type(program, shapes, result_type, binders, false)
}

/// The result binding of a value a literal constructs in place. Beyond
/// [`checked_structural_result_type`], its scalar members may carry a plain
/// domain that means exactly an interval: the construction proves membership
/// through each member's own range obligation. Call results and other
/// producers keep the narrower classifier.
pub(crate) fn checked_constructed_result_type(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    structural_result_type(program, shapes, result_type, binders, true)
}

fn structural_result_type(
    program: &TypedTrees,
    shapes: &mut ShapeCollector<'_>,
    result_type: TypeReferenceHandle,
    binders: &[(SymbolHandle, String)],
    constructed: bool,
) -> Option<CheckedUnitStructuralResultBindingPlan> {
    if validation::reference_result_custody::parts(program, result_type).is_some() {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_reference_type(result_type, binders)?,
            multiplicity: Multiplicity::Affine,
        });
    }
    // A `&'a V` named view carries its reference shell in the plan and loans
    // the referent's storage, the same custody shape a `&mut` primitive
    // reference result carries.
    if crate::execution::terminal_unit::types::borrowed_named_view(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_named_view_type(result_type, binders)?,
            multiplicity: Multiplicity::Affine,
        });
    }
    // A `&[u8]`/`&[T]` borrowed slice view is the same shared-loan result as
    // the named view above: affine custody against the callee's storage. The
    // `add_type` mint peels the borrow — identical to what an anonymous
    // operand result of the same call produces — so bound and anonymous
    // borrowed-view results carry the same plan shape.
    if crate::execution::terminal_unit::types::borrowed_slice_view(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_type(result_type, binders, &[])?,
            multiplicity: Multiplicity::Affine,
        });
    }
    // A `&mut R`/`&mut [T]` exclusive structural view is the exclusive-loan
    // analog of the shared views above: affine custody against the callee's
    // storage. Like `parts` and the named views it keeps its reference shell
    // — the `Reference` shape is exactly the custody the result carries.
    // `parts` already covers the primitive-referent case; this arm is what
    // `let cell: &mut Cell = self.pick(bag)` needs to bind.
    if crate::execution::terminal_unit::types::mutable_borrowed_view(program, result_type) {
        return Some(CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity: shapes.add_mutable_view_type(result_type, binders)?,
            multiplicity: Multiplicity::Affine,
        });
    }
    let multiplicity = crate::checks::type_multiplicity(program, result_type);
    let qualifications = parameter_qualifications(program, shapes, result_type, binders)?;
    if is_unit(program, result_type)
        || program.primitive_type_reference(result_type).is_some()
        || is_reference(program, result_type)
        || type_graph_requires_nominal_drop(program, result_type)
        || (multiplicity != Multiplicity::Linear
            && (!(validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                result_type,
            ) || validation::has_cleanup_owned_contents(program, result_type)
                || (constructed
                    && validation::has_case_construction_contents(program, result_type))
                || validation::is_closed_primitive_array_type(program, result_type)
                || validation::reference_result_custody::is_reference_record(
                    program,
                    result_type,
                )
                || validation::has_owned_or_shared_view_fields(program, result_type))
                || !qualifications.is_empty()))
    {
        return None;
    }
    Some(CheckedUnitStructuralResultBindingPlan {
        statement_index: 0,
        binding_ordinal: 0,
        type_identity: shapes.add_type(result_type, binders, &[])?,
        multiplicity,
    })
}
