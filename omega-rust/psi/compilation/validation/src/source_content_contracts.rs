/*
Normalize authored content clauses for static callable contract identity. The
result describes declarations; it does not prove a body preserves their content.
*/
pub mod content_conservation {
    pub(crate) mod contexts {
        use super::{ContentConservationSourcePlan, collect_callable_plans};
        use arena::Handle;
        use diagnostics::Diagnostic;
        use language_semantics::content::{ContentConservationOwnerKind, ContentProjectionPlan};
        use std::collections::HashSet;
        use std::sync::Arc;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::references::{self, TypedReferences};
        use typed_trees::signature::StateSignature;

        pub(crate) fn collect_nested_contracts(
            program: &dyn crate::CheckingInput,
            projections: &[ContentProjectionPlan],
            proof_nodes: &mut HashSet<(u32, u32)>,
            diagnostics: &mut Vec<Diagnostic>,
            checking_plans: &mut Vec<ContentConservationSourcePlan>,
        ) {
            let mut contexts = ContractContexts {
                program,
                projections,
                proof_nodes,
                diagnostics,
                checking_plans,
            };
            references::visit_program(program.typed_program(), &mut contexts);
            for signature in program.prepared_callable_signatures() {
                references::visit_signature(program.typed_program(), &mut contexts, signature);
            }
        }

        pub(crate) struct ContractContexts<'program, 'output> {
            program: &'program dyn crate::CheckingInput,
            projections: &'program [ContentProjectionPlan],
            proof_nodes: &'output mut HashSet<(u32, u32)>,
            diagnostics: &'output mut Vec<Diagnostic>,
            checking_plans: &'output mut Vec<ContentConservationSourcePlan>,
        }

        impl Extend<SymbolHandle> for ContractContexts<'_, '_> {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }

        impl TypedReferences for ContractContexts<'_, '_> {
            fn expression(&mut self, _: ExpressionHandle) -> bool {
                false
            }
            fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
                false
            }
            fn proof_fact(&mut self, _: Handle<ProofFact>) -> bool {
                false
            }

            fn callable_signature(&mut self, signature: &StateSignature) {
                let contracts = self
                    .program
                    .state_signature_contracts(signature)
                    .iter()
                    .collect::<Vec<_>>();
                let has_unvisited_fact = contracts.iter().any(|contract| {
                    self.program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .any(|fact| {
                            let root = fact.as_expression().copied().or_else(|| {
                                fact.as_membership().map(|membership| membership.value)
                            });
                            root.is_some_and(|root| {
                                !self
                                    .proof_nodes
                                    .contains(&(root.arena_index(), root.generation()))
                            })
                        })
                });
                if !has_unvisited_fact {
                    return;
                }
                collect_callable_plans(
                    self.program,
                    self.projections,
                    ContentConservationOwnerKind::Machine,
                    signature.symbol,
                    signature.symbol,
                    signature.name.as_str(),
                    self.program.state_signature_parameters(signature),
                    signature.return_type,
                    SymbolHandle::invalid(),
                    &contracts,
                    &Arc::from([]),
                    self.proof_nodes,
                    self.checking_plans,
                    self.diagnostics,
                );
            }
        }
    }
    pub(crate) mod normalization {
        use super::{
            algebra_key, domain_label, is_old_call, is_separate_call, projection_label,
            projection_plan_for_call, structural_place_label,
        };
        use crate::frozen_program::expression_table;
        use language_semantics::ReferenceAccess;
        use language_semantics::content::{
            ContentAlgebraIdentity, ContentCaseSegment, ContentConservationEquation,
            ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment,
            ContentPlaceVersion, ContentProjectionPlan, ContentStructuralPlace,
        };
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};
        use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) struct NormalizationContext<'program, 'contracts> {
            pub(crate) program: &'program dyn crate::CheckingInput,
            pub(crate) callable: SymbolHandle,
            pub(crate) projections: &'program [ContentProjectionPlan],
            pub(crate) parameters: &'program [StateParameter],
            pub(crate) return_type: TypeReferenceHandle,
            pub(crate) self_data: SymbolHandle,
            pub(crate) contracts: &'contracts [&'contracts SignatureContract],
            pub(crate) boundary_subjects: &'contracts [crate::BoundaryQualificationObligation],
        }

        pub(crate) fn normalize_equation(
            context: &NormalizationContext<'_, '_>,
            expression: ExpressionHandle,
        ) -> Result<(ContentAlgebraIdentity, ContentConservationEquation), String> {
            let identity = crate::mathematical_applications::identity_application_at(
                context.program,
                expression,
            )
            .map_err(|reason| {
                format!("content conservation requires the sealed typed core::Identity: {reason}")
            })?;
            let carrier = super::super::content_projections::content_algebra_for_type(
                context.program,
                identity.carrier,
            )
            .ok_or_else(|| {
                "content conservation identity requires an exact closed content algebra carrier"
                    .to_owned()
            })?;
            let [left, right] = identity.subjects;
            let (left, left_algebra) = normalize_term(context, left)?;
            let (right, right_algebra) = normalize_term(context, right)?;
            if left_algebra != right_algebra {
                return Err(format!(
                    "the two sides select different content algebras (`{}` versus `{}`)",
                    algebra_key(&left_algebra),
                    algebra_key(&right_algebra),
                ));
            }
            if carrier != left_algebra {
                return Err(format!(
                    "the identity carrier selects content algebra `{}` but its subjects select `{}`",
                    algebra_key(&carrier),
                    algebra_key(&left_algebra),
                ));
            }
            Ok((left_algebra, ContentConservationEquation::new(left, right)))
        }

        fn normalize_term(
            context: &NormalizationContext<'_, '_>,
            expression: ExpressionHandle,
        ) -> Result<(ContentConservationTerm, ContentAlgebraIdentity), String> {
            let Some(call) = expression_table(context.program).call_node(expression) else {
                return Err(format!(
                    "term `{}` is not an exact content projection or `separate(...)` composition",
                    expression_table(context.program).display_name(expression),
                ));
            };

            if is_separate_call(context.program, call) {
                if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                    return Err("`separate(...)` is a receiverless compiler-owned proof intrinsic and accepts no static machine arguments".to_owned());
                }
                let arguments =
                    expression_table(context.program).expression_handles(call.arguments);
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
                "call `{}` is not the exact owner-unique `Content<Owner, A>::project` conformance machine; there is no generic `content(...)` intrinsic",
                expression_table(context.program).display_name(expression),
            )
        })?;
            if !call.machine_arguments.is_empty() {
                return Err(
                    "a content projection call accepts no static machine arguments".to_owned(),
                );
            }
            let [argument] = expression_table(context.program).expression_handles(call.arguments)
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
                    projection_report_fingerprint: projection.report_fingerprint,
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
            let (version, borrowed) = match expression_table(context.program).expression(expression)
            {
                ExpressionNode::Call(call) if is_old_call(context.program, call) => {
                    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
                        return Err(
                            "`old(place)` is a receiverless compiler-owned proof intrinsic"
                                .to_owned(),
                        );
                    }
                    let [argument] =
                        expression_table(context.program).expression_handles(call.arguments)
                    else {
                        return Err(
                    "`old(place)` requires exactly one parameter, `self`, or structural subplace"
                        .to_owned(),
                );
                    };
                    (ContentPlaceVersion::Entry, *argument)
                }
                _ => (ContentPlaceVersion::Current, expression),
            };

            let borrowed = match expression_table(context.program).expression(borrowed) {
                ExpressionNode::Borrow(inner) if inner.access == ReferenceAccess::Shared => {
                    inner.target
                }
                ExpressionNode::Borrow(_) => {
                    return Err("content projection subjects must use a shared borrow (`&place`), not a mutable borrow, so the proof observation never mutates authority".to_owned());
                }
                _ => borrowed,
            };
            let (root_name, root_symbol, mut segments) =
                collect_structural_place(context.program, borrowed)?;
            let (root, root_type) = if language_core::is_reserved_result(&root_name) {
                if version == ContentPlaceVersion::Entry {
                    return Err(
                        "`old(result)` is invalid: `result` does not exist at callable entry"
                            .to_owned(),
                    );
                }
                if !context.return_type.is_valid() {
                    return Err(
                        "the projected `result` place belongs to a callable with no result type"
                            .to_owned(),
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
                            root_symbol.is_valid() && parameter.symbol == root_symbol
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
                    },
                    parameter.type_reference,
                )
            };

            let final_type = structural_place_type(
                context.program,
                root_type,
                &mut segments,
                context.self_data,
            )?;
            retain_payload_case_segments(context.program, &mut segments);
            let carrier = observation_carrier(context.program, final_type)?;
            let carrier_identity = context.program.normalized_type_identity_text(carrier);
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
                && !boundary_postcondition_forms_domain(context, &subject, projection)
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

        fn boundary_postcondition_forms_domain(
            context: &NormalizationContext<'_, '_>,
            subject: &ContentStructuralPlace,
            projection: &ContentProjectionPlan,
        ) -> bool {
            if subject.version != ContentPlaceVersion::Current || !subject.segments.is_empty() {
                return false;
            }
            context.boundary_subjects.iter().any(|obligation| {
                obligation.requirement.is_valid()
                    && obligation.realization_state == context.callable
                    && obligation.realization_state.is_valid()
                    && obligation.domain == projection.domain
                    && obligation.semantic_domain.is_valid()
                    && obligation.semantic_domain == projection.semantic_domain
                    && match &subject.root {
                        ContentPlaceRoot::Result => !obligation.parameter.is_valid(),
                        ContentPlaceRoot::Parameter { symbol, .. } => {
                            obligation.parameter.is_valid() && obligation.parameter == *symbol
                        }
                    }
            })
        }

        fn collect_structural_place(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Result<(String, SymbolHandle, Vec<ContentPlaceSegment>), String> {
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    let names = expression_table(program).name_path_members(path.members);
                    if names.is_empty() {
                        return Err("an empty name path is not a structural place".to_owned());
                    }
                    let symbols =
                        expression_table(program).name_path_member_symbols(path.member_symbols);
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
                    let (root, symbol, mut segments) =
                        collect_structural_place(program, member.receiver)?;
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
                    let Some(index) = expression_table(program).integer_node(indexed.index) else {
                        return Err(
                            "old/current structural places admit only literal fixed-array indices"
                                .to_owned(),
                        );
                    };
                    let Some(index) = index.value_u64() else {
                        return Err(
                            "a structural fixed-array index must be a nonnegative `u64` literal"
                                .to_owned(),
                        );
                    };
                    segments.push(ContentPlaceSegment::FixedIndex(index));
                    Ok((root, symbol, segments))
                }
                _ => Err(format!(
                    "`{}` is not a parameter, `self`, `result`, or structural subplace",
                    expression_table(program).display_name(expression),
                )),
            }
        }

        pub fn push_structural_field(
            program: &dyn crate::CheckingInput,
            segments: &mut Vec<ContentPlaceSegment>,
            field_symbol: SymbolHandle,
            field_name: &str,
        ) {
            if let Some(variant_symbol) = facts::payload_variant_for_field(program, field_symbol)
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

        fn retain_payload_case_segments(
            program: &dyn crate::CheckingInput,
            segments: &mut Vec<ContentPlaceSegment>,
        ) {
            let source = std::mem::take(segments);
            for segment in source {
                if let ContentPlaceSegment::Field(field) = &segment
                    && let Some(variant_symbol) =
                        facts::payload_variant_for_field(program, field.symbol)
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

        pub fn data_variant_name(
            program: &dyn crate::CheckingInput,
            variant_symbol: SymbolHandle,
        ) -> Option<&str> {
            program.data_definitions().iter().find_map(|definition| {
                program
                    .data_variant(definition, variant_symbol)
                    .map(|variant| variant.name.as_str())
            })
        }

        fn observation_carrier(
            program: &dyn crate::CheckingInput,
            mut current: TypeReferenceHandle,
        ) -> Result<TypeReferenceHandle, String> {
            for _ in 0..program.type_reference_table.type_reference_count() {
                if !current.is_valid() {
                    break;
                }
                match program.type_reference_table.type_reference(current) {
                    TypeReferenceNode::Constrained { base_type, .. } => current = *base_type,
                    TypeReferenceNode::Reference {
                        access, referee, ..
                    } if access.is_readable() => {
                        current = *referee;
                    }
                    TypeReferenceNode::Reference { .. } => {
                        return Err(
                            "content projection subjects cannot observe write-only reference storage"
                                .to_owned(),
                        );
                    }
                    _ => return Ok(current),
                }
            }
            Err("the projected structural place has no resolved readable carrier type".to_owned())
        }

        fn structural_place_type(
            program: &dyn crate::CheckingInput,
            mut current: TypeReferenceHandle,
            segments: &mut [ContentPlaceSegment],
            self_data: SymbolHandle,
        ) -> Result<TypeReferenceHandle, String> {
            let mut active_variant = None;
            let mut at_root = true;
            for segment in segments {
                let unwrapped = observation_carrier(program, current)?;
                match segment {
                    ContentPlaceSegment::Case(case) => {
                        if active_variant.is_some() {
                            return Err("a sum-case path must select a payload field".to_owned());
                        }
                        let data = crate::value_custody::places::data_definition_for_type(
                            program, unwrapped,
                        )
                        .or_else(|| {
                            (at_root && self_data.is_valid())
                                .then(|| {
                                    crate::declarations::data_queries::data_definition_by_symbol(
                                        program, self_data,
                                    )
                                })
                                .flatten()
                        })
                        .ok_or_else(|| {
                            format!("`{}` is selected from a non-sum carrier", case.name)
                        })?;
                        let variant = program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant)
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
                        let direct_data = crate::value_custody::places::data_definition_for_type(
                            program, unwrapped,
                        );
                        let through_attached_self =
                            direct_data.is_none() && at_root && self_data.is_valid();
                        let data = direct_data
                            .or_else(|| {
                                through_attached_self.then_some(()).and_then(|()| {
                                    crate::declarations::data_queries::data_definition_by_symbol(
                                        program, self_data,
                                    )
                                })
                            })
                            .ok_or_else(|| {
                                format!("`{}` is selected from a non-record carrier", field.name)
                            })?;
                        let selected = if let Some(variant_symbol) = active_variant.take() {
                            program.data_members(data).iter().find_map(|member| {
                                let variant = member.as_variant()?;
                                (variant.symbol == variant_symbol).then(|| {
                                    program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|candidate| candidate.name.as_str() == field.name)
                                        .map(|candidate| {
                                            (candidate.symbol, candidate.type_reference)
                                        })
                                })?
                            })
                        } else {
                            program
                                .data_members(data)
                                .iter()
                                .find_map(|member| match member {
                                    DataMember::Field(candidate)
                                        if candidate.name.as_str() == field.name =>
                                    {
                                        Some((candidate.symbol, candidate.type_reference))
                                    }
                                    DataMember::Variant(variant) => program
                                        .data_payload_fields(variant)
                                        .iter()
                                        .find(|candidate| candidate.name.as_str() == field.name)
                                        .map(|candidate| {
                                            (candidate.symbol, candidate.type_reference)
                                        }),
                                    _ => None,
                                })
                        };
                        let (field_symbol, field_type) = selected.ok_or_else(|| {
                            format!(
                                "data `{}` has no structural field `{}`",
                                data.name, field.name
                            )
                        })?;
                        if field.symbol.is_valid()
                            && field.symbol != field_symbol
                            && !through_attached_self
                        {
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
                at_root = false;
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
            semantic_domain: language_semantics::SemanticDomainId,
        ) -> bool {
            if context
                .program
                .domain_definition(domain)
                .is_none_or(|definition| definition.semantic_id != semantic_domain)
            {
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
                            let Some(membership) = fact.as_membership() else {
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
                ContentPlaceRoot::Result => language_core::is_reserved_result(root_name),
                ContentPlaceRoot::Parameter { symbol, name, .. } => {
                    (*symbol == root_symbol && symbol.is_valid()) || name == root_name
                }
            };
            root_matches && expected.segments == segments
        }

        fn type_has_domain(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            domain: language_semantics::SemanticDomainId,
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
                .any(|constraint| matches!(constraint, typed_trees::types::TypeConstraintNode::Domain(candidate) if candidate.semantic_id == domain))
                || type_has_domain(program, *base_type, domain)
        }
        _ => false,
            }
        }
    }

    use crate::CheckingInput;
    pub use normalization::{data_variant_name, push_structural_field};

    use crate::frozen_program::expression_table;
    use normalization::{NormalizationContext, normalize_equation};

    use diagnostics::Diagnostic;
    use language_semantics::content::{
        ContentAlgebraIdentity, ContentConservationOwnerKind, ContentConservationPlan,
        ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
        ContentProjectionPlan, ContentStructuralPlace, conservation_report_fingerprint,
    };
    use std::collections::HashSet;
    use std::sync::Arc;
    use symbols::{BuiltinFunction, SymbolHandle};
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::types::TypeReferenceHandle;

    pub use typed_trees::source_content::ContentConservationSourcePlan;

    #[derive(Debug)]
    pub(crate) struct ContentConservationInputs {
        plans: Arc<Vec<ContentConservationSourcePlan>>,
        diagnostics: Vec<Diagnostic>,
    }

    pub(crate) fn content_conservation_inputs(
        program: &dyn CheckingInput,
    ) -> Arc<ContentConservationInputs> {
        crate::frozen_program::frozen_memo(
            program,
            |memos| &memos.content_conservation,
            || {
                let projections =
                    crate::value_custody::content_projections::build_content_projection_plans(
                        program,
                    );
                let mut diagnostics = Vec::new();
                let mut checking_plans = Vec::new();
                let mut plans = collect_content_conservation_plans(
                    program,
                    &projections,
                    &mut diagnostics,
                    &mut checking_plans,
                );
                plans.extend(checking_plans);
                ContentConservationInputs {
                    plans: Arc::new(plans),
                    diagnostics,
                }
            },
        )
    }

    pub fn content_conservation_plans(
        program: &dyn CheckingInput,
    ) -> Result<Arc<Vec<ContentConservationSourcePlan>>, Diagnostic> {
        let inputs = content_conservation_inputs(program);
        if inputs.diagnostics.is_empty() {
            Ok(Arc::clone(&inputs.plans))
        } else {
            Err(Diagnostic::error(diagnostics::join_messages(
                &inputs.diagnostics,
            )))
        }
    }

    fn collect_content_conservation_plans(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        diagnostics: &mut Vec<Diagnostic>,

        checking_plans: &mut Vec<ContentConservationSourcePlan>,
    ) -> Vec<ContentConservationSourcePlan> {
        let mut plans = Vec::new();
        let mut proof_nodes = HashSet::new();
        let no_boundary_subjects: Arc<[crate::BoundaryQualificationObligation]> = Arc::from([]);

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
                    SymbolHandle::invalid(),
                    &contracts,
                    &no_boundary_subjects,
                    &mut proof_nodes,
                    &mut plans,
                    diagnostics,
                );
            }
        }

        let mut boundary_owner = SymbolHandle::invalid();
        let mut boundary_subjects = Arc::clone(&no_boundary_subjects);
        program.for_each_machine_state_contract_frame(|machine, state, contracts, label| {
            if boundary_owner != machine.symbol {
                boundary_subjects =
                    crate::boundary_provider_qualification_obligations(program, machine).into();
                boundary_owner = machine.symbol;
            }
            collect_callable_plans(
                program,
                projections,
                ContentConservationOwnerKind::Machine,
                machine.symbol,
                state.symbol,
                &label,
                program.state_parameters(state),
                state.return_type,
                machine.attached_data_symbol,
                &contracts,
                &boundary_subjects,
                &mut proof_nodes,
                &mut plans,
                diagnostics,
            );
        });

        contexts::collect_nested_contracts(
            program,
            projections,
            &mut proof_nodes,
            diagnostics,
            checking_plans,
        );

        plans
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_callable_plans(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        owner_kind: ContentConservationOwnerKind,
        owner: SymbolHandle,
        callable: SymbolHandle,
        label: &str,
        parameters: &[StateParameter],
        return_type: TypeReferenceHandle,
        self_data: SymbolHandle,
        contracts: &[&SignatureContract],
        boundary_subjects: &Arc<[crate::BoundaryQualificationObligation]>,
        proof_nodes: &mut HashSet<(u32, u32)>,
        plans: &mut Vec<ContentConservationSourcePlan>,

        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for contract in contracts {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    collect_expression_nodes(program, *expression, proof_nodes);
                } else if let Some(membership) = fact.as_membership() {
                    collect_expression_nodes(program, membership.value, proof_nodes);
                }
            }
        }

        let first_callable_plan = plans.len();
        for contract in contracts {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(expression) = fact.as_expression() else {
                    continue;
                };
                if !expression_uses_content_surface(program, projections, *expression) {
                    continue;
                }
                if contract.kind != SignatureContractKind::Ensures {
                    diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` uses proof-only content operations outside an `ensures` contract; callable-entry/current conservation relates callable outcomes",
                )));
                    continue;
                }

                let context = NormalizationContext {
                    program,
                    callable,
                    projections,
                    parameters,
                    return_type,
                    self_data,
                    contracts,
                    boundary_subjects,
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
                if let Some(first) = plans[first_callable_plan..].iter().find(|previous| {
                    previous.plan.algebra == algebra
                        && [
                            previous.plan.equation.left(),
                            previous.plan.equation.right(),
                        ]
                        .into_iter()
                        .any(|previous_term| {
                            [equation.left(), equation.right()]
                                .into_iter()
                                .any(|term| share_projection_owner(previous_term, term))
                        })
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                    "callable `{label}` publishes more than one content-conservation equation for the same qualification owner and algebra (facts `{}` and `{}`); author one normalized equation per outcome row and qualification owner",
                    expression_table(program).display_name(first.source_expression),
                    expression_table(program).display_name(*expression),
                )));
                    continue;
                }
                let report_fingerprint = conservation_report_fingerprint(&algebra, &equation);
                plans.push(ContentConservationSourcePlan {
                    source_expression: *expression,
                    plan: ContentConservationPlan {
                        owner_kind,
                        owner,
                        callable,
                        algebra,
                        equation,
                        report_fingerprint,
                    },
                });
            }
        }
    }

    fn share_projection_owner(
        left: &ContentConservationTerm,
        right: &ContentConservationTerm,
    ) -> bool {
        match (left, right) {
            (ContentConservationTerm::Separate(children), other)
            | (other, ContentConservationTerm::Separate(children)) => children
                .iter()
                .any(|child| share_projection_owner(child, other)),
            (
                ContentConservationTerm::Projection {
                    domain: left_domain,
                    semantic_domain: left_instance,
                    projection_machine: left_projection,
                    ..
                },
                ContentConservationTerm::Projection {
                    domain: right_domain,
                    semantic_domain: right_instance,
                    projection_machine: right_projection,
                    ..
                },
            ) => {
                left_domain == right_domain
                    && left_instance == right_instance
                    && left_projection == right_projection
            }
        }
    }

    fn projection_plan_for_call<'a>(
        program: &dyn CheckingInput,
        projections: &'a [ContentProjectionPlan],
        call: &TableCallExpression,
    ) -> Option<&'a ContentProjectionPlan> {
        projections.iter().find(|projection| {
            program.machine(projection.machine).is_some_and(|machine| {
                call.target_symbol == machine.symbol
                    || typed_trees::state::state_by_symbol(
                        program.machine_states(machine),
                        call.target_symbol,
                    )
                    .is_some()
            })
        })
    }

    pub(crate) fn intrinsic_result_type_reference(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
        mut operand_type: impl FnMut(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> Option<TypeReferenceHandle> {
        let function = program
            .symbols
            .builtin_function_for_symbol(call.target_symbol)?;
        if !matches!(
            function,
            BuiltinFunction::ContentOld | BuiltinFunction::ContentSeparate
        ) || call.receiver.is_valid()
            || !call.machine_arguments.is_empty()
            || !call.evidence_arguments.is_empty()
        {
            return None;
        }
        let arguments = expression_table(program).expression_handles(call.arguments);
        match function {
            BuiltinFunction::ContentOld if arguments.len() == 1 => operand_type(arguments[0]),
            BuiltinFunction::ContentSeparate if arguments.len() >= 2 => {
                let carrier = operand_type(arguments[0])?;
                arguments[1..]
                    .iter()
                    .all(|argument| {
                        operand_type(*argument).is_some_and(|other| {
                            program.normalized_type_identity(carrier)
                                == program.normalized_type_identity(other)
                        })
                    })
                    .then_some(carrier)
            }
            _ => None,
        }
    }

    fn is_old_call(program: &dyn CheckingInput, call: &TableCallExpression) -> bool {
        is_builtin_call(program, call, BuiltinFunction::ContentOld)
    }

    fn is_separate_call(program: &dyn CheckingInput, call: &TableCallExpression) -> bool {
        is_builtin_call(program, call, BuiltinFunction::ContentSeparate)
    }

    fn is_builtin_call(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
        function: BuiltinFunction,
    ) -> bool {
        !call.receiver.is_valid()
            && program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(function)
    }

    fn expression_uses_content_surface(
        program: &dyn CheckingInput,
        projections: &[ContentProjectionPlan],
        expression: ExpressionHandle,
    ) -> bool {
        program.typed_program().subtree_any(
            expression,
            typed_trees::SubtreeEdges {
                saved_sources: false,
                atomic_results: false,
                pattern_dependencies: true,
            },
            |_, node| {
                if let ExpressionNode::Call(call) = node
                    && (is_old_call(program, call)
                        || is_separate_call(program, call)
                        || projection_plan_for_call(program, projections, call).is_some())
                {
                    typed_trees::SubtreeVisit::Found
                } else {
                    typed_trees::SubtreeVisit::Descend
                }
            },
        )
    }

    fn collect_expression_nodes(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        nodes: &mut HashSet<(u32, u32)>,
    ) {
        if !expression.is_valid()
            || !nodes.insert((expression.arena_index(), expression.generation()))
        {
            return;
        }
        crate::queries::expression_traversal::children(
            program,
            expression_table(program).expression(expression),
            |child| collect_expression_nodes(program, child, nodes),
        );
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

    fn projection_label(program: &dyn CheckingInput, projection: &ContentProjectionPlan) -> String {
        program
            .machine(projection.machine)
            .map(|machine| machine.name.to_string())
            .unwrap_or_else(|| format!("projection#{}", projection.machine.arena_index()))
    }

    fn domain_label(program: &dyn CheckingInput, domain: SymbolHandle) -> String {
        program
            .domain_definition(domain)
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
            format!("old(&{label})")
        } else {
            format!("&{label}")
        }
    }
}

pub mod content_projections {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::value_custody::type_references::type_references_match;
    use language_semantics::content::{
        ContentAlgebraIdentity, ContentArithmeticOperator, ContentFieldSegment,
        ContentIntervalExpression, ContentProjectionExpression, ContentProjectionPlan,
        ContentScalarExpression, projection_report_fingerprint,
    };
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::domain::DomainDefinition;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
    use typed_trees::trait_definition::TraitDefinition;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ProjectionSubject {
        machine: SymbolHandle,
        state: SymbolHandle,
        symbol: SymbolHandle,
        qualified: TypeReferenceHandle,
        carrier: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct ProjectionDomainCandidate<'program> {
        definition: &'program DomainDefinition,
        semantic_domain: language_semantics::SemanticDomainId,
    }

    pub fn build_content_projection_plans(
        program: &dyn CheckingInput,
    ) -> Vec<ContentProjectionPlan> {
        program
            .machines()
            .iter()
            .flat_map(|machine| {
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .filter_map(move |conformance| {
                        let content = program
                            .trait_definition_by_symbol(conformance.symbol)
                            .filter(|candidate| is_content_projection_trait(program, candidate))?;
                        if conformance.requirement.as_ref().map(|name| name.as_str())
                            != Some("project")
                        {
                            return None;
                        }
                        let subject = projection_subject(program, machine)?;
                        let candidates = projection_domain_candidates(program, machine, subject);
                        let [candidate] = candidates.as_slice() else {
                            return None;
                        };
                        let domain = candidate.definition;
                        if domain.alias.is_some() || content.is_boundary {
                            return None;
                        }
                        let algebra = selected_content_algebra(program, conformance)?;
                        let expression = normalize_projection_expression(
                            program,
                            subject,
                            &algebra,
                            projection_result_expression(program, machine)?,
                        )?;
                        let report_fingerprint =
                            projection_report_fingerprint(&algebra, &expression);
                        Some(ContentProjectionPlan {
                            domain: domain.symbol,
                            semantic_domain: candidate.semantic_domain,
                            carrier_identity: program
                                .normalized_type_identity_text(subject.carrier),
                            machine: machine.symbol,
                            algebra,
                            expression,
                            report_fingerprint,
                        })
                    })
            })
            .collect()
    }

    pub(crate) fn is_content_projection_machine(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> bool {
        program
            .machine_trait_conformances(machine)
            .iter()
            .any(|conformance| {
                if conformance.requirement.as_ref().map(|name| name.as_str()) != Some("project")
                    || !program
                        .trait_definition_by_symbol(conformance.symbol)
                        .is_some_and(|candidate| is_content_projection_trait(program, candidate))
                {
                    return false;
                }
                let Some(subject) = projection_subject(program, machine) else {
                    return false;
                };
                let Some(algebra) = selected_content_algebra(program, conformance) else {
                    return false;
                };
                projection_result_expression(program, machine).is_some_and(|expression| {
                    normalize_projection_expression(program, subject, &algebra, expression)
                        .is_some()
                })
            })
    }

    fn selected_content_algebra(
        program: &dyn CheckingInput,
        conformance: &typed_trees::machine::TraitConformance,
    ) -> Option<ContentAlgebraIdentity> {
        let [_owner, argument] = program
            .type_reference_table
            .type_reference_handles(conformance.arguments)
        else {
            return None;
        };
        content_algebra_for_type(program, *argument)
    }

    pub use typed_trees::source_content::{
        ContentAlgebraApplication, content_algebra_application, content_algebra_for_type,
    };

    fn projection_result_expression(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ExpressionHandle> {
        let [state] = program.machine_states(machine) else {
            return None;
        };
        let [statement] = program.statement_table.statements(state.statement_nodes) else {
            return None;
        };
        match statement {
            StatementNode::Expression(expression) => Some(*expression),
            StatementNode::Transition(transition)
                if transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() =>
            {
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(expression) => Some(*expression),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(crate) fn content_projection_body_expression(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ExpressionHandle> {
        program
            .machine_trait_conformances(machine)
            .iter()
            .any(|conformance| {
                conformance.requirement.as_ref().map(|name| name.as_str()) == Some("project")
                    && program
                        .trait_definition_by_symbol(conformance.symbol)
                        .is_some_and(|candidate| is_content_projection_trait(program, candidate))
            })
            .then(|| projection_result_expression(program, machine))
            .flatten()
    }

    fn normalize_projection_expression(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        algebra: &ContentAlgebraIdentity,
        expression: ExpressionHandle,
    ) -> Option<ContentProjectionExpression> {
        let literal = expression_table(program).struct_literal_node(expression)?;
        let expected_name = match algebra {
            ContentAlgebraIdentity::IntervalSet { .. } => "IntervalSet",
            ContentAlgebraIdentity::CountedQuantity { .. } => "CountedQuantity",
        };
        let literal_leaf = literal
            .type_name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(literal.type_name.as_str());
        if literal_leaf != expected_name || literal.case_name.is_some() {
            return None;
        }
        let fields = expression_table(program).struct_fields(literal.fields);
        match algebra {
            ContentAlgebraIdentity::IntervalSet { .. } => {
                let [start, end] = ["start", "end"].map(|name| {
                    fields
                        .iter()
                        .find(|field| field.name.as_str() == name)
                        .and_then(|field| {
                            normalize_projection_scalar(program, subject, field.value)
                        })
                });
                (fields.len() == 2).then_some(ContentProjectionExpression::IntervalSet {
                    members: vec![ContentIntervalExpression::new(start?, end?)],
                })
            }
            ContentAlgebraIdentity::CountedQuantity { .. } => {
                let magnitude = fields
                    .iter()
                    .find(|field| field.name.as_str() == "magnitude")
                    .and_then(|field| normalize_projection_scalar(program, subject, field.value))?;
                (fields.len() == 1)
                    .then_some(ContentProjectionExpression::CountedQuantity { magnitude })
            }
        }
    }

    fn normalize_projection_scalar(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> Option<ContentScalarExpression> {
        normalize_projection_scalar_context(program, subject, expression, false)
    }

    fn normalize_projection_scalar_context(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
        within_integer_coercion: bool,
    ) -> Option<ContentScalarExpression> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Cast(cast)
                if program
                    .named_type_reference(cast.target_type)
                    .is_some_and(|name| name.as_str() == "Nat")
                    && cast.domain == numerics::arithmetic::ArithmeticDomain::Exact
                    && cast.semantic_domain.count() == 0 =>
            {
                normalize_projection_scalar_context(program, subject, cast.value, true)
            }
            ExpressionNode::Integer(value) => {
                let value = value.value_bignum()?;
                (!value.is_negative()).then(|| ContentScalarExpression::Natural(value.to_string()))
            }
            ExpressionNode::Name(path) => {
                let members = expression_table(program).name_path_members(path.members);
                matches!(members, [first, second] if first.as_str() == "Nat" && second.as_str() == "Zero")
                .then(|| ContentScalarExpression::Natural("0".to_owned()))
            }
            ExpressionNode::Member(_) => normalize_subject_field(program, subject, expression)
                .map(|(path, _)| ContentScalarExpression::SubjectField(path)),
            ExpressionNode::Call(call)
                if within_integer_coercion
                    && crate::proof_contracts::proof_embeddings::is_exact_embed_call(
                        program, call,
                    ) =>
            {
                let [argument] = expression_table(program).expression_handles(call.arguments)
                else {
                    return None;
                };
                let (path, field_type) = normalize_subject_field(program, subject, *argument)?;
                runtime_scalar_can_embed(program, field_type)
                    .then_some(ContentScalarExpression::RuntimeScalarEmbedding(path))
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
                ) && projection_arithmetic_has_integer_meaning(program, subject, expression) =>
            {
                Some(ContentScalarExpression::Arithmetic {
                    operator: match binary.operator {
                        BinaryOperator::Add => ContentArithmeticOperator::Add,
                        BinaryOperator::Subtract => ContentArithmeticOperator::Subtract,
                        BinaryOperator::Multiply => ContentArithmeticOperator::Multiply,
                        _ => unreachable!("guarded closed arithmetic operator"),
                    },
                    left: Box::new(normalize_projection_scalar_context(
                        program,
                        subject,
                        binary.left,
                        within_integer_coercion,
                    )?),
                    right: Box::new(normalize_projection_scalar_context(
                        program,
                        subject,
                        binary.right,
                        within_integer_coercion,
                    )?),
                })
            }
            ExpressionNode::StructLiteral(literal)
                if literal.type_name.as_str().rsplit("::").next() == Some("Nat") =>
            {
                match literal.case_name.as_ref().map(|name| name.as_str()) {
                    Some("Zero")
                        if expression_table(program)
                            .struct_fields(literal.fields)
                            .is_empty() =>
                    {
                        Some(ContentScalarExpression::Natural("0".to_owned()))
                    }
                    Some("Succ") => {
                        let fields = expression_table(program).struct_fields(literal.fields);
                        let [field] = fields else {
                            return None;
                        };
                        if field.name.as_str() != "prev" {
                            return None;
                        }
                        Some(ContentScalarExpression::Successor(Box::new(
                            normalize_projection_scalar(program, subject, field.value)?,
                        )))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn projection_arithmetic_has_integer_meaning(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> bool {
        use crate::value_custody::expression_types::{BuiltinBinaryResult, builtin_binary_result};
        use numerics::arithmetic::ArithmeticDomain;

        let Some(machine) = program.machine_by_symbol(subject.machine) else {
            return false;
        };
        let Some(state) = program.state_by_symbol(subject.state) else {
            return false;
        };
        let Some(binary) = expression_table(program).binary_node(expression) else {
            return false;
        };
        let mut expression_type =
            |operand| crate::expression_result_type_reference(program, machine, state, operand);
        let operands = [binary.left, binary.right].map(&mut expression_type);
        if !crate::proof_contracts::bound_expression_meaning::builtin_arithmetic_for_operand_types(
            program,
            machine.symbol,
            expression,
            &operands,
        ) {
            return false;
        }
        if [binary.left, binary.right].into_iter().all(|operand| {
            crate::value_custody::literals::has_anonymous_numeric_results(program, operand)
        }) {
            return true;
        }
        match builtin_binary_result(program, binary, operands, &mut expression_type) {
            Some(BuiltinBinaryResult::ProofInteger) => true,
            Some(BuiltinBinaryResult::Arithmetic {
                symbol,
                policy: ArithmeticDomain::Exact,
            }) => program
                .symbols
                .builtin_type_atom(symbol)
                .is_some_and(symbols::BuiltinTypeAtom::is_fixed_integer),
            _ => false,
        }
    }

    fn normalize_subject_field(
        program: &dyn CheckingInput,
        subject: ProjectionSubject,
        expression: ExpressionHandle,
    ) -> Option<(Vec<ContentFieldSegment>, TypeReferenceHandle)> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) if path.symbol == subject.symbol => {
                Some((Vec::new(), subject.carrier))
            }
            ExpressionNode::Member(member) => {
                let (mut path, receiver_type) =
                    normalize_subject_field(program, subject, member.receiver)?;
                let field = data_field(program, receiver_type, member.member.as_str())?;
                path.push(ContentFieldSegment {
                    symbol: field.symbol,
                    name: member.member.as_str().to_owned(),
                });
                Some((path, field.type_reference))
            }
            _ => None,
        }
    }

    fn data_field<'program>(
        program: &'program dyn CheckingInput,
        receiver_type: TypeReferenceHandle,
        name: &str,
    ) -> Option<&'program typed_trees::data::DataField> {
        let receiver_type = program
            .type_reference_table
            .peeled_type_reference(receiver_type);
        let (symbol, fallback) = match program.type_reference_table.type_reference(receiver_type) {
            TypeReferenceNode::Named { symbol, name } => (*symbol, name),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => (*base_symbol, base_name),
            _ => return None,
        };
        let definition =
            crate::field_domains::data_definition_by_symbol_or_name(program, symbol, fallback)?;
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == name => Some(field),
                _ => None,
            })
    }

    fn is_content_projection_trait(
        program: &dyn CheckingInput,
        candidate: &TraitDefinition,
    ) -> bool {
        compiler_owned_symbol(program, candidate.symbol)
            && candidate
                .name
                .as_str()
                .rsplit("::")
                .next()
                .is_some_and(|name| name == "Content")
            && !candidate.is_boundary
            && program.trait_type_parameters(candidate).len() == 2
            && matches!(program.trait_machine_signatures(candidate), [requirement] if requirement.name.as_str() == "project")
    }

    fn runtime_scalar_can_embed(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        let type_reference = program
            .type_reference_table
            .unconstrained_type_reference(type_reference);
        matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Named { name, .. }
                if matches!(name.as_str(), "u8" | "u16" | "u32" | "u64" | "addr")
        )
    }

    fn compiler_owned_symbol(program: &dyn CheckingInput, symbol: SymbolHandle) -> bool {
        let owning_machine = program.machine_by_own_or_state_symbol(symbol);
        let declaration = owning_machine.map_or(symbol, |machine| machine.symbol);
        let authored = program
            .machine_specialization(declaration)
            .map_or(declaration, |specialization| specialization.template);
        match program.symbols.symbol_source_origin(authored) {
            Some(source::SourceOrigin::Toolchain) => true,
            Some(source::SourceOrigin::User | source::SourceOrigin::ToolchainContract) => false,
            None => false,
        }
    }

    fn projection_subject(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> Option<ProjectionSubject> {
        let state = program.machine_states(machine).first()?;
        let [parameter] = program.state_parameters(state) else {
            return None;
        };
        if parameter.is_mutable {
            return None;
        }
        match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() && !access.is_exclusive() => Some(ProjectionSubject {
                machine: machine.symbol,
                state: state.symbol,
                symbol: parameter.symbol,
                qualified: *referee,
                carrier: program
                    .type_reference_table
                    .unconstrained_type_reference(*referee),
            }),
            _ => None,
        }
    }

    fn projection_domain_candidates<'program>(
        program: &'program dyn CheckingInput,
        machine: &Machine,
        subject: ProjectionSubject,
    ) -> Vec<ProjectionDomainCandidate<'program>> {
        let Some(owner) = machine.attached_data.as_ref().map(|name| name.as_str()) else {
            return Vec::new();
        };
        let owner_leaf = owner.rsplit("::").next().unwrap_or(owner);
        program
            .domain_definitions()
            .iter()
            .filter_map(|domain| {
                if program.symbols.has_source_metadata()
                    && !program
                        .symbols
                        .same_symbol_source_package(machine.symbol, domain.symbol)
                {
                    return None;
                }
                let name = domain.name.as_str();
                let leaf = name.rsplit("::").next().unwrap_or(name);
                if (name != owner && leaf != owner_leaf)
                    || !domain_accepts_projection_carrier(program, domain, subject.carrier)
                {
                    return None;
                }
                let exact = declared_domain_constraints(program, subject.qualified)
                    .into_iter()
                    .filter(|constraint| {
                        (constraint.symbol.is_valid() && constraint.symbol == domain.symbol)
                            || (!constraint.symbol.is_valid()
                                && constraint.name.as_str().rsplit("::").next() == Some(owner_leaf))
                    })
                    .collect::<Vec<_>>();
                match exact.as_slice() {
                    [constraint] => Some(ProjectionDomainCandidate {
                        definition: domain,
                        semantic_domain: constraint.semantic_id,
                    }),
                    [] if domain.index_arguments.is_empty() => Some(ProjectionDomainCandidate {
                        definition: domain,
                        semantic_domain: domain.semantic_id,
                    }),
                    _ => None,
                }
            })
            .collect()
    }

    fn domain_accepts_projection_carrier(
        program: &dyn CheckingInput,
        domain: &DomainDefinition,
        carrier: TypeReferenceHandle,
    ) -> bool {
        if type_references_match(program, carrier, domain.target_type) {
            return true;
        }
        let parameters = program.domain_type_parameters(domain);
        let TypeReferenceNode::Named { symbol, name } = program
            .type_reference_table
            .type_reference(domain.target_type)
        else {
            return false;
        };
        typed_trees::domain::has_generic_carrier(program, domain)
            && carrier.is_valid()
            && parameters.iter().any(|parameter| {
                matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    && (*symbol == parameter.symbol || name == &parameter.name)
            })
    }

    fn declared_domain_constraints(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Vec<&typed_trees::types::DomainConstraint> {
        typed_trees::operator::declared_domain_constraints(program.typed_program(), type_reference)
    }
}
