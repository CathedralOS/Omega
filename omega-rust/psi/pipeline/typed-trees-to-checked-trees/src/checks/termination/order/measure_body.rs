use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::measure::MeasureDefinition;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

pub(super) enum MeasureBodyShape {
    ParameterForward,
    FieldProjection {
        field: Identifier,
        owner: SymbolHandle,
        field_type: typed_trees::types::TypeReferenceHandle,
    },
}

/// Classify only bodies whose projection is justified by resolved declarations.
pub(super) fn measure_body_shape(
    program: &TypedTrees,
    measure: &MeasureDefinition,
) -> Option<MeasureBodyShape> {
    let [body] = program.expression_table.expression_handles(measure.body) else {
        return None;
    };
    let parameter = measure.parameter.as_ref()?;
    let binder = program.symbols.get(parameter.symbol);
    if !parameter.symbol.is_valid()
        || program.symbols.get(measure.symbol).kind != SymbolKind::Measure
        || binder.kind != SymbolKind::Parameter
        || binder.parent != measure.symbol
    {
        return None;
    }
    if is_parameter(program, *body, parameter.symbol) {
        return Some(MeasureBodyShape::ParameterForward);
    }
    let ExpressionNode::Member(member) = program.expression_table.expression(*body) else {
        return None;
    };
    if !is_parameter(program, member.receiver, parameter.symbol)
        || !member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let TypeReferenceNode::Named { symbol: owner, .. } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let declaration = program
        .data_definitions()
        .iter()
        .find(|data| owner.is_valid() && data.symbol == *owner)?;
    let field_symbol = program.symbols.get(member.member_symbol);
    if field_symbol.kind != SymbolKind::Field || field_symbol.parent != *owner {
        return None;
    }
    let mut fields = program
        .data_members(declaration)
        .iter()
        .filter_map(|member_definition| match member_definition {
            DataMember::Field(field) if field.symbol == member.member_symbol => Some(field),
            _ => None,
        });
    let field = fields.next()?;
    // Returning a field with another carrier does not establish a u64 measure.
    // Field constraints narrow its values without changing that carrier.
    let field_type = super::unwrap_constraint_shells(program, field.type_reference);
    if fields.next().is_some()
        || field.name != member.member
        || ![field_type, measure.return_type]
            .into_iter()
            .all(|reference| {
                matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. }
                    if program.symbols.builtin_type_atom(*symbol) == Some(BuiltinTypeAtom::U64))
            })
    {
        return None;
    }
    Some(MeasureBodyShape::FieldProjection {
        field: field.name.clone(),
        owner: *owner,
        field_type: field.type_reference,
    })
}

fn is_parameter(program: &TypedTrees, expression: ExpressionHandle, binder: SymbolHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    path.symbol == binder
        && path.head_symbol == binder
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
}
