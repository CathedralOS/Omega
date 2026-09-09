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
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
) -> Result<
    (
        &'a checked_trees::statement::TableLocalData,
        &'a checked_trees::data::DataDefinition,
    ),
    LoweringError,
> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, .. } = &state.terminator
    else {
        return unsupported("Unit case has no structural result");
    };
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("Unit case lost its result local");
    };
    let TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return unsupported("Unit case result is not a closed declaration");
    };
    let declaration = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == *symbol)
        .ok_or(LoweringError::Unsupported(
            "Unit case result declaration is absent",
        ))?;
    if result.multiplicity != Multiplicity::Affine
        || !validation::has_plain_owned_contents_with_numeric_constraints(
            &checked.typed,
            local.type_reference,
        )
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
        return unsupported("Unit case result lacks plain affine sum custody");
    }
    Ok((local, declaration))
}

pub(super) fn validate_markers(
    checked: &CheckedTrees,
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
    let (result_local, declaration) = result_source(checked, source, state)?;
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
            || !root(checked, local.initial_value, result_local.symbol)
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
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases } = &state.terminator
    else {
        return unsupported("Unit case terminator missing");
    };
    let (local, declaration) = result_source(checked, source, state)?;
    // This lane has no owned parameter cleanup: its only dying owned root is
    // the inspected boundary local. Rejoin that evidence directly rather than
    // requiring the parameter-only cleanup catalog to describe a local root.
    if state.structural_parameters.iter().any(|parameter| {
        parameter.multiplicity != Multiplicity::Unrestricted
            || !matches!(
                parameter.access,
                checked_trees::CheckedStructuralAccess::SharedBorrow
                    | checked_trees::CheckedStructuralAccess::MutableBorrow
            )
    }) {
        return unsupported("Unit case cleanup requires unrestricted borrowed parameters");
    }
    let mut has_result_discard = false;
    let expected_provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: plan.machine,
        state_symbol: state.state,
        source: language_semantics::PermissionEventSource::Statement {
            statement_index: result.statement_index as usize,
        },
    };
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
        if event.root != facts::PlaceRoot::Symbol(local.symbol)
            || event.access != language_semantics::PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || event.provenance != expected_provenance
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
        || cases.len() != tail.len()
        || cases.len() != declared.len()
        || result.binding_ordinal != 0
        || state
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                )
            })
            .count()
            != 1
    {
        return unsupported("Unit case result or complete case roster drifted");
    }
    for (offset, (case, statement)) in cases.iter().zip(tail).enumerate() {
        let StatementNode::Transition(transition) = statement else {
            return unsupported("Unit case tail contains an effect");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return unsupported("Unit case guard is not an exact case test");
        };
        let (subject, variant_symbol) = case_test(checked, guard).ok_or(
            LoweringError::Unsupported("Unit case guard lost its source identity"),
        )?;
        let variant = declared
            .iter()
            .find_map(|member| match member {
                DataMember::Variant(variant) if variant.symbol == variant_symbol => Some(variant),
                _ => None,
            })
            .ok_or(LoweringError::Unsupported(
                "Unit case guard names another sum",
            ))?;
        if !root(checked, subject, local.symbol)
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
                    root(checked, member.receiver, local.symbol)
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
                    path.head_symbol == local.symbol
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
            ordinal + offset,
            &case.payloads,
        )?;
    }
    Ok(())
}
