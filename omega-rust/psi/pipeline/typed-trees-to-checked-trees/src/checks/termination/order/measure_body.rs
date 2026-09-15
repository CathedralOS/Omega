use symbols::{BuiltinTypeAtom, SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::measure::MeasureDefinition;
use typed_trees::name::Identifier;
use typed_trees::types::{TypeConstraintNode, TypeReferenceNode};

pub(super) enum MeasureBodyShape {
    ParameterForward {
        carrier: BuiltinTypeAtom,
        /// Every `Range` constraint declared on the measure's parameter and
        /// result types. They are the view's domain contract and rank claim:
        /// the caller must prove the subject's enforced bounds fit inside
        /// each one before the forward can be selected.
        constraints: Vec<(ExpressionHandle, ExpressionHandle, bool)>,
    },
    FieldProjection {
        field: Identifier,
        owner: SymbolHandle,
        field_type: typed_trees::types::TypeReferenceHandle,
        field_symbol: SymbolHandle,
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
        // Identity does not widen a value or discharge a qualification on the
        // measure's input/result. Both must share one unsigned carrier, and
        // only range refinements can be discharged against the subject's
        // enforced bounds when the view is applied.
        let (carrier, mut constraints) = unsigned_carrier(program, parameter.type_reference)?;
        let (result, result_constraints) = unsigned_carrier(program, measure.return_type)?;
        if result != carrier {
            return None;
        }
        constraints.extend(result_constraints);
        return Some(MeasureBodyShape::ParameterForward {
            carrier,
            constraints,
        });
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
        field_symbol: field.symbol,
    })
}

/// Unwrap only `Range` refinement shells so the carrier stays exact: other
/// constraint kinds change what the view may assume and cannot be discharged
/// here. Returns the carrier and the collected `(minimum, maximum,
/// end_inclusive)` constraints for the caller to prove against the subject.
fn unsigned_carrier(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    BuiltinTypeAtom,
    Vec<(ExpressionHandle, ExpressionHandle, bool)>,
)> {
    let mut constraints = Vec::new();
    let mut reference = reference;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints: declared,
    } = program.type_reference_table.type_reference(reference)
    {
        for constraint in program.type_reference_table.constraints(*declared) {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return None;
            };
            constraints.push((*minimum, *maximum, *end_inclusive));
        }
        reference = *base_type;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let carrier = program.symbols.builtin_type_atom(*symbol)?;
    matches!(
        carrier,
        BuiltinTypeAtom::U8 | BuiltinTypeAtom::U16 | BuiltinTypeAtom::U32 | BuiltinTypeAtom::U64
    )
    .then_some((carrier, constraints))
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
