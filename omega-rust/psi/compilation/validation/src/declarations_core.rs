/*
Declaration validation and queries (`declarations`): visibility, definition
facts, selection authority and resolution, source completion, standard
declarations, state signatures, top-level symbols, transitions, and type
aliases.
*/
pub mod data_queries {
    use typed_trees::data::DataMember;

    #[derive(Clone, Copy)]
    pub(crate) enum DataDefinitionLookup {
        Missing,
        Unique(u32),
        Duplicate,
    }

    pub(crate) fn memoized_data_definition_lookup(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> DataDefinitionLookup {
        program.with_data_definition_index(|index| match index.positions_for_symbol(symbol) {
            [] => DataDefinitionLookup::Missing,
            [first] => DataDefinitionLookup::Unique(*first),
            [_, ..] => DataDefinitionLookup::Duplicate,
        })
    }

    pub(crate) fn data_definition_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        program.data_definition_by_symbol(symbol)
    }

    pub(crate) fn data_field_type_by_symbol(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_members(data))
            .find_map(|member| match member {
                DataMember::Field(field) if symbol.is_valid() && field.symbol == symbol => {
                    Some(field.type_reference)
                }
                DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| symbol.is_valid() && field.symbol == symbol)
                    .map(|field| field.type_reference),
                _ => None,
            })
    }

    pub fn data_field_is_requires_independent(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        field: &typed_trees::data::DataField,
    ) -> bool {
        let case_facts = program
            .data_members(definition)
            .iter()
            .find_map(|member| {
                let variant = member.as_variant()?;
                program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|payload| payload.symbol == field.symbol)
                    .then(|| program.proof_facts.span_or_empty(variant.requires_facts))
            })
            .unwrap_or(&[]);
        program
            .proof_facts
            .span_or_empty(definition.requires_facts)
            .iter()
            .chain(case_facts)
            .all(|fact| {
                !crate::proof_contracts::default_domains::data_predicate_fields(
                    program, definition, fact,
                )
                .contains(&field.symbol)
            })
    }
}

pub mod declaration_visibility {
    use crate::CheckingInput;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    };
    use numerics::float_projection::FloatProjectionOperation;
    use symbols::SymbolHandle;
    use symbols::SymbolKind;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    fn is_sealed_toolchain_float_projection(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
    ) -> bool {
        let Some(operator) = typed_trees::operator::declaration_by_symbol(program, symbol) else {
            return false;
        };
        let [namespace, name] = program.operator_path_members(operator.name) else {
            return false;
        };
        let Some(projection) =
            FloatProjectionOperation::from_source_identity(namespace.as_str(), name.as_str())
        else {
            return false;
        };
        crate::proof_contracts::float_projection_bindings::exact_toolchain_float_projection_primitive(
        program, operator, projection,
    )
    .is_some()
    }

    pub(crate) fn collect_declaration_visibility_diagnostics(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_public_const_declared_types(program, diagnostics);
        validate_public_conformance_headers(program, diagnostics);

        for selection in program.authored_declaration_selections() {
            if !program.checks_authored_selection(selection)
                || selection.exposure() != AuthoredDeclarationSelectionExposure::PublicInterface
            {
                continue;
            }
            let selection_target = selection.target();
            let Some(target) = selection_target.as_resolved() else {
                continue;
            };
            let symbol = target.selected_symbol();
            let symbol_kind = program.symbols.get(symbol).kind;
            if !typed_trees::visibility::requires_declaration_visibility(symbol_kind) {
                continue;
            }
            let Some(visibility) = typed_trees::visibility::authored_selection_visibility(
                program.product_declarations(),
                symbol,
            ) else {
                diagnostics.push(
                Diagnostic::error(format!(
                    "public interface selects {:?} `{}` without retained declaration visibility",
                    symbol_kind,
                    program.symbols.display_path(symbol, "::")
                ))
                .with_source_span(selection.source_span()),
            );
                continue;
            };
            if matches!(
                selection.kind(),
                AuthoredDeclarationSelectionKind::DomainIssuerAuthorization
                    | AuthoredDeclarationSelectionKind::DomainCleanupSelection
            ) && program
                .symbols
                .source_reference_can_see_symbol(selection.source_span(), symbol)
            {
                continue;
            }
            if !visibility.is_public()
                && !is_sealed_toolchain_float_projection(program.product_declarations(), symbol)
            {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "public interface selects private {} `{}`",
                        visibility.kind(),
                        program.symbols.display_path(symbol, "::")
                    ))
                    .with_source_span(selection.source_span()),
                );
            }
        }
    }

    fn validate_public_conformance_headers(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for conformance in program
            .conformances()
            .iter()
            .filter(|conformance| conformance.is_public)
        {
            validate_public_conformance_header_selection(
                program,
                conformance.symbol,
                conformance.trait_symbol,
                diagnostics,
            );
        }
    }

    fn validate_public_conformance_header_selection(
        program: &dyn CheckingInput,
        conformance_symbol: SymbolHandle,
        selected_symbol: SymbolHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if program.symbols.get(selected_symbol).kind == SymbolKind::TypeParameter
            && program.symbols.get(selected_symbol).parent == conformance_symbol
        {
            return;
        }
        let Some(visibility) = typed_trees::visibility::declaration_visibility(
            program.product_declarations(),
            selected_symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "public conformance `{}` selects `{}` without retained declaration visibility",
                program.symbols.display_path(conformance_symbol, "::"),
                program.symbols.display_path(selected_symbol, "::"),
            )));
            return;
        };
        if !visibility.is_public() {
            diagnostics.push(Diagnostic::error(format!(
                "public conformance `{}` exposes private {} `{}` in its header",
                program.symbols.display_path(conformance_symbol, "::"),
                visibility.kind(),
                program.symbols.display_path(selected_symbol, "::"),
            )));
        }
    }

    pub fn validate_declaration_visibility(
        program: &dyn CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        collect_declaration_visibility_diagnostics(program, &mut diagnostics);
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    fn validate_public_const_declared_types(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for declaration in program
            .const_declarations()
            .iter()
            .filter(|declaration| declaration.is_public)
        {
            let mut visited = Vec::new();
            validate_public_const_type_reference(
                program,
                declaration.symbol,
                declaration.declared_type,
                &mut visited,
                diagnostics,
            );
        }
    }

    fn validate_public_const_type_reference(
        program: &dyn CheckingInput,
        const_symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
        visited: &mut Vec<TypeReferenceHandle>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if visited.contains(&type_reference) {
            return;
        }
        visited.push(type_reference);

        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Callable { contract } => {
                if let Err(error) = program.runtime_callable_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(type_reference),
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "public machine value type has an unresolved contract: {error}"
                    )));
                    return;
                }
                let inputs = typed_trees::references::signature_type_inputs(
                    program.typed_program(),
                    &program.runtime_callable_contract(*contract).signature,
                );
                for reference in inputs.types {
                    if reference != type_reference {
                        validate_public_const_type_reference(
                            program,
                            const_symbol,
                            reference,
                            visited,
                            diagnostics,
                        );
                    }
                }
            }
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => validate_public_const_type_reference(
                program,
                const_symbol,
                *referee,
                visited,
                diagnostics,
            ),
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => validate_public_const_type_reference(
                program,
                const_symbol,
                *element_type,
                visited,
                diagnostics,
            ),
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                validate_public_const_data_visibility(
                    program,
                    const_symbol,
                    *base_symbol,
                    diagnostics,
                );
                for argument in program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                {
                    validate_public_const_type_reference(
                        program,
                        const_symbol,
                        *argument,
                        visited,
                        diagnostics,
                    );
                }
            }
            TypeReferenceNode::Named { symbol, .. } => {
                validate_public_const_data_visibility(program, const_symbol, *symbol, diagnostics);
            }
            TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::Unit => {}
        }
    }

    fn validate_public_const_data_visibility(
        program: &dyn CheckingInput,
        const_symbol: SymbolHandle,
        selected_symbol: SymbolHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if program.symbols.get(selected_symbol).kind != SymbolKind::Data {
            return;
        }
        let Some(data) = crate::declarations::data_queries::data_definition_by_symbol(
            program.product_declarations(),
            selected_symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "public const `{}` selects data `{}` without retained declaration visibility",
                program.symbols.display_path(const_symbol, "::"),
                program.symbols.display_path(selected_symbol, "::"),
            )));
            return;
        };
        if !data.is_public {
            diagnostics.push(Diagnostic::error(format!(
                "public const `{}` exposes private data `{}` in its declared type",
                program.symbols.display_path(const_symbol, "::"),
                program.symbols.display_path(selected_symbol, "::"),
            )));
        }
    }
}

pub mod definition_facts {
    use crate::frozen_program::expression_table;
    use arena::{Handle, HandleSpan};
    use typed_trees::data::DataMember;
    use typed_trees::domain::ProofFact;

    use facts::{
        DataDefinitionFactDependency, DataDefinitionFactRecord, DomainDefinitionFactDependency,
        DomainDefinitionFactRecord, Fact, FactOrigin, FactPayload, FactPlace, FactPlan,
        PlaceHandle, ProgramPoint,
    };

    pub fn build_definition_fact_plan(program: &dyn crate::CheckingInput) -> FactPlan {
        let mut facts = FactPlan::with_capacity(
            estimated_definition_fact_capacity(program),
            program.domain_definitions().len() + program.data_definitions().len(),
        );

        append_domain_definition_facts(program, &mut facts);
        append_data_definition_facts(program, &mut facts);

        facts
    }

    fn estimated_definition_fact_capacity(program: &dyn crate::CheckingInput) -> usize {
        let domain_facts = program
            .domain_definitions()
            .iter()
            .map(|domain| program.proof_facts(domain).len())
            .sum::<usize>();
        let data_facts = program
            .data_definitions()
            .iter()
            .map(|data| program.proof_facts.span_or_empty(data.requires_facts).len())
            .sum::<usize>();
        domain_facts + data_facts
    }

    fn append_domain_definition_facts(program: &dyn crate::CheckingInput, facts: &mut FactPlan) {
        for domain in program.domain_definitions() {
            let mut refs = HandleSpan::empty();
            for fact_handle in proof_fact_handles(domain.facts) {
                let proof_fact = program.proof_facts.get(fact_handle);
                let dependencies =
                    domain_fact_dependency_places(program, facts, domain, proof_fact);
                let place = append_proof_fact_place(program, facts, proof_fact);
                let payload = fact_payload(proof_fact, fact_handle);
                let fact = facts.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: ProgramPoint::Definition {
                        symbol: domain.symbol,
                    },
                    origin: FactOrigin::DomainDefinition {
                        domain_symbol: domain.symbol,
                    },
                    evidence: Default::default(),
                    payload,
                });
                facts
                    .domain_definition_facts
                    .append(DomainDefinitionFactRecord {
                        domain_symbol: domain.symbol,
                        fact: fact_handle,
                        semantic_fact: fact,
                        dependencies,
                    });
                facts.append_ref(&mut refs, fact);
            }
            facts.append_context(
                ProgramPoint::Definition {
                    symbol: domain.symbol,
                },
                refs,
            );
            facts.append_symbol_set(domain.symbol, refs);
        }
    }

    fn append_data_definition_facts(program: &dyn crate::CheckingInput, facts: &mut FactPlan) {
        for data in program.data_definitions() {
            let mut refs = HandleSpan::empty();
            for fact_handle in proof_fact_handles(data.requires_facts) {
                let proof_fact = program.proof_facts.get(fact_handle);
                let dependencies = data_fact_dependency_places(program, facts, data, proof_fact);
                let place = append_data_proof_fact_place(program, facts, data, proof_fact);
                let payload = fact_payload(proof_fact, fact_handle);
                let semantic_fact = facts.append_fact(Fact {
                    place: FactPlace::Place(place),
                    point: ProgramPoint::Definition {
                        symbol: data.symbol,
                    },
                    origin: FactOrigin::DataDefinition {
                        data_symbol: data.symbol,
                    },
                    evidence: Default::default(),
                    payload,
                });
                facts
                    .data_definition_facts
                    .append(DataDefinitionFactRecord {
                        data_symbol: data.symbol,
                        fact: fact_handle,
                        semantic_fact,
                        dependencies,
                    });
                facts.append_ref(&mut refs, semantic_fact);
            }
            facts.append_context(
                ProgramPoint::Definition {
                    symbol: data.symbol,
                },
                refs,
            );
            facts.append_symbol_set(data.symbol, refs);
        }
    }

    fn visit_proof_fact_expressions(
        program: &dyn crate::CheckingInput,
        proof_fact: &ProofFact,
        mut visit: impl FnMut(typed_trees::expression::ExpressionHandle),
    ) {
        match proof_fact {
            ProofFact::Expression(expression) => visit(*expression),
            ProofFact::Membership(membership) => visit(membership.value),
            ProofFact::Proposition(application) => {
                for argument in expression_table(program).expression_handles(application.arguments)
                {
                    visit(*argument);
                }
            }
        }
    }

    fn data_fact_dependency_places(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        data: &typed_trees::data::DataDefinition,
        proof_fact: &ProofFact,
    ) -> Vec<DataDefinitionFactDependency> {
        let mut dependencies = Vec::new();
        visit_proof_fact_expressions(program, proof_fact, |expression| {
            append_data_expression_dependency_places(
                program,
                facts,
                data,
                expression,
                &mut dependencies,
            );
        });
        dependencies
    }

    fn append_expression_dependency_places<D>(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        expression: typed_trees::expression::ExpressionHandle,
        dependencies: &mut Vec<D>,
        leaf: impl Fn(&mut FactPlan, typed_trees::expression::ExpressionHandle) -> D + Copy,
    ) {
        use typed_trees::expression::ExpressionNode;

        if !expression.is_valid() {
            return;
        }
        let node = expression_table(program).expression(expression);
        match node {
            ExpressionNode::Member(member)
                if matches!(
                    expression_table(program).expression(member.receiver),
                    ExpressionNode::Call(_)
                ) =>
            {
                append_expression_dependency_places(
                    program,
                    facts,
                    member.receiver,
                    dependencies,
                    leaf,
                );
                return;
            }
            ExpressionNode::SavedSubject(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_) => {
                dependencies.push(leaf(facts, expression));
                return;
            }
            _ => {}
        }
        crate::queries::expression_traversal::children(program, node, |child| {
            append_expression_dependency_places(program, facts, child, dependencies, leaf);
        });
    }

    fn append_data_expression_dependency_places(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        data: &typed_trees::data::DataDefinition,
        expression: typed_trees::expression::ExpressionHandle,
        dependencies: &mut Vec<DataDefinitionFactDependency>,
    ) {
        append_expression_dependency_places(
            program,
            facts,
            expression,
            dependencies,
            |facts, expression| DataDefinitionFactDependency {
                expression,
                place: append_data_expression_place(program, facts, data, expression),
            },
        );
    }

    fn append_domain_expression_dependency_places(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        domain_target: typed_trees::types::TypeReferenceHandle,
        expression: typed_trees::expression::ExpressionHandle,
        dependencies: &mut Vec<DomainDefinitionFactDependency>,
    ) {
        append_expression_dependency_places(
            program,
            facts,
            expression,
            dependencies,
            |facts, expression| DomainDefinitionFactDependency {
                expression,
                place: append_domain_expression_place(program, facts, domain_target, expression),
            },
        );
    }

    fn append_data_expression_place(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        data: &typed_trees::data::DataDefinition,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> PlaceHandle {
        use typed_trees::expression::ExpressionNode;

        match expression_table(program).expression(expression) {
            ExpressionNode::Borrow(inner) => {
                append_data_expression_place(program, facts, data, inner.target)
            }
            ExpressionNode::Name(path) => {
                let member_names = expression_table(program).name_path_members(path.members);
                let member_symbols =
                    expression_table(program).name_path_member_symbols(path.member_symbols);
                if member_names.is_empty() {
                    return facts.append_place_from_expression(program, expression);
                }
                let resolved_root = path
                    .head_symbol
                    .is_valid()
                    .then_some(path.head_symbol)
                    .or_else(|| {
                        member_symbols
                            .first()
                            .copied()
                            .filter(|symbol| symbol.is_valid())
                    })
                    .or_else(|| {
                        (member_names.len() == 1 && path.symbol.is_valid()).then_some(path.symbol)
                    });
                let first_field =
                    resolved_root.and_then(|symbol| data_field_symbol(program, data, symbol));
                let Some(first_field) = first_field else {
                    return facts.append_place_from_expression(program, expression);
                };

                let place = facts.append_symbol_place(data.symbol);
                facts.push_place_segment(
                    place,
                    facts::PlaceSegment::Field {
                        symbol: first_field,
                    },
                );
                for (offset, member_name) in member_names.iter().enumerate().skip(1) {
                    let exact_symbol = member_symbols
                        .get(offset)
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .or_else(|| {
                            (offset + 1 == member_names.len() && path.symbol.is_valid())
                                .then_some(path.symbol)
                        });
                    let symbol = exact_symbol
                        .or_else(|| {
                            facts::resolve_place_member_symbol(
                                program,
                                facts,
                                place,
                                member_name.as_str(),
                            )
                        })
                        .unwrap_or_else(symbols::SymbolHandle::invalid);
                    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
                        facts.push_place_segment(place, facts::PlaceSegment::Case { variant });
                    }
                    facts.push_place_segment(place, facts::PlaceSegment::Field { symbol });
                }
                place
            }
            ExpressionNode::Member(member) => {
                let place = append_data_expression_place(program, facts, data, member.receiver);
                let symbol = facts::effective_member_symbol(program, member.receiver, member);
                let symbol = if symbol.is_valid() {
                    symbol
                } else {
                    facts::resolve_place_member_symbol(
                        program,
                        facts,
                        place,
                        member.member.as_str(),
                    )
                    .unwrap_or_else(symbols::SymbolHandle::invalid)
                };
                push_member_field_segments(program, facts, place, symbol);
                place
            }
            ExpressionNode::Indexed(indexed) => {
                let place = append_data_expression_place(program, facts, data, indexed.collection);
                push_index_place_segment(program, facts, indexed, place);
                place
            }
            _ => facts.append_place_from_expression(program, expression),
        }
    }

    fn push_index_place_segment(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        indexed: &typed_trees::expression::TableIndexedExpression,
        place: PlaceHandle,
    ) {
        facts.push_place_segment(
            place,
            crate::value_custody::places::element_place_segment(program, indexed.index),
        );
    }

    fn push_member_field_segments(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        place: PlaceHandle,
        symbol: symbols::SymbolHandle,
    ) {
        if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
            facts.push_place_segment(place, facts::PlaceSegment::Case { variant });
        }
        facts.push_place_segment(place, facts::PlaceSegment::Field { symbol });
    }

    fn data_field_symbol(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        symbol: symbols::SymbolHandle,
    ) -> Option<symbols::SymbolHandle> {
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.symbol == symbol => Some(field.symbol),
                DataMember::Field(_) | DataMember::Variant(_) => None,
            })
    }

    fn domain_fact_dependency_places(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        domain: &typed_trees::domain::DomainDefinition,
        proof_fact: &ProofFact,
    ) -> Vec<DomainDefinitionFactDependency> {
        let mut dependencies = Vec::new();
        visit_proof_fact_expressions(program, proof_fact, |expression| {
            append_domain_expression_dependency_places(
                program,
                facts,
                domain.target_type,
                expression,
                &mut dependencies,
            );
        });
        dependencies
    }

    fn append_domain_expression_place(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        domain_target: typed_trees::types::TypeReferenceHandle,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> PlaceHandle {
        use typed_trees::expression::ExpressionNode;

        match expression_table(program).expression(expression) {
            ExpressionNode::Borrow(inner) => {
                append_domain_expression_place(program, facts, domain_target, inner.target)
            }
            ExpressionNode::Member(member) => {
                let place =
                    append_domain_expression_place(program, facts, domain_target, member.receiver);
                let symbol = facts::effective_member_symbol(program, member.receiver, member);
                let symbol = if symbol.is_valid() {
                    symbol
                } else {
                    facts::resolve_place_member_symbol(
                        program,
                        facts,
                        place,
                        member.member.as_str(),
                    )
                    .or_else(|| {
                        facts
                            .places
                            .get(place)
                            .segments
                            .is_empty()
                            .then(|| {
                                domain_target_member_symbol(
                                    program,
                                    domain_target,
                                    member.member.as_str(),
                                )
                            })
                            .flatten()
                    })
                    .unwrap_or_else(symbols::SymbolHandle::invalid)
                };
                push_member_field_segments(program, facts, place, symbol);
                place
            }
            ExpressionNode::Indexed(indexed) => {
                let place = append_domain_expression_place(
                    program,
                    facts,
                    domain_target,
                    indexed.collection,
                );
                push_index_place_segment(program, facts, indexed, place);
                place
            }
            _ => facts.append_place_from_expression(program, expression),
        }
    }

    fn domain_target_member_symbol(
        program: &dyn crate::CheckingInput,
        target: typed_trees::types::TypeReferenceHandle,
        member_name: &str,
    ) -> Option<symbols::SymbolHandle> {
        use typed_trees::data::DataMember;
        use typed_trees::types::TypeReferenceNode;

        let target_symbol = match program.type_reference_table.type_reference(target) {
            TypeReferenceNode::Reference { referee, .. } => {
                return domain_target_member_symbol(program, *referee, member_name);
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                return domain_target_member_symbol(program, *base_type, member_name);
            }
            TypeReferenceNode::Generic { base_symbol, .. }
            | TypeReferenceNode::Named {
                symbol: base_symbol,
                ..
            } => *base_symbol,
            _ => return None,
        };
        let data =
            crate::declarations::data_queries::data_definition_by_symbol(program, target_symbol)?;
        program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == member_name => {
                    Some(field.symbol)
                }
                DataMember::Variant(variant) if variant.name.as_str() == member_name => {
                    Some(variant.symbol)
                }
                DataMember::Variant(_) | DataMember::Field(_) => None,
            })
    }

    fn append_proof_fact_place(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        proof_fact: &ProofFact,
    ) -> PlaceHandle {
        match proof_fact {
            ProofFact::Expression(expression) => {
                facts.append_place_from_expression(program, *expression)
            }
            ProofFact::Membership(membership) => {
                facts.append_place_from_expression(program, membership.value)
            }
            ProofFact::Proposition(application) => {
                facts.append_symbol_place(application.proposition)
            }
        }
    }

    fn append_data_proof_fact_place(
        program: &dyn crate::CheckingInput,
        facts: &mut FactPlan,
        data: &typed_trees::data::DataDefinition,
        proof_fact: &ProofFact,
    ) -> PlaceHandle {
        match proof_fact {
            ProofFact::Expression(expression) => {
                append_data_expression_place(program, facts, data, *expression)
            }
            ProofFact::Membership(membership) => {
                append_data_expression_place(program, facts, data, membership.value)
            }
            ProofFact::Proposition(application) => {
                facts.append_symbol_place(application.proposition)
            }
        }
    }

    fn proof_fact_handles(facts: HandleSpan<ProofFact>) -> impl Iterator<Item = Handle<ProofFact>> {
        facts.handles()
    }

    fn fact_payload(proof_fact: &ProofFact, fact_handle: Handle<ProofFact>) -> FactPayload {
        match proof_fact {
            ProofFact::Expression(expression) => FactPayload::BooleanExpression(*expression),
            ProofFact::Membership(membership) => FactPayload::DomainMembership {
                value: membership.value,
                domain: membership.domain,
                domain_symbol: membership.domain_symbol,
                semantic_domain: membership.semantic_domain,
            },
            ProofFact::Proposition(application) => FactPayload::PropositionApplication {
                fact: fact_handle,
                proposition: application.proposition,
            },
        }
    }
}

pub mod selection_authority {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget;
    use semantic_vocabulary::PackageKeyIdentity;
    use source::SourceOrigin;
    use symbols::FixedCoreDeclaration;

    pub use semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority;

    pub fn validate_authored_declaration_selections(
        program: &CheckedTrees,
        packages: &dyn DeclarationSelectionAuthority,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        if program
            .facts
            .authored_declaration_selections()
            .into_iter()
            .len()
            != program.facts.declaration_descriptions.selections.len()
        {
            return Err(vec![Diagnostic::error(
                "checked declaration selections lost their normalized authority obligations",
            )]);
        }
        for (_, selection) in program.facts.declaration_descriptions.selections.iter() {
            let checked_trees::CheckedSelectionRequirement::Selection {
                source_span,
                excluded,
                request,
                ..
            } = selection
            else {
                return Err(vec![Diagnostic::error(
                    "checked declaration-selection requirement is absent",
                )]);
            };
            if *excluded {
                continue;
            }
            match request {
                Ok(Some(request))
                    if !packages.allows_declaration_selection(request.requester, request.owner) =>
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "package {} selects a declaration owned by package {} without declaring that package as a direct dependency",
                    packages.package_label(request.requester), packages.package_label(request.owner),
                )).with_source_span(*source_span));
                }
                Err(reason) => diagnostics
                    .push(Diagnostic::error(reason.clone()).with_source_span(*source_span)),
                _ => {}
            }
        }
        crate::program_validation::finish_strict_diagnostics(diagnostics)
    }

    pub fn checked_selection_requirements(
        program: &CheckedTrees,
    ) -> arena::Arena<checked_trees::CheckedSelectionRequirement> {
        let skipped =
            crate::queries::execution_projection::descriptor_selection_occurrences(&program.typed);
        let mut result = Vec::new();
        for selection in program.facts.authored_declaration_selections() {
            let selected_is_pruned = match selection.target() {
                AuthoredDeclarationSelectionTarget::Resolved(selected) => {
                    let selected = program
                        .closed_requirement_application(selected.selected_symbol())
                        .map_or(selected.selected_symbol(), |application| {
                            application.template
                        });
                    !typed_trees::visibility::owning_machine_is_retained(
                        program.product_declarations(),
                        selected,
                    )
                }
                _ => false,
            };
            let excluded = skipped.contains(&selection.occurrence_id()) || selected_is_pruned;
            result.push(checked_trees::CheckedSelectionRequirement::Selection {
                occurrence: selection.occurrence_id(),
                source_span: selection.source_span(),
                excluded,
                request: normalize_checked_selection(program, *selection),
            });
        }
        result.sort_by_key(checked_trees::CheckedSelectionRequirement::occurrence_ordinal);
        let mut retained = arena::Arena::new();
        retained.insert_many(result);
        retained
    }

    fn normalize_checked_selection(
        program: &CheckedTrees,
        selection: typed_trees::AuthoredDeclarationSelection,
    ) -> Result<Option<semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest>, String>
    {
        let source_span = selection.source_span();
        let source_file = program
            .symbols
            .source_file(source_span)
            .ok_or("authored declaration selection has no compiler-owned source custody")?;
        if source_file.origin == SourceOrigin::Toolchain {
            return Ok(None);
        }
        let requester = if source_file.origin == SourceOrigin::User {
            Some(source_file.package_identity.ok_or_else(|| format!(
                "authored declaration selection in {} has no reconciled requesting package identity",
                source_file.path.display()
            ))?)
        } else {
            None
        };
        let selected = match selection.target() {
            AuthoredDeclarationSelectionTarget::Intrinsic(_) => return Ok(None),
            AuthoredDeclarationSelectionTarget::LateBound(binding) => {
                return Err(format!(
                    "package-authored declaration selection remained unresolved after successful checking: {:?} ({binding:?})",
                    selection.kind()
                ));
            }
            AuthoredDeclarationSelectionTarget::Resolved(selected) => selected.selected_symbol(),
        };
        let selected = program
            .closed_requirement_application(selected)
            .map_or(selected, |application| application.template);
        if program.symbols.builtin_type_atom(selected).is_some()
            || program
                .symbols
                .builtin_function_for_symbol(selected)
                .is_some()
            || program.symbols.builtin_type_member(selected).is_some()
            || FixedCoreDeclaration::ALL.into_iter().any(|declaration| {
                program.symbols.fixed_core_declaration_symbol(declaration) == Some(selected)
            })
        {
            return Ok(None);
        }
        if source_file.origin == SourceOrigin::ToolchainContract {
            let local = program
                .symbols
                .symbol_provenance_source_span(selected)
                .is_some_and(|declaration| {
                    program
                        .symbols
                        .same_source_package(source_span, declaration)
                        && program
                            .symbols
                            .source_reference_can_see_symbol(source_span, selected)
                });
            return if local
                || program.symbols.symbol_source_origin(selected) == Some(SourceOrigin::Toolchain)
            {
                Ok(None)
            } else {
                Err(
                    "closed contract selects a declaration outside its checked owner or core"
                        .into(),
                )
            };
        }
        let requester =
            requester.ok_or("declaration selection lost its requesting package custody")?;
        let selected_kind = program.symbols.get(selected).kind;
        if typed_trees::visibility::requires_declaration_visibility(selected_kind) {
            let visibility = typed_trees::visibility::authored_selection_visibility(
                program.product_declarations(),
                selected,
            )
            .ok_or_else(|| {
                format!(
                    "selected {:?} `{}` has no retained declaration visibility",
                    selected_kind,
                    program.symbols.display_path(selected, "::")
                )
            })?;
            if !visibility.is_public()
                && program.symbols.symbol_package_identity(selected) != Some(requester)
            {
                return Err(format!(
                    "package {requester:?} selects private {} `{}`",
                    visibility.kind(),
                    program.symbols.display_path(selected, "::")
                ));
            }
        }
        if let Some(owner) = program.symbols.symbol_package_identity(selected) {
            return Ok(Some(
                semantic_vocabulary::compiler_evaluation::DeclarationAuthorityRequest {
                    requester,
                    owner,
                },
            ));
        }
        match program.symbols.symbol_source_origin(selected) {
            Some(SourceOrigin::Toolchain) => Ok(None),
            Some(SourceOrigin::ToolchainContract) => Err(
                "package source cannot select a closed toolchain contract without package custody"
                    .into(),
            ),
            Some(SourceOrigin::User) => Err(format!(
                "selected user declaration `{}` has no reconciled owning package identity",
                program.symbols.display_path(selected, "::")
            )),
            None => Err(format!(
                "selected declaration `{}` has no package or toolchain provenance",
                program.symbols.display_path(selected, "::")
            )),
        }
    }

    #[derive(Clone, PartialEq, Eq, compiler_storage::Record)]
    pub enum RecordedDeclarationAuthority {
        Absent,
        Packages(std::collections::BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>),
    }
    impl RecordedDeclarationAuthority {
        pub fn capture(
            authority: Option<&dyn DeclarationSelectionAuthority>,
        ) -> Result<Self, &'static str> {
            let Some(authority) = authority else {
                return Ok(Self::Absent);
            };
            let mut scope = authority.declaration_selection_scope().ok_or(
                "declaration selection predicate does not export a complete comparison scope",
            )?;
            for owners in scope.values_mut() {
                owners.sort_unstable();
                owners.dedup();
            }
            Ok(Self::Packages(scope))
        }
        pub fn has_authority(&self) -> bool {
            matches!(self, Self::Packages(_))
        }
        pub fn allows(
            &self,
            requester: PackageKeyIdentity,
            owner: PackageKeyIdentity,
        ) -> Option<bool> {
            match self {
                Self::Absent => None,
                Self::Packages(scope) => {
                    scope.get(&requester).map(|owners| owners.contains(&owner))
                }
            }
        }
        pub fn require_current(
            &self,
            authority: Option<&dyn DeclarationSelectionAuthority>,
            sources: &source::SourceMap,
        ) -> Result<(), &'static str> {
            self.require_requesters(
                authority,
                sources
                    .selected_files()
                    .filter(|file| !file.origin.is_toolchain())
                    .filter_map(|file| file.package_identity),
            )
        }
        pub fn require_current_sites(
            &self,
            authority: Option<&dyn DeclarationSelectionAuthority>,
            program: &typed_trees::TypedTrees,
            sites: impl Iterator<Item = source::SourceSpan>,
        ) -> Result<(), &'static str> {
            let mut requesters = std::collections::BTreeSet::new();
            for site in sites {
                let file = program
                    .symbols
                    .source_file(site)
                    .ok_or("retained pending invocation has no source custody")?;
                if !file.origin.is_toolchain()
                    && let Some(requester) = file.package_identity
                {
                    requesters.insert(requester);
                }
            }
            self.require_requesters(authority, requesters.into_iter())
        }
        fn require_requesters(
            &self,
            authority: Option<&dyn DeclarationSelectionAuthority>,
            requesters: impl Iterator<Item = PackageKeyIdentity>,
        ) -> Result<(), &'static str> {
            match (self, Self::capture(authority)?) {
                (Self::Absent, Self::Absent) => Ok(()),
                (Self::Packages(previous), Self::Packages(current)) => {
                    for requester in requesters {
                        let old = previous.get(&requester).ok_or(
                            "retained declaration scope does not cover a requesting package",
                        )?;
                        let new = current.get(&requester).ok_or(
                            "current declaration scope does not cover a requesting package",
                        )?;
                        if old != new {
                            return Err("requesting package declaration scope changed");
                        }
                    }
                    Ok(())
                }
                _ => Err("presence of declaration selection authority changed"),
            }
        }
    }
}

pub mod selection_resolution {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionFinalizationError, AuthoredDeclarationSelectionIntrinsic,
        AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
        AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionTarget,
    };
    use symbols::SymbolHandle;
    use typed_trees::machine::Machine;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AuthoredSelectionResolution {
        pub occurrence: AuthoredDeclarationSelectionOccurrenceId,
        pub binding: AuthoredDeclarationSelectionLateBinding,
        pub target: AuthoredSelectionResolutionTarget,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AuthoredSelectionResolutionTarget {
        Declaration(SymbolHandle),
        Intrinsic(AuthoredDeclarationSelectionIntrinsic),
    }

    pub fn push_consistent_resolution(
        program: &dyn CheckingInput,
        resolutions: &mut Vec<AuthoredSelectionResolution>,
        candidate: AuthoredSelectionResolution,
    ) -> Result<(), Diagnostic> {
        let candidate = match (candidate.binding, candidate.target) {
            (
                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                AuthoredSelectionResolutionTarget::Declaration(target),
            ) => AuthoredSelectionResolution {
                target: AuthoredSelectionResolutionTarget::Declaration(authored_callable(
                    program, target,
                )?),
                ..candidate
            },
            _ => candidate,
        };
        if let Some(existing) = resolutions
            .iter()
            .find(|resolution| resolution.occurrence == candidate.occurrence)
        {
            if *existing != candidate {
                return Err(Diagnostic::error(format!(
                    "authored declaration selection occurrence {} resolved inconsistently across compiler-derived copies: {:?} selected {:?} and {:?}",
                    candidate.occurrence.ordinal(),
                    candidate.binding,
                    existing.target,
                    candidate.target
                )));
            }
            return Ok(());
        }
        resolutions.push(candidate);
        Ok(())
    }

    fn authored_callable(
        program: &dyn CheckingInput,
        mut selected: SymbolHandle,
    ) -> Result<SymbolHandle, Diagnostic> {
        for _ in 0..=program.machine_specializations.len() {
            let declaration = program.machine_by_symbol(selected);
            let owner = declaration.or_else(|| program.machine_holding_state(selected));
            let Some(owner) = owner else {
                return Ok(selected);
            };
            let Some(application) = program.machine_specialization(owner.symbol) else {
                return Ok(selected);
            };
            if application.instance == application.template {
                return Ok(selected);
            }
            let template = program
                .machine_by_symbol(application.template)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "authored call selection lost its specialization template declaration",
                    )
                })?;
            selected = if declaration.is_some() {
                template.symbol
            } else {
                let ordinal =
                    typed_trees::state::state_position(program.machine_states(owner), selected)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "authored call selection lost its specialized state position",
                            )
                        })?;
                program
                    .machine_states(template)
                    .get(ordinal)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "authored call selection has no corresponding template state",
                        )
                    })?
                    .symbol
            };
        }
        Err(Diagnostic::error(
            "authored call selection has cyclic specialization provenance",
        ))
    }

    pub fn finalization_diagnostic(
        resolution: AuthoredSelectionResolution,
        error: AuthoredDeclarationSelectionFinalizationError,
    ) -> Diagnostic {
        Diagnostic::error(format!(
            "failed to finalize authored declaration selection occurrence {}: {error:?}",
            resolution.occurrence.ordinal()
        ))
    }

    pub(crate) fn machine_is_template_instance(
        program: &dyn CheckingInput,
        machine: &Machine,
    ) -> bool {
        program
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.instance == machine.symbol
                    && specialization.template != specialization.instance
            })
    }

    pub fn collect_checked_transition_target_selections(
        program: &dyn CheckingInput,
        resolutions: &mut Vec<AuthoredSelectionResolution>,
    ) -> Result<(), Diagnostic> {
        for machine in program.machines() {
            if machine_is_template_instance(program, machine) {
                continue;
            }
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(transition) = statement.as_transition() else {
                        continue;
                    };
                    for target in transition.targets() {
                        let typed_trees::statement::TransitionTargetNode::Named {
                            path,
                            source_span,
                            authored_call_selection: Some(occurrence),
                            ..
                        } = program.statement_table.transition_target(target)
                        else {
                            continue;
                        };
                        let Some(selection) =
                            program.authored_declaration_selections().get(*occurrence)
                        else {
                            return Err(Diagnostic::error(format!(
                            "transition target retains unknown authored selection occurrence {}",
                            occurrence.ordinal(),
                        ))
                        .with_source_span(*source_span));
                        };
                        if selection.kind() != AuthoredDeclarationSelectionKind::Call
                            || selection.source_span() != *source_span
                        {
                            return Err(Diagnostic::error(
                            "transition target retains mismatched authored call-selection evidence",
                        )
                        .with_source_span(*source_span));
                        }
                        if selection.target()
                            == AuthoredDeclarationSelectionTarget::LateBound(
                                AuthoredDeclarationSelectionLateBinding::CheckedCall,
                            )
                        {
                            let target_symbol = if path.symbol.is_valid() {
                                path.symbol
                            } else {
                                let Some(target_name) = program
                                    .statement_table
                                    .name_path_members(path.members)
                                    .last()
                                else {
                                    continue;
                                };
                                let matching = program
                                    .machine_type_parameters(machine)
                                    .iter()
                                    .filter(|parameter| {
                                        parameter.name == *target_name
                                            && matches!(
                                                parameter.kind,
                                                typed_trees::data::TypeParameterKind::Machine { .. }
                                            )
                                    })
                                    .map(|parameter| parameter.symbol)
                                    .collect::<Vec<_>>();
                                let [target] = matching.as_slice() else {
                                    continue;
                                };
                                *target
                            };
                            if !target_symbol.is_valid() {
                                continue;
                            }
                            push_consistent_resolution(
                                program,
                                resolutions,
                                AuthoredSelectionResolution {
                                    occurrence: *occurrence,
                                    binding: AuthoredDeclarationSelectionLateBinding::CheckedCall,
                                    target: AuthoredSelectionResolutionTarget::Declaration(
                                        target_symbol,
                                    ),
                                },
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn authored_data_member(program: &dyn CheckingInput, symbol: SymbolHandle) -> SymbolHandle {
        use symbols::SymbolKind;
        use typed_trees::data::DataMember;

        if !symbol.is_valid() || program.symbols.get(symbol).kind != SymbolKind::Field {
            return symbol;
        }
        let parent = program.symbols.get(symbol).parent;
        let owner = if program.symbols.get(parent).kind == SymbolKind::Variant {
            program.symbols.get(parent).parent
        } else {
            parent
        };
        let Some(instance) = program.unique_data_definition_by_symbol(owner) else {
            return symbol;
        };
        let Some(origin) = instance.generic_instance else {
            return symbol;
        };
        if !program.type_reference_table.contains_type_reference(origin) {
            return symbol;
        }
        let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } =
            program.type_reference_table.type_reference(origin)
        else {
            return symbol;
        };
        let Some(template) = program.unique_data_definition_by_symbol(*base_symbol) else {
            return symbol;
        };
        if instance.symbol == template.symbol
            || program.symbols.get(instance.symbol).kind != SymbolKind::Data
            || program.symbols.get(template.symbol).kind != SymbolKind::Data
            || program.data_members(instance).len() != program.data_members(template).len()
        {
            return symbol;
        }
        let mut resolved = None;
        for (selected, original) in program
            .data_members(instance)
            .iter()
            .zip(program.data_members(template))
        {
            let (selected_fields, original_fields, selected_parent, original_parent) =
                match (selected, original) {
                    (DataMember::Field(selected), DataMember::Field(original)) => (
                        std::slice::from_ref(selected),
                        std::slice::from_ref(original),
                        instance.symbol,
                        template.symbol,
                    ),
                    (DataMember::Variant(selected), DataMember::Variant(original)) => {
                        if selected.name != original.name
                            || selected.identity != original.identity
                            || selected.retired_payload_identities
                                != original.retired_payload_identities
                            || !selected.symbol.is_valid()
                            || !original.symbol.is_valid()
                            || program.symbols.get(selected.symbol).kind != SymbolKind::Variant
                            || program.symbols.get(original.symbol).kind != SymbolKind::Variant
                            || program.symbols.get(selected.symbol).parent != instance.symbol
                            || program.symbols.get(original.symbol).parent != template.symbol
                        {
                            return symbol;
                        }
                        (
                            program.data_payload_fields(selected),
                            program.data_payload_fields(original),
                            selected.symbol,
                            original.symbol,
                        )
                    }
                    _ => return symbol,
                };
            if selected_fields.len() != original_fields.len() {
                return symbol;
            }
            for (selected, original) in selected_fields.iter().zip(original_fields) {
                if selected.name != original.name
                    || selected.identity != original.identity
                    || selected.relevance != original.relevance
                    || !selected.symbol.is_valid()
                    || !original.symbol.is_valid()
                    || program.symbols.get(selected.symbol).kind != SymbolKind::Field
                    || program.symbols.get(original.symbol).kind != SymbolKind::Field
                    || program.symbols.get(selected.symbol).parent != selected_parent
                    || program.symbols.get(original.symbol).parent != original_parent
                {
                    return symbol;
                }
                if selected.symbol == symbol {
                    if resolved.is_some() {
                        return symbol;
                    }
                    resolved = Some(original.symbol);
                }
            }
        }
        resolved.unwrap_or(symbol)
    }

    pub fn exact_named_operator_call<'program>(
        program: &'program dyn CheckingInput,
        call: &typed_trees::expression::TableCallExpression,
    ) -> Option<&'program typed_trees::operator::OperatorDefinition> {
        typed_trees::operator::resolve_named_expression_call(program, call).or_else(|| {
            let path = expression_table(program).name_path_node(call.receiver)?;
            let static_segments = expression_table(program)
                .name_path_members(path.members)
                .iter()
                .map(|segment| segment.as_str())
                .collect::<Vec<_>>();
            (!static_segments.is_empty()).then_some(())?;
            typed_trees::operator::resolve_named_call(
                program,
                call.target_symbol,
                Some(&static_segments),
                call.target.as_str(),
                expression_table(program)
                    .expression_handles(call.arguments)
                    .len(),
                false,
            )
        })
    }

    pub fn declaration_target(symbol: SymbolHandle) -> Option<AuthoredSelectionResolutionTarget> {
        symbol
            .is_valid()
            .then_some(AuthoredSelectionResolutionTarget::Declaration(symbol))
    }
}

pub mod source_completion {
    use diagnostics::Diagnostic;
    use semantic_vocabulary::PackageKeyIdentity;
    use symbols::SymbolHandle;
    use typed_trees::boundary_completions::completion_declaration;
    use typed_trees::{BoundaryCompletionDeclaration, TypedTrees};

    #[derive(Clone, Copy, Default)]
    pub enum SourceCompletionAdmission<'request> {
        #[default]
        None,
        AcceptedProcessExit {
            package: PackageKeyIdentity,
            declaration_path: &'request str,
        },
        DiscoverProcessExit,
    }

    pub fn prepare_source_completion(
        typed: &mut TypedTrees,
        admission: SourceCompletionAdmission<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        typed.boundary_completion_declarations.clear();
        let SourceCompletionAdmission::AcceptedProcessExit {
            package,
            declaration_path,
        } = admission
        else {
            if matches!(admission, SourceCompletionAdmission::DiscoverProcessExit) {
                nominate_source_completion(typed)?;
            }
            return Ok(());
        };
        let declarations = typed
            .traits()
            .iter()
            .filter(|definition| {
                is_product_declaration(typed, definition.symbol)
                    && typed.symbols.symbol_package_identity(definition.symbol) == Some(package)
                    && typed.symbols.display_path(definition.symbol, "::") == declaration_path
            })
            .collect::<Vec<_>>();
        let [definition] = declarations.as_slice() else {
            return Err(vec![Diagnostic::error(
                "accepted completion role has no unique package-owned declaration",
            )]);
        };
        let declaration =
            completion_declaration(typed, definition.symbol, package).ok_or_else(|| {
                vec![Diagnostic::error(
                    "accepted completion role has no exact process-exit requirement",
                )]
            })?;
        let _ = typed.boundary_completion_declarations.append(declaration);
        Ok(())
    }

    fn nominate_source_completion(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let declarations = typed
            .traits()
            .iter()
            .filter_map(|definition| {
                if !is_product_declaration(typed, definition.symbol)
                    || typed.symbols.display_path(definition.symbol, "::") != "Console"
                {
                    return None;
                }
                let package = typed.symbols.symbol_package_identity(definition.symbol)?;
                let declaration = completion_declaration(typed, definition.symbol, package)?;
                has_external_source_candidate(typed, &declaration).then_some(declaration)
            })
            .collect::<Vec<_>>();
        if declarations.len() > 1 {
            return Err(vec![Diagnostic::error(
                "completion role discovery has multiple package-owned source declarations",
            )]);
        }
        for declaration in declarations {
            let _ = typed.boundary_completion_declarations.append(declaration);
        }
        Ok(())
    }

    fn has_external_source_candidate(
        typed: &TypedTrees,
        declaration: &BoundaryCompletionDeclaration,
    ) -> bool {
        typed.machines().iter().any(|machine| {
            !machine.body_is_present
                && matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::Boundary
                        | language_semantics::MachineSupplyMode::ExternalRealization { .. }
                )
                && is_product_declaration(typed, machine.symbol)
                && typed.symbols.symbol_package_identity(machine.symbol) == declaration.package
                && typed
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        typed_trees::machine::resolve_satisfied_declaration(
                            typed,
                            machine,
                            conformance,
                        )
                        .is_some_and(|requirement| requirement.symbol() == declaration.requirement)
                    })
        })
    }

    fn is_product_declaration(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
        typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| typed.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product)
    }
}

pub mod standard_declarations {
    use symbols::SymbolHandle;
    use typed_trees::trait_definition::TraitDefinition;

    pub fn boundary_has_explicit_calling_policy(
        program: &dyn crate::CheckingInput,
        definition: &TraitDefinition,
    ) -> bool {
        definition.is_boundary
            && standard_calling_vocabularies(program)
                .iter()
                .any(|(calling, _)| {
                    program
                        .trait_requirements(definition)
                        .iter()
                        .any(|requirement| requirement.symbol == calling.symbol)
                })
    }

    pub fn standard_calling_vocabularies(
        program: &dyn crate::CheckingInput,
    ) -> Vec<(&TraitDefinition, &TraitDefinition)> {
        typed_trees::calling_vocabulary::standard_calling_vocabularies(program.typed_program())
    }

    pub fn validate_calling_vocabulary_uniqueness(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let mut paths = std::collections::BTreeSet::new();
        for (definition, _) in standard_calling_vocabularies(program) {
            if let Some(source) = program
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
            {
                paths.insert(source.path.clone());
            }
        }
        let duplicate_paths: Vec<_> = paths.into_iter().skip(1).collect();
        if duplicate_paths.is_empty() {
            return;
        }
        for data_definition in program.data_definitions() {
            let duplicate = program
                .symbols
                .symbol_source_span(data_definition.symbol)
                .and_then(|span| program.symbols.source_file(span))
                .is_some_and(|source| duplicate_paths.contains(&source.path));
            if duplicate {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }
        }
    }

    pub(crate) fn is_core_vector(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
        program.data_definitions().iter().any(|definition| {
            definition.symbol == symbol
                && definition.name.as_str() == "Vec"
                && has_toolchain_declaration_source(program, symbol, "vec.omg")
        })
    }

    pub(crate) fn is_core_vector_surface_state(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> bool {
        core_vector_surface_placeholder(program, machine, state)
            || specialized_core_vector_surface_placeholder(program, machine, state)
    }

    fn core_vector_surface_placeholder(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> bool {
        has_toolchain_declaration_source(program, machine.symbol, "vec.omg")
            && program.machine_states(machine).len() == 1
            && program
                .statement_table
                .statements(state.statement_nodes)
                .is_empty()
    }

    fn specialized_core_vector_surface_placeholder(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> bool {
        if !program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
        {
            return false;
        }
        let Some(specialization) = program.machine_specialization(machine.symbol) else {
            return false;
        };
        if specialization.template == machine.symbol {
            return false;
        }
        let Some(template) = program.machine_by_symbol(specialization.template) else {
            return false;
        };
        program
            .machine_states(template)
            .iter()
            .any(|template_state| {
                core_vector_surface_placeholder(program, template, template_state)
            })
    }

    fn has_toolchain_declaration_source(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
        relative_path: &str,
    ) -> bool {
        if !symbol.is_valid() {
            return false;
        }
        program
            .symbols
            .symbol_source_span(symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_some_and(|source| {
                source.origin == source::SourceOrigin::Toolchain
                    && source.path.strip_prefix(&source.package_root).ok()
                        == Some(std::path::Path::new(relative_path))
            })
    }
}

pub mod state_signatures {
    use crate::declarations::symbols::TopLevelSymbols;
    use crate::proof_contracts::proof_facts::{ProofFactOwner, validate_proof_facts};
    use crate::value_custody::type_references::{
        TypeParameterScope, TypeReferenceOwner, validate_type_reference_handle,
    };
    use diagnostics::Diagnostic;
    use std::fmt;
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::signature::{SignatureContract, StateParameter, StateSignature};
    use typed_trees::state::State;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) mod value_scope {
        use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use typed_trees::{domain::ProofFact, machine::Machine, signature::SignatureContractKind};

        pub(crate) fn validate(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            symbols: &TopLevelSymbols<'_>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            MachineSymbols::build(program, machine, diagnostics);
            for (ordinal, state) in program.machine_states(machine).iter().enumerate() {
                let scope = crate::value_custody::locals::StateValueScope {
                    program,
                    machine,
                    state,
                    symbols,
                    prior_statements: &[],
                    context: "arrival contract",
                };
                let type_scope = crate::value_custody::locals::StateValueScope {
                    context: "type bound",
                    ..scope
                };
                for parameter in program.state_parameters(state) {
                    type_scope.type_reference(parameter.type_reference, diagnostics);
                }
                type_scope.type_reference(state.return_type, diagnostics);
                let machine_requires =
                    program
                        .machine_contracts(machine)
                        .iter()
                        .filter(|contract| {
                            ordinal == 0 && contract.kind == SignatureContractKind::Requires
                        });
                for contract in program
                    .state_contracts(state)
                    .iter()
                    .chain(machine_requires)
                {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        match fact {
                            ProofFact::Expression(expression) => {
                                scope.expression(*expression, diagnostics)
                            }
                            ProofFact::Membership(membership) => {
                                scope.expression(membership.value, diagnostics)
                            }
                            ProofFact::Proposition(application) => {
                                for argument in expression_table(program)
                                    .expression_handles(application.arguments)
                                {
                                    scope.expression(*argument, diagnostics);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn validate_callable_state_signatures(
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for machine in program.machines() {
            value_scope::validate(program, machine, symbols, diagnostics);
            let (type_parameters, lifetime_parameters) = program
                .machine_type_reference_scope(machine, |existing, parameter| {
                    existing.name == parameter.name
                });
            crate::value_custody::type_references::validate_const_binder_carriers(
                program,
                symbols,
                &format!("machine `{}`", machine.name),
                program.machine_type_parameters(machine),
                TypeParameterScope::authored(
                    machine.symbol,
                    &type_parameters,
                    &lifetime_parameters,
                ),
                diagnostics,
                formation_obligations,
            );
            validate_state_signature_types(
                program
                    .machine_states(machine)
                    .iter()
                    .map(|state| StateSignatureView {
                        symbol: state.symbol,
                        name: state.name.as_str(),
                        lifetime_parameters: &[],
                        parameters: program.state_parameters(state),
                        return_type: state.return_type,
                        contracts: program.state_contracts(state),
                    }),
                program,
                symbols,
                diagnostics,
                StateSignatureOwner::Machine(machine),
                TypeParameterScope::authored(
                    machine.symbol,
                    &type_parameters,
                    &lifetime_parameters,
                ),
                formation_obligations,
            );
            validate_machine_parameter_signatures(
                program,
                symbols,
                diagnostics,
                program.machine_type_parameters(machine),
                &type_parameters,
                &lifetime_parameters,
                machine.name.as_str(),
                formation_obligations,
            );
            validate_machine_state_result_agreement(program, machine, diagnostics);
        }

        for definition in program.data_definitions() {
            let type_parameters = program.data_type_parameters(definition);
            validate_machine_parameter_signatures(
                program,
                symbols,
                diagnostics,
                type_parameters,
                type_parameters,
                &definition.lifetime_parameters,
                definition.name.as_str(),
                formation_obligations,
            );
        }

        for trait_definition in program.traits() {
            for machine in program
                .trait_machine_signatures(trait_definition)
                .iter()
                .chain(
                    program
                        .closed_requirement_applications()
                        .filter(|application| {
                            application.declaring_trait == trait_definition.symbol
                        })
                        .map(|application| application.signature),
                )
            {
                let mut type_parameters = program.trait_type_parameters(trait_definition).to_vec();
                type_parameters.extend_from_slice(program.state_signature_type_parameters(machine));
                for lifetime in &machine.lifetime_parameters {
                    if trait_definition
                        .lifetime_parameters
                        .iter()
                        .any(|inherited| inherited == lifetime)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "trait requirement `{}` redeclares inherited lifetime `'{}'",
                            machine.name,
                            lifetime.as_str()
                        )));
                    }
                }
                let mut lifetime_parameters = trait_definition.lifetime_parameters.clone();
                lifetime_parameters.extend_from_slice(&machine.lifetime_parameters);
                validate_state_signature_types(
                    std::iter::once(StateSignatureView {
                        symbol: machine.symbol,
                        name: machine.name.as_str(),
                        lifetime_parameters: &machine.lifetime_parameters,
                        parameters: program.state_signature_parameters(machine),
                        return_type: machine.return_type,
                        contracts: program.state_signature_contracts(machine),
                    }),
                    program,
                    symbols,
                    diagnostics,
                    StateSignatureOwner::Trait(trait_definition.name.as_str()),
                    TypeParameterScope::authored(
                        machine.symbol,
                        &type_parameters,
                        &lifetime_parameters,
                    ),
                    formation_obligations,
                );
                validate_machine_parameter_signatures(
                    program,
                    symbols,
                    diagnostics,
                    program.state_signature_type_parameters(machine),
                    &type_parameters,
                    &lifetime_parameters,
                    machine.name.as_str(),
                    formation_obligations,
                );
            }
        }
    }

    fn validate_machine_state_result_agreement(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let states = program.machine_states(machine);
        let Some(result_state) = states.first() else {
            return;
        };
        let result = program.normalized_type_identity(result_state.return_type);
        for state in &states[1..] {
            if program.normalized_type_identity(state.return_type) == result {
                continue;
            }
            let declared = if state.return_type.is_valid() {
                format!(
                    "result `{}`",
                    program.normalized_type_identity_text(state.return_type)
                )
            } else {
                "no result".to_string()
            };
            let expected = if result_state.return_type.is_valid() {
                format!(
                    "result `{}`",
                    program.normalized_type_identity_text(result_state.return_type)
                )
            } else {
                "no result".to_string()
            };
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` declares {declared}, which differs from the machine's {expected}",
            machine.name,
            state.name.as_str(),
        )));
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct StateSignatureView<'program> {
        symbol: symbols::SymbolHandle,
        name: &'program str,
        lifetime_parameters: &'program [Identifier],
        parameters: &'program [StateParameter],
        return_type: TypeReferenceHandle,
        contracts: &'program [SignatureContract],
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum StateSignatureOwner<'program> {
        Machine(&'program Machine),
        Trait(&'program str),
        Requirement(&'program str),
        RuntimeCallable,
    }

    impl fmt::Display for StateSignatureOwner<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Machine(machine) => write!(formatter, "machine `{}`", machine.name),
                Self::Trait(trait_definition) => write!(formatter, "trait `{trait_definition}`"),
                Self::Requirement(parameter) => {
                    write!(formatter, "machine-parameter requirement `{parameter}`")
                }
                Self::RuntimeCallable => formatter.write_str("machine value contract"),
            }
        }
    }

    pub(crate) fn validate_runtime_callable_type(
        program: &dyn crate::CheckingInput,
        contract: typed_trees::callable_values::RuntimeCallableContractHandle,
        symbols: &TopLevelSymbols<'_>,
        inherited: TypeParameterScope<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let signature = &program.runtime_callable_contract(contract).signature;
        let own_parameters = program.state_signature_type_parameters(signature);
        let mut parameters = inherited.type_parameters.to_vec();
        parameters.extend_from_slice(own_parameters);
        let mut lifetimes = inherited.lifetime_parameters.to_vec();
        lifetimes.extend_from_slice(&signature.lifetime_parameters);
        let scope = TypeParameterScope::authored(signature.symbol, &parameters, &lifetimes);
        crate::value_custody::type_references::validate_const_binder_carriers(
            program,
            symbols,
            "machine value contract",
            own_parameters,
            scope,
            diagnostics,
            obligations,
        );
        validate_state_signature_types(
            std::iter::once(StateSignatureView {
                symbol: signature.symbol,
                name: signature.name.as_str(),
                lifetime_parameters: &signature.lifetime_parameters,
                parameters: program.state_signature_parameters(signature),
                return_type: signature.return_type,
                contracts: program.state_signature_contracts(signature),
            }),
            program,
            symbols,
            diagnostics,
            StateSignatureOwner::RuntimeCallable,
            scope,
            obligations,
        );
        validate_machine_parameter_signatures(
            program,
            symbols,
            diagnostics,
            own_parameters,
            &parameters,
            &lifetimes,
            "machine value contract",
            obligations,
        );
    }

    pub(crate) fn validate_machine_parameter_signatures<'program>(
        program: &'program dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        parameters: &'program [typed_trees::data::TypeParameter],
        inherited_type_parameters: &[typed_trees::data::TypeParameter],
        inherited_lifetime_parameters: &[Identifier],
        declaration: &'program str,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for parameter in parameters {
            let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let contract = program
                .machine_parameter_contract_view(contract)
                .expect("typed machine-parameter contract must retain a valid requirement identity")
                .signature();
            let nested = program.state_signature_type_parameters(contract);
            for (index, nested_parameter) in nested.iter().enumerate() {
                if nested[..index]
                    .iter()
                    .any(|previous| previous.name == nested_parameter.name)
                {
                    diagnostics.push(Diagnostic::error(format!(
                    "machine-parameter requirement `{}` on `{declaration}` has duplicate generic parameter `{}`",
                    parameter.name, nested_parameter.name
                )));
                }
            }

            let mut local_type_parameters = inherited_type_parameters.to_vec();
            let mut local_lifetime_parameters = inherited_lifetime_parameters.to_vec();
            for nested_parameter in nested {
                if !local_type_parameters
                    .iter()
                    .any(|existing| existing.symbol == nested_parameter.symbol)
                {
                    local_type_parameters.push(nested_parameter.clone());
                }
            }
            for nested_parameter in &contract.lifetime_parameters {
                if !local_lifetime_parameters
                    .iter()
                    .any(|existing| existing == nested_parameter)
                {
                    local_lifetime_parameters.push(nested_parameter.clone());
                }
            }
            validate_state_signature_types(
                std::iter::once(StateSignatureView {
                    symbol: contract.symbol,
                    name: contract.name.as_str(),
                    lifetime_parameters: &contract.lifetime_parameters,
                    parameters: program.state_signature_parameters(contract),
                    return_type: contract.return_type,
                    contracts: program.state_signature_contracts(contract),
                }),
                program,
                symbols,
                diagnostics,
                StateSignatureOwner::Requirement(parameter.name.as_str()),
                TypeParameterScope::authored(
                    contract.symbol,
                    &local_type_parameters,
                    &local_lifetime_parameters,
                ),
                formation_obligations,
            );
            validate_machine_parameter_signatures(
                program,
                symbols,
                diagnostics,
                nested,
                &local_type_parameters,
                &local_lifetime_parameters,
                declaration,
                formation_obligations,
            );
        }
    }

    fn validate_state_signature_types<'program>(
        signatures: impl Iterator<Item = StateSignatureView<'program>>,
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        owner: StateSignatureOwner<'program>,
        type_parameter_scope: TypeParameterScope<'_>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for signature in signatures {
            let mut lifetime_parameters = type_parameter_scope.lifetime_parameters.to_vec();
            for parameter in signature.lifetime_parameters {
                if !lifetime_parameters
                    .iter()
                    .any(|existing| existing == parameter)
                {
                    lifetime_parameters.push(parameter.clone());
                }
            }
            validate_state_parameter_names(signature, owner, diagnostics);
            validate_state_signature_contracts(program, signature, owner, diagnostics);

            for parameter in signature.parameters {
                validate_type_reference_handle(
                    program,
                    parameter.type_reference,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::StateParameter {
                        signature: signature.symbol,
                        owner,
                        state: signature.name,
                        parameter: parameter.name.as_str(),
                        generic_depth: 0,
                    },
                    TypeParameterScope {
                        lifetime_parameters: &lifetime_parameters,
                        ..type_parameter_scope
                    },
                    formation_obligations,
                );
            }

            if signature.return_type.is_valid() {
                validate_type_reference_handle(
                    program,
                    signature.return_type,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::StateReturn {
                        signature: signature.symbol,
                        owner,
                        state: signature.name,
                        generic_depth: 0,
                    },
                    TypeParameterScope {
                        lifetime_parameters: &lifetime_parameters,
                        ..type_parameter_scope
                    },
                    formation_obligations,
                );
            }
        }
    }

    fn validate_state_signature_contracts(
        program: &dyn crate::CheckingInput,
        signature: StateSignatureView<'_>,
        owner: StateSignatureOwner<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for contract in signature.contracts {
            if matches!(owner, StateSignatureOwner::Machine(_))
                && contract.kind != SignatureContractKind::Requires
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} state `{}` admits only arrival `requires` contracts",
                    signature.name
                )));
                continue;
            }
            validate_named_evidence_binding(
                program,
                &format!("{owner} state `{}`", signature.name),
                contract,
                diagnostics,
            );
            validate_crash_route_shapes(program, contract, diagnostics);
            validate_proof_facts(
                program,
                program.proof_facts.span_or_empty(contract.facts),
                diagnostics,
                ProofFactOwner::StateSignatureContract {
                    owner,
                    state: signature.name,
                    kind: contract_kind_label(&contract.kind),
                },
            );
        }
    }

    pub(crate) fn signature_entry_substitutions(
        program: &dyn crate::CheckingInput,
        requirement: &StateSignature,
        entry_state: &State,
    ) -> Vec<(symbols::SymbolHandle, String, String)> {
        program
            .state_signature_parameters(requirement)
            .iter()
            .zip(program.state_parameters(entry_state))
            .map(|(required, actual)| {
                (
                    required.symbol,
                    required.name.as_str().to_owned(),
                    actual.name.as_str().to_owned(),
                )
            })
            .collect()
    }

    pub(crate) fn validate_machine_contracts(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for contract in program.machine_contracts(machine) {
            validate_named_evidence_binding(
                program,
                &format!("machine `{}`", machine.name),
                contract,
                diagnostics,
            );
            validate_crash_route_shapes(program, contract, diagnostics);
            validate_proof_facts(
                program,
                program.proof_facts.span_or_empty(contract.facts),
                diagnostics,
                ProofFactOwner::MachineContract {
                    machine,
                    kind: contract_kind_label(&contract.kind),
                },
            );
        }
    }

    fn validate_named_evidence_binding(
        program: &dyn crate::CheckingInput,
        owner: &str,
        contract: &typed_trees::signature::SignatureContract,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(binding) = contract.binding.as_ref() else {
            return;
        };
        let facts = program.proof_facts.span_or_empty(contract.facts);
        let [typed_trees::domain::ProofFact::Proposition(application)] = facts else {
            diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` must bind exactly one proposition application",
            contract_kind_label(&contract.kind),
        )));
            return;
        };
        let Some(normalized) = program.normalize_nominal_proposition_application(application, None)
        else {
            diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` does not resolve to one nominal proposition endpoint",
            contract_kind_label(&contract.kind),
        )));
            return;
        };
        if !matches!(
            normalized.classification,
            typed_trees::proposition::PropositionEvidenceClassification::Witness { .. }
        ) {
            diagnostics.push(Diagnostic::error(format!(
            "{owner} named {} evidence `{binding}` binds fact-only proposition `{}`; only a witness-bearing proposition has a projectable evidence term",
            contract_kind_label(&contract.kind),
            normalized.name,
        )));
        }
    }

    fn validate_crash_route_shapes(
        program: &dyn crate::CheckingInput,
        contract: &typed_trees::signature::SignatureContract,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let SignatureContractKind::Crashes { cause } = &contract.kind else {
            return;
        };
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if !matches!(fact, typed_trees::domain::ProofFact::Expression(_)) {
                diagnostics.push(Diagnostic::error(format!(
                "`crashes {cause:?}` routes must be Boolean expressions; domain memberships and proposition applications are proof facts, not runtime-refinable crash routes",
            )));
            }
        }
    }

    fn contract_kind_label(kind: &SignatureContractKind) -> &'static str {
        match kind {
            SignatureContractKind::Requires => "requires",
            SignatureContractKind::Ensures => "ensures",
            SignatureContractKind::EnsuresForResultCase { .. } => "outcome-specific ensures",
            SignatureContractKind::Crashes { .. } => "crashes",
        }
    }

    fn validate_state_parameter_names(
        state: StateSignatureView<'_>,
        owner: StateSignatureOwner<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (parameter_index, parameter) in state.parameters.iter().enumerate() {
            if state.parameters[..parameter_index]
                .iter()
                .any(|previous| previous.name == parameter.name)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} state `{}` has duplicate parameter `{}`",
                    state.name, parameter.name
                )));
            }
        }
    }
}

pub mod symbols {
    pub(crate) mod machine {
        use diagnostics::Diagnostic;
        use symbols::SymbolHandle;
        use symbols::SymbolKeyMap;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Debug)]
        pub struct MachineSymbols<'program> {
            callable_fields: Vec<CallableFieldSymbol<'program>>,
            member_symbols: Vec<MemberSymbol<'program>>,
            owned_data_symbols: Vec<MemberSymbol<'program>>,
            states: Vec<StateSymbol<'program>>,
        }

        #[derive(Debug)]
        pub(crate) struct CallableFieldSymbol<'program> {
            name: &'program str,
            type_name: &'program str,
        }

        #[derive(Debug)]
        pub(crate) struct MemberSymbol<'program> {
            name: &'program str,
            symbol: SymbolHandle,
        }

        #[derive(Debug)]
        pub(crate) struct StateSymbol<'program> {
            name: &'program str,
            state: &'program State,
        }

        impl<'program> MachineSymbols<'program> {
            pub fn build(
                program: &'program dyn crate::CheckingInput,
                machine: &'program Machine,
                diagnostics: &mut Vec<Diagnostic>,
            ) -> Self {
                let machine_symbol = retained_machine_symbol(program, machine);
                let mut member_verdicts: SymbolKeyMap<&str, bool> = SymbolKeyMap::default();
                let mut owned_data_verdicts: SymbolKeyMap<&str, bool> = SymbolKeyMap::default();
                let mut state_verdicts: SymbolKeyMap<&str, bool> = SymbolKeyMap::default();
                let mut symbols = Self {
                    callable_fields: Vec::new(),
                    member_symbols: Vec::with_capacity(program.machine_owned_data(machine).len()),
                    owned_data_symbols: Vec::with_capacity(
                        program.machine_owned_data(machine).len(),
                    ),
                    states: Vec::with_capacity(program.machine_states(machine).len()),
                };

                if let Some(data_definition) =
                    crate::value_custody::places::machine_attached_data(program, machine)
                {
                    for member in program.data_members(data_definition) {
                        let Some(field) = member.as_field() else {
                            continue;
                        };

                        if let Some(type_name) =
                            callable_receiver_type_name(program, field.type_reference)
                        {
                            symbols.callable_fields.push(CallableFieldSymbol {
                                name: field.name.as_str(),
                                type_name,
                            });
                        }
                    }
                }

                for owned_data in program.machine_owned_data(machine) {
                    if member_verdicts
                        .get(owned_data.name.as_str())
                        .copied()
                        .unwrap_or(false)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` has duplicate member `{}`",
                            machine.name, owned_data.name
                        )));
                    }

                    if owned_data_verdicts
                        .get(owned_data.name.as_str())
                        .copied()
                        .unwrap_or(false)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` has duplicate owned data `{}`",
                            machine.name, owned_data.name
                        )));
                    }

                    let symbol = retained_child_symbol(
                        program,
                        machine_symbol,
                        owned_data.symbol,
                        owned_data.name.as_str(),
                    );
                    member_verdicts
                        .entry(owned_data.name.as_str())
                        .or_insert(symbol.is_valid());
                    owned_data_verdicts
                        .entry(owned_data.name.as_str())
                        .or_insert(symbol.is_valid());
                    symbols.member_symbols.push(MemberSymbol {
                        name: owned_data.name.as_str(),
                        symbol,
                    });
                    symbols.owned_data_symbols.push(MemberSymbol {
                        name: owned_data.name.as_str(),
                        symbol,
                    });
                }

                for state in program.machine_states(machine) {
                    if state_verdicts
                        .get(state.name.as_str())
                        .copied()
                        .unwrap_or(false)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` has duplicate state `{}`",
                            machine.name, state.name
                        )));
                    }

                    let symbol = retained_child_symbol(
                        program,
                        machine_symbol,
                        state.symbol,
                        state.name.as_str(),
                    );
                    state_verdicts
                        .entry(state.name.as_str())
                        .or_insert(symbol.is_valid());
                    symbols.states.push(StateSymbol {
                        name: state.name.as_str(),
                        state,
                    });
                }

                symbols
            }

            pub fn state(&self, name: &str) -> Option<&'program State> {
                super::named_value(&self.states, name, |e| e.name, |e| e.state)
            }

            pub fn callable_field_type(&self, name: &str) -> Option<&'program str> {
                super::named_value(&self.callable_fields, name, |e| e.name, |e| e.type_name)
            }

            pub fn has_member(&self, name: &str) -> bool {
                self.member_symbol(name).is_valid()
            }

            pub fn has_owned_data(&self, name: &str) -> bool {
                self.owned_data_symbol(name).is_valid()
            }

            pub fn member_symbol(&self, name: &str) -> SymbolHandle {
                super::named_symbol(&self.member_symbols, name, |e| e.name, |e| e.symbol)
            }

            pub fn owned_data_symbol(&self, name: &str) -> SymbolHandle {
                super::named_symbol(&self.owned_data_symbols, name, |e| e.name, |e| e.symbol)
            }
        }

        fn retained_machine_symbol(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
        ) -> SymbolHandle {
            let spelling = machine.symbol_spelling();
            let authored =
                retained_child_symbol(program, program.symbols.root(), machine.symbol, &spelling);
            if authored.is_valid() {
                return authored;
            }
            let symbol = machine.symbol;
            if symbol.is_valid()
                && program.symbols.get(symbol).kind == symbols::SymbolKind::Machine
                && program.symbols.get(symbol).generated_from.is_valid()
                && program.symbols.name(symbol) == spelling
            {
                symbol
            } else {
                SymbolHandle::invalid()
            }
        }

        fn retained_child_symbol(
            program: &dyn crate::CheckingInput,
            parent: SymbolHandle,
            symbol: SymbolHandle,
            name: &str,
        ) -> SymbolHandle {
            if symbol.is_valid()
                && program.symbols.get(symbol).parent == parent
                && program.symbols.name(symbol) == name
            {
                symbol
            } else {
                SymbolHandle::invalid()
            }
        }

        fn callable_receiver_type_name(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<&str> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Callable { .. } => None,
                TypeReferenceNode::Reference { referee, .. } => {
                    callable_receiver_type_name(program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    callable_receiver_type_name(program, *base_type)
                }
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => None,
                TypeReferenceNode::Generic { .. } | TypeReferenceNode::ConstExpression(_) => None,
                TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
                TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
                TypeReferenceNode::Unit => None,
            }
        }
    }
    pub(crate) mod top_level {
        use crate::CheckingInput;
        use diagnostics::Diagnostic;
        use language_semantics::MachineSupplyMode;
        use std::sync::Mutex;
        use symbols::{SymbolHandle, SymbolKeyMap as HashMap, SymbolKind};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::trait_definition::TraitDefinition;

        #[derive(Debug, Default)]
        pub(crate) struct CallerSiteCaches {
            pub tracked_origin_machines: Mutex<HashMap<SymbolHandle, bool>>,
            pub site_statements:
                Mutex<HashMap<(SymbolHandle, u8, usize, usize), Option<(SymbolHandle, usize)>>>,
            pub prefix_site_entries:
                Mutex<HashMap<(SymbolHandle, u8, usize, usize), Option<PrefixSiteEntry>>>,
            complete_state_summaries: Mutex<HashMap<SymbolHandle, Vec<String>>>,
        }

        impl CallerSiteCaches {
            pub(crate) fn completed_state_writes(
                &self,
                state: SymbolHandle,
            ) -> Option<Vec<String>> {
                self.complete_state_summaries
                    .lock()
                    .ok()?
                    .get(&state)
                    .cloned()
            }

            pub(crate) fn record_completed_state_writes(
                &self,
                state: SymbolHandle,
                writes: &[String],
            ) {
                if let Ok(mut summaries) = self.complete_state_summaries.lock() {
                    summaries.entry(state).or_insert_with(|| writes.to_vec());
                }
            }
        }

        #[derive(Debug, Clone, Copy)]
        pub(crate) enum PrefixSiteEntry {
            Untracked,
            Tracked { state: SymbolHandle, index: usize },
        }

        #[derive(Debug)]
        pub struct TopLevelSymbols<'program> {
            machines: Vec<MachineSymbol<'program>>,
            traits: Vec<TraitSymbol<'program>>,
            types: Vec<TypeSymbol<'program>>,
            pub(crate) caller_sites: std::sync::Arc<CallerSiteCaches>,
        }

        #[derive(Debug)]
        pub(crate) struct MachineSymbol<'program> {
            name: &'program str,
            machine: &'program Machine,
            symbol: SymbolHandle,
            nested: bool,
        }

        #[derive(Debug)]
        pub(crate) struct TraitSymbol<'program> {
            name: &'program str,
            trait_definition: &'program TraitDefinition,
            symbol: SymbolHandle,
        }

        #[derive(Debug)]
        pub(crate) struct TypeSymbol<'program> {
            name: &'program str,
            symbol: SymbolHandle,
        }

        impl<'program> TopLevelSymbols<'program> {
            pub fn build(
                program: &'program dyn CheckingInput,
                diagnostics: &mut Vec<Diagnostic>,
            ) -> Self {
                let data_definition_count = program.data_definitions().len();
                let machine_count = program.machines().len();
                let trait_count = program.traits().len();
                let mut symbols = Self {
                    machines: Vec::with_capacity(machine_count),
                    traits: Vec::with_capacity(trait_count),
                    types: builtin_type_symbols(program),
                    caller_sites: crate::frozen_program::frozen_memo(
                        program,
                        |memos| &memos.caller_sites,
                        CallerSiteCaches::default,
                    ),
                };
                symbols.types.reserve(data_definition_count + trait_count);

                let mut data_by_name: symbols::SymbolKeyMap<&str, Vec<SymbolHandle>> =
                    symbols::SymbolKeyMap::default();
                let mut machines_by_name: symbols::SymbolKeyMap<
                    &str,
                    Vec<&'program typed_trees::machine::Machine>,
                > = symbols::SymbolKeyMap::default();
                let mut traits_by_name: symbols::SymbolKeyMap<&str, Vec<SymbolHandle>> =
                    symbols::SymbolKeyMap::default();

                for data_definition in program.data_definitions() {
                    let conflicts =
                        data_by_name
                            .get(data_definition.name.as_str())
                            .is_some_and(|previous| {
                                previous.iter().any(|previous| {
                                    !program
                                        .symbols
                                        .source_scopes_separate(*previous, data_definition.symbol)
                                })
                            });
                    if conflicts {
                        diagnostics.push(Diagnostic::error(format!(
                            "duplicate data `{}`",
                            data_definition.name
                        )));
                    }

                    symbols.types.push(TypeSymbol {
                        name: data_definition.name.as_str(),
                        symbol: data_definition.symbol,
                    });
                    data_by_name
                        .entry(data_definition.name.as_str())
                        .or_default()
                        .push(data_definition.symbol);
                }

                for machine in program.machines() {
                    let same_named = machines_by_name
                        .get(machine.name.as_str())
                        .map(|previous| {
                            previous
                                .iter()
                                .filter(|previous| {
                                    let previous_origin =
                                        program.symbols.authored_origin(previous.symbol);
                                    let current_origin =
                                        program.symbols.authored_origin(machine.symbol);
                                    program.symbols.get(previous_origin).parent
                                        == program.symbols.get(current_origin).parent
                                        && !program
                                            .symbols
                                            .source_scopes_separate(previous.symbol, machine.symbol)
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let identity = program.machine_callable_identity(machine);
                    let is_named_overload_family = !same_named.is_empty()
                        && identity.is_valid()
                        && same_named.iter().all(|previous| {
                            let previous_identity = program.machine_callable_identity(previous);
                            program.callable_identities_share_path(previous_identity, identity)
                                && (program
                                    .callable_identities_share_family(previous_identity, identity)
                                    || (machine.spelling.is_none() && previous.spelling.is_none()))
                        });
                    let is_boundary_token_overload_family = machine.spelling.is_some()
                        && machine.supply_mode == MachineSupplyMode::TopLevelRequirement
                        && identity.is_valid()
                        && same_named.iter().all(|previous| {
                            let previous_identity = program.machine_callable_identity(previous);
                            previous.spelling == machine.spelling
                                && previous.supply_mode == MachineSupplyMode::TopLevelRequirement
                                && program
                                    .callable_identities_share_path(previous_identity, identity)
                                && !program
                                    .callable_identities_share_family(previous_identity, identity)
                        });
                    if !same_named.is_empty()
                        && !is_named_overload_family
                        && !is_boundary_token_overload_family
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "duplicate machine `{}`",
                            machine.name
                        )));
                    }

                    let parent = program.symbols.get(machine.symbol).parent;
                    symbols.machines.push(MachineSymbol {
                        name: machine.name.as_str(),
                        machine,
                        symbol: machine.symbol,
                        nested: parent.is_valid()
                            && program.symbols.get(parent).kind == SymbolKind::Machine,
                    });
                    machines_by_name
                        .entry(machine.name.as_str())
                        .or_default()
                        .push(machine);
                }

                for trait_definition in program.traits() {
                    if traits_by_name
                        .get(trait_definition.name.as_str())
                        .is_some_and(|previous| {
                            previous.iter().any(|previous| {
                                !program
                                    .symbols
                                    .source_scopes_separate(*previous, trait_definition.symbol)
                            })
                        })
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "duplicate trait `{}`",
                            trait_definition.name
                        )));
                    }

                    if data_by_name
                        .get(trait_definition.name.as_str())
                        .is_some_and(|previous| {
                            previous.iter().any(|previous| {
                                !program
                                    .symbols
                                    .source_scopes_separate(*previous, trait_definition.symbol)
                            })
                        })
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "`{}` is declared as both data and a trait",
                            trait_definition.name
                        )));
                    }

                    if machines_by_name
                        .get(trait_definition.name.as_str())
                        .is_some_and(|previous| {
                            previous.iter().any(|previous| {
                                !program.symbols.source_scopes_separate(
                                    previous.symbol,
                                    trait_definition.symbol,
                                )
                            })
                        })
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "`{}` is declared as both a machine and a trait",
                            trait_definition.name
                        )));
                    }

                    symbols.traits.push(TraitSymbol {
                        name: trait_definition.name.as_str(),
                        trait_definition,
                        symbol: trait_definition.symbol,
                    });
                    symbols.types.push(TypeSymbol {
                        name: trait_definition.name.as_str(),
                        symbol: trait_definition.symbol,
                    });
                    traits_by_name
                        .entry(trait_definition.name.as_str())
                        .or_default()
                        .push(trait_definition.symbol);
                }

                symbols
            }

            pub fn has_type(&self, name: &str) -> bool {
                self.type_symbol(name).is_valid()
                    || self.machine_symbol(name).is_valid()
                    || self.trait_symbol(name).is_valid()
            }

            pub fn has_type_symbol(&self, symbol: SymbolHandle) -> bool {
                symbol.is_valid()
                    && (self
                        .types
                        .iter()
                        .any(|candidate| candidate.symbol == symbol)
                        || self
                            .machines
                            .iter()
                            .any(|candidate| candidate.symbol == symbol)
                        || self
                            .traits
                            .iter()
                            .any(|candidate| candidate.symbol == symbol))
            }

            pub fn trait_definition_by_symbol(
                &self,
                symbol: SymbolHandle,
            ) -> Option<&'program TraitDefinition> {
                self.traits
                    .iter()
                    .find(|candidate| candidate.symbol == symbol)
                    .map(|candidate| candidate.trait_definition)
            }

            fn type_symbol(&self, name: &str) -> SymbolHandle {
                super::named_symbol(&self.types, name, |e| e.name, |e| e.symbol)
            }

            pub fn machine(&self, name: &str) -> Option<&'program Machine> {
                self.nameable_machine(name).map(|entry| entry.machine)
            }

            fn nameable_machine(&self, name: &str) -> Option<&MachineSymbol<'program>> {
                self.machines
                    .iter()
                    .find(|entry| !entry.nested && entry.name == name)
            }

            pub fn attached_machine_state(
                &self,
                program: &'program dyn CheckingInput,
                data_name: &str,
                state_name: &str,
            ) -> Option<(&'program Machine, &'program State)> {
                self.machines.iter().find_map(|symbol| {
                    (symbol
                        .machine
                        .attached_data
                        .as_ref()
                        .is_some_and(|attached_data| attached_data.as_str() == data_name))
                    .then(|| {
                        program
                            .machine_states(symbol.machine)
                            .first()
                            .filter(|state| state.name.as_str() == state_name)
                            .map(|state| (symbol.machine, state))
                    })?
                })
            }

            fn machine_symbol(&self, name: &str) -> SymbolHandle {
                self.nameable_machine(name)
                    .map_or_else(SymbolHandle::invalid, |entry| entry.symbol)
            }

            pub fn trait_definition(&self, name: &str) -> Option<&'program TraitDefinition> {
                super::named_value(&self.traits, name, |e| e.name, |e| e.trait_definition)
            }

            fn trait_symbol(&self, name: &str) -> SymbolHandle {
                super::named_symbol(&self.traits, name, |e| e.name, |e| e.symbol)
            }
        }

        fn builtin_type_symbols(program: &dyn CheckingInput) -> Vec<TypeSymbol<'_>> {
            let Some(root_children) = program.symbols.child_handles(program.symbols.root()) else {
                return Vec::new();
            };
            let (min_count, _) = root_children.size_hint();
            let mut symbols = Vec::with_capacity(min_count);

            for symbol in root_children {
                let kind = program.symbols.get(symbol).kind;

                if kind != SymbolKind::BuiltinType {
                    continue;
                }

                symbols.push(TypeSymbol {
                    name: program.symbols.name(symbol),
                    symbol,
                });
            }

            symbols
        }
    }

    use symbols::SymbolHandle;

    pub use machine::MachineSymbols;
    pub use top_level::TopLevelSymbols;
    pub(crate) use top_level::{CallerSiteCaches, PrefixSiteEntry};

    fn named_value<Entry, Value>(
        entries: &[Entry],
        name: &str,
        key: impl Fn(&Entry) -> &str,
        value: impl FnOnce(&Entry) -> Value,
    ) -> Option<Value> {
        entries.iter().find(|entry| key(entry) == name).map(value)
    }

    fn named_symbol<Entry>(
        entries: &[Entry],
        name: &str,
        key: impl Fn(&Entry) -> &str,
        symbol: impl FnOnce(&Entry) -> SymbolHandle,
    ) -> SymbolHandle {
        named_value(entries, name, key, symbol).unwrap_or_else(SymbolHandle::invalid)
    }
}

pub mod transitions {
    use crate::machine_calls::calls::validate_call_arguments;
    use crate::proof_contracts::arithmetic_domains::ValueEnvironment;
    use crate::value_custody::locals::WritableRoots;
    use diagnostics::Diagnostic;
    use symbols::JoinedPath;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;
    use typed_trees::statement::{
        StatementNode, TableTransition, TransitionGuardNode, TransitionTargetHandle,
        TransitionTargetNode,
    };

    pub(crate) mod evaluation {
        use super::{
            ExpressionHandle, Machine, State, TransitionTargetHandle, TransitionTargetNode,
            ValueEnvironment,
        };
        use crate::machine_calls::calls::{CallFrameResolver, expression_has_calls};
        use typed_trees::statement::StatementNode;

        #[derive(Default)]
        pub(crate) struct TransitionValueEnvironments {
            targets: Vec<(TransitionTargetHandle, Vec<ValueEnvironment>)>,
        }

        impl TransitionValueEnvironments {
            pub(crate) fn collect<'program>(
                program: &'program dyn crate::CheckingInput,
                machine: &'program Machine,
                state: &State,
                statement: &StatementNode,
                before: &ValueEnvironment,
                frames: Option<&CallFrameResolver<'program>>,
            ) -> Self {
                let Some(transition) = statement.as_transition() else {
                    return Self::default();
                };
                let mut result = Self::default();
                for (positive, target) in
                    [(true, transition.target), (false, transition.continuation)]
                {
                    if !target.is_valid() {
                        continue;
                    }
                    let expressions = match program.statement_table.transition_target(target) {
                        TransitionTargetNode::Named { arguments, .. } => {
                            program.statement_table.expression_handles(*arguments)
                        }
                        TransitionTargetNode::Value(expression) => std::slice::from_ref(expression),
                        TransitionTargetNode::Terminal => continue,
                    };
                    let mut current = if positive {
                        crate::proof_contracts::arithmetic_domains::guard_narrowed_environment(
                            program,
                            machine,
                            Some(state),
                            &transition.guard,
                            before,
                        )
                    } else {
                        crate::proof_contracts::arithmetic_domains::fall_through_narrowed_environment(
                    program,
                    machine,
                    Some(state),
                    &transition.guard,
                    before,
                )
                    };
                    let mut environments = Vec::new();
                    for expression in expressions {
                        cross_expression_effects(
                            &mut current,
                            program,
                            machine,
                            state,
                            *expression,
                            frames,
                        );
                        environments.push(current.clone());
                    }
                    result.targets.push((target, environments));
                }
                result
            }

            pub(crate) fn for_target(&self, target: TransitionTargetHandle) -> &[ValueEnvironment] {
                self.targets
                    .iter()
                    .find(|(candidate, _)| *candidate == target)
                    .map_or(&[], |(_, environments)| environments.as_slice())
            }
        }

        fn cross_expression_effects<'program>(
            environment: &mut ValueEnvironment,
            program: &'program dyn crate::CheckingInput,
            machine: &'program Machine,
            state: &State,
            expression: ExpressionHandle,
            frames: Option<&CallFrameResolver<'program>>,
        ) {
            if let Some(written) =
                frames.and_then(|frames| frames.expression_may_write_paths(machine, expression))
            {
                environment.invalidate_call_paths(program, Some(state), &written);
                return;
            }
            if frames.is_none() && !expression_has_calls(program, expression) {
                return;
            }
            environment.invalidate_unknown_writes(program, Some(state));
        }
    }
    pub(crate) mod terminal_results {
        use crate::CheckingInput;
        use diagnostics::Diagnostic;
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::types::PrimitiveType;
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum ResultShape {
            Unit,
            Primitive(PrimitiveType, numerics::arithmetic::ArithmeticDomain),
            Data(symbols::SymbolHandle),
        }

        pub(crate) fn validate(
            program: &dyn CheckingInput,
            caller_machine: &Machine,
            caller_state: &State,
            callee_machine: &Machine,
            completed_reference: TypeReferenceHandle,
            target: typed_trees::statement::TransitionTargetHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if program.static_transition_type_context_is_unreachable(target) {
                return;
            }
            let expected_reference = program
                .statically_refined_transition_type_reference(target, caller_state.return_type);
            let (Some(expected), Some(actual)) = (
                shape(program, expected_reference),
                shape(program, completed_reference),
            ) else {
                return;
            };
            if expected == actual {
                return;
            }
            let expected_spelling = spelling(program, expected_reference);
            let remedy = match (expected, actual) {
                (ResultShape::Primitive(..), ResultShape::Primitive(..)) => format!(
                    " -- return a converted value instead (`-> ({}(..) as {expected_spelling})`)",
                    callee_machine.name
                ),
                _ => String::new(),
            };
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` terminal call to `{}` completes with its `{}` result, but \
         the state returns `{expected_spelling}`; a terminal machine call's normal result \
         is its caller's result, so the two must match{remedy}",
                caller_machine.name,
                caller_state.name,
                callee_machine.name,
                spelling(program, completed_reference),
            )));
        }

        fn shape(
            program: &dyn CheckingInput,
            mut handle: TypeReferenceHandle,
        ) -> Option<ResultShape> {
            if !handle.is_valid() {
                return Some(ResultShape::Unit);
            }
            if let Some(primitive) = program.primitive_type_reference(handle) {
                return Some(ResultShape::Primitive(
                    primitive,
                    program.arithmetic_domain_for_type_reference(handle),
                ));
            }
            loop {
                match program.type_reference_table.type_reference(handle) {
                    TypeReferenceNode::Unit => return Some(ResultShape::Unit),
                    TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
                    TypeReferenceNode::Named { symbol, .. }
                        if symbol.is_valid()
                            && crate::declarations::data_queries::data_definition_by_symbol(
                                program, *symbol,
                            )
                            .is_some() =>
                    {
                        return Some(ResultShape::Data(*symbol));
                    }
                    _ => return None,
                }
            }
        }

        fn spelling(program: &dyn CheckingInput, handle: TypeReferenceHandle) -> String {
            if matches!(shape(program, handle), Some(ResultShape::Unit)) {
                "Unit".to_owned()
            } else {
                program.display_type_reference_with_constraints(handle)
            }
        }
    }
    pub(crate) use evaluation::TransitionValueEnvironments;

    pub(crate) fn admitted_dispatch_ends(
        transition: &TableTransition,
        following_statement: Option<&StatementNode>,
    ) -> bool {
        transition.guard == TransitionGuardNode::Always
            || transition.continuation.is_valid()
            || !matches!(following_statement, Some(StatementNode::Transition(_)))
    }

    pub(crate) fn resolved_transition_target_state(
        program: &dyn crate::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> Option<(&Machine, &State)> {
        if !target.is_valid() {
            return None;
        }
        if let Some(machine) = program.machine_by_symbol(target) {
            return program
                .machine_states(machine)
                .first()
                .map(|state| (machine, state));
        }
        let machine_symbol = program.symbols.get(target).parent;
        let machine = program.machine_by_symbol(machine_symbol)?;
        program
            .state_in_machine(machine, target)
            .map(|state| (machine, state))
    }

    pub(crate) fn named_transition_call_symbol(
        program: &dyn crate::CheckingInput,
        caller: symbols::SymbolHandle,
        target: symbols::SymbolHandle,
    ) -> symbols::SymbolHandle {
        if let Some((owner, state)) = resolved_transition_target_state(program, target) {
            return if owner.symbol != caller || target == owner.symbol {
                state.symbol
            } else {
                symbols::SymbolHandle::invalid()
            };
        }
        if program.machine_parameter_signature(target).is_some()
            || program.traits().iter().any(|definition| {
                program
                    .trait_machine_signature_by_symbol(definition, target)
                    .is_some()
            })
        {
            target
        } else {
            symbols::SymbolHandle::invalid()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_transition_target_node(
        program: &dyn crate::CheckingInput,
        current_machine: &Machine,
        current_state: Option<&State>,
        value_environment: &ValueEnvironment,
        argument_environments: &[ValueEnvironment],
        target: TransitionTargetHandle,
        writable_roots: &WritableRoots<'_, '_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        let target_node = program.statement_table.transition_target(target);
        if let TransitionTargetNode::Value(expression) = target_node {
            if let Some(call) = program.expression_table.call_node(*expression)
                && call.quotient_operation.is_none()
                && let Some((owner, state)) = resolved_transition_target_state(
                    program,
                    crate::call_target_symbol(program, call),
                )
                && owner.symbol != current_machine.symbol
                && let Some(current_state) = current_state
            {
                /*
                    A canonical float operation completes with its operands'
                    arithmetic policy, not with the bare carrier its declaration
                    names: `value * 3.0f32` over a `Trapping` operand is an
                    `f32 in Trapping`. The checked type of the call expression
                    already carries that adapter, so it is the terminal result
                    here. Operands whose policies disagree yield no checked
                    type, and the declared carrier is compared instead.
                */
                let completed =
                    crate::machine_calls::calls::selected_float_operation_retains_argument_policy(
                        program,
                        owner.symbol,
                    )
                    .then(|| {
                        crate::value_custody::expression_types::expression_result_type_reference(
                            program,
                            current_machine,
                            current_state,
                            *expression,
                        )
                    })
                    .flatten()
                    .unwrap_or(state.return_type);
                terminal_results::validate(
                    program,
                    current_machine,
                    current_state,
                    owner,
                    completed,
                    target,
                    diagnostics,
                );
            }
            return;
        }
        let TransitionTargetNode::Named {
            path,
            arguments,
            source_span,
            ..
        } = target_node
        else {
            return;
        };

        let arguments = program.statement_table.expression_handles(*arguments);

        if let Some((owner, state)) = resolved_transition_target_state(program, path.symbol) {
            if owner.symbol != current_machine.symbol
                && let Some(current_state) = current_state
            {
                terminal_results::validate(
                    program,
                    current_machine,
                    current_state,
                    owner,
                    state.return_type,
                    target,
                    diagnostics,
                );
            }
            validate_transition_arguments_handles(
                program,
                current_machine,
                current_state,
                value_environment,
                argument_environments,
                arguments,
                state.name.as_str(),
                program.state_parameters(state),
                writable_roots,
                diagnostics,
                formation_obligations,
            );
            return;
        }

        if let Some(signature) =
            program.machine_parameter_signature_in(current_machine, path.symbol)
        {
            validate_call_arguments(
                program,
                current_machine,
                current_state,
                value_environment,
                arguments,
                signature.name.as_str(),
                signature.name.as_str(),
                program.state_signature_parameters(signature),
                writable_roots.statements,
                false,
                argument_environments,
                &[],
                |argument, required| {
                    crate::value_custody::expression_types::argument_matches_carrier(
                        program,
                        current_machine.symbol,
                        argument,
                        required,
                        &[],
                    )
                },
                diagnostics,
                formation_obligations,
            );
            return;
        }

        let members = program.statement_table.name_path_members(path.members);
        diagnostics.push(
        Diagnostic::error(format!(
            "terminal call `{}` does not select a declared machine entry or callable requirement",
            members.joined_path(),
        ))
        .with_source_span(*source_span),
    );
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_transition_arguments_handles(
        program: &dyn crate::CheckingInput,
        current_machine: &Machine,
        current_state: Option<&State>,
        value_environment: &ValueEnvironment,
        argument_environments: &[ValueEnvironment],
        arguments: &[ExpressionHandle],
        target_name: &str,
        parameters: &[StateParameter],
        writable_roots: &WritableRoots<'_, '_>,
        diagnostics: &mut Vec<Diagnostic>,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        validate_call_arguments(
            program,
            current_machine,
            current_state,
            value_environment,
            arguments,
            target_name,
            target_name,
            parameters,
            writable_roots.statements,
            false,
            argument_environments,
            &[],
            |argument, required| {
                crate::value_custody::expression_types::argument_matches_carrier(
                    program,
                    current_machine.symbol,
                    argument,
                    required,
                    &[],
                )
            },
            diagnostics,
            formation_obligations,
        );
    }
}

pub mod type_aliases {
    use crate::declarations::symbols::TopLevelSymbols;
    use crate::value_custody::type_references::{
        TypeParameterScope, TypeReferenceOwner, validate_const_binder_carriers,
        validate_type_reference_handle,
    };
    use diagnostics::Diagnostic;

    pub(crate) fn validate(
        program: &dyn crate::CheckingInput,
        symbols: &TopLevelSymbols<'_>,
        diagnostics: &mut Vec<Diagnostic>,
        obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
    ) {
        for alias in program.type_aliases() {
            let parameters = program
                .tables
                .data_type_parameters
                .span_or_empty(alias.type_parameters);
            let scope =
                TypeParameterScope::authored(alias.symbol, parameters, &alias.lifetime_parameters);
            if symbols::builtin_type_symbols()
                .iter()
                .any(|(_, name)| name.as_str() == alias.name.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "type alias `{}` shadows a builtin type",
                    alias.name
                )));
            }
            validate_const_binder_carriers(
                program,
                symbols,
                &format!("type alias `{}`", alias.name),
                parameters,
                scope,
                diagnostics,
                obligations,
            );
            crate::declarations::state_signatures::validate_machine_parameter_signatures(
                program,
                symbols,
                diagnostics,
                parameters,
                parameters,
                &alias.lifetime_parameters,
                alias.name.as_str(),
                obligations,
            );
            crate::declarations::traits::validate_conformance_bounds(
                program,
                "type alias",
                alias.name.as_str(),
                &alias.conformance_bounds,
                symbols,
                scope,
                diagnostics,
                obligations,
            );
            validate_type_reference_handle(
                program,
                alias.expansion,
                symbols,
                diagnostics,
                TypeReferenceOwner::TypeAlias {
                    name: alias.name.as_str(),
                    generic_depth: 0,
                },
                scope,
                obligations,
            );
        }
    }
}
