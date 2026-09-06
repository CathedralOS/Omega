//! Validate receiver-rooted owned projections in their exact attached scope.
//! Typed `Self` names the machine, not a data declaration or a frozen load.

use super::*;

/// Check every retained selector before fact-place normalization can map an
/// inherited field to its storage declaration. This also covers a suffix
/// selected through a local alias, whose canonical root may later become self.
/// Missing ordinary selectors are resolved by the subsequent exact nominal
/// projection; a conflicting retained selector must not be silently replaced.
pub(super) fn validate_selectors(
    program: &TypedTrees,
    machine: &Machine,
    mut expression: ExpressionHandle,
) -> Option<()> {
    loop {
        expression = match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Member(member) => {
                if let ExpressionNode::Name(root) =
                    program.expression_table.expression(member.receiver)
                    && root.head_symbol == machine.symbol
                {
                    crate::places::exact_self_field(program, machine, expression)?;
                } else if member.member_symbol.is_valid()
                    && member.member_symbol
                        != facts::effective_member_symbol(program, member.receiver, member)
                {
                    return None;
                }
                member.receiver
            }
            ExpressionNode::Name(_) => return Some(()),
            _ => return None,
        };
    }
}

pub(super) fn validate_owned_source(
    program: &TypedTrees,
    machine: &Machine,
    reference: TypeReferenceHandle,
    source: &FrameSourcePlace,
) -> Option<()> {
    if source.root != machine.symbol || source.segments.is_empty() {
        return validate_owned_projection(program, reference, &source.segments);
    }
    if !machine.symbol.is_valid() || !machine.attached_data_symbol.is_valid() {
        return None;
    }
    let (PlaceSegment::Field { symbol }, suffix) = source.segments.split_first()? else {
        return None;
    };
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|definition| definition.symbol == machine.attached_data_symbol);
    let definition = definitions.next()?;
    if definitions.next().is_some() || !definition.type_parameters.is_empty() {
        return None;
    }
    let mut fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.symbol == *symbol && symbol.is_valid()).then_some(field)
        });
    let field = fields.next()?;
    if fields.next().is_some() || type_reference_is_reference(program, field.type_reference) {
        // A loaded self field still requires independent frozen-load evidence.
        return None;
    }
    stored_origins::projected_type(program, field.type_reference, suffix).map(|_| ())
}
