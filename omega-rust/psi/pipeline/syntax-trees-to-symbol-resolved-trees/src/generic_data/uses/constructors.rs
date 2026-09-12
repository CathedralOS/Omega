//! Constructor relabeling against an exact expected type.

use super::super::*;

/// Copied type references retain their original application. Reuse that exact
/// template/argument identity rather than indexing generated display spellings.
pub(in crate::generic_data) fn expected_instance<'a>(
    syntax: &SyntaxTrees,
    instances: &'a [Instantiation],
    selection: Option<&constant_selection::ConstantSelection>,
    expected: TypeReferenceHandle,
) -> Option<&'a Instantiation> {
    let expected = match syntax.type_references.type_reference(expected) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        _ => expected,
    };
    let ClosedArgumentIdentity::Instance(template, arguments) =
        closed_argument_identity(syntax, selection, expected, false)?
    else {
        return None;
    };
    let mut candidates = instances.iter().filter(|instance| {
        instance.template == template && instance.argument_identity == arguments
    });
    let instance = candidates.next()?;
    candidates.next().is_none().then_some(instance)
}

pub(in crate::generic_data) fn selected_constructor(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
    name: &Identifier,
) -> Option<(syntax_trees::item::ItemHandle, Option<Identifier>)> {
    if let Some(selection) = selection {
        let (definition, case) = selection.constructor(syntax, name).ok()?;
        let owner = syntax.root_item_handles().iter().copied().find(|handle|
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, definition)))?;
        return Some((owner, case));
    }
    if let Some(owner) = selected_data_item(syntax, None, name) {
        return Some((owner, None));
    }
    let (carrier, case) = name.as_str().rsplit_once("::")?;
    let owner = selected_data_item(syntax, None, &Identifier::new(carrier, name.source_span()))?;
    let Item::Data(data) = syntax.root_item(owner) else {
        return None;
    };
    syntax.items.data_members(data.members).iter().any(|member|
        matches!(member, DataMember::Variant(variant) if variant.name.as_str() == case))
        .then(|| {
            // The syntax-only entry selects the same declaration as the header
            // route, but the case receipt still names only the final token.
            let mut span = name.source_span();
            if span.span.end >= case.len() {
                span.span.start = span.span.end - case.len();
            }
            (owner, Some(Identifier::new(case, span)))
        })
}

pub(in crate::generic_data) fn selected_case_value(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
    name: &Identifier,
) -> Option<(syntax_trees::item::ItemHandle, Option<Identifier>)> {
    if let Some(selection) = selection {
        let (definition, case) = selection.bare_case(syntax, name).ok()??;
        let owner = syntax.root_item_handles().iter().copied().find(|handle|
            matches!(syntax.root_item(*handle), Item::Data(data) if std::ptr::eq(data, definition)))?;
        return Some((owner, Some(case)));
    }
    // The syntax-only root entry has no namespace header. Keep any root value
    // prefix authored for ordinary resolution, just as the header route does.
    let (prefix, _) = name.as_str().split_once("::")?;
    if syntax.root_items().any(|item| matches!(item, Item::Const(constant) if crate::constant::semantic_const_name(constant) == prefix)) {
        return None;
    }
    selected_constructor(syntax, None, name)
}

pub(in crate::generic_data) fn constructor_path_name(
    syntax: &SyntaxTrees,
    path: HandleSpan<Identifier>,
) -> Option<Identifier> {
    let members = syntax.expressions.identifier_path_members(path);
    let first = members.first()?;
    let last = members.last()?;
    let mut span = first.source_span();
    span.span.end = last.source_span().span.end;
    Some(Identifier::new(
        members
            .iter()
            .map(Identifier::as_str)
            .collect::<Vec<_>>()
            .join("::"),
        span,
    ))
}

/// Keep the authored carrier occurrence separate from the generated lookup
/// spelling and final case. Qualified carriers retain their complete prefix.
pub(in crate::generic_data) fn constructor_carrier_span(
    syntax: &SyntaxTrees,
    path: HandleSpan<Identifier>,
) -> Option<source::SourceSpan> {
    let members = syntax.expressions.identifier_path_members(path);
    let (_, carrier) = members.split_last()?;
    let mut span = carrier.first()?.source_span();
    span.span.end = carrier.last()?.source_span().span.end;
    Some(span)
}

/// Derive lookup metadata in the constructor's own source context, after its
/// authored template has been selected. The roster remains the identity owner.
pub(in crate::generic_data) fn closed_constructor_carrier(
    syntax: &SyntaxTrees,
    selection: Option<&constant_selection::ConstantSelection>,
    instance: &Instantiation,
    constructor: &Identifier,
    has_case: bool,
) -> Option<Identifier> {
    let carrier = if has_case {
        constructor.as_str().rsplit_once("::")?.0
    } else {
        constructor.as_str()
    };
    let carrier = Identifier::new(carrier, constructor.source_span());
    let lookup = if let Some(selection) = selection {
        let path = selection.data_lookup_path(syntax, &carrier)?;
        path.rsplit_once("::").map_or_else(
            || instance.synthetic_name.clone(),
            |(prefix, _)| format!("{prefix}::{}", instance.synthetic_name),
        )
    } else {
        instance.synthetic_name.clone()
    };
    Some(Identifier::new(lookup, constructor.source_span()))
}

pub(in crate::generic_data) fn relabel_data_literal_for_expected_type(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    instances: &[Instantiation],
    selection: Option<&constant_selection::ConstantSelection>,
    frontier: &ConstructorFrontier<'_>,
) {
    // The destination constrains each result arm, not the dispatch subject or
    // patterns. Keep the match intact for ordinary coverage, ownership and
    // selective execution; only its value-producing children inherit this type.
    if let ExpressionNode::Match(dispatch) = syntax.expressions.expression(expression) {
        let arms = syntax.expressions.match_arms(dispatch.arms).to_vec();
        for arm in arms {
            relabel_data_literal_for_expected_type(
                syntax,
                arm.value,
                expected_type,
                instances,
                selection,
                frontier,
            );
        }
        return;
    }
    if let ExpressionNode::Name(path) = syntax.expressions.expression(expression)
        && frontier.captures(syntax, *path)
    {
        return;
    }
    let expected_type = match syntax.type_references.type_reference(expected_type) {
        TypeReferenceNode::Constrained { base_type, .. } => *base_type,
        _ => expected_type,
    };
    let instance = expected_instance(syntax, instances, selection, expected_type);
    let expected_owner = if let Some(instance) = instance {
        instance.declaration
    } else {
        let TypeReferenceNode::Named(name) = syntax.type_references.type_reference(expected_type)
        else {
            return;
        };
        let Some(owner) = selected_data_item(syntax, selection, name) else {
            return;
        };
        owner
    };
    let Item::Data(definition) = syntax.root_item(expected_owner) else {
        return;
    };
    let definition = definition.clone();
    let authored_name = match syntax.expressions.expression(expression) {
        ExpressionNode::Name(path) => constructor_path_name(syntax, *path),
        ExpressionNode::StructLiteral(literal) => Some(literal.constructor_name.clone()),
        _ => None,
    };
    let Some(authored_name) = authored_name else {
        return;
    };
    let selected = if matches!(
        syntax.expressions.expression(expression),
        ExpressionNode::Name(_)
    ) {
        selected_case_value(syntax, selection, &authored_name)
    } else {
        selected_constructor(syntax, selection, &authored_name)
    };
    let Some((owner, case)) = selected else {
        return;
    };
    if owner != instance.map_or(expected_owner, |instance| instance.template) {
        return;
    }
    let closed = if let Some(instance) = instance {
        let Some(closed) =
            closed_constructor_carrier(syntax, selection, instance, &authored_name, case.is_some())
        else {
            return;
        };
        closed
    } else {
        Identifier::new(definition.name.as_str(), authored_name.source_span())
    };
    if let ExpressionNode::Name(path) = syntax.expressions.expression(expression) {
        if instance.is_some()
            && let Some(case) = case
            && let Some(carrier_span) = constructor_carrier_span(syntax, *path)
        {
            let path = closed_sum_path(syntax, closed, carrier_span, case);
            syntax
                .expressions
                .replace_expression(expression, ExpressionNode::Name(path));
        }
        return;
    }
    let ExpressionNode::StructLiteral(mut literal) =
        syntax.expressions.expression(expression).clone()
    else {
        return;
    };
    let case_name = case.as_ref().map(|case| case.as_str().to_owned());
    if instance.is_some() {
        let name = case_name.as_ref().map_or_else(
            || closed.as_str().to_owned(),
            |case| format!("{closed}::{case}"),
        );
        literal.constructor_name = Identifier::new(name, authored_name.source_span());
        syntax
            .expressions
            .replace_expression(expression, ExpressionNode::StructLiteral(literal.clone()));
    }

    let mut declared_fields = syntax
        .tables
        .items
        .data_members(definition.members)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => {
                Some((field.name.as_str().to_owned(), field.type_reference))
            }
            DataMember::Variant(_) | DataMember::Retired(_) => None,
        })
        .collect::<Vec<_>>();
    if let Some(case_name) = case_name.as_ref()
        && let Some(variant) = syntax
            .tables
            .items
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == case_name.as_str() => {
                    Some(variant)
                }
                _ => None,
            })
    {
        declared_fields.extend(
            syntax
                .tables
                .items
                .data_payload_fields(variant.payload)
                .iter()
                .map(|field| (field.name.as_str().to_owned(), field.type_reference)),
        );
    }
    let authored = syntax.expressions.struct_fields(literal.fields).to_vec();
    for field in authored {
        let Some((_, field_type)) = declared_fields
            .iter()
            .find(|(name, _)| name == field.name.as_str())
        else {
            continue;
        };
        relabel_data_literal_for_expected_type(
            syntax,
            field.value,
            *field_type,
            instances,
            selection,
            frontier,
        );
    }
}
