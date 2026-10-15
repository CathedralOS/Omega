//! Scalar stores below mutable element-view descriptors.
//!
//! The resolved storage walk owns field identity, reference crossings and
//! selector occurrences. Its typed descriptor crossing selects the view element;
//! the rest selects a scalar leaf within that element. Separating these two
//! coordinates keeps a nested fixed array from treating descriptor storage as
//! inline primitive backing. Whole elements and record fields use this same
//! operation, with no dependency on the right-hand side's source arrangement.
//! Byte views retain their independent predicate-preservation contract.

use super::destination::Destination;
use super::value::{self, AssignmentSource};
use super::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
};
use crate::execution::terminal_unit::control::LocalConstructionTrace;

pub(super) fn store(
    program: &dyn ::validation::CheckingInput,
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
        let field = carrier.field(program, *symbol)?;
        if super::scalar_leaf(program, field)? != primitive_type {
            return None;
        }
    }
    let (descriptor_path, element_path) = projection.path.split_at(projection.element_view_index?);
    let (selector, path) = element_path.split_first()?;
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
    let selected = element_selector_target(program, state, assignment.target, path)?;
    let value = value::assignment_value(
        program,
        facts,
        machine,
        state,
        statement_index,
        assignment,
        source,
        primitive_type,
        trace,
    )?;
    Some(CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        destination.root.destination(),
        descriptor_path.to_vec(),
        checked_trees::CheckedStoreLeaf::ViewElement {
            index: element_selector,
            element_path: path.to_vec(),
            element: primitive_type,
            range_decision: facts.proof.integer_assignment_at(
                machine.symbol,
                state.symbol,
                statement_index as usize,
                assignment.target,
                assignment.value,
            ),
            decisions: checked_trees::CheckedIndexedWriteDecisions {
                index_in_bounds: facts.scalar_index_is_proven(selected),
                predicates_preserved: false,
            },
        },
        checked_trees::CheckedStoreValue::assigned(value),
    ))
}

/// The authored `collection[index]` whose selector this write's element
/// coordinate names; index checking records its judgment against it. The
/// target's member and fixed-index steps within the element (`path`) peel
/// off first. A `local.field = value` store through a `&mut collection[index]`
/// element view local has no indexed step of its own, so the local's
/// borrowed `collection[index]` stands in.
fn element_selector_target(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    target: typed_trees::expression::ExpressionHandle,
    path: &[CheckedUnitStructuralPathSegment],
) -> Option<typed_trees::expression::ExpressionHandle> {
    let mut target = target;
    for segment in path.iter().rev() {
        target = match (segment, program.expression_table.expression(target)) {
            (CheckedUnitStructuralPathSegment::Field(_), ExpressionNode::Member(member))
                if member.case_variant.is_none() =>
            {
                member.receiver
            }
            (CheckedUnitStructuralPathSegment::FixedIndex(_), ExpressionNode::Indexed(indexed)) => {
                indexed.collection
            }
            _ => return None,
        };
    }
    if let Some(name) = program.expression_table.name_path_node(target)
        && let Some((_, local)) = program.state_local_declaration(state, name.symbol)
        && let ExpressionNode::Borrow(borrow) =
            program.expression_table.expression(local.initial_value)
        && borrow.access == language_core::ReferenceAccess::Mutable
    {
        return Some(borrow.target);
    }
    Some(target)
}
