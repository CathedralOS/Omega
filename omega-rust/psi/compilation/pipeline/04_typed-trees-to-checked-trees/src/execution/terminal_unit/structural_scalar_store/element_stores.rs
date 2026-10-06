//! Scalar stores below mutable element-view descriptors.
//!
//! The resolved storage walk owns field identity, reference crossings and
//! selector occurrences. Its first index selects the live descriptor element;
//! the rest selects a scalar leaf within that element. Separating these two
//! coordinates keeps a nested fixed array from treating descriptor storage as
//! inline primitive backing. Whole elements and record fields use this same
//! operation, with no dependency on the right-hand side's source arrangement.
//! Byte views retain their independent predicate-preservation contract.

use super::destination::{Destination, Root, exact_relevant_field};
use super::value::{self, AssignmentSource};
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralPathSegment, Multiplicity, TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;

pub(super) fn store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let projection = destination.storage_projection(program)?;
    let primitive_type = program.primitive_type_reference(projection.type_reference)?;
    if projection.ends_at_field {
        // Preserve the ordinary field's retention judgment, including refusal
        // of atomic storage. A primitive type alone does not authorize a field.
        let (facts::PlaceSegment::Field { symbol }, carriers) =
            destination.place.segments.split_last()?
        else {
            return None;
        };
        let carrier = destination.leaf_carrier(program, carriers)?;
        let field = exact_relevant_field(program, carrier.owner, *symbol)?;
        if super::scalar_leaf(program, field)? != primitive_type {
            return None;
        }
    }
    let (selector, path) = projection.path.split_first()?;
    let element_selector = match selector {
        CheckedUnitStructuralPathSegment::FixedIndex(element_index) => {
            checked_trees::CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(
                    i64::try_from(*element_index).ok()?,
                )
                .with_landing(numerics::literals::IntegerLanding {
                    landed_type: numerics::literals::LandedIntegerType::U64,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                }),
            }
        }
        CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
            super::runtime_index_expression(program, facts, state, statement_index, selector)?
        }
        _ => return None,
    };
    Some(CheckedUnitEffectOperationPlan::ElementViewWrite(
        checked_trees::CheckedElementViewWritePlan {
            statement_index,
            destination: destination.root.destination(),
            index: element_selector,
            path: path.to_vec(),
            element: primitive_type,
            range_decision: facts.proof.integer_assignment_at(
                machine.symbol,
                state.symbol,
                statement_index as usize,
                assignment.target,
                assignment.value,
            ),
            value: value::assignment_value(
                program,
                facts,
                machine,
                state,
                &destination.root,
                statement_index,
                assignment,
                source,
                primitive_type,
                trace,
            )?,
        },
    ))
}

/// Exact mutable view root, independent of the selected leaf. A parameter
/// retains its admitted borrow; a local retains its resolved declaration.
pub(super) fn element_reference(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let type_reference = match &destination.root {
        Root::Parameter { plan, parameter } => {
            if plan.access != CheckedStructuralAccess::MutableBorrow
                || plan.multiplicity != Multiplicity::Unrestricted
                || !plan.qualifications.is_empty()
                || plan.fused_service_erasure.is_some()
            {
                return None;
            }
            parameter.type_reference
        }
        Root::Local { symbol } => {
            program
                .state_local_declaration(state, *symbol)?
                .1
                .type_reference
        }
    };
    let (referee, access) =
        super::super::reference_results::mutable_borrowed_parts(program, type_reference)?;
    if access != CheckedStructuralAccess::MutableBorrow {
        return None;
    }
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(referee)
    else {
        return None;
    };
    (program.primitive_type_reference(*element_type) != Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*element_type)
}
