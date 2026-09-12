//! Reconstruct closed-case dispatch from the authored result and payload paths.

use super::*;
use checked_trees::data::{DataMember, DataVariant};
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use checked_trees::types::TypeReferenceNode;

fn root(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    symbol: symbols::SymbolHandle,
) -> bool {
    matches!(checked.expression_table.expression(expression), ExpressionNode::Name(path)
        if path.head_symbol == symbol
            && path.symbol == symbol
            && checked.expression_table.name_path_members(path.members).len() == 1)
}

fn case_test(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, symbols::SymbolHandle)> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    if matches!(
        checked.expression_table.expression(binary.left),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.right);
    }
    if matches!(
        checked.expression_table.expression(binary.right),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.left);
    }
    for (subject, candidate) in [(binary.left, binary.right), (binary.right, binary.left)] {
        if let ExpressionNode::Name(path) = checked.expression_table.expression(candidate)
            && checked.data_definitions().iter().any(|definition| {
                checked.data_members(definition).iter().any(|member|
                matches!(member, DataMember::Variant(variant) if variant.symbol == path.symbol))
            })
        {
            return Some((subject, path.symbol));
        }
    }
    None
}

fn identity(variant: &DataVariant) -> String {
    variant
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| variant.name.as_str().to_owned())
}

fn result_source<'a>(
    checked: &'a CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<
    (
        symbols::SymbolHandle,
        &'a checked_trees::data::DataDefinition,
        language_semantics::PermissionProvenance,
    ),
    LoweringError,
> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } = &state.terminator
    else {
        return unsupported("Unit case has no structural subject");
    };
    if subject.access != checked_trees::CheckedStructuralAccess::Owned || !subject.path.is_empty() {
        return unsupported("Unit case subject is not whole owned custody");
    }
    let (symbol, reference, provenance) = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let retained = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported("Unit case parameter missing"))?;
            let parameter = checked
                .state_parameters(source)
                .get(retained.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "Unit case source parameter missing",
                ))?;
            if retained.access != checked_trees::CheckedStructuralAccess::Owned
                || retained.multiplicity != Multiplicity::Affine
                || !retained.qualifications.is_empty()
                || retained.type_identity != subject.type_identity
            {
                return unsupported("Unit case parameter custody drifted");
            }
            (
                parameter.symbol,
                parameter.type_reference,
                language_semantics::PermissionProvenance::Unknown,
            )
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            let mut matching = state
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result,
                        discard_result_on_return: false,
                        ..
                    } if result.binding_ordinal == binding_ordinal => Some(result),
                    _ => None,
                });
            let result = matching.next().ok_or(LoweringError::Unsupported(
                "Unit case result producer missing",
            ))?;
            if matching.next().is_some()
                || result.type_identity != subject.type_identity
                || result.multiplicity != Multiplicity::Affine
            {
                return unsupported("Unit case result producer duplicated or drifted");
            }
            let Some(StatementNode::LocalData(local)) = checked
                .statement_table
                .statements(source.statement_nodes)
                .get(result.statement_index as usize)
            else {
                return unsupported("Unit case result local missing");
            };
            if local.is_mutable {
                return unsupported("Unit case local is mutable");
            }
            (
                local.symbol,
                local.type_reference,
                language_semantics::PermissionProvenance::Established {
                    machine_symbol: machine,
                    state_symbol: state.state,
                    source: language_semantics::PermissionEventSource::Statement {
                        statement_index: result.statement_index as usize,
                    },
                },
            )
        }
        _ => return unsupported("Unit case subject has unsupported ownership"),
    };
    let TypeReferenceNode::Named {
        symbol: type_symbol,
        ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return unsupported("Unit case subject is not a closed declaration");
    };
    let declaration = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *type_symbol)
        .ok_or(LoweringError::Unsupported(
            "Unit case subject declaration absent",
        ))?;
    if checked.type_multiplicity(reference) != Multiplicity::Affine
        || checked.normalized_type_identity(reference).as_str() != subject.type_identity
        || !validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, reference)
        || checked
            .data_members(declaration)
            .iter()
            .any(|member| match member {
                DataMember::Variant(variant) => {
                    checked.data_payload_fields(variant).iter().any(|field| {
                        checked
                            .primitive_type_reference(field.type_reference)
                            .is_none()
                    })
                }
                DataMember::Field(_) => true,
            })
    {
        return unsupported("Unit case lacks exact plain affine sum custody");
    }
    Ok((symbol, declaration, provenance))
}

pub(super) fn validate_markers(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    end: usize,
) -> Result<usize, LoweringError> {
    if !matches!(
        state.terminator,
        CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
    ) {
        return Ok(0);
    }
    let (result_symbol, declaration, _) = result_source(checked, machine, source, state)?;
    let statements = checked.statement_table.statements(source.statement_nodes);
    let start = state.bindings.len() + state.operations.len();
    let markers = statements
        .get(start..end)
        .ok_or(LoweringError::Unsupported("Unit case body roster drifted"))?;
    let mut spelled_fields = Vec::new();
    for statement in markers {
        let StatementNode::LocalData(local) = statement else {
            return unsupported("Unit case skipped a source effect");
        };
        let Some(encoded) = local.name.as_str().strip_prefix("__arm_destructure#V=") else {
            return unsupported("Unit case skipped a non-marker local");
        };
        let Some((variant_name, fields)) = encoded.split_once('#') else {
            return unsupported("Unit case marker has no declared case");
        };
        let variant = checked
            .data_members(declaration)
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.name.as_str() == variant_name => {
                    Some(variant)
                }
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "Unit case marker names an unknown case",
            ))?;
        let Some((fields, suffix)) = fields.split_once("~subject=") else {
            return unsupported("Unit case marker lost its subject");
        };
        let fields = fields
            .trim_end_matches('#')
            .split('#')
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        let (subject, rest) = suffix
            .split_once('#')
            .map_or((suffix, false), |(subject, tail)| {
                (subject, tail == "~rest")
            });
        let declared = checked.data_payload_fields(variant);
        if local.is_mutable
            || !root(checked, local.initial_value, result_symbol)
            || subject.parse::<u32>().is_err()
            || fields.iter().enumerate().any(|(position, field)| {
                fields[..position].contains(field)
                    || !declared
                        .iter()
                        .any(|declaration| declaration.name.as_str() == *field)
            })
            || (!rest && fields.len() != declared.len())
        {
            return unsupported("Unit case destructure marker disagrees with source declaration");
        }
        for field_name in fields {
            let field = declared
                .iter()
                .find(|field| field.name.as_str() == field_name)
                .ok_or(LoweringError::Unsupported(
                    "Unit case destructure field missing",
                ))?;
            let field_identity = field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned());
            let spelling = (identity(variant), field_identity);
            if spelled_fields.contains(&spelling) {
                return unsupported("Unit case duplicated a destructure marker field");
            }
            spelled_fields.push(spelling);
        }
    }
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &state.terminator
    else {
        return unsupported("Unit case marker lost its terminator");
    };
    if spelled_fields.iter().any(|(variant, field)| {
        !cases.iter().any(|case| {
            &case.case_identity == variant
                && case
                    .payloads
                    .iter()
                    .any(|payload| &payload.field_identity == field)
        })
    }) || cases.iter().any(|case| {
        case.payloads.iter().any(|payload| {
            !spelled_fields.iter().any(|(variant, field)| {
                variant == &case.case_identity && field == &payload.field_identity
            })
        })
    }) {
        return unsupported("Unit case destructure and payload rosters disagree");
    }
    Ok(markers.len())
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    tail: &[StatementNode],
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } = &state.terminator
    else {
        return unsupported("Unit case terminator missing");
    };
    let (result_symbol, declaration, expected_provenance) =
        result_source(checked, plan.machine, source, state)?;
    if state.structural_parameters.iter().enumerate().any(|(index, parameter)| {
        !(matches!(subject.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } if parameter_index as usize == index)) && (parameter.multiplicity != Multiplicity::Unrestricted || !matches!(parameter.access, checked_trees::CheckedStructuralAccess::SharedBorrow | checked_trees::CheckedStructuralAccess::MutableBorrow))
    }) { return unsupported("Unit case has unrelated owned parameter cleanup"); }
    let mut has_result_discard = false;
    for (_, event) in checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == plan.machine
                && event.state_symbol == state.state
                && event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
        })
    {
        if event.root != facts::PlaceRoot::Symbol(result_symbol)
            || event.access != language_semantics::PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || event.provenance != expected_provenance
            || has_result_discard
            || event.obligation_live
            || !checked
                .facts
                .flow
                .ownership
                .segments
                .span_or_empty(event.segments)
                .is_empty()
        {
            return unsupported("Unit case affine exit custody escaped its result local");
        }
        has_result_discard = true;
    }
    if !has_result_discard {
        return unsupported("Unit case result has no source affine exit custody");
    }
    let declared = checked.data_members(declaration);
    if cases.is_empty()
        || tail.is_empty()
        || cases.len() < tail.len()
        || cases.len() != declared.len()
    {
        return unsupported("Unit case result or complete case roster drifted");
    }
    let mut previous_case_order = None;
    for (offset, case) in cases.iter().enumerate() {
        let source_offset = (case.successor.statement_ordinal as usize)
            .checked_sub(ordinal)
            .ok_or(LoweringError::Unsupported(
                "Unit case source ordinal precedes dispatch",
            ))?;
        let statement = tail.get(source_offset).ok_or(LoweringError::Unsupported(
            "Unit case source ordinal exceeds dispatch",
        ))?;
        let StatementNode::Transition(transition) = statement else {
            return unsupported("Unit case tail contains an effect");
        };
        let (tested_subject, variant) = match transition.guard {
            TransitionGuardNode::When(guard) => {
                let (tested, variant_symbol) = case_test(checked, guard).ok_or(
                    LoweringError::Unsupported("Unit case guard lost its source identity"),
                )?;
                let variant = declared
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if variant.symbol == variant_symbol => {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit case guard names another sum",
                    ))?;
                (Some(tested), variant)
            }
            TransitionGuardNode::Always if source_offset + 1 == tail.len() => {
                let variant = declared
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) if identity(variant) == case.case_identity => {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or(LoweringError::Unsupported(
                        "Unit case fallback names absent case",
                    ))?;
                if tail[..source_offset]
                    .iter()
                    .any(|statement| match statement {
                        StatementNode::Transition(transition) => match transition.guard {
                            TransitionGuardNode::When(guard) => case_test(checked, guard)
                                .is_some_and(|(_, selected)| selected == variant.symbol),
                            _ => true,
                        },
                        _ => true,
                    })
                {
                    return unsupported("Unit case fallback overlaps an earlier case");
                }
                (None, variant)
            }
            _ => return unsupported("Unit case guard is not an exact case test"),
        };
        let declaration_position = declared.iter().position(|member| {
            matches!(member, DataMember::Variant(candidate) if candidate.symbol == variant.symbol)
        }).ok_or(LoweringError::Unsupported("Unit case declaration position missing"))?;
        let order = (source_offset, declaration_position);
        if previous_case_order.is_some_and(|previous| previous >= order) {
            return unsupported("Unit case roster no longer follows authored dispatch order");
        }
        previous_case_order = Some(order);
        if tested_subject.is_some_and(|tested| !root(checked, tested, result_symbol))
            || identity(variant) != case.case_identity
            || cases[..offset]
                .iter()
                .any(|previous| previous.case_identity == case.case_identity)
        {
            return unsupported("Unit case source or declaration roster drifted");
        }
        let target = plan
            .states
            .iter()
            .find(|target| target.state == case.successor.target_state)
            .ok_or(LoweringError::Unsupported("Unit case target missing"))?;
        let checked_trees::statement::TransitionTargetNode::Named { arguments, .. } =
            checked.statement_table.transition_target(transition.target)
        else {
            return unsupported("Unit case target is not named");
        };
        let arguments = checked.statement_table.expression_handles(*arguments);
        for (payload_ordinal, payload) in case.payloads.iter().enumerate() {
            let parameter = target
                .scalar_parameters
                .get(payload.target_scalar_parameter_index as usize)
                .ok_or(LoweringError::Unsupported("Unit case payload slot invalid"))?;
            let field = checked
                .data_payload_fields(variant)
                .iter()
                .find(|field| {
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned())
                        == payload.field_identity
                })
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload field missing",
                ))?;
            let argument_position = (parameter.source_position as usize)
                .checked_sub(
                    target
                        .structural_parameters
                        .iter()
                        .filter(|candidate| {
                            candidate.is_self && candidate.position < parameter.source_position
                        })
                        .count(),
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload argument position invalid",
                ))?;
            let argument = arguments
                .get(argument_position)
                .ok_or(LoweringError::Unsupported(
                    "Unit case payload source missing",
                ))?;
            let valid_path = match checked.expression_table.expression(*argument) {
                ExpressionNode::Member(member) => {
                    root(checked, member.receiver, result_symbol)
                        && member.member == field.name
                        && (member.member_symbol == field.symbol
                            || (!member.member_symbol.is_valid()
                                && member
                                    .case_variant
                                    .as_ref()
                                    .is_some_and(|name| name.as_str() == variant.name.as_str())))
                        && member
                            .case_variant
                            .as_ref()
                            .is_none_or(|name| name.as_str() == variant.name.as_str())
                }
                ExpressionNode::Name(path) => {
                    path.head_symbol == result_symbol
                        && path.symbol == field.symbol
                        && checked
                            .expression_table
                            .name_path_members(path.members)
                            .len()
                            == 2
                        && checked
                            .expression_table
                            .name_path_member_symbols(path.member_symbols)
                            .last()
                            == Some(&field.symbol)
                }
                _ => false,
            };
            if !valid_path
                || parameter.primitive_type != payload.primitive_type
                || checked.primitive_type_reference(field.type_reference)
                    != Some(payload.primitive_type)
                || case.payloads[..payload_ordinal].iter().any(|previous| {
                    previous.target_scalar_parameter_index == payload.target_scalar_parameter_index
                })
            {
                return unsupported("Unit case payload path, type, or target slot drifted");
            }
        }
        edges::validate_bindings(
            checked,
            plan,
            source,
            state,
            transition,
            &case.successor,
            case.successor.statement_ordinal as usize,
            &case.payloads,
        )?;
    }
    if tail.iter().enumerate().any(|(offset, _)| {
        !cases
            .iter()
            .any(|case| case.successor.statement_ordinal as usize == ordinal + offset)
    }) {
        return unsupported("Unit case omitted an authored arm");
    }
    Ok(())
}
