use validation::effective_member_symbol;

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

pub(crate) fn relative_place_segments_from_expression(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    domain: &typed_trees::domain::DomainDefinition,
) -> Option<Vec<facts::PlaceSegment>> {
    if !expression.is_valid() {
        return None;
    }

    let subject_type_symbol =
        validation::machine_symbol_from_type_reference_handle(program, domain.target_type);
    let subject_type_symbol = subject_type_symbol
        .is_valid()
        .then_some(subject_type_symbol);
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            relative_place_segments_from_expression(program, inner.target, domain)
        }
        ExpressionNode::Name(_) => {
            typed_trees::domain::domain_subject_reference(program, domain, expression)
                .then(Vec::new)
        }
        ExpressionNode::Member(member) => {
            let mut segments =
                relative_place_segments_from_expression(program, member.receiver, domain)?;
            let receiver_type = match segments.last() {
                None => subject_type_symbol,
                Some(facts::PlaceSegment::Field { symbol }) => {
                    validation::symbol_type_symbol(program, *symbol)
                }
                _ => None,
            };
            let member_symbol = if let Some(symbol) =
                resolve_member_symbol_from_type(program, receiver_type, member.member.as_str())
            {
                symbol
            } else {
                effective_member_symbol(program, member.receiver, member)
            };
            validation::push_field_place_segments(program, &mut segments, member_symbol);
            Some(segments)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut segments =
                relative_place_segments_from_expression(program, indexed.collection, domain)?;
            segments.push(validation::index_place_segment(program, indexed.index));
            Some(segments)
        }
        _ => None,
    }
}

fn resolve_member_symbol_from_type(
    program: &dyn ::validation::CheckingInput,
    type_symbol: Option<SymbolHandle>,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = type_symbol?;

    if let Some(data) = program.data_definition_by_symbol(type_symbol) {
        if let Some(symbol) = validation::data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    if let Some(machine) = program.machine_by_symbol(type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
    }

    None
}
