use super::{
    algebra_key, domain_label, is_entry_call, is_separate_call, projection_label,
    projection_plan_for_call, structural_place_label,
};
use psi_language_semantics::content::{
    ContentAlgebraIdentity, ContentCaseSegment, ContentConservationEquation,
    ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) struct NormalizationContext<'program, 'contracts> {
    pub(super) program: &'program TypedTrees,
    pub(super) projections: &'program [ContentProjectionPlan],
    pub(super) parameters: &'program [StateParameter],
    pub(super) return_type: TypeReferenceHandle,
    pub(super) contracts: &'contracts [&'contracts SignatureContract],
}

pub(super) fn normalize_equation(
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
        ExpressionNode::Borrow(_)
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
    if !type_has_domain(context.program, final_type, projection.semantic_domain)
        && !contracts_establish_domain(
            context,
            &subject,
            projection.domain,
            projection.semantic_domain,
        )
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
    semantic_domain: psi_language_semantics::SemanticDomainId,
) -> bool {
    if context
        .program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain)
        .is_none_or(|definition| definition.semantic_id != semantic_domain)
    {
        // Membership proof facts retain only the nominal family today, so a
        // contract cannot select one closed indexed application exactly.
        return false;
    }
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
    domain: psi_language_semantics::SemanticDomainId,
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
                .any(|constraint| matches!(constraint, psi_typed_trees::types::TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain))
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
    }
}
