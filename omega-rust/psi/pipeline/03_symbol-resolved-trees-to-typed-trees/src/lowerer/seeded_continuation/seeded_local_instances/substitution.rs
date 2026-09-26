use super::super::exact_field_symbol;
use symbol_resolved_trees::{
    SymbolResolvedTrees, domain::ProofFact, expression::BinaryOperator,
    expression::ExpressionHandle, expression::ExpressionNode, types::TypeReference,
};
use symbols::SymbolHandle;

pub(super) fn member_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &symbol_resolved_trees::data::DataMember,
    instance: &symbol_resolved_trees::data::DataMember,
) -> bool {
    use symbol_resolved_trees::data::DataMember;
    match (template, instance) {
        (DataMember::Field(template), DataMember::Field(instance)) => field_matches(
            source,
            template_owner,
            instance_owner,
            substitutions,
            validated_instances,
            template,
            instance,
        ),
        (DataMember::Variant(template), DataMember::Variant(instance)) => variant_matches(
            source,
            template_owner,
            instance_owner,
            substitutions,
            validated_instances,
            template,
            instance,
        ),
        _ => false,
    }
}

fn field_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &symbol_resolved_trees::data::DataField,
    instance: &symbol_resolved_trees::data::DataField,
) -> bool {
    template.identity == instance.identity
        && template.name.as_str() == instance.name.as_str()
        && template.relevance == instance.relevance
        && template.symbol != instance.symbol
        && exact_field_symbol(source, template_owner, template)
        && exact_field_symbol(source, instance_owner, instance)
        && type_matches(
            source,
            substitutions,
            validated_instances,
            &template.type_reference,
            &instance.type_reference,
        )
}

fn variant_matches(
    source: &SymbolResolvedTrees,
    template_owner: SymbolHandle,
    instance_owner: SymbolHandle,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &symbol_resolved_trees::data::DataVariant,
    instance: &symbol_resolved_trees::data::DataVariant,
) -> bool {
    let template_payload = source.data_payload_fields(template.payload);
    let instance_payload = source.data_payload_fields(instance.payload);
    template.identity == instance.identity
        && template.name.as_str() == instance.name.as_str()
        && template.retired_payload_identities == instance.retired_payload_identities
        && template.symbol != instance.symbol
        && exact_variant_symbol(source, template_owner, template)
        && exact_variant_symbol(source, instance_owner, instance)
        && template_payload.len() == instance_payload.len()
        && template_payload
            .iter()
            .zip(instance_payload)
            .all(|(template_field, instance_field)| {
                field_matches(
                    source,
                    template.symbol,
                    instance.symbol,
                    substitutions,
                    validated_instances,
                    template_field,
                    instance_field,
                )
            })
        && case_where_facts_match(
            source,
            substitutions,
            validated_instances,
            template.where_facts,
            instance.where_facts,
        )
}

/// Replay one case `where` fact span. The instance carries the template's
/// facts in order, each fact replaying structurally, with two exceptions a
/// faithful synthesis can make: a `const` binder arrives as its literal
/// argument, and a decided type-parameter equality (`T == i32`) is discharged
/// conjunct-by-conjunct -- a proved conjunct drops out of the carried `and`
/// chain (a fact with no remaining conjuncts is omitted entirely), while a
/// refuted conjunct collapses the whole fact to the literal `0` witness the
/// construction gate folds FALSE. A dropped non-discharged or reordered fact
/// fails the pair instead of riding the member shape check.
fn case_where_facts_match(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: arena::HandleSpan<ProofFact>,
    instance: arena::HandleSpan<ProofFact>,
) -> bool {
    let template_facts = source.proof_facts(template);
    let instance_facts = source.proof_facts(instance);
    let mut instance_index = 0usize;
    for template_fact in template_facts {
        let ProofFact::Expression(template_expression) = template_fact else {
            let Some(instance_fact) = instance_facts.get(instance_index) else {
                return false;
            };
            if !case_fact_matches(
                source,
                substitutions,
                validated_instances,
                template_fact,
                instance_fact,
            ) {
                return false;
            }
            instance_index += 1;
            continue;
        };
        let mut conjuncts = Vec::new();
        flatten_and_conjuncts(source, *template_expression, &mut conjuncts);
        let mut kept = Vec::with_capacity(conjuncts.len());
        let mut refuted = false;
        for conjunct in conjuncts {
            match decide_template_type_equality(source, substitutions, conjunct) {
                // Proved at instantiation: the conjunct discharged.
                Some(true) => {}
                // Refuted: the whole fact collapsed to the `0` witness.
                Some(false) => {
                    refuted = true;
                    break;
                }
                None => kept.push(conjunct),
            }
        }
        let Some(instance_fact) = instance_facts.get(instance_index) else {
            // Instance facts exhausted: only a fully discharged template fact
            // legitimately produces nothing -- a refuted one still owes the
            // `0` witness, and kept conjuncts owe their carried shape.
            return !refuted && kept.is_empty();
        };
        if refuted {
            let ProofFact::Expression(instance_expression) = instance_fact else {
                return false;
            };
            if !matches!(
                source
                    .tables
                    .bodies
                    .expressions
                    .expression(*instance_expression),
                ExpressionNode::Integer(literal) if literal.text() == "0"
            ) {
                return false;
            }
            instance_index += 1;
            continue;
        }
        if kept.is_empty() {
            // Discharged at instantiation: this fact produced no instance
            // fact, so the cursor does not advance.
            continue;
        }
        let ProofFact::Expression(instance_expression) = instance_fact else {
            return false;
        };
        let mut instance_conjuncts = Vec::new();
        flatten_and_conjuncts(source, *instance_expression, &mut instance_conjuncts);
        if instance_conjuncts.len() != kept.len()
            || !kept
                .iter()
                .zip(instance_conjuncts)
                .all(|(template, instance)| {
                    fact_expression_matches(
                        source,
                        substitutions,
                        validated_instances,
                        *template,
                        instance,
                    )
                })
        {
            return false;
        }
        instance_index += 1;
    }
    instance_index == instance_facts.len()
}

/// Split a resolved fact expression into its top-level `and` conjuncts.
fn flatten_and_conjuncts(
    source: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    conjuncts: &mut Vec<ExpressionHandle>,
) {
    let expressions = &source.tables.bodies.expressions;
    if let ExpressionNode::Binary(binary) = expressions.expression(expression)
        && binary.operator == BinaryOperator::And
    {
        flatten_and_conjuncts(source, binary.left, conjuncts);
        flatten_and_conjuncts(source, binary.right, conjuncts);
        return;
    }
    conjuncts.push(expression);
}

/// The symbol a fact-position `Name` leaf refers to: its own resolved symbol,
/// else the last stamped member symbol of a multi-segment path.
fn fact_name_leaf_symbol(
    source: &SymbolResolvedTrees,
    path: &symbol_resolved_trees::expression::TableNamePath,
) -> SymbolHandle {
    if path.symbol.is_valid() {
        return path.symbol;
    }
    source
        .tables
        .bodies
        .expressions
        .name_path_member_symbols(path.member_symbols)
        .last()
        .copied()
        .unwrap_or_else(SymbolHandle::invalid)
}

/// Re-derive the synthesis decision for a `T == name` / `T != name` conjunct:
/// the binder side's substituted argument must be the very symbol the other
/// side names. `None` when the conjunct is not an admitted type equality, and
/// when the opposite name has no resolved type symbol the pair fails as an
/// ordinary conjunct instead of guessing.
fn decide_template_type_equality(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    conjunct: ExpressionHandle,
) -> Option<bool> {
    let expressions = &source.tables.bodies.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(conjunct) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return None;
    }
    let side_symbol = |side| -> Option<SymbolHandle> {
        let ExpressionNode::Name(path) = expressions.expression(side) else {
            return None;
        };
        Some(fact_name_leaf_symbol(source, path))
    };
    let (Some(left), Some(right)) = (side_symbol(binary.left), side_symbol(binary.right)) else {
        return None;
    };
    let (parameter, other) = if substitutions
        .iter()
        .any(|(parameter, _)| *parameter == left)
    {
        (left, right)
    } else if substitutions
        .iter()
        .any(|(parameter, _)| *parameter == right)
    {
        (right, left)
    } else {
        return None;
    };
    if !other.is_valid() {
        return None;
    }
    let (_, argument) = substitutions
        .iter()
        .find(|(substituted, _)| *substituted == parameter)?;
    let equal = matches!(
        *argument,
        TypeReference::Named { symbol, .. } if *symbol == other
    );
    Some(match binary.operator {
        BinaryOperator::Equal => equal,
        BinaryOperator::NotEqual => !equal,
        _ => unreachable!("the equality check above admits only == or !="),
    })
}

fn case_fact_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &ProofFact,
    instance: &ProofFact,
) -> bool {
    match (template, instance) {
        (ProofFact::Expression(template), ProofFact::Expression(instance)) => {
            fact_expression_matches(
                source,
                substitutions,
                validated_instances,
                *template,
                *instance,
            )
        }
        (ProofFact::Membership(template), ProofFact::Membership(instance)) => {
            template.domain_symbol == instance.domain_symbol
                && diagnostic_names_match(
                    source.domain_path_members(template.domain),
                    source.domain_path_members(instance.domain),
                )
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.value,
                    instance.value,
                )
        }
        _ => false,
    }
}

fn diagnostic_names_match(
    template: &[symbol_resolved_trees::name::DiagnosticName],
    instance: &[symbol_resolved_trees::name::DiagnosticName],
) -> bool {
    template.len() == instance.len()
        && template
            .iter()
            .zip(instance)
            .all(|(template, instance)| template.as_str() == instance.as_str())
}

/// Structural replay for a case-fact expression. Payload field and common
/// names spell identically across the template/instance pair while their
/// symbols legitimately differ, so name positions compare spellings; a
/// template `const` binder is the one position where the instance carries a
/// different node -- its literal argument.
fn fact_expression_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: ExpressionHandle,
    instance: ExpressionHandle,
) -> bool {
    let expressions = &source.tables.bodies.expressions;
    match (
        expressions.expression(template),
        expressions.expression(instance),
    ) {
        (ExpressionNode::Integer(template), ExpressionNode::Integer(instance)) => {
            template == instance
        }
        (ExpressionNode::Boolean(template), ExpressionNode::Boolean(instance)) => {
            template == instance
        }
        (ExpressionNode::Float(template), ExpressionNode::Float(instance)) => {
            template.text() == instance.text()
        }
        (ExpressionNode::String(template), ExpressionNode::String(instance)) => {
            template == instance
        }
        (ExpressionNode::Name(template_path), instance_node) => {
            if let Some((_, argument)) = substitutions
                .iter()
                .find(|(parameter, _)| *parameter == template_path.symbol)
            {
                // The only binder a carried case fact may still name is a
                // `const` parameter, which synthesis rewrites to the literal
                // argument. Any other substituted shape is a producer bug.
                let TypeReference::Named { name, .. } = *argument else {
                    return false;
                };
                return matches!(
                    instance_node,
                    ExpressionNode::Integer(literal)
                        if literal.text() == name.as_str()
                );
            }
            let ExpressionNode::Name(instance_path) = instance_node else {
                return false;
            };
            template_path.is_self_value == instance_path.is_self_value
                && diagnostic_names_match(
                    expressions.name_path_members(template_path.members),
                    expressions.name_path_members(instance_path.members),
                )
        }
        (ExpressionNode::Binary(template), ExpressionNode::Binary(instance)) => {
            template.operator == instance.operator
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.left,
                    instance.left,
                )
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.right,
                    instance.right,
                )
        }
        (ExpressionNode::Unary(template), ExpressionNode::Unary(instance)) => {
            template.operator == instance.operator
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.operand,
                    instance.operand,
                )
        }
        (ExpressionNode::Member(template), ExpressionNode::Member(instance)) => {
            template.member.as_str() == instance.member.as_str()
                && match (&template.case_variant, &instance.case_variant) {
                    (Some(template), Some(instance)) => template.as_str() == instance.as_str(),
                    (None, None) => true,
                    _ => false,
                }
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.receiver,
                    instance.receiver,
                )
        }
        (ExpressionNode::Borrow(template), ExpressionNode::Borrow(instance)) => {
            template.access == instance.access
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.target,
                    instance.target,
                )
        }
        (ExpressionNode::Indexed(template), ExpressionNode::Indexed(instance)) => {
            fact_expression_matches(
                source,
                substitutions,
                validated_instances,
                template.collection,
                instance.collection,
            ) && fact_expression_matches(
                source,
                substitutions,
                validated_instances,
                template.index,
                instance.index,
            )
        }
        (ExpressionNode::Range(template), ExpressionNode::Range(instance)) => {
            template.end_inclusive == instance.end_inclusive
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.start,
                    instance.start,
                )
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.end,
                    instance.end,
                )
        }
        (ExpressionNode::ArrayLiteral(template), ExpressionNode::ArrayLiteral(instance)) => {
            let template_elements = expressions.expression_handles(*template);
            let instance_elements = expressions.expression_handles(*instance);
            template_elements.len() == instance_elements.len()
                && template_elements
                    .iter()
                    .zip(instance_elements)
                    .all(|(template, instance)| {
                        fact_expression_matches(
                            source,
                            substitutions,
                            validated_instances,
                            *template,
                            *instance,
                        )
                    })
        }
        (ExpressionNode::Membership(template), ExpressionNode::Membership(instance)) => {
            template.domain_symbol == instance.domain_symbol
                && diagnostic_names_match(
                    expressions.name_path_members(template.domain),
                    expressions.name_path_members(instance.domain),
                )
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.value,
                    instance.value,
                )
        }
        (ExpressionNode::Cast(template), ExpressionNode::Cast(instance)) => {
            template.domain == instance.domain
                && template.form == instance.form
                && diagnostic_names_match(
                    expressions.name_path_members(template.semantic_domain),
                    expressions.name_path_members(instance.semantic_domain),
                )
                && fact_expression_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.value,
                    instance.value,
                )
                && fact_type_reference_matches(
                    source,
                    substitutions,
                    validated_instances,
                    template.target_type,
                    instance.target_type,
                )
                && {
                    let template_arguments =
                        source.child_type_references(template.semantic_domain_arguments);
                    let instance_arguments =
                        source.child_type_references(instance.semantic_domain_arguments);
                    template_arguments.len() == instance_arguments.len()
                        && template_arguments.iter().zip(instance_arguments).all(
                            |(template, instance)| {
                                type_matches(
                                    source,
                                    substitutions,
                                    validated_instances,
                                    template,
                                    instance,
                                )
                            },
                        )
                }
        }
        (ExpressionNode::ZeroValue(template), ExpressionNode::ZeroValue(instance)) => {
            fact_type_reference_matches(
                source,
                substitutions,
                validated_instances,
                *template,
                *instance,
            )
        }
        // Proposition applications, matches, struct literals, and atomics are
        // fenced at the syntax-to-resolved lowering for generic case facts; a
        // pair that reaches here carrying one is a producer bug, not a shape
        // to equate.
        _ => false,
    }
}

/// A type reference nested inside a carried case fact replays through the same
/// substitution rules as a field type.
fn fact_type_reference_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: arena::Handle<TypeReference>,
    instance: arena::Handle<TypeReference>,
) -> bool {
    type_matches(
        source,
        substitutions,
        validated_instances,
        source.child_type_reference(template),
        source.child_type_reference(instance),
    )
}

fn exact_variant_symbol(
    source: &SymbolResolvedTrees,
    owner: SymbolHandle,
    variant: &symbol_resolved_trees::data::DataVariant,
) -> bool {
    variant.symbol.is_valid()
        && source.symbols.get(variant.symbol).kind == symbols::SymbolKind::Variant
        && source.symbols.get(variant.symbol).parent == owner
        && source.symbols.name(variant.symbol) == variant.name.as_str()
}

fn type_matches(
    source: &SymbolResolvedTrees,
    substitutions: &[(SymbolHandle, &TypeReference)],
    validated_instances: &[SymbolHandle],
    template: &TypeReference,
    instance: &TypeReference,
) -> bool {
    match template {
        TypeReference::Named { symbol, name }
            if substitutions
                .iter()
                .any(|(parameter, _)| parameter == symbol) =>
        {
            name.as_str() == source.symbols.name(*symbol)
                && substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                    .is_some_and(|(_, argument)| {
                        exact_substituted_argument_matches(source, argument, instance)
                    })
        }
        TypeReference::Named { symbol, name } => {
            symbol.is_valid()
                && name.as_str() == source.symbols.name(*symbol)
                && template == instance
        }
        TypeReference::Unit => matches!(instance, TypeReference::Unit),
        TypeReference::FixedArray(template_array) => {
            let TypeReference::FixedArray(instance_array) = instance else {
                return false;
            };
            super::const_arguments::substituted_array_length_matches(
                source,
                substitutions,
                &template_array.length,
                &instance_array.length,
            ) && type_matches(
                source,
                substitutions,
                validated_instances,
                source.child_type_reference(template_array.element_type),
                source.child_type_reference(instance_array.element_type),
            )
        }
        TypeReference::Reference(template_reference) => {
            let TypeReference::Reference(instance_reference) = instance else {
                return false;
            };
            template_reference.access == instance_reference.access
                && template_reference.lifetime == instance_reference.lifetime
                && type_matches(
                    source,
                    substitutions,
                    validated_instances,
                    source.child_type_reference(template_reference.referee),
                    source.child_type_reference(instance_reference.referee),
                )
        }
        TypeReference::Slice(template_slice) => {
            let TypeReference::Slice(instance_slice) = instance else {
                return false;
            };
            type_matches(
                source,
                substitutions,
                validated_instances,
                source.child_type_reference(template_slice.element_type),
                source.child_type_reference(instance_slice.element_type),
            )
        }
        TypeReference::Generic(template_generic) => {
            let (symbol, name) = match instance {
                TypeReference::Named { symbol, name }
                    if template_generic.lifetime_arguments.is_empty() =>
                {
                    (*symbol, name)
                }
                TypeReference::Generic(instance_generic)
                    if instance_generic.lifetime_arguments
                        == template_generic.lifetime_arguments
                        && source
                            .child_type_references(instance_generic.arguments)
                            .is_empty() =>
                {
                    (instance_generic.base_symbol, &instance_generic.base_name)
                }
                _ => return false,
            };
            symbol.is_valid()
                && name.as_str() == source.symbols.name(symbol)
                && validated_instances.contains(&symbol)
                && source
                    .data_definitions
                    .iter()
                    .find(|definition| definition.symbol == symbol)
                    .is_some_and(|definition| {
                        let Some(TypeReference::Generic(origin)) =
                            definition.generic_instance.as_ref()
                        else {
                            return false;
                        };
                        definition.lifetime_parameters.len()
                            == template_generic.lifetime_arguments.len()
                            && origin.base_symbol == template_generic.base_symbol
                            && origin.base_name.as_str() == template_generic.base_name.as_str()
                            && origin.lifetime_arguments.is_empty()
                            && {
                                let template_arguments =
                                    source.child_type_references(template_generic.arguments);
                                let instance_arguments =
                                    source.child_type_references(origin.arguments);
                                template_arguments.len() == instance_arguments.len()
                                    && template_arguments.iter().zip(instance_arguments).all(
                                        |(template_argument, instance_argument)| {
                                            type_matches(
                                                source,
                                                substitutions,
                                                validated_instances,
                                                template_argument,
                                                instance_argument,
                                            )
                                        },
                                    )
                            }
                    })
        }
        TypeReference::Constrained(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::SelfType { .. } => false,
    }
}

fn exact_substituted_argument_matches(
    source: &SymbolResolvedTrees,
    expected: &TypeReference,
    actual: &TypeReference,
) -> bool {
    match (expected, actual) {
        (TypeReference::Constrained(expected), TypeReference::Constrained(actual)) => {
            let Some(expected_constraint) = super::exact_constrained_argument(source, expected)
            else {
                return false;
            };
            let Some(actual_constraint) = super::exact_constrained_argument(source, actual) else {
                return false;
            };
            exact_substituted_argument_matches(
                source,
                source.child_type_reference(expected.base_type),
                source.child_type_reference(actual.base_type),
            ) && expected_constraint == actual_constraint
        }
        _ => expected == actual,
    }
}
