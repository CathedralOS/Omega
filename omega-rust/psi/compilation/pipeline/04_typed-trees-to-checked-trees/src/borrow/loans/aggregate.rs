use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

use super::super::tracker::BorrowOwnerSegment;

#[derive(Clone)]
pub(in crate::borrow) struct BorrowedInitializer {
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) expression: ExpressionHandle,
    pub(super) kind: BorrowedInitializerKind,
}

#[derive(Clone, Copy)]
pub(in crate::borrow) enum BorrowedInitializerKind {
    Reference {
        access: language_semantics::ReferenceAccess,
        type_reference: typed_trees::types::TypeReferenceHandle,
    },
    Aggregate {
        type_reference: typed_trees::types::TypeReferenceHandle,
    },
}

/// Resolve every structurally carried reference leaf or direct aggregate-value
/// source, together with its projection within the aggregate owner. Aggregate
/// sources are expanded later from call-return or transferred-local loan facts.
pub(super) fn borrowed_initializers(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
) -> Vec<BorrowedInitializer> {
    use typed_trees::types::TypeReferenceNode;

    // Selection contributes every possible leaf at the same result path.
    // The runtime chooses one arm, but all captured origins remain constrained
    // for the result's live range. This is not proof of an every-arm transfer;
    // retirement derives that separately from the retained value graph.
    if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
        return program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                borrowed_initializers(
                    program,
                    type_reference,
                    arm.value,
                    substitutions,
                    owner_path,
                )
            })
            .collect();
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { access, .. } => vec![BorrowedInitializer {
            owner_path: owner_path.to_vec(),
            expression,
            kind: BorrowedInitializerKind::Reference {
                access: *access,
                type_reference,
            },
        }],
        TypeReferenceNode::Constrained { base_type, .. } => {
            borrowed_initializers(program, *base_type, expression, substitutions, owner_path)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            if let Some(initializer) = aggregate_value_initializer(
                program,
                type_reference,
                expression,
                substitutions,
                owner_path,
            ) {
                return vec![initializer];
            }
            let Some(values) = program.expression_table.array_literal_node(expression) else {
                return Vec::new();
            };
            program
                .expression_table
                .expression_handles(*values)
                .iter()
                .enumerate()
                .flat_map(|(index, value)| {
                    let mut element_path = owner_path.to_vec();
                    element_path.push(BorrowOwnerSegment::FixedIndex(index));
                    borrowed_initializers(
                        program,
                        *element_type,
                        *value,
                        substitutions,
                        &element_path,
                    )
                })
                .collect()
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            if let Some(initializer) = aggregate_value_initializer(
                program,
                type_reference,
                expression,
                substitutions,
                owner_path,
            ) {
                return vec![initializer];
            }
            let Some(definition) = program.data_definition_by_symbol(*base_symbol) else {
                return Vec::new();
            };
            if let Some(application) = program.data_application_definition(type_reference) {
                return borrowed_data_literal_initializers(
                    program,
                    application,
                    expression,
                    &[],
                    owner_path,
                );
            }
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let mut nested_substitutions = substitutions.to_vec();
            nested_substitutions.extend(
                program
                    .data_type_parameters(definition)
                    .iter()
                    .zip(arguments.iter())
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            borrowed_data_literal_initializers(
                program,
                definition,
                expression,
                &nested_substitutions,
                owner_path,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, concrete)) = substitutions
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return borrowed_initializers(
                    program,
                    *concrete,
                    expression,
                    substitutions,
                    owner_path,
                );
            }
            if let Some(initializer) = aggregate_value_initializer(
                program,
                type_reference,
                expression,
                substitutions,
                owner_path,
            ) {
                return vec![initializer];
            }
            let Some(definition) = program.data_definition_by_symbol(*symbol) else {
                return Vec::new();
            };
            borrowed_data_literal_initializers(
                program,
                definition,
                expression,
                substitutions,
                owner_path,
            )
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => Vec::new(),
    }
}

fn aggregate_value_initializer(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
) -> Option<BorrowedInitializer> {
    let is_direct_aggregate_value = match program.expression_table.expression(expression) {
        ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Name(_) => true,
        ExpressionNode::Cast(cast) => !cast.form.is_recast(),
        _ => false,
    };
    if !crate::borrow::view_link::substituted_returns_borrow(
        program,
        type_reference,
        substitutions,
        &[0],
    ) || !is_direct_aggregate_value
    {
        return None;
    }
    Some(BorrowedInitializer {
        owner_path: owner_path.to_vec(),
        expression,
        kind: BorrowedInitializerKind::Aggregate { type_reference },
    })
}

fn borrowed_data_literal_initializers(
    program: &dyn ::validation::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    expression: ExpressionHandle,
    substitutions: &[(SymbolHandle, typed_trees::types::TypeReferenceHandle)],
    owner_path: &[BorrowOwnerSegment],
) -> Vec<BorrowedInitializer> {
    let Some(literal) = program.expression_table.struct_literal_node(expression) else {
        return Vec::new();
    };
    let literal_fields = program.expression_table.struct_fields(literal.fields);

    let fields: Vec<&typed_trees::data::DataField> = if let Some(case_name) = &literal.case_name {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == case_name.as_str() =>
                {
                    Some(program.data_payload_fields(variant).iter().collect())
                }
                _ => None,
            })
            .unwrap_or_default()
    } else {
        program
            .data_members(definition)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => Some(field),
                typed_trees::data::DataMember::Variant(_) => None,
            })
            .collect()
    };

    fields
        .into_iter()
        .flat_map(|field| {
            let Some(literal_field) = literal_fields
                .iter()
                .find(|literal_field| literal_field.name.as_str() == field.name.as_str())
            else {
                return Vec::new();
            };
            let mut field_path = owner_path.to_vec();
            if let Some(case_name) = &literal.case_name
                && let Some(variant) = program.data_members(definition).iter().find_map(|member| {
                    let variant = member.as_variant()?;
                    (variant.name.as_str() == case_name.as_str()).then_some(variant)
                })
            {
                field_path.push(BorrowOwnerSegment::Case(variant.symbol));
            }
            field_path.push(BorrowOwnerSegment::Field(field.symbol));
            borrowed_initializers(
                program,
                field.type_reference,
                literal_field.value,
                substitutions,
                &field_path,
            )
        })
        .collect()
}
