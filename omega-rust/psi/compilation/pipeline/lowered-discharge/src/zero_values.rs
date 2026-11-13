//! Zero initialization owes the declaration's constraints as goals. No input
//! constraint is a premise until an ordinary source witness has established it.

use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use semantic_vocabulary::MachineId;
use structure_elaboration::{
    SourceExpressionAssumptions, SourceExpressionBinding, SourceExpressionContext,
    SourceExpressionGoal, SourceExpressionValue, source_zero_projection_bindings,
    source_zero_scalar,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use typed_trees::{
    TypedTrees,
    data::{DataDefinition, DataMember, TypeParameter, TypeParameterKind},
    domain::{DomainDefinition, ProofFact},
    expression::{ExpressionHandle, ExpressionNode},
    types::{DomainConstraint, TypeReferenceHandle, TypeReferenceNode},
};

/// Establish a data declaration's own predicates over its raw zero fields.
/// Nested or refined carriers the ordinary reader cannot represent refuse.
pub fn prove_data_zero(
    typed: &TypedTrees,
    definition: &DataDefinition,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    if typed.data_definition_by_symbol(definition.symbol) != Some(definition) {
        return Err(unavailable(
            "zero data declaration is not the retained source row",
        ));
    }
    if !definition.type_parameters.is_empty() || definition.requires_facts.is_empty() {
        return Err(unavailable(
            "zero data predicate needs a closed declaration and declared constraints",
        ));
    }
    let mut actuals = Vec::new();
    let mut types = Vec::new();
    if let Some(application) = definition.generic_instance {
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = typed.type_reference_table.type_reference(application)
        else {
            return Err(unavailable(
                "zero data instance lost its retained application",
            ));
        };
        let template = typed
            .data_definition_by_symbol(*base_symbol)
            .ok_or_else(|| unavailable("zero data instance lost its source template"))?;
        static_actuals(
            typed,
            typed.data_type_parameters(template),
            typed
                .type_reference_table
                .type_reference_handles(*arguments),
            &mut types,
            &mut actuals,
        )?;
    }
    for member in typed.data_members(definition) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            return Err(unavailable(format!(
                "zero construction of field `{}` needs explicit erased evidence; erased fields have no zero value",
                field.name
            )));
        }
        let value = source_zero_scalar(typed, field.type_reference).ok_or_else(|| unavailable(format!(
            "zero field `{}` needs a represented raw value before its type constraints can be used", field.name
        )))?;
        actuals.push(Actual {
            symbol: field.symbol,
            carrier: field.type_reference,
            value: ActualValue::Constant(value),
        });
    }
    let bindings = bindings(&actuals);
    for fact in typed.conjunctive_proof_facts(definition.requires_facts) {
        let ProofFact::Expression(expression) = fact else {
            return Err(unavailable(
                "zero data predicate needs a represented membership or proposition goal",
            ));
        };
        let projections = source_zero_projection_bindings(typed, definition.symbol, expression)?;
        let mut context = SourceExpressionContext::declaration(definition.symbol);
        context.assumptions = SourceExpressionAssumptions::Explicit(&[]);
        context.types = &types;
        context.bindings = &bindings;
        context.projections = &projections;
        crate::prove_source_expression(
            typed,
            expression,
            SourceExpressionGoal::Establish,
            context,
            activity,
        )?;
    }
    Ok(())
}

/// Establish the exact selected domain instance for the carrier's zero value.
/// Aliases and direct subject memberships retain their declared obligations.
pub fn prove_domain_zero(
    typed: &TypedTrees,
    carrier: TypeReferenceHandle,
    constraint: &DomainConstraint,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    if !constraint.establishment_routes.is_empty() || !constraint.semantic_roles.is_empty() {
        return Err(unavailable(
            "zero domain admission requires explicit establishment or semantic authority",
        ));
    }
    let definition = typed
        .domain_definition_for_instance(constraint.symbol, constraint.semantic_id)
        .ok_or_else(|| unavailable("zero domain admission lost its exact selected instance"))?;
    let arguments = typed
        .domain_arguments_for_instance(constraint.symbol, constraint.semantic_id)
        .ok_or_else(|| unavailable("zero domain admission lost its retained arguments"))?;
    if arguments != constraint.arguments {
        return Err(unavailable(
            "zero domain admission arguments differ from the selected instance",
        ));
    }
    let zero = source_zero_scalar(typed, carrier)
        .ok_or_else(|| unavailable("zero domain subject has no represented raw scalar carrier"))?;
    domain_zero(
        typed,
        definition,
        arguments,
        &zero,
        activity,
        &mut Vec::new(),
    )
}

fn domain_zero(
    typed: &TypedTrees,
    selected: &DomainDefinition,
    arguments: &[TypeReferenceHandle],
    zero: &CanonicalConstIdentity,
    activity: &mut crate::ProofSearchActivity,
    active: &mut Vec<SymbolHandle>,
) -> Checked<()> {
    if active.contains(&selected.symbol) {
        return Err(unavailable(
            "zero domain admission has a circular membership obligation",
        ));
    }
    let definition = typed
        .domain_definition(selected.symbol)
        .ok_or_else(|| unavailable("zero domain admission lost its source declaration"))?;
    if !definition.establishment_routes.is_empty()
        || !definition.semantic_roles.is_empty()
        || !selected.establishment_routes.is_empty()
        || !selected.semantic_roles.is_empty()
    {
        return Err(unavailable(
            "zero domain admission requires declared establishment or semantic authority",
        ));
    }
    let expected = source_zero_scalar(typed, selected.target_type)
        .ok_or_else(|| unavailable("zero domain target has no represented raw scalar carrier"))?;
    if zero.decode_declaration_encoding() != expected.decode_declaration_encoding() {
        return Err(unavailable(
            "zero domain subject does not have the declared target carrier",
        ));
    }
    active.push(selected.symbol);
    let result = (|| {
        if let Some(alias) = &definition.alias {
            if alias.constituents.is_empty() {
                return Err(unavailable("zero domain alias has no constituent evidence"));
            }
            for constituent in &alias.constituents {
                let nested = typed
                    .domain_definition(constituent.domain_symbol)
                    .ok_or_else(|| {
                        unavailable("zero domain alias lost a constituent declaration")
                    })?;
                domain_zero(typed, nested, &[], zero, activity, active)?;
            }
            return Ok(());
        }
        let mut types = Vec::new();
        let mut actuals = Vec::new();
        static_actuals(
            typed,
            typed.domain_type_parameters(definition),
            arguments,
            &mut types,
            &mut actuals,
        )?;
        actuals.push(Actual {
            symbol: definition.subject_symbol,
            carrier: definition.target_type,
            value: ActualValue::Constant(zero.clone()),
        });
        let bindings = bindings(&actuals);
        let facts = typed
            .proof_facts
            .span(definition.facts)
            .ok_or_else(|| unavailable("zero domain declaration has a stale predicate span"))?;
        for fact in facts {
            match fact {
                ProofFact::Expression(expression) => {
                    let projections =
                        source_zero_projection_bindings(typed, definition.symbol, *expression)?;
                    let mut context = SourceExpressionContext::declaration(definition.symbol);
                    context.assumptions = SourceExpressionAssumptions::Explicit(&[]);
                    context.types = &types;
                    context.bindings = &bindings;
                    context.projections = &projections;
                    crate::prove_source_expression(
                        typed,
                        *expression,
                        SourceExpressionGoal::Establish,
                        context,
                        activity,
                    )?;
                }
                ProofFact::Membership(membership) => {
                    if !matches!(typed.expression_table.expression(membership.value), ExpressionNode::Name(path) if path.symbol == definition.subject_symbol && typed.expression_table.name_path_members(path.members).len() == 1)
                    {
                        return Err(unavailable(
                            "zero membership needs the retained expression substitution for its subject",
                        ));
                    }
                    let nested = typed
                        .domain_definition_for_instance(
                            membership.domain_symbol,
                            membership.semantic_domain,
                        )
                        .ok_or_else(|| {
                            unavailable("zero membership lost its exact domain instance")
                        })?;
                    let arguments = typed
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments);
                    domain_zero(typed, nested, arguments, zero, activity, active)?;
                }
                ProofFact::Proposition(_) => {
                    return Err(unavailable(
                        "zero domain predicate needs retained proposition evidence",
                    ));
                }
            }
        }
        Ok(())
    })();
    active.pop();
    result
}

enum ActualValue {
    Constant(CanonicalConstIdentity),
    Expression(ExpressionHandle),
    Symbol(SymbolHandle),
}
struct Actual {
    symbol: SymbolHandle,
    carrier: TypeReferenceHandle,
    value: ActualValue,
}

fn bindings(actuals: &[Actual]) -> Vec<SourceExpressionBinding<'_>> {
    actuals
        .iter()
        .map(|actual| {
            SourceExpressionBinding::input(
                actual.symbol,
                actual.carrier,
                match &actual.value {
                    ActualValue::Constant(value) => SourceExpressionValue::Constant(value),
                    ActualValue::Expression(expression) => {
                        SourceExpressionValue::Expression(*expression)
                    }
                    ActualValue::Symbol(symbol) => SourceExpressionValue::Symbol(*symbol),
                },
            )
        })
        .collect()
}

fn static_actuals(
    typed: &TypedTrees,
    parameters: &[TypeParameter],
    arguments: &[TypeReferenceHandle],
    types: &mut Vec<(SymbolHandle, TypeReferenceHandle)>,
    actuals: &mut Vec<Actual>,
) -> Checked<()> {
    if parameters.len() != arguments.len() {
        return Err(unavailable(
            "zero declaration's static argument count differs from its source binders",
        ));
    }
    for (parameter, argument) in parameters.iter().zip(arguments) {
        if !typed.contains_type_reference(*argument) {
            return Err(unavailable("zero declaration has a stale static argument"));
        }
        if parameter.kind == TypeParameterKind::Type {
            types.push((parameter.symbol, *argument));
        }
    }
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let (TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference }) = parameter.kind
        else {
            continue;
        };
        let carrier =
            typed_trees::static_applications::project_type_operand(typed, type_reference, types);
        let value = match typed.type_reference_table.type_reference(*argument) {
            TypeReferenceNode::ConstExpression(expression) => ActualValue::Expression(*expression),
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                ActualValue::Symbol(*symbol)
            }
            TypeReferenceNode::Named { name, .. } => {
                let value = CanonicalConstValue::from_atom(name.as_str()).ok_or_else(|| unavailable("zero declaration static argument lost its canonical value or source expression"))?;
                ActualValue::Constant(value.identity())
            }
            _ => {
                return Err(unavailable(
                    "zero declaration static argument has no retained value computation",
                ));
            }
        };
        actuals.push(Actual {
            symbol: parameter.symbol,
            carrier,
            value,
        });
    }
    Ok(())
}

fn unavailable(detail: impl Into<String>) -> Refusal {
    Refusal {
        machine: MachineId::new(1).expect("one is nonzero"),
        concerns: Construct::Machine,
        reason: RefusalReason::Unsupported,
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::{
        BuiltinTypeAtom, SymbolKind, SymbolNameRef, SymbolTableBuilder, builtin_type_symbols,
    };
    use typed_trees::{
        data::DataField,
        expression::{BinaryExpression, BinaryOperator, Expression, MemberExpression, NamePath},
        name::Identifier,
    };

    fn zero_predicate(expected: bool, through_subject: bool) -> TypedTrees {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            builtin_type_symbols()
                .into_iter()
                .chain([(SymbolKind::Data, SymbolNameRef::Static("Flag"))]),
        );
        let owner = declarations.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
        let fields = symbols.insert_children(
            owner,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("self")),
                (SymbolKind::Field, SymbolNameRef::Static("ready")),
            ],
        );
        let subject = fields.handle_at(0);
        let field = fields.handle_at(1);
        let mut typed = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let boolean = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: declarations.handle_at(BuiltinTypeAtom::Bool.ordinal() as u32),
            name: Identifier::generated("bool"),
        });
        let carrier = typed.type_reference_table.insert(TypeReferenceNode::Named {
            symbol: owner,
            name: Identifier::generated("Flag"),
        });
        let read = if through_subject {
            Expression::Member(Box::new(MemberExpression {
                receiver: Expression::Name(NamePath::resolved(
                    vec![Identifier::generated("self")],
                    subject,
                    subject,
                )),
                member_symbol: field,
                member: Identifier::generated("ready"),
                case_variant: None,
            }))
        } else {
            Expression::Name(NamePath::resolved(
                vec![Identifier::generated("ready")],
                field,
                field,
            ))
        };
        let expression = typed
            .expression_table
            .insert_tree(&Expression::Binary(Box::new(BinaryExpression {
                left: read,
                operator: BinaryOperator::Equal,
                right: Expression::Boolean(expected),
            })));
        let facts = typed
            .proof_facts
            .insert_many([ProofFact::Expression(expression)]);
        let members = typed
            .data_members
            .insert_many([DataMember::Field(DataField {
                symbol: field,
                name: Identifier::generated("ready"),
                type_reference: boolean,
                ..Default::default()
            })]);
        typed.push_data_definition(DataDefinition {
            symbol: owner,
            name: Identifier::generated("Flag"),
            predicate_subject_symbol: subject,
            predicate_subject_type: carrier,
            requires_facts: facts,
            zero_gated: true,
            members,
            ..Default::default()
        });
        typed
    }

    #[test]
    fn zero_fields_establish_only_the_constraints_their_values_satisfy() {
        for through_subject in [false, true] {
            let typed = zero_predicate(false, through_subject);
            prove_data_zero(
                &typed,
                &typed.data_definitions()[0],
                &mut Default::default(),
            )
            .unwrap();
            let typed = zero_predicate(true, through_subject);
            let refused = prove_data_zero(
                &typed,
                &typed.data_definitions()[0],
                &mut Default::default(),
            )
            .unwrap_err();
            assert!(
                matches!(refused.reason, RefusalReason::Unproved(_)),
                "{refused:?}"
            );
        }
    }

    #[test]
    fn a_projection_cannot_supply_a_nonzero_value_as_raw_zero() {
        let typed = zero_predicate(false, true);
        let definition = &typed.data_definitions()[0];
        let expression = *typed.proof_facts.span_or_empty(definition.requires_facts)[0]
            .as_expression()
            .unwrap();
        let mut projections =
            source_zero_projection_bindings(&typed, definition.symbol, expression).unwrap();
        assert_eq!(projections.len(), 1);
        projections[0].value = CanonicalConstValue::boolean(true).identity();
        let mut context = SourceExpressionContext::declaration(definition.symbol);
        context.assumptions = SourceExpressionAssumptions::Explicit(&[]);
        context.projections = &projections;
        let refused = crate::prove_source_expression(
            &typed,
            expression,
            SourceExpressionGoal::Establish,
            context,
            &mut Default::default(),
        )
        .unwrap_err();
        assert_eq!(refused.reason, RefusalReason::Unsupported);
        assert!(
            refused.detail.contains("canonical scalar zero"),
            "{refused:?}"
        );
    }
}
