//! Source-visible content-conservation contracts.
//!
//! The surface is deliberately closed: exact owner-unique projection calls,
//! proof-only `entry(place)`, compiler-owned `separate(...)`, and equality.
//! This module normalizes authored equations for checked facts and semantic
//! identity; it does not infer sealed introductions or custody exits.

use psi_diagnostics::Diagnostic;
use psi_language_semantics::content::{
    ContentAlgebraIdentity, ContentCaseSegment, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
    ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionPlan, ContentStructuralPlace, conservation_fingerprint,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression,
};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationSourcePlan {
    pub source_expression: ExpressionHandle,
    pub plan: ContentConservationPlan,
}

pub(crate) fn validate_content_conservation_contracts(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let projections = crate::content_projections::build_content_projection_plans(program);
    let _ = collect_content_conservation_plans(program, &projections, diagnostics);
}

pub fn build_content_conservation_plans(
    program: &TypedTrees,
) -> Vec<ContentConservationSourcePlan> {
    let projections = crate::content_projections::build_content_projection_plans(program);
    let mut diagnostics = Vec::new();
    collect_content_conservation_plans(program, &projections, &mut diagnostics)
}

fn collect_content_conservation_plans(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ContentConservationSourcePlan> {
    let mut plans = Vec::new();
    let mut proof_nodes = HashSet::new();

    for trait_definition in program.traits() {
        for signature in program.trait_machine_signatures(trait_definition) {
            let contracts = program
                .state_signature_contracts(signature)
                .iter()
                .collect::<Vec<_>>();
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::TraitRequirement,
                trait_definition.symbol,
                signature.symbol,
                &format!("{}::{}", trait_definition.name, signature.name),
                program.state_signature_parameters(signature),
                signature.return_type,
                &contracts,
                &mut proof_nodes,
                &mut plans,
                diagnostics,
            );
        }
    }

    for machine in program.machines() {
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            let mut contracts = program.state_contracts(state).iter().collect::<Vec<_>>();
            if state_index == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
            let label = if state_index == 0 {
                machine.name.to_string()
            } else {
                format!("{}::{}", machine.name, state.name)
            };
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::Machine,
                machine.symbol,
                state.symbol,
                &label,
                program.state_parameters(state),
                state.return_type,
                &contracts,
                &mut proof_nodes,
                &mut plans,
                diagnostics,
            );
        }
    }

    // These calls erase completely. Any occurrence outside a proof fact is a
    // runtime-use attempt, including a direct call to a projection machine.
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
            continue;
        };
        if proof_nodes.contains(&(handle.arena_index(), handle.generation())) {
            continue;
        }
        if is_entry_call(program, call)
            || is_separate_call(program, call)
            || projection_plan_for_call(program, projections, call).is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "proof-only content operation `{}` is used in executable expression position; `entry`, `separate`, and exact `Content<A>::project` machines are contract-only",
                call.target.as_str(),
            )));
        }
    }

    plans
}

#[allow(clippy::too_many_arguments)]
fn collect_callable_plans(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    owner_kind: ContentConservationOwnerKind,
    owner: SymbolHandle,
    callable: SymbolHandle,
    label: &str,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    proof_nodes: &mut HashSet<(u32, u32)>,
    plans: &mut Vec<ContentConservationSourcePlan>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_expression_nodes(program, *expression, proof_nodes);
            } else if let ProofFact::Membership(membership) = fact {
                collect_expression_nodes(program, membership.value, proof_nodes);
            }
        }
    }

    let mut by_algebra: HashMap<String, ExpressionHandle> = HashMap::new();
    for contract in contracts {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            if !expression_uses_content_surface(program, projections, *expression) {
                continue;
            }
            if contract.kind != SignatureContractKind::Ensures {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` uses proof-only content operations outside an `ensures` contract; entry/current conservation relates callable outcomes",
                )));
                continue;
            }

            let context = NormalizationContext {
                program,
                projections,
                parameters,
                return_type,
                contracts,
            };
            let (algebra, equation) = match normalize_equation(&context, *expression) {
                Ok(normalized) => normalized,
                Err(message) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "callable `{label}` has an invalid content-conservation contract: {message}",
                    )));
                    continue;
                }
            };
            let algebra_key = algebra_key(&algebra);
            if let Some(first) = by_algebra.insert(algebra_key, *expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` publishes more than one content-conservation equation for the same algebra (facts `{}` and `{}`); author one normalized equation per outcome row and algebra",
                    program.expression_table.display_name(first),
                    program.expression_table.display_name(*expression),
                )));
                continue;
            }
            let fingerprint = conservation_fingerprint(&algebra, &equation);
            plans.push(ContentConservationSourcePlan {
                source_expression: *expression,
                plan: ContentConservationPlan {
                    owner_kind,
                    owner,
                    callable,
                    algebra,
                    equation,
                    fingerprint,
                },
            });
        }
    }
}

struct NormalizationContext<'program, 'contracts> {
    program: &'program TypedTrees,
    projections: &'program [ContentProjectionPlan],
    parameters: &'program [StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &'contracts [&'contracts SignatureContract],
}

fn normalize_equation(
    context: &NormalizationContext<'_, '_>,
    expression: ExpressionHandle,
) -> Result<(ContentAlgebraIdentity, ContentConservationEquation), String> {
    let ExpressionNode::Binary(binary) = context.program.expression_table.expression(expression)
    else {
        return Err(
            "the contract must be one equality over exact projection and `separate(...)` terms"
                .to_owned(),
        );
    };
    if binary.operator != BinaryOperator::Equal {
        return Err("content conservation is authored with `==`; containment or scalar comparisons are not the n-ary conservation theorem".to_owned());
    }
    let (left, left_algebra) = normalize_term(context, binary.left)?;
    let (right, right_algebra) = normalize_term(context, binary.right)?;
    if left_algebra != right_algebra {
        return Err(format!(
            "the two sides select different content algebras (`{}` versus `{}`)",
            algebra_key(&left_algebra),
            algebra_key(&right_algebra),
        ));
    }
    Ok((left_algebra, ContentConservationEquation::new(left, right)))
}

fn normalize_term(
    context: &NormalizationContext<'_, '_>,
    expression: ExpressionHandle,
) -> Result<(ContentConservationTerm, ContentAlgebraIdentity), String> {
    let ExpressionNode::Call(call) = context.program.expression_table.expression(expression) else {
        return Err(format!(
            "term `{}` is not an exact content projection or `separate(...)` composition",
            context.program.expression_table.display_name(expression),
        ));
    };

    if is_separate_call(context.program, call) {
        if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
            return Err("`separate(...)` is a receiverless compiler-owned proof intrinsic and accepts no static machine arguments".to_owned());
        }
        let arguments = context
            .program
            .expression_table
            .expression_handles(call.arguments);
        if arguments.len() < 2 {
            return Err("`separate(...)` requires at least two content terms".to_owned());
        }
        let mut terms = Vec::with_capacity(arguments.len());
        let mut selected_algebra = None;
        for argument in arguments {
            let (term, algebra) = normalize_term(context, *argument)?;
            if let Some(selected) = &selected_algebra {
                if selected != &algebra {
                    return Err(format!(
                        "`separate(...)` mixes incompatible algebras (`{}` and `{}`)",
                        algebra_key(selected),
                        algebra_key(&algebra),
                    ));
                }
            } else {
                selected_algebra = Some(algebra.clone());
            }
            terms.push(term);
        }
        return Ok((
            ContentConservationTerm::separate(terms),
            selected_algebra.expect("two terms select an algebra"),
        ));
    }

    let projection = projection_plan_for_call(context.program, context.projections, call)
        .ok_or_else(|| {
            format!(
                "call `{}` is not the exact owner-unique `Content<A>::project` conformance machine; there is no generic `content(...)` intrinsic",
                context.program.expression_table.display_name(expression),
            )
        })?;
    if !call.machine_arguments.is_empty() {
        return Err("a content projection call accepts no static machine arguments".to_owned());
    }
    let [argument] = context
        .program
        .expression_table
        .expression_handles(call.arguments)
    else {
        return Err(
            "an exact content projection call requires exactly one borrowed subject place"
                .to_owned(),
        );
    };
    let subject = normalize_projection_subject(context, *argument, projection)?;
    Ok((
        ContentConservationTerm::Projection {
            domain: projection.domain,
            semantic_domain: projection.semantic_domain,
            projection_machine: projection.machine,
            projection_fingerprint: projection.fingerprint,
            subject,
        },
        projection.algebra.clone(),
    ))
}

fn normalize_projection_subject(
    context: &NormalizationContext<'_, '_>,
    expression: ExpressionHandle,
    projection: &ContentProjectionPlan,
) -> Result<ContentStructuralPlace, String> {
    let (version, borrowed) = match context.program.expression_table.expression(expression) {
        ExpressionNode::Call(call) if is_entry_call(context.program, call) => {
            if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                return Err(
                    "`entry(place)` is a receiverless compiler-owned proof intrinsic".to_owned(),
                );
            }
            let [argument] = context
                .program
                .expression_table
                .expression_handles(call.arguments)
            else {
                return Err(
                    "`entry(place)` requires exactly one parameter, `self`, or structural subplace"
                        .to_owned(),
                );
            };
            (ContentPlaceVersion::Entry, *argument)
        }
        _ => (ContentPlaceVersion::Current, expression),
    };

    // The parser intentionally erases the ordinary shared-borrow `&` marker;
    // the exact projection machine's `&Self` parameter carries that typing
    // obligation. `Mutable` is retained only for `&mut` and is not a valid
    // proof observation.
    if matches!(
        context.program.expression_table.expression(borrowed),
        ExpressionNode::Mutable(_)
    ) {
        return Err("content projection subjects must use a shared borrow (`&place`), not a mutable borrow, so the proof observation never mutates authority".to_owned());
    }
    let (root_name, root_symbol, mut segments) =
        collect_structural_place(context.program, borrowed)?;
    let (root, root_type) = if root_name == "result" {
        if version == ContentPlaceVersion::Entry {
            return Err(
                "`entry(result)` is invalid: `result` does not exist at callable entry".to_owned(),
            );
        }
        if !context.return_type.is_valid() {
            return Err(
                "the projected `result` place belongs to a callable with no result type".to_owned(),
            );
        }
        (ContentPlaceRoot::Result, context.return_type)
    } else {
        let Some((position, parameter)) =
            context
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    (root_symbol.is_valid() && parameter.symbol == root_symbol)
                        || parameter.name.as_str() == root_name
                        || (parameter.is_self && root_name == "self")
                })
        else {
            return Err(format!(
                "projection root `{root_name}` is not a callable parameter, `self`, or `result`",
            ));
        };
        (
            ContentPlaceRoot::Parameter {
                position: u32::try_from(position).expect("parameter position fits u32"),
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                is_self: parameter.is_self,
            },
            parameter.type_reference,
        )
    };

    let final_type = structural_place_type(context.program, root_type, &mut segments)?;
    retain_payload_case_segments(context.program, &mut segments);
    let carrier = crate::places::unwrapped_type_reference(context.program, final_type)
        .ok_or_else(|| "the projected structural place has no resolved carrier type".to_owned())?;
    let carrier_identity = context
        .program
        .normalized_type_identity(carrier)
        .into_string();
    if carrier_identity != projection.carrier_identity {
        return Err(format!(
            "projection `{}` expects carrier `{}`, but the selected structural place has `{carrier_identity}`",
            projection_label(context.program, projection),
            projection.carrier_identity,
        ));
    }

    let subject = ContentStructuralPlace {
        version,
        root,
        segments,
    };
    if !type_has_domain(context.program, final_type, projection.domain)
        && !contracts_establish_domain(context, &subject, projection.domain)
    {
        return Err(format!(
            "projection `{}` requires exact qualification `{}`, but `{}` is not qualified by its type or an ordinary callable contract",
            projection_label(context.program, projection),
            domain_label(context.program, projection.domain),
            structural_place_label(&subject),
        ));
    }
    Ok(subject)
}

fn collect_structural_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Result<(String, SymbolHandle, Vec<ContentPlaceSegment>), String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let names = program.expression_table.name_path_members(path.members);
            if names.is_empty() {
                return Err("an empty name path is not a structural place".to_owned());
            }
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let root_name = names[0].as_str().to_owned();
            let root_symbol = symbols.first().copied().unwrap_or(path.head_symbol);
            let mut segments = Vec::new();
            for (index, name) in names.iter().enumerate().skip(1) {
                push_structural_field(
                    program,
                    &mut segments,
                    symbols
                        .get(index)
                        .copied()
                        .unwrap_or(SymbolHandle::invalid()),
                    name.as_str(),
                );
            }
            Ok((root_name, root_symbol, segments))
        }
        ExpressionNode::Member(member) => {
            let (root, symbol, mut segments) = collect_structural_place(program, member.receiver)?;
            push_structural_field(
                program,
                &mut segments,
                member.member_symbol,
                member.member.as_str(),
            );
            Ok((root, symbol, segments))
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, symbol, mut segments) =
                collect_structural_place(program, indexed.collection)?;
            let ExpressionNode::Integer(index) = program.expression_table.expression(indexed.index)
            else {
                return Err(
                    "entry/current structural places admit only literal fixed-array indices"
                        .to_owned(),
                );
            };
            let Some(index) = index.value_u64() else {
                return Err(
                    "a structural fixed-array index must be a nonnegative `u64` literal".to_owned(),
                );
            };
            segments.push(ContentPlaceSegment::FixedIndex(index));
            Ok((root, symbol, segments))
        }
        _ => Err(format!(
            "`{}` is not a parameter, `self`, `result`, or structural subplace",
            program.expression_table.display_name(expression),
        )),
    }
}

fn push_structural_field(
    program: &TypedTrees,
    segments: &mut Vec<ContentPlaceSegment>,
    field_symbol: SymbolHandle,
    field_name: &str,
) {
    if let Some(variant_symbol) = psi_facts::payload_variant_for_field(program, field_symbol)
        && let Some(variant_name) = data_variant_name(program, variant_symbol)
    {
        segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
            symbol: variant_symbol,
            name: variant_name.to_owned(),
        }));
    }
    segments.push(ContentPlaceSegment::Field(ContentFieldSegment {
        symbol: field_symbol,
        name: field_name.to_owned(),
    }));
}

fn retain_payload_case_segments(program: &TypedTrees, segments: &mut Vec<ContentPlaceSegment>) {
    let source = std::mem::take(segments);
    for segment in source {
        if let ContentPlaceSegment::Field(field) = &segment
            && let Some(variant_symbol) =
                psi_facts::payload_variant_for_field(program, field.symbol)
            && !matches!(
                segments.last(),
                Some(ContentPlaceSegment::Case(case)) if case.symbol == variant_symbol
            )
            && let Some(variant_name) = data_variant_name(program, variant_symbol)
        {
            segments.push(ContentPlaceSegment::Case(ContentCaseSegment {
                symbol: variant_symbol,
                name: variant_name.to_owned(),
            }));
        }
        segments.push(segment);
    }
}

fn data_variant_name(program: &TypedTrees, variant_symbol: SymbolHandle) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.symbol == variant_symbol).then_some(variant.name.as_str())
        })
    })
}

fn structural_place_type(
    program: &TypedTrees,
    mut current: TypeReferenceHandle,
    segments: &mut [ContentPlaceSegment],
) -> Result<TypeReferenceHandle, String> {
    let mut active_variant = None;
    for segment in segments {
        let unwrapped = crate::places::unwrapped_type_reference(program, current)
            .ok_or_else(|| "an unresolved type appears in the structural place".to_owned())?;
        match segment {
            ContentPlaceSegment::Case(case) => {
                if active_variant.is_some() {
                    return Err("a sum-case path must select a payload field".to_owned());
                }
                let data = crate::places::data_definition_for_type(program, unwrapped)
                    .ok_or_else(|| format!("`{}` is selected from a non-sum carrier", case.name))?;
                let variant = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        psi_typed_trees::data::DataMember::Variant(variant)
                            if variant.name.as_str() == case.name =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        format!("data `{}` has no sum case `{}`", data.name, case.name)
                    })?;
                if case.symbol.is_valid() && case.symbol != variant.symbol {
                    return Err(format!(
                        "sum case `{}` does not match its resolved identity",
                        case.name
                    ));
                }
                case.symbol = variant.symbol;
                active_variant = Some(variant.symbol);
            }
            ContentPlaceSegment::Field(field) => {
                let data = crate::places::data_definition_for_type(program, unwrapped).ok_or_else(
                    || format!("`{}` is selected from a non-record carrier", field.name),
                )?;
                let selected = if let Some(variant_symbol) = active_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let psi_typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == variant_symbol).then(|| {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == field.name)
                                .map(|candidate| (candidate.symbol, candidate.type_reference))
                        })?
                    })
                } else {
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            psi_typed_trees::data::DataMember::Field(candidate)
                                if candidate.name.as_str() == field.name =>
                            {
                                Some((candidate.symbol, candidate.type_reference))
                            }
                            psi_typed_trees::data::DataMember::Variant(variant) => program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|candidate| candidate.name.as_str() == field.name)
                                .map(|candidate| (candidate.symbol, candidate.type_reference)),
                            _ => None,
                        })
                };
                let (field_symbol, field_type) = selected.ok_or_else(|| {
                    format!(
                        "data `{}` has no structural field `{}`",
                        data.name, field.name
                    )
                })?;
                if field.symbol.is_valid() && field.symbol != field_symbol {
                    return Err(format!(
                        "structural field `{}` does not match its resolved identity",
                        field.name
                    ));
                }
                field.symbol = field_symbol;
                current = field_type;
            }
            ContentPlaceSegment::FixedIndex(_) => {
                if active_variant.is_some() {
                    return Err("a sum-case path must select a payload field".to_owned());
                }
                current = match program.type_reference_table.type_reference(unwrapped) {
                    TypeReferenceNode::FixedArray { element_type, .. } => *element_type,
                    _ => {
                        return Err(
                            "a fixed index is applied to a non-fixed-array structural place"
                                .to_owned(),
                        );
                    }
                };
            }
        }
    }
    if active_variant.is_some() {
        return Err("a sum-case path must select a payload field".to_owned());
    }
    Ok(current)
}

fn contracts_establish_domain(
    context: &NormalizationContext<'_, '_>,
    subject: &ContentStructuralPlace,
    domain: SymbolHandle,
) -> bool {
    context.contracts.iter().any(|contract| {
        let allowed_kind = match (&subject.root, subject.version) {
            (ContentPlaceRoot::Result, ContentPlaceVersion::Current) => {
                contract.kind == SignatureContractKind::Ensures
            }
            (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Entry) => {
                contract.kind == SignatureContractKind::Requires
            }
            (ContentPlaceRoot::Parameter { .. }, ContentPlaceVersion::Current) => matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            ),
            (ContentPlaceRoot::Result, ContentPlaceVersion::Entry) => false,
        };
        allowed_kind
            && context
                .program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .any(|fact| {
                    let ProofFact::Membership(membership) = fact else {
                        return false;
                    };
                    membership.domain_symbol == domain
                        && collect_structural_place(context.program, membership.value)
                            .ok()
                            .is_some_and(|(root, symbol, segments)| {
                                structural_place_matches(subject, &root, symbol, &segments)
                            })
                })
    })
}

fn structural_place_matches(
    expected: &ContentStructuralPlace,
    root_name: &str,
    root_symbol: SymbolHandle,
    segments: &[ContentPlaceSegment],
) -> bool {
    let root_matches = match &expected.root {
        ContentPlaceRoot::Result => root_name == "result",
        ContentPlaceRoot::Parameter { symbol, name, .. } => {
            (*symbol == root_symbol && symbol.is_valid()) || name == root_name
        }
    };
    root_matches && expected.segments == segments
}

fn type_has_domain(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain: SymbolHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => type_has_domain(program, *referee, domain),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, psi_typed_trees::types::TypeConstraintNode::Domain(candidate) if candidate.symbol == domain))
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
    }
}

fn projection_plan_for_call<'a>(
    program: &TypedTrees,
    projections: &'a [ContentProjectionPlan],
    call: &TableCallExpression,
) -> Option<&'a ContentProjectionPlan> {
    if !call.receiver.is_valid() {
        return None;
    }
    projections.iter().find(|projection| {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == projection.machine)
        else {
            return false;
        };
        let target_matches = call.target_symbol == machine.symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.name.as_str() == call.target.as_str());
        target_matches && receiver_selects_domain(program, call.receiver, projection.domain)
    })
}

fn receiver_selects_domain(
    program: &TypedTrees,
    receiver: ExpressionHandle,
    domain: SymbolHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(receiver) else {
        return false;
    };
    if path.symbol == domain || path.head_symbol == domain {
        return true;
    }
    let Some(spelled) = program
        .expression_table
        .name_path_members(path.members)
        .last()
    else {
        return false;
    };
    let expected = domain_label(program, domain);
    spelled.as_str() == expected.rsplit("::").next().unwrap_or(expected.as_str())
}

fn is_entry_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentEntry)
}

fn is_separate_call(program: &TypedTrees, call: &TableCallExpression) -> bool {
    is_builtin_call(program, call, BuiltinFunction::ContentSeparate)
}

fn is_builtin_call(
    program: &TypedTrees,
    call: &TableCallExpression,
    function: BuiltinFunction,
) -> bool {
    !call.receiver.is_valid()
        && call.target.as_str() == function.name()
        && (!call.target_symbol.is_valid()
            || (program.symbols.get(call.target_symbol).kind
                == psi_symbols::SymbolKind::BuiltinFunction
                && program.symbols.name(call.target_symbol) == function.name()))
}

fn expression_uses_content_surface(
    program: &TypedTrees,
    projections: &[ContentProjectionPlan],
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            is_entry_call(program, call)
                || is_separate_call(program, call)
                || projection_plan_for_call(program, projections, call).is_some()
                || expression_uses_content_surface(program, projections, call.receiver)
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| {
                        expression_uses_content_surface(program, projections, *argument)
                    })
        }
        ExpressionNode::Atomic(atomic) => {
            expression_uses_content_surface(program, projections, atomic.value)
        }
        ExpressionNode::Binary(binary) => {
            expression_uses_content_surface(program, projections, binary.left)
                || expression_uses_content_surface(program, projections, binary.right)
        }
        ExpressionNode::Unary(unary) => {
            expression_uses_content_surface(program, projections, unary.operand)
        }
        ExpressionNode::Cast(cast) => {
            expression_uses_content_surface(program, projections, cast.value)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_uses_content_surface(program, projections, indexed.collection)
                || expression_uses_content_surface(program, projections, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_uses_content_surface(program, projections, member.receiver)
        }
        ExpressionNode::Mutable(inner) => {
            expression_uses_content_surface(program, projections, *inner)
        }
        ExpressionNode::Range(range) => {
            expression_uses_content_surface(program, projections, range.start)
                || expression_uses_content_surface(program, projections, range.end)
        }
        ExpressionNode::ArrayLiteral(items) => program
            .expression_table
            .expression_handles(*items)
            .iter()
            .any(|item| expression_uses_content_surface(program, projections, *item)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_uses_content_surface(program, projections, field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut HashSet<(u32, u32)>,
) {
    if !expression.is_valid() || !nodes.insert((expression.arena_index(), expression.generation()))
    {
        return;
    }
    let recurse =
        |child, nodes: &mut HashSet<(u32, u32)>| collect_expression_nodes(program, child, nodes);
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            recurse(atomic.value, nodes);
            recurse(atomic.result, nodes);
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left, nodes);
            recurse(binary.right, nodes);
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand, nodes),
        ExpressionNode::Cast(cast) => recurse(cast.value, nodes),
        ExpressionNode::Call(call) => {
            recurse(call.receiver, nodes);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument, nodes);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection, nodes);
            recurse(indexed.index, nodes);
        }
        ExpressionNode::Member(member) => recurse(member.receiver, nodes),
        ExpressionNode::Mutable(inner) => recurse(*inner, nodes),
        ExpressionNode::Range(range) => {
            recurse(range.start, nodes);
            recurse(range.end, nodes);
        }
        ExpressionNode::ArrayLiteral(items) => {
            for item in program.expression_table.expression_handles(*items) {
                recurse(*item, nodes);
            }
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value, nodes);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn algebra_key(algebra: &ContentAlgebraIdentity) -> String {
    match algebra {
        ContentAlgebraIdentity::IntervalSet { coordinate_space } => {
            format!("IntervalSet<{coordinate_space}>")
        }
        ContentAlgebraIdentity::CountedQuantity { unit } => {
            format!("CountedQuantity<{unit}>")
        }
    }
}

fn projection_label(program: &TypedTrees, projection: &ContentProjectionPlan) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == projection.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| format!("projection#{}", projection.machine.arena_index()))
}

fn domain_label(program: &TypedTrees, domain: SymbolHandle) -> String {
    program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain)
        .map(|definition| definition.name.to_string())
        .unwrap_or_else(|| format!("domain#{}", domain.arena_index()))
}

fn structural_place_label(place: &ContentStructuralPlace) -> String {
    let mut label = match &place.root {
        ContentPlaceRoot::Parameter { name, .. } => name.clone(),
        ContentPlaceRoot::Result => "result".to_owned(),
    };
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Case(case) => {
                label.push_str("::");
                label.push_str(&case.name);
            }
            ContentPlaceSegment::Field(field) => {
                label.push('.');
                label.push_str(&field.name);
            }
            ContentPlaceSegment::FixedIndex(index) => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
        }
    }
    if place.version == ContentPlaceVersion::Entry {
        format!("entry(&{label})")
    } else {
        format!("&{label}")
    }
}
