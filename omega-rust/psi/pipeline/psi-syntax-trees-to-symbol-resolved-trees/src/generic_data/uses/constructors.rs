//! Constructor relabeling against an exact expected type.

use super::super::*;

pub(in crate::generic_data) fn relabel_data_literal_for_expected_type(
    syntax: &mut SyntaxTrees,
    expression: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    synthesized_origins: &HashMap<String, String>,
) {
    let expected_type = match syntax
        .tables
        .type_references
        .type_reference(expected_type)
        .clone()
    {
        TypeReferenceNode::Constrained { base_type, .. } => base_type,
        TypeReferenceNode::Named(_) => expected_type,
        _ => return,
    };
    let TypeReferenceNode::Named(expected_name) = syntax
        .tables
        .type_references
        .type_reference(expected_type)
        .clone()
    else {
        return;
    };
    let Some(definition) = syntax.root_items().find_map(|item| match item {
        Item::Data(definition) if definition.name.as_str() == expected_name.as_str() => {
            Some(definition.clone())
        }
        _ => None,
    }) else {
        return;
    };
    if let ExpressionNode::Name(path) = syntax.expressions.expression(expression).clone() {
        let members = syntax.expressions.identifier_path_members(path);
        let Some(base) = synthesized_origins.get(expected_name.as_str()) else {
            return;
        };
        if let [literal_base, case] = members
            && literal_base.as_str() == base
            && syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .any(|member| matches!(member, DataMember::Variant(variant) if variant.name == *case))
        {
            let case = case.clone();
            let path = closed_sum_path(syntax, expected_name.as_str(), case);
            syntax.expressions.replace_expression(
                expression,
                ExpressionNode::Name(path),
            );
        }
        return;
    }
    let ExpressionNode::StructLiteral(mut literal) =
        syntax.expressions.expression(expression).clone()
    else {
        return;
    };

    let literal_names_expected = literal.type_name.as_str() == expected_name.as_str();
    let literal_names_generic_origin = synthesized_origins
        .get(expected_name.as_str())
        .is_some_and(|base| literal.type_name.as_str() == base.as_str());
    if !literal_names_expected && !literal_names_generic_origin {
        return;
    }
    if literal_names_generic_origin {
        literal.type_name = Identifier::generated(expected_name.as_str());
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
    if let Some(case_name) = literal.case_name.as_ref()
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
            synthesized_origins,
        );
    }
}
