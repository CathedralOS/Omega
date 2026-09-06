//! The older `subject - 1` recognizer is only a scalar natural-ranking
//! judgment. Failure of a declared projection must not fall through to it.

use language_semantics::RankingViewId;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::types::{PrimitiveType, TypeReferenceNode};

pub(in crate::call_cycles) fn has_scalar_legacy_rank(
    program: &TypedTrees,
    machine: &Machine,
) -> bool {
    let Some(witness) = machine.termination_plan.implementation_witness.as_ref() else {
        return false;
    };
    if witness.ranking_view != RankingViewId::NAT_DESCENDING
        || Some(witness.view_path.as_str()) != witness.ranking_view.canonical_path()
        || !witness.view_arguments.is_empty()
        || witness.rank_range.is_some()
    {
        return false;
    }
    let [rendered] = witness.subjects.as_slice() else {
        return false;
    };
    let Some(custody) = program.ranking_expression_custody_for(machine.symbol) else {
        return false;
    };
    if !custody.view_arguments.is_empty() || custody.rank_range.is_some() {
        return false;
    }
    let [subject] = custody.subjects.as_slice() else {
        return false;
    };
    let ExpressionNode::Name(path) = program
        .expression_table
        .expression(super::projection::unwrapped(program, *subject))
    else {
        return false;
    };
    if !path.symbol.is_valid() {
        return false;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let mut parameters = program
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self && parameter.symbol == path.symbol);
    let Some(parameter) = parameters.next() else {
        return false;
    };
    if parameters.next().is_some() || parameter.name.as_str() != rendered.as_str() {
        return false;
    }
    let mut reference = parameter.type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    matches!(
        crate::recasts::exact_primitive_type(program, reference),
        Some(PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64)
    )
}
