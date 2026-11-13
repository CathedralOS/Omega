/*
Proof contract structure, immutable integer bounds, properties, qualification
evidence, relevance, proof embeddings, and slice ranking (`proof_contracts`).
*/
pub mod argument_subjects {
    pub fn contract_argument_root(
        program: &dyn crate::CheckingInput,
        target_state: &typed_trees::state::State,
        occurrence: typed_trees::expression::ExpressionHandle,
    ) -> checked_trees::OutcomeOccurrenceRoot {
        if !program.expression_table.expression_is_valid(occurrence) {
            return checked_trees::OutcomeOccurrenceRoot::Other;
        }
        if program.expression_table.names_reserved_result(occurrence) {
            return checked_trees::OutcomeOccurrenceRoot::CalleeResult;
        }
        if let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(occurrence)
            && path.symbol.is_valid()
            && let Some(position) = program
                .state_parameters(target_state)
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)
            && let Ok(source_position) = u32::try_from(position)
        {
            return checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position };
        }
        if let typed_trees::expression::ExpressionNode::Member(member) =
            program.expression_table.expression(occurrence)
            && member.member_symbol.is_valid()
            && member.case_variant.is_none()
        {
            let (source_position, mut fields) =
                match contract_argument_root(program, target_state, member.receiver) {
                    checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position } => {
                        (source_position, Vec::new())
                    }
                    checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                        source_position,
                        fields,
                    } => (source_position, fields),
                    _ => return checked_trees::OutcomeOccurrenceRoot::Other,
                };
            fields.push(member.member_symbol);
            return checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                source_position,
                fields,
            };
        }
        if program
            .closed_integer_value_in(occurrence, target_state.symbol)
            .is_some()
            || matches!(
                program.expression_table.expression(occurrence),
                typed_trees::expression::ExpressionNode::Boolean(_)
            )
        {
            return checked_trees::OutcomeOccurrenceRoot::ClosedValue;
        }
        checked_trees::OutcomeOccurrenceRoot::Other
    }
}

pub mod carrier_contents {
    use std::collections::HashSet;

    use typed_trees::domain::{DomainAliasDefinition, DomainDefinition};
    use typed_trees::proof_only::ProofOnlyClassification;
    use typed_trees::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CarrierFenceViolation {
        UnshareableType(String),
        RoutedQualification(String),
    }

    pub(crate) fn first_forbidden_quotient_content(
        program: &dyn crate::CheckingInput,
        proof_only: &ProofOnlyClassification,
        type_reference: TypeReferenceHandle,
        visited_data: &mut HashSet<u32>,
    ) -> Option<CarrierFenceViolation> {
        first_forbidden_content(
            program,
            proof_only,
            type_reference,
            visited_data,
            ContentUse::Quotient,
        )
    }

    pub(crate) fn type_has_forbidden_value_observation_content(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        first_forbidden_content(
            program,
            &crate::proof_only_classification(program),
            type_reference,
            &mut HashSet::new(),
            ContentUse::ValueObservation,
        )
        .is_some()
    }

    pub(crate) fn type_has_forbidden_argument_observation_content(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } => shared_referent_has_forbidden_observation_content(program, *referee),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    matches!(constraint, TypeConstraintNode::Domain(domain)
                        if first_routed_qualification(program, domain, &mut HashSet::new()).is_some())
                })
                || type_has_forbidden_argument_observation_content(program, *base_type)
        }
        _ => type_has_forbidden_value_observation_content(program, type_reference),
    }
    }

    fn shared_referent_has_forbidden_observation_content(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        first_forbidden_content(
            program,
            &crate::proof_only_classification(program),
            type_reference,
            &mut HashSet::new(),
            ContentUse::SharedObservation,
        )
        .is_some()
    }

    #[derive(Clone, Copy)]
    pub(crate) enum ContentUse {
        Quotient,
        ValueObservation,
        SharedObservation,
    }

    fn first_forbidden_content(
        program: &dyn crate::CheckingInput,
        proof_only: &ProofOnlyClassification,
        type_reference: TypeReferenceHandle,
        visited_data: &mut HashSet<u32>,
        purpose: ContentUse,
    ) -> Option<CarrierFenceViolation> {
        if !matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Constrained { .. }
        ) && let Some(storage) = program.layout_storage_arguments(type_reference)
        {
            return first_forbidden_content(
                program,
                proof_only,
                storage.semantic_type,
                visited_data,
                purpose,
            );
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, name } => {
                if program
                    .type_reference_table
                    .primitive_type(type_reference)
                    .is_some()
                {
                    return None;
                }
                if let Some(definition) =
                    crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                {
                    return first_forbidden_data_content(
                        program,
                        proof_only,
                        definition,
                        visited_data,
                        purpose,
                    );
                }
                if matches!(
                    purpose,
                    ContentUse::ValueObservation | ContentUse::SharedObservation
                ) && matches!(
                    program.symbols.builtin_type_atom(*symbol),
                    Some(symbols::BuiltinTypeAtom::Int | symbols::BuiltinTypeAtom::UInt)
                ) {
                    return None;
                }
                let parameter = match purpose {
                    ContentUse::Quotient => program
                        .data_definitions()
                        .iter()
                        .flat_map(|definition| program.data_type_parameters(definition))
                        .find(|parameter| parameter.symbol == *symbol),
                    ContentUse::ValueObservation | ContentUse::SharedObservation => {
                        program.data_type_parameter_by_symbol(*symbol)
                    }
                };
                let Some(parameter) = parameter else {
                    return matches!(
                        purpose,
                        ContentUse::ValueObservation | ContentUse::SharedObservation
                    )
                    .then(|| CarrierFenceViolation::UnshareableType(name.as_str().to_owned()));
                };
                (matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    && (match purpose {
                        ContentUse::Quotient => {
                            parameter.bounds.multiplicity
                                != language_semantics::Multiplicity::Unrestricted
                        }
                        ContentUse::ValueObservation | ContentUse::SharedObservation => {
                            parameter.bounds.multiplicity
                                == language_semantics::Multiplicity::Linear
                        }
                    } || parameter.bounds.carry.is_some()))
                .then(|| CarrierFenceViolation::UnshareableType(name.as_str().to_owned()))
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let definition = crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    *base_symbol,
                );
                let Some(definition) = definition else {
                    return matches!(
                        purpose,
                        ContentUse::ValueObservation | ContentUse::SharedObservation
                    )
                    .then(|| {
                        CarrierFenceViolation::UnshareableType(
                            program.display_type_reference(type_reference),
                        )
                    });
                };
                if matches!(
                    purpose,
                    ContentUse::ValueObservation | ContentUse::SharedObservation
                ) && program.data_type_parameters(definition).len() != arguments.len()
                {
                    return Some(CarrierFenceViolation::UnshareableType(
                        program.display_type_reference(type_reference),
                    ));
                }
                for (parameter, argument) in program.data_type_parameters(definition).iter().zip(
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments),
                ) {
                    if matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                        && let Some(forbidden) = first_forbidden_content(
                            program,
                            proof_only,
                            *argument,
                            visited_data,
                            purpose,
                        )
                    {
                        return Some(forbidden);
                    }
                }
                first_forbidden_data_content(program, proof_only, definition, visited_data, purpose)
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                for constraint in program.type_reference_table.constraints(*constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint
                        && let Some(name) =
                            first_routed_qualification(program, domain, &mut HashSet::new())
                    {
                        return Some(CarrierFenceViolation::RoutedQualification(name));
                    }
                }
                first_forbidden_content(program, proof_only, *base_type, visited_data, purpose)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                first_forbidden_content(program, proof_only, *element_type, visited_data, purpose)
            }
            TypeReferenceNode::Reference {
                referee, access, ..
            } if matches!(purpose, ContentUse::SharedObservation) && access.is_readable() => {
                // The enclosing shared loan permits reads through these fields;
                // their actual observations still retain storage and backing identity.
                first_forbidden_content(program, proof_only, *referee, visited_data, purpose)
            }
            TypeReferenceNode::Slice { element_type }
                if matches!(purpose, ContentUse::SharedObservation) =>
            {
                first_forbidden_content(
                    program,
                    proof_only,
                    *element_type,
                    &mut HashSet::new(),
                    ContentUse::ValueObservation,
                )
            }
            TypeReferenceNode::Reference { .. }
            | TypeReferenceNode::Slice { .. }
            | TypeReferenceNode::Callable { .. }
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::ConstExpression(_) => {
                Some(CarrierFenceViolation::UnshareableType(
                    program.display_type_reference_with_constraints(type_reference),
                ))
            }
            TypeReferenceNode::Unit => None,
        }
    }

    fn first_routed_qualification(
        program: &dyn crate::CheckingInput,
        constraint: &DomainConstraint,
        visited_aliases: &mut HashSet<u32>,
    ) -> Option<String> {
        if !constraint.establishment_routes.is_empty() {
            return Some(constraint.name.as_str().to_owned());
        }
        let definition = constraint
            .symbol
            .is_valid()
            .then(|| program.domain_definition(constraint.symbol));
        match definition {
            Some(Some(definition)) => {
                first_routed_domain_definition(program, definition, visited_aliases)
            }
            _ => None,
        }
    }

    fn first_routed_domain_definition(
        program: &dyn crate::CheckingInput,
        definition: &DomainDefinition,
        visited_aliases: &mut HashSet<u32>,
    ) -> Option<String> {
        if let Some(alias) = definition.alias.as_ref() {
            if !visited_aliases.insert(definition.symbol.arena_index()) {
                return None;
            }
            let routed = first_routed_alias_constituent(program, alias, visited_aliases);
            visited_aliases.remove(&definition.symbol.arena_index());
            return routed;
        }
        (!definition.establishment_routes.is_empty()).then(|| definition.name.as_str().to_owned())
    }

    fn first_routed_alias_constituent(
        program: &dyn crate::CheckingInput,
        alias: &DomainAliasDefinition,
        visited_aliases: &mut HashSet<u32>,
    ) -> Option<String> {
        alias.constituents.iter().find_map(|constituent| {
            program
                .domain_definition(constituent.domain_symbol)
                .and_then(|definition| {
                    first_routed_domain_definition(program, definition, visited_aliases)
                })
        })
    }

    fn first_forbidden_data_content(
        program: &dyn crate::CheckingInput,
        proof_only: &ProofOnlyClassification,
        definition: &typed_trees::data::DataDefinition,
        visited_data: &mut HashSet<u32>,
        purpose: ContentUse,
    ) -> Option<CarrierFenceViolation> {
        if !visited_data.insert(definition.symbol.arena_index()) {
            return None;
        }
        let forbidden = match purpose {
            ContentUse::Quotient => {
                !proof_only.is_proof_only(definition.symbol)
                    && (definition.properties.multiplicity
                        != language_semantics::Multiplicity::Unrestricted
                        || definition.properties.carry.is_some())
            }
            ContentUse::ValueObservation | ContentUse::SharedObservation => {
                definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                    || definition.properties.multiplicity
                        == language_semantics::Multiplicity::Linear
                    || definition.properties.carry.is_some()
            }
        };
        if forbidden {
            return Some(CarrierFenceViolation::UnshareableType(
                definition.name.as_str().to_owned(),
            ));
        }

        for field in program.data_member_fields(program.data_members(definition)) {
            if let Some(forbidden) = first_forbidden_content(
                program,
                proof_only,
                field.type_reference,
                visited_data,
                purpose,
            ) {
                return Some(forbidden);
            }
        }
        None
    }
}

pub mod proposition_context {
    /*
    Logical connective children retain proposition context. Calls and ordinary
    value expressions keep their declared argument roles; comparison spelling
    supplies no separate identity interpretation.
    */

    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
    };
    use typed_trees::types::PrimitiveType;

    /// Logical composition supplies proposition context to its children. Ordinary
    /// expression-call arguments are value terms; nominal proposition applications
    /// have their own declared proof telescope, outside this expression grammar.
    pub fn logical_proposition_children(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> [Option<ExpressionHandle>; 2] {
        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                [Some(unary.operand), None]
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    || (matches!(
                        binary.operator,
                        BinaryOperator::Equal | BinaryOperator::NotEqual
                    ) && [binary.left, binary.right].into_iter().all(|operand| {
                        crate::mathematical_applications::operand_type_reference(program, operand)
                            .and_then(|reference| program.primitive_type_reference(reference))
                            == Some(PrimitiveType::Bool)
                    })) =>
            {
                [Some(binary.left), Some(binary.right)]
            }
            _ => [None, None],
        }
    }
}

pub mod contract_results {
    use crate::frozen_program::expression_table;
    use symbols::SymbolKind;
    use typed_trees::data::TypeParameter;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone)]
    pub struct ReservedResultPlace {
        pub machine_symbol: symbols::SymbolHandle,
        pub runtime_contract: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
        pub root: ExpressionHandle,
        pub type_reference: TypeReferenceHandle,
        pub segments: Vec<facts::PlaceSegment>,
        pub members: Vec<ReservedResultMember>,
    }

    #[derive(Debug, Clone)]
    pub struct ReservedResultMember {
        pub expression: ExpressionHandle,
        pub receiver_type: TypeReferenceHandle,
        pub authored_field: symbols::SymbolHandle,
        pub applied_field: symbols::SymbolHandle,
        pub applied_case: symbols::SymbolHandle,
        pub authored_case: symbols::SymbolHandle,
    }

    pub fn reserved_result_place(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ReservedResultPlace> {
        reserved_result_place_from_source(program, expression, None)
    }

    pub fn runtime_reserved_result_place(
        program: &dyn crate::CheckingInput,
        contract: typed_trees::callable_values::RuntimeCallableContractHandle,
        expression: ExpressionHandle,
    ) -> Option<ReservedResultPlace> {
        if !program.runtime_callable_contracts.is_valid(contract) {
            return None;
        }
        reserved_result_place_from_source(program, expression, Some(contract))
    }

    fn reserved_result_place_from_source(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        runtime_contract: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
    ) -> Option<ReservedResultPlace> {
        let mut root = expression;
        let mut projections = Vec::new();
        loop {
            if !program.expression_table.expression_is_valid(root) || projections.len() >= 128 {
                return None;
            }
            match expression_table(program).expression(root) {
                ExpressionNode::Member(member) => {
                    projections.push(root);
                    root = member.receiver;
                }
                ExpressionNode::Indexed(indexed) => {
                    projections.push(root);
                    root = indexed.collection;
                }
                ExpressionNode::Name(_) => break,
                _ => return None,
            }
        }
        if !expression_table(program).names_reserved_result(root) {
            return None;
        }
        let (machine_symbol, mut type_reference) =
            contract_occurrence_owner_from_source(program, root, root, runtime_contract)?;
        if expression != root
            && let Some(owner) =
                contract_occurrence_owner_from_source(program, root, expression, runtime_contract)
            && owner != (machine_symbol, type_reference)
        {
            return None;
        }
        let mut segments = Vec::with_capacity(projections.len());
        let mut members = Vec::new();
        for projection in projections.into_iter().rev() {
            if let ExpressionNode::Indexed(indexed) =
                expression_table(program).expression(projection)
            {
                let literal = expression_table(program).integer_node(indexed.index)?;
                if literal.landing().is_some_and(|landing| {
                    landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                        || landing.landed_type == numerics::literals::LandedIntegerType::Addr
                }) {
                    return None;
                }
                use language_semantics::declaration_selection::{
                    AuthoredDeclarationSelectionIntrinsic as Intrinsic,
                    AuthoredDeclarationSelectionKind as Kind,
                    AuthoredDeclarationSelectionLateBinding as LateBinding,
                    AuthoredDeclarationSelectionTarget as Target,
                };
                let mut occurrences =
                    expression_table(program).authored_selection_occurrences(projection);
                let occurrence = occurrences.next()?;
                let selection = program.authored_declaration_selections().get(occurrence)?;
                if occurrences.next().is_some()
                    || selection.kind() != Kind::Operator
                    || !matches!(
                        selection.target(),
                        Target::Intrinsic(Intrinsic::BuiltinOperator)
                            | Target::LateBound(LateBinding::CheckedOperator)
                    )
                {
                    return None;
                }
                let index = usize::try_from(literal.value_u64()?).ok()?;
                let parameters = if let Some(contract) = runtime_contract {
                    program.state_signature_parameters(
                        &program.runtime_callable_contracts.get(contract).signature,
                    )
                } else {
                    source_contract_parameters(program, machine_symbol, root)?
                };
                let index_type = crate::parameter_expression_result_type_reference(
                    program,
                    machine_symbol,
                    parameters,
                    indexed.index,
                );
                if !typed_trees::operator::builtin_index_read_is_spelled(
                    program,
                    machine_symbol,
                    projection,
                    &[Some(type_reference), index_type],
                ) {
                    return None;
                }
                let receiver = crate::value_custody::places::unwrapped_type_reference(
                    program,
                    type_reference,
                )?;
                let element_type = match program.type_reference_table.type_reference(receiver) {
                    typed_trees::types::TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(extent),
                    } if index < *extent => *element_type,
                    typed_trees::types::TypeReferenceNode::Slice { element_type } => *element_type,
                    _ => return None,
                };
                if !program
                    .type_reference_table
                    .contains_type_reference(element_type)
                {
                    return None;
                }
                segments.push(facts::PlaceSegment::FixedIndex { index });
                type_reference = element_type;
                continue;
            }
            let ExpressionNode::Member(member) = expression_table(program).expression(projection)
            else {
                return None;
            };
            let receiver =
                crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
            let data = program
                .closed_data_definition(receiver)
                .or_else(|| program.data_application_definition(receiver))?;
            if !data.symbol.is_valid() || program.symbols.get(data.symbol).kind != SymbolKind::Data
            {
                return None;
            }
            let field =
                crate::value_custody::places::applied_data_member_field(program, receiver, member)?;
            let declaration = program.symbols.get(field.symbol);
            if declaration.kind != SymbolKind::Field
                || program.symbols.name(field.symbol) != field.name.as_str()
                || !program
                    .type_reference_table
                    .contains_type_reference(field.type_reference)
            {
                return None;
            }
            let mut applied_case = symbols::SymbolHandle::invalid();
            if declaration.parent != data.symbol {
                let mut cases = program.data_members(data).iter().filter_map(|member| {
                    let typed_trees::data::DataMember::Variant(case) = member else {
                        return None;
                    };
                    (case.symbol == declaration.parent).then_some(case)
                });
                let case = cases.next()?;
                if cases.next().is_some()
                    || program.symbols.get(case.symbol).kind != SymbolKind::Variant
                    || program.symbols.get(case.symbol).parent != data.symbol
                    || !program
                        .data_payload_fields(case)
                        .iter()
                        .any(|candidate| candidate.symbol == field.symbol)
                {
                    return None;
                }
                segments.push(facts::PlaceSegment::Case {
                    variant: case.symbol,
                });
                applied_case = case.symbol;
            }
            let authored_field =
                crate::selection_resolution::authored_data_member(program, field.symbol);
            if program
                .data_constructor_field(receiver, data.symbol, authored_field)?
                .symbol
                != field.symbol
            {
                return None;
            }
            members.push(ReservedResultMember {
                expression: projection,
                receiver_type: receiver,
                authored_field,
                applied_field: field.symbol,
                applied_case,
                authored_case: {
                    let parent = program.symbols.get(authored_field).parent;
                    if program.symbols.get(parent).kind == SymbolKind::Variant {
                        parent
                    } else {
                        symbols::SymbolHandle::invalid()
                    }
                },
            });
            segments.push(facts::PlaceSegment::Field {
                symbol: field.symbol,
            });
            type_reference = field.type_reference;
        }
        Some(ReservedResultPlace {
            machine_symbol,
            runtime_contract,
            root,
            type_reference,
            segments,
            members,
        })
    }

    pub(crate) fn type_reference(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        reserved_result_owner(program, expression).map(|(_, type_reference)| type_reference)
    }

    pub fn reserved_result_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
        if !expression_table(program).names_reserved_result(expression) {
            return None;
        }

        contract_occurrence_owner(program, expression, expression)
    }

    pub(crate) fn expression_uses_reserved_result(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::<_, symbols::BuildSymbolHasher>::default();
        crate::queries::expression_traversal::collect_expression_nodes(
            program, expression, &mut nodes, &mut seen,
        );
        nodes
            .into_iter()
            .any(|node| reserved_result_owner(program, node).is_some())
    }

    pub(crate) fn expression_has_source_builtin_index_meaning(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::<_, symbols::BuildSymbolHasher>::default();
        crate::queries::expression_traversal::collect_expression_nodes(
            program, expression, &mut nodes, &mut seen,
        );
        nodes.into_iter().all(|node| {
            let Some(indexed) = expression_table(program).indexed_node(node) else {
                return true;
            };
            if expression_table(program).indexed_on_range(indexed) {
                return true;
            }
            let Ok(owner) = source_contract_declaration_owner(program, node) else {
                return false;
            };
            let Some(parameters) = source_contract_parameters(program, owner, node) else {
                return false;
            };
            let operands = [indexed.collection, indexed.index].map(|operand| {
                crate::parameter_expression_result_type_reference(
                    program, owner, parameters, operand,
                )
            });
            operands[0].is_some()
                && typed_trees::operator::builtin_index_read_is_spelled(
                    program, owner, node, &operands,
                )
        })
    }

    pub(crate) fn fact_has_source_builtin_index_meaning(
        program: &dyn crate::CheckingInput,
        fact: &ProofFact,
    ) -> bool {
        match fact {
            ProofFact::Expression(expression) => {
                expression_has_source_builtin_index_meaning(program, *expression)
            }
            ProofFact::Membership(membership) => {
                let mut expressions = Vec::new();
                let mut visited = Vec::new();
                for argument in program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                {
                    crate::queries::expression_traversal::collect_type_expressions(
                        program,
                        *argument,
                        &mut visited,
                        &mut expressions,
                    );
                }
                expression_has_source_builtin_index_meaning(program, membership.value)
                    && expressions.into_iter().all(|expression| {
                        expression_has_source_builtin_index_meaning(program, expression)
                    })
            }
            ProofFact::Proposition(application) => {
                proposition_has_source_builtin_index_meaning(program, application, &mut Vec::new())
            }
        }
    }

    fn proposition_has_source_builtin_index_meaning(
        program: &dyn crate::CheckingInput,
        application: &typed_trees::proposition::PropositionApplication,
        expanding: &mut Vec<symbols::SymbolHandle>,
    ) -> bool {
        use typed_trees::proposition::{PropositionBody, PropositionFormula};

        if !expression_table(program)
            .expression_handles(application.arguments)
            .iter()
            .all(|argument| expression_has_source_builtin_index_meaning(program, *argument))
        {
            return false;
        }
        let Some(declaration) = program.proposition_definition_by_symbol(application.proposition)
        else {
            return true;
        };
        let PropositionBody::Transparent { proposition } = &declaration.body else {
            return true;
        };
        if expanding.len() >= 128 || expanding.contains(&declaration.symbol) {
            return false;
        }
        expanding.push(declaration.symbol);
        let supported = match proposition {
            PropositionFormula::Application(nested) => {
                proposition_has_source_builtin_index_meaning(program, nested, expanding)
            }
            PropositionFormula::BooleanExpression(expression) => {
                expression_has_source_builtin_index_meaning(program, *expression)
            }
        };
        expanding.pop();
        supported
    }

    pub(crate) fn source_contract_declaration_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Result<symbols::SymbolHandle, ()> {
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            return memos
                .contract_occurrences
                .get_or_init(|| std::sync::Arc::new(ContractOccurrenceIndex::of(program)))
                .declaration_owner(expression);
        }
        ContractOccurrenceIndex::of(program).declaration_owner(expression)
    }

    fn contract_occurrence_owner(
        program: &dyn crate::CheckingInput,
        root: ExpressionHandle,
        expression: ExpressionHandle,
    ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
        contract_occurrence_owner_from_source(program, root, expression, None)
    }

    fn contract_occurrence_owner_from_source(
        program: &dyn crate::CheckingInput,
        root: ExpressionHandle,
        expression: ExpressionHandle,
        runtime_contract: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
    ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            return memos
                .contract_occurrences
                .get_or_init(|| std::sync::Arc::new(ContractOccurrenceIndex::of(program)))
                .owner(root, expression, runtime_contract);
        }
        ContractOccurrenceIndex::of(program).owner(root, expression, runtime_contract)
    }

    pub(crate) fn source_contract_parameters(
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        expression: ExpressionHandle,
    ) -> Option<&[StateParameter]> {
        let parameters = if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            memos
                .contract_occurrences
                .get_or_init(|| std::sync::Arc::new(ContractOccurrenceIndex::of(program)))
                .parameters(owner, expression)?
        } else {
            ContractOccurrenceIndex::of(program).parameters(owner, expression)?
        };
        Some(program.state_parameters.span_or_empty(parameters))
    }

    /// Preserve the supplying scope alongside each exact copied telescope.
    pub(crate) fn retained_callable_contracts(
        program: &dyn crate::CheckingInput,
    ) -> impl Iterator<Item = typed_trees::typed_trees::CallableContractScope<'_>> {
        fn unscoped(
            signature: &typed_trees::signature::StateSignature,
        ) -> typed_trees::typed_trees::CallableContractScope<'_> {
            typed_trees::typed_trees::CallableContractScope {
                signature,
                trait_scope: None,
            }
        }
        program
            .traits()
            .iter()
            .flat_map(move |definition| {
                program.trait_machine_signatures(definition).iter().chain(
                    definition
                        .refinement_clauses
                        .iter()
                        .map(|clause| &clause.signature),
                )
            })
            .map(unscoped)
            .chain(program.prepared_callable_contracts())
            .chain(
                machine_parameter_contract_signatures(program)
                    .into_iter()
                    .map(|(_, signature)| unscoped(signature)),
            )
    }

    /// Every callable contract in the order owner resolution visits them:
    /// machines with an entry state, operators, then authored and applied requirement
    /// signatures and reachable machine-parameter contracts.
    /// Operators own their result guarantees even when their boundary body is
    /// absent; ignoring them leaves the result projection without an owner. A trait
    /// symbol cannot distinguish sibling requirements, and manufacturing a machine
    /// for a bodyless signature would give it the wrong result owner.
    pub fn callable_contract_scopes(
        program: &dyn crate::CheckingInput,
    ) -> impl Iterator<
        Item = (
            symbols::SymbolHandle,
            TypeReferenceHandle,
            arena::HandleSpan<StateParameter>,
            Option<typed_trees::trait_definition::RetainedTraitScope>,
            &SignatureContract,
        ),
    > + '_ {
        let machines = program.machines().iter().filter_map(|machine| {
            let entry = program.machine_states(machine).first()?;
            Some((
                machine.symbol,
                entry.return_type,
                entry.parameters,
                None,
                program.machine_contracts(machine),
            ))
        });
        let requirements = retained_callable_contracts(program).map(|scope| {
            let signature = scope.signature;
            (
                signature.symbol,
                signature.return_type,
                signature.parameters,
                scope.trait_scope,
                program.state_signature_contracts(signature),
            )
        });
        let operators = program.operators().iter().map(|operator| {
            (
                operator.symbol,
                operator.return_type,
                operator.parameters,
                None,
                program.operator_contracts(operator),
            )
        });
        machines.chain(operators).chain(requirements).flat_map(
            |(callable, return_type, parameters, trait_scope, contracts)| {
                contracts
                    .iter()
                    .map(move |contract| (callable, return_type, parameters, trait_scope, contract))
            },
        )
    }

    pub(crate) fn machine_parameter_contract_signatures(
        program: &dyn crate::CheckingInput,
    ) -> Vec<(&TypeParameter, &typed_trees::signature::StateSignature)> {
        let mut parameters = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_type_parameters(machine))
            .chain(
                program
                    .data_definitions()
                    .iter()
                    .flat_map(|data| program.data_type_parameters(data)),
            )
            .chain(
                program
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| program.domain_type_parameters(domain)),
            )
            .chain(
                program
                    .conformances()
                    .iter()
                    .flat_map(|conformance| program.conformance_type_parameters(conformance)),
            )
            .chain(
                program
                    .traits()
                    .iter()
                    .flat_map(|definition| program.trait_type_parameters(definition)),
            )
            .chain(
                program
                    .all_operators()
                    .flat_map(|operator| program.operator_type_parameters(operator)),
            )
            .collect::<Vec<_>>();
        for signature in program
            .traits()
            .iter()
            .flat_map(|definition| {
                program.trait_machine_signatures(definition).iter().chain(
                    definition
                        .refinement_clauses
                        .iter()
                        .map(|clause| &clause.signature),
                )
            })
            .chain(program.prepared_callable_signatures())
        {
            parameters.extend(program.state_signature_type_parameters(signature));
        }
        let mut signatures =
            Vec::<(&TypeParameter, &typed_trees::signature::StateSignature)>::new();
        while let Some(parameter) = parameters.pop() {
            let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
                continue;
            };
            let Some(view) = program.machine_parameter_contract_view(contract) else {
                continue;
            };
            let signature = view.signature();
            if signatures.iter().any(|(_, previous)| {
                previous.symbol == signature.symbol
                    && previous.parameters == signature.parameters
                    && previous.contracts == signature.contracts
            }) {
                continue;
            }
            parameters.extend(program.state_signature_type_parameters(signature));
            signatures.push((parameter, signature));
        }
        signatures
    }

    pub(crate) type ContractNodeSet =
        std::collections::HashSet<ExpressionHandle, symbols::BuildSymbolHasher>;

    fn contract_nodes(
        program: &dyn crate::CheckingInput,
        contract: &SignatureContract,
        nodes: &mut Vec<ExpressionHandle>,
        seen: &mut ContractNodeSet,
    ) {
        nodes.clear();
        seen.clear();
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(root) => {
                    crate::queries::expression_traversal::collect_expression_nodes(
                        program, *root, nodes, seen,
                    );
                }
                ProofFact::Membership(membership) => {
                    crate::queries::expression_traversal::collect_expression_nodes(
                        program,
                        membership.value,
                        nodes,
                        seen,
                    );
                }
                ProofFact::Proposition(application) => {
                    for argument in
                        expression_table(program).expression_handles(application.arguments)
                    {
                        crate::queries::expression_traversal::collect_expression_nodes(
                            program, *argument, nodes, seen,
                        );
                    }
                }
            }
        }
    }

    fn owns_result(
        program: &dyn crate::CheckingInput,
        callable: symbols::SymbolHandle,
        return_type: TypeReferenceHandle,
        parameters: &[StateParameter],
        contract: &SignatureContract,
    ) -> bool {
        matches!(
            contract.kind,
            SignatureContractKind::Ensures | SignatureContractKind::EnsuresForResultCase { .. }
        ) && callable.is_valid()
            && program
                .type_reference_table
                .contains_type_reference(return_type)
            && language_core::signature_reserves_result(
                parameters.iter().map(|parameter| parameter.name.as_str()),
            )
    }

    pub(crate) struct ContractOccurrenceIndex {
        contracts: Vec<(
            symbols::SymbolHandle,
            TypeReferenceHandle,
            bool,
            arena::HandleSpan<StateParameter>,
            Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
        )>,
        containing: symbols::SymbolKeyMap<ExpressionHandle, Vec<u32>>,
    }

    impl ContractOccurrenceIndex {
        fn parameters(
            &self,
            owner: symbols::SymbolHandle,
            expression: ExpressionHandle,
        ) -> Option<arena::HandleSpan<StateParameter>> {
            let mut parameters = None;
            let mut previous_source = None;
            for position in self.containing.get(&expression)? {
                let (callable, _, _, candidate, source) = self.contracts[*position as usize];
                if callable != owner
                    || parameters.is_some_and(|prior| prior != candidate)
                    || previous_source.is_some_and(|previous| previous != source)
                {
                    return None;
                }
                parameters = Some(candidate);
                previous_source = Some(source);
            }
            parameters
        }

        fn of(program: &dyn crate::CheckingInput) -> Self {
            let mut contracts = Vec::new();
            let mut containing = symbols::SymbolKeyMap::<ExpressionHandle, Vec<u32>>::default();
            let (mut nodes, mut seen) = (Vec::new(), ContractNodeSet::default());
            let mut retain =
                |callable, return_type, parameters, contract: &SignatureContract, source| {
                    let position = u32::try_from(contracts.len())
                        .expect("callable contract position overflow");
                    contract_nodes(program, contract, &mut nodes, &mut seen);
                    for node in nodes.iter().copied() {
                        containing.entry(node).or_default().push(position);
                    }
                    contracts.push((
                        callable,
                        return_type,
                        owns_result(
                            program,
                            callable,
                            return_type,
                            program.state_parameters.span_or_empty(parameters),
                            contract,
                        ),
                        parameters,
                        source,
                    ));
                };
            for (callable, return_type, parameters, _, contract) in
                callable_contract_scopes(program)
            {
                retain(callable, return_type, parameters, contract, None);
            }
            for (source, row) in program.runtime_callable_contracts.iter() {
                for contract in program.state_signature_contracts(&row.signature) {
                    retain(
                        row.signature.symbol,
                        row.signature.return_type,
                        row.signature.parameters,
                        contract,
                        Some(source),
                    );
                }
            }
            Self {
                contracts,
                containing,
            }
        }

        fn declaration_owner(
            &self,
            expression: ExpressionHandle,
        ) -> Result<symbols::SymbolHandle, ()> {
            let mut owner = symbols::SymbolHandle::default();
            let mut previous_source = None;
            for position in self.containing.get(&expression).into_iter().flatten() {
                let (callable, _, _, _, source) = self.contracts[*position as usize];
                if !callable.is_valid()
                    || (owner.is_valid() && owner != callable)
                    || previous_source.is_some_and(|previous| previous != source)
                {
                    return Err(());
                }
                owner = callable;
                previous_source = Some(source);
            }
            Ok(owner)
        }

        fn owner(
            &self,
            root: ExpressionHandle,
            expression: ExpressionHandle,
            expected_source: Option<typed_trees::callable_values::RuntimeCallableContractHandle>,
        ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
            let with_expression = self.containing.get(&expression)?;
            let with_root = self.containing.get(&root)?;
            let mut owner = None;
            let mut previous_source = None;
            for position in with_expression
                .iter()
                .filter(|position| with_root.binary_search(position).is_ok())
            {
                let (callable, return_type, owns_result, _, source) =
                    self.contracts[*position as usize];
                if expected_source.is_some() && source != expected_source {
                    continue;
                }
                if !owns_result {
                    return None;
                }
                if owner.is_some_and(|candidate| candidate != (callable, return_type))
                    || previous_source.is_some_and(|previous| previous != source)
                {
                    return None;
                }
                owner = Some((callable, return_type));
                previous_source = Some(source);
            }
            owner
        }
    }
}

pub mod contract_structure {
    use checked_trees::{
        DeclaredCallableContract, DeclaredContractClause, DeclaredContractFact,
        DeclaredProgressPremise, DeclaredTermination,
    };
    use symbols::SymbolHandle;
    use typed_trees::signature::{SignatureContract, StateParameter};
    use typed_trees::type_identity::{TypeIdentityRequest, normalized_expression_identity};

    pub(crate) mod correspondence {
        use crate::frozen_program::expression_table;
        use checked_trees::DeclaredContractFact;
        use symbols::SymbolHandle;
        use typed_trees::domain::ProofFact;
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::proposition::{
            PropositionApplication, PropositionBody, PropositionFormula,
        };
        use typed_trees::type_identity::{
            AppliedTypeIdentityScope, IndexIdentityArgument, try_normalized_index_identity,
        };

        #[derive(PartialEq, Eq)]
        pub(crate) enum CorrespondenceFact {
            Structural(DeclaredContractFact),
            Proposition {
                declaration: PropositionIdentity,
                binders: Vec<String>,
                arguments: Vec<String>,
            },
        }

        #[derive(PartialEq, Eq)]
        pub(crate) enum PropositionIdentity {
            Declaration(SymbolHandle),
            Binder(String),
        }

        pub(crate) fn correspondence_fact(
            program: &dyn crate::CheckingInput,
            fact: &ProofFact,
            binders: &[(SymbolHandle, String)],
        ) -> Option<CorrespondenceFact> {
            if !crate::proof_contracts::contract_results::fact_has_source_builtin_index_meaning(
                program, fact,
            ) {
                return None;
            }
            match fact {
                ProofFact::Proposition(application) => {
                    proposition_fact(program, application, binders, &[])
                }
                _ => super::try_declared_fact(program, fact, binders, &[])
                    .map(CorrespondenceFact::Structural),
            }
        }

        fn proposition_fact(
            program: &dyn crate::CheckingInput,
            application: &PropositionApplication,
            binders: &[(SymbolHandle, String)],
            scopes: &[AppliedTypeIdentityScope<'_>],
        ) -> Option<CorrespondenceFact> {
            if !expression_table(program)
        .expression_handles(application.arguments)
        .iter()
        .all(|argument| {
            crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(
                program, *argument,
            )
        })
            {
        return None;
            }
            if !application.proposition.is_valid()
                || scopes
                    .iter()
                    .any(|scope| scope.declaration == application.proposition)
            {
                return None;
            }
            let operands = application
                .binder_arguments
                .iter()
                .map(|argument| StaticMachineArgument {
                    symbol: argument.symbol,
                    path: argument.path.clone(),
                    const_literal: argument.const_literal.clone(),
                    evidence_projection: argument.evidence_projection.clone(),
                    ..StaticMachineArgument::default()
                })
                .collect::<Vec<_>>();
            let arguments = expression_table(program).expression_handles(application.arguments);
            let declaration = program.proposition_definition_by_symbol(application.proposition);
            if let Some(declaration) = declaration {
                let parameters = program.proposition_parameters(declaration);
                let static_parameters = program.proposition_binders(declaration);
                if parameters.len() != arguments.len() || static_parameters.len() != operands.len()
                {
                    return None;
                }
                if let PropositionBody::Transparent { proposition } = &declaration.body {
                    let replacements = static_parameters
                        .iter()
                        .zip(&operands)
                        .map(|(parameter, operand)| {
                            (
                                parameter.symbol,
                                IndexIdentityArgument::StaticArgument(operand),
                            )
                        })
                        .chain(
                            parameters
                                .iter()
                                .zip(arguments)
                                .map(|(parameter, argument)| {
                                    (
                                        parameter.symbol,
                                        IndexIdentityArgument::Expression(*argument),
                                    )
                                }),
                        )
                        .collect::<Vec<_>>();
                    let mut applied = scopes.to_vec();
                    applied.push(AppliedTypeIdentityScope {
                        declaration: declaration.symbol,
                        arguments: &replacements,
                        lifetimes: &[],
                    });
                    return match proposition {
                        PropositionFormula::Application(expansion) => {
                            proposition_fact(program, expansion, binders, &applied)
                        }
                        PropositionFormula::BooleanExpression(expression) => {
                            if !crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(program, *expression) {
                        return None;
                    }
                            let identity = try_normalized_index_identity(
                                program.typed_program(),
                                IndexIdentityArgument::Expression(*expression),
                                binders,
                                &applied,
                            )?;
                            Some(CorrespondenceFact::Structural(
                                DeclaredContractFact::Expression(identity),
                            ))
                        }
                    };
                }
            } else if program.symbols.get(application.proposition).kind
                != symbols::SymbolKind::PropositionParameter
            {
                return None;
            }
            let declaration = binders
                .iter()
                .find(|(symbol, _)| *symbol == application.proposition)
                .map_or(
                    PropositionIdentity::Declaration(application.proposition),
                    |(_, position)| PropositionIdentity::Binder(position.clone()),
                );
            Some(CorrespondenceFact::Proposition {
                declaration,
                binders: operands
                    .iter()
                    .map(|operand| {
                        try_normalized_index_identity(
                            program.typed_program(),
                            IndexIdentityArgument::StaticArgument(operand),
                            binders,
                            scopes,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        try_normalized_index_identity(
                            program.typed_program(),
                            IndexIdentityArgument::Expression(*argument),
                            binders,
                            scopes,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            })
        }
    }
    pub(crate) use correspondence::{CorrespondenceFact, correspondence_fact};

    pub use typed_trees::contract_structure::{callable_binders, generic_binders};

    pub fn signature_generic_binders(
        program: &dyn crate::CheckingInput,
        signature: &typed_trees::signature::StateSignature,
    ) -> Vec<(SymbolHandle, String)> {
        let mut scopes = Vec::new();
        let mut scope = signature.symbol;
        while scope.is_valid() && !scopes.contains(&scope) {
            scopes.push(scope);
            scope = program.symbols.get(scope).parent;
        }
        let mut binders = Vec::<(SymbolHandle, String)>::new();
        for scope in scopes.into_iter().rev() {
            for (_, parameter) in program.data_type_parameters.iter() {
                if parameter.symbol.is_valid()
                    && program.symbols.get(parameter.symbol).parent == scope
                    && !binders
                        .iter()
                        .any(|(symbol, _)| *symbol == parameter.symbol)
                {
                    let position = binders.len();
                    binders.push((parameter.symbol, format!("$G{position}")));
                }
            }
        }
        for parameter in program.state_signature_type_parameters(signature) {
            if !binders
                .iter()
                .any(|(symbol, _)| *symbol == parameter.symbol)
            {
                let position = binders.len();
                binders.push((parameter.symbol, format!("$G{position}")));
            }
        }
        binders
    }

    pub fn declared_callable(
        program: &dyn crate::CheckingInput,
        parameters: &[StateParameter],
        result: typed_trees::types::TypeReferenceHandle,
        contracts: &[SignatureContract],
        generics: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        include_crashes: bool,
    ) -> Result<DeclaredCallableContract, diagnostics::Diagnostic> {
        let binders = callable_binders(generics, parameters);
        let mut projection = typed_trees::contract_structure::ContractProjection {
            type_identity: |reference, binders: &[(SymbolHandle, String)]| {
                Ok(program.type_identity(TypeIdentityRequest {
                    binders,
                    ..TypeIdentityRequest::ordinary(reference)
                }))
            },
            fact_identity: |fact: &typed_trees::domain::ProofFact,
                            binders: &[(SymbolHandle, String)]| {
                declared_fact(program, fact, binders, content_conservation)
            },
            symbol_identity: |symbol, binders: &[(SymbolHandle, String)]| {
                symbol_identity(program, symbol, binders)
            },
            crash_cause: checked_trees::CrashCause::from,
        };
        typed_trees::contract_structure::declared_callable_with(
            program.typed_program(),
            parameters,
            result,
            contracts,
            &binders,
            include_crashes,
            &mut projection,
        )
    }

    pub fn declared_contracts(
        program: &dyn crate::CheckingInput,
        contracts: &[SignatureContract],
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        include_crashes: bool,
    ) -> Result<Vec<DeclaredContractClause>, diagnostics::Diagnostic> {
        let mut projection = typed_trees::contract_structure::ContractProjection {
            type_identity: (),
            fact_identity: |fact: &typed_trees::domain::ProofFact,
                            binders: &[(SymbolHandle, String)]| {
                declared_fact(program, fact, binders, content_conservation)
            },
            symbol_identity: |symbol, binders: &[(SymbolHandle, String)]| {
                symbol_identity(program, symbol, binders)
            },
            crash_cause: checked_trees::CrashCause::from,
        };
        typed_trees::contract_structure::declared_contracts_with(
            program.typed_program(),
            contracts,
            binders,
            include_crashes,
            &mut projection,
        )
    }

    pub fn declared_fact(
        program: &dyn crate::CheckingInput,
        fact: &typed_trees::domain::ProofFact,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
    ) -> Result<DeclaredContractFact, diagnostics::Diagnostic> {
        project_declared_fact(program, fact, binders, content_conservation, |expression| {
        normalized_expression_identity(program, expression, binders)
    })
    .ok_or_else(|| diagnostics::Diagnostic::error("declared contract fact has no complete normalized identity; an unbound subject or unresolved proposition cannot define a contract"))
    }

    fn try_declared_fact(
        program: &dyn crate::CheckingInput,
        fact: &typed_trees::domain::ProofFact,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
    ) -> Option<DeclaredContractFact> {
        project_declared_fact(program, fact, binders, content_conservation, |expression| {
            if !crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(
            program, expression,
        ) {
            return None;
        }
            typed_trees::type_identity::try_normalized_index_identity(
                program.typed_program(),
                typed_trees::type_identity::IndexIdentityArgument::Expression(expression),
                binders,
                &[],
            )
        })
    }

    fn project_declared_fact(
        program: &dyn crate::CheckingInput,
        fact: &typed_trees::domain::ProofFact,
        binders: &[(SymbolHandle, String)],
        content_conservation: &[crate::ContentConservationSourcePlan],
        expression_identity: impl Fn(typed_trees::expression::ExpressionHandle) -> Option<String>,
    ) -> Option<DeclaredContractFact> {
        typed_trees::contract_structure::project_declared_fact_with(
            program.typed_program(),
            fact,
            expression_identity,
            |reference| {
                Some(program.type_identity(TypeIdentityRequest {
                    binders,
                    ..TypeIdentityRequest::ordinary(reference)
                }))
            },
            |symbol| symbol_identity(program, symbol, binders),
            |expression| {
                content_conservation
                    .iter()
                    .find(|candidate| candidate.source_expression == expression)
                    .map(|conservation| {
                        language_semantics::content::content_conservation_plan_bytes(
                            &conservation.plan,
                        )
                    })
            },
        )
    }

    fn symbol_identity(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
        binders: &[(SymbolHandle, String)],
    ) -> String {
        if let Some((_, identity)) = binders.iter().find(|(binder, _)| *binder == symbol) {
            return identity.clone();
        }
        program.symbols.display_path(symbol, "::")
    }

    pub fn declared_termination(
        program: &dyn crate::CheckingInput,
        termination: &language_semantics::TerminationInterface,
        binders: &[(SymbolHandle, String)],
    ) -> DeclaredTermination {
        use language_semantics::{TerminationGuarantee, TerminationInterface};
        match termination {
            TerminationInterface::InternalDerived => DeclaredTermination::InternalDerived,
            TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => {
                DeclaredTermination::NoGuarantee
            }
            TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
                let mut premises = premises
                    .iter()
                    .map(|premise| DeclaredProgressPremise {
                        profile: program
                            .semantic_domains
                            .name(premise.profile)
                            .expect("checked progress premise must retain its semantic domain")
                            .to_owned(),
                        subject: symbol_identity(program, premise.subject.root, binders),
                        projections: premise
                            .subject
                            .projections
                            .iter()
                            .map(|projection| symbol_identity(program, *projection, &[]))
                            .collect(),
                    })
                    .collect::<Vec<_>>();
                premises.sort();
                premises.dedup();
                DeclaredTermination::Terminates(premises)
            }
        }
    }

    pub fn is_true_crash_route(
        program: &dyn crate::CheckingInput,
        fact: &typed_trees::domain::ProofFact,
    ) -> bool {
        typed_trees::contract_structure::is_true_crash_route(program.typed_program(), fact)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn public_contract_hash_uses_owned_type_content_across_mint_orders() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::types::TypeReferenceNode;
            let mut symbols = SymbolTableBuilder::new();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let children = symbols.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("Alpha")),
                    (SymbolKind::Data, SymbolNameRef::Static("Beta")),
                ],
            );
            let mut children = SymbolTableBuilder::child_handles(children);
            let alpha = children.next().unwrap();
            let beta = children.next().unwrap();
            let mut first = typed_trees::TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let alpha_type = first.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: alpha,
                name: "Alpha".into(),
            });
            let beta_type = first.type_reference_table.insert(TypeReferenceNode::Named {
                symbol: beta,
                name: "Beta".into(),
            });
            let second = first.clone();
            let different = first.clone();
            let first_alpha = first.type_identity(TypeIdentityRequest::ordinary(alpha_type));
            second.type_identity(TypeIdentityRequest::ordinary(beta_type));
            let second_alpha = second.type_identity(TypeIdentityRequest::ordinary(alpha_type));
            let different_beta = different.type_identity(TypeIdentityRequest::ordinary(beta_type));
            assert_ne!(first_alpha, second_alpha);
            assert_eq!(first_alpha, different_beta);
            let contract = |program: &typed_trees::TypedTrees, value| {
                let parameters = [StateParameter {
                    type_reference: value,
                    ..Default::default()
                }];
                let declaration = checked_trees::DeclaredContractStructure {
                    callables: vec![
                        declared_callable(program, &parameters, value, &[], &[], &[], true)
                            .expect("formed callable contract"),
                    ],
                    clauses: Vec::new(),
                };
                checked_trees::contract_identity(
                    language_semantics::MachineSupplyMode::CheckedBody,
                    None,
                    &[],
                    language_semantics::SynchronousInvocationInterface::PublishedCeiling,
                    &[],
                    language_semantics::SuspensionInterface::PublishedMaySuspend(false),
                    language_semantics::BlockingInterface::PublishedMayBlock(false),
                    checked_trees::CrashInterface::PublishedCeiling,
                    &checked_trees::DeclaredTermination::NoGuarantee,
                    &declaration,
                    &program.type_identities,
                )
                .commitment
            };
            assert_eq!(contract(&first, alpha_type), contract(&second, alpha_type));
            assert_ne!(
                contract(&first, alpha_type),
                contract(&different, beta_type)
            );
        }

        #[test]
        fn nominal_type_spelling_cannot_be_replaced_by_a_generic_binder() {
            use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
            use typed_trees::types::TypeReferenceNode;

            let mut symbols = SymbolTableBuilder::new();
            let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = symbols.insert_children(
                root,
                [
                    (SymbolKind::Data, SymbolNameRef::Static("T")),
                    (SymbolKind::Machine, SymbolNameRef::Static("convert")),
                ],
            );
            let mut declarations = SymbolTableBuilder::child_handles(declarations);
            let nominal = declarations.next().unwrap();
            let machine = declarations.next().unwrap();
            let parameters = symbols.insert_children(
                machine,
                [
                    (SymbolKind::TypeParameter, SymbolNameRef::Static("T")),
                    (SymbolKind::Parameter, SymbolNameRef::Static("value")),
                ],
            );
            let mut parameters = SymbolTableBuilder::child_handles(parameters);
            let generic = parameters.next().unwrap();
            let formal = parameters.next().unwrap();
            let mut program = typed_trees::TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let nominal_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: nominal,
                    name: "T".into(),
                });
            let generic_type = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: generic,
                    name: "T".into(),
                });
            let mut parameters = [StateParameter {
                symbol: formal,
                name: "value".into(),
                type_reference: nominal_type,
                ..Default::default()
            }];
            let binders = [(generic, "$G0".to_owned())];
            let original = declared_callable(
                &program,
                &parameters,
                generic_type,
                &[],
                &binders,
                &[],
                true,
            )
            .expect("formed original callable contract");
            program.symbols.rename(generic, "Element");
            program.symbols.rename(formal, "argument");
            parameters[0].name = "argument".into();
            let renamed = declared_callable(
                &program,
                &parameters,
                generic_type,
                &[],
                &binders,
                &[],
                true,
            )
            .expect("formed renamed callable contract");
            assert_eq!(original, renamed);
            assert_ne!(original.parameters[0].type_identity, original.result);
            parameters[0].type_reference = generic_type;
            assert_ne!(
                original,
                declared_callable(
                    &program,
                    &parameters,
                    generic_type,
                    &[],
                    &binders,
                    &[],
                    true
                )
                .expect("formed generic callable contract")
            );
        }
    }
}

pub mod domain_weakening {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use language_semantics::const_value::CanonicalConstValue;
    use numerics::arithmetic::ArithmeticDomain;
    use symbols::SymbolHandle;
    use typed_trees::expression::TableAtomicExpression;
    use typed_trees::expression::TableCastExpression;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::signature::{SignatureContractKind, StateParameter};
    use typed_trees::state::State;
    use typed_trees::types::DomainConstraint;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone)]
    pub(crate) enum DomainAtom {
        Declared {
            family: SymbolHandle,
            instance: language_semantics::SemanticDomainId,
            indexed: bool,
            label: String,
        },
        Arithmetic(ArithmeticDomain),
    }

    impl PartialEq for DomainAtom {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Arithmetic(left), Self::Arithmetic(right)) => left == right,
                (
                    Self::Declared {
                        family: left_family,
                        instance: left_instance,
                        indexed: left_indexed,
                        ..
                    },
                    Self::Declared {
                        family: right_family,
                        instance: right_instance,
                        indexed: right_indexed,
                        ..
                    },
                ) => {
                    left_family == right_family
                        && left_instance == right_instance
                        && left_indexed == right_indexed
                }
                _ => false,
            }
        }
    }

    impl Eq for DomainAtom {}

    pub(crate) fn validate_implicit_domain_weakening(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        owner: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_argument_domain_weakening(
            program,
            machine,
            state,
            value,
            target_type,
            owner,
            false,
            None,
            diagnostics,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_argument_domain_weakening(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        owner: &str,
        retains_float_result_policy: bool,
        callee_parameter: Option<&StateParameter>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let obligations = callee_parameter
            .map(|parameter| parameter_premise_atoms(program, parameter))
            .unwrap_or_default();
        validate_domain_delivery(
            program,
            machine,
            state,
            value,
            target_type,
            owner,
            retains_float_result_policy,
            &obligations,
            diagnostics,
        );
    }

    pub(crate) fn validate_return_domain_weakening(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: &State,
        value: ExpressionHandle,
        owner: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut obligations = Vec::new();
        append_callable_ensured_domain_atoms(program, state.symbol, &mut obligations);
        validate_domain_delivery(
            program,
            machine,
            Some(state),
            value,
            state.return_type,
            owner,
            false,
            &obligations,
            diagnostics,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_domain_delivery(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        value: ExpressionHandle,
        target_type: TypeReferenceHandle,
        owner: &str,
        retains_float_result_policy: bool,
        obligations: &[DomainAtom],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !value.is_valid()
        || !target_type.is_valid()
        || crate::proof_contracts::arithmetic_domains::expression_reads_unresolved_local_initializer(
            program, machine, state, value,
        )
    {
        return;
    }
        if let Some(state) = state {
            crate::value_custody::cleanup::validate_cleanup_delivery(
                program,
                machine,
                state,
                value,
                target_type,
                owner,
                diagnostics,
            );
        }
        let source = expression_atoms(program, machine, state, value);
        if source.is_empty() {
            return;
        }
        let mut target = Vec::new();
        append_type_atoms(program, target_type, &mut target, &mut Vec::new());
        let integer_delivery = program
            .primitive_type_reference(target_type)
            .and_then(crate::proof_contracts::arithmetic_domains::integer_bit_width)
            .is_some()
            && state
                .and_then(|state| {
                    crate::value_custody::expression_types::expression_result_type_reference(
                        program, machine, state, value,
                    )
                })
                .and_then(|reference| program.primitive_type_reference(reference))
                .and_then(crate::proof_contracts::arithmetic_domains::integer_bit_width)
                .is_some();
        let dropped = source
            .iter()
            .filter(|atom| !(integer_delivery && matches!(atom, DomainAtom::Arithmetic(_))))
            .filter(|atom| {
                !(retains_float_result_policy && matches!(atom, DomainAtom::Arithmetic(_)))
            })
            .filter(|atom| atom_requires_explicit_removal(program, atom))
            .filter(|atom| {
                !obligations
                    .iter()
                    .any(|premise| atoms_equivalent(program, atom, premise))
            })
            .filter(|atom| {
                !target
                    .iter()
                    .any(|candidate| is_deferred_index_compatibility(atom, candidate))
            })
            .filter(|atom| {
                !target
                    .iter()
                    .any(|candidate| atoms_equivalent(program, atom, candidate))
            })
            .map(atom_label)
            .collect::<Vec<_>>();

        let mut carried = source.clone();
        append_call_ensured_domain_atoms(program, value, &mut carried);
        let assumed = target
            .iter()
            .filter(|atom| atom_requires_carried_evidence(program, atom))
            .filter(|atom| {
                !carried
                    .iter()
                    .any(|candidate| atoms_equivalent(program, atom, candidate))
            })
            .filter(|atom| {
                !carried
                    .iter()
                    .any(|candidate| is_deferred_index_compatibility(atom, candidate))
            })
            .map(atom_label)
            .collect::<Vec<_>>();

        if !dropped.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
            "implicit domain weakening in {owner} drops {}; semantic meaning, routed provenance, \
             and arithmetic policy may be removed only by an explicit `as` to the intended target",
            dropped
                .iter()
                .map(|label| format!("`{label}`"))
                .collect::<Vec<_>>()
                .join(", "),
        )));
        }
        if !assumed.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
                "implicit domain strengthening in {owner} assumes {}; a qualification with no \
             predicate or establishment route may be introduced only by an explicit `as` to \
             the intended target",
                assumed
                    .iter()
                    .map(|label| format!("`{label}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            )));
        }
    }

    fn parameter_premise_atoms(
        program: &dyn CheckingInput,
        parameter: &StateParameter,
    ) -> Vec<DomainAtom> {
        if !parameter.symbol.is_valid() {
            return Vec::new();
        }
        let owner = program.symbols.get(parameter.symbol).parent;
        let Some(contracts) = callable_signature_contracts(program, owner) else {
            return Vec::new();
        };
        let mut atoms = Vec::new();
        for contract in contracts {
            if !matches!(contract.kind, SignatureContractKind::Requires) {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if !expression_table(program).names_parameter(membership.value, parameter) {
                    continue;
                }
                append_declared_atom(
                    program,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    !membership.domain_arguments.is_empty(),
                    None,
                    &mut atoms,
                    &mut Vec::new(),
                );
            }
        }
        atoms
    }

    fn atom_requires_carried_evidence(program: &dyn CheckingInput, atom: &DomainAtom) -> bool {
        let DomainAtom::Declared { family: symbol, .. } = atom else {
            return false;
        };
        program.domain_definition(*symbol).is_some_and(|domain| {
            !domain.predicate_presence.is_present() && domain.establishment_routes.is_empty()
        })
    }

    fn append_call_ensured_domain_atoms(
        program: &dyn CheckingInput,
        value: ExpressionHandle,
        atoms: &mut Vec<DomainAtom>,
    ) {
        let mut inner = value;
        let call = loop {
            match expression_table(program).expression(inner) {
                ExpressionNode::Borrow(borrow) => inner = borrow.target,
                ExpressionNode::Atomic(atomic) => inner = atomic.value,
                ExpressionNode::Call(call) => break call,
                _ => return,
            }
        };
        append_callable_ensured_domain_atoms(program, call.target_symbol, atoms);
    }

    fn append_callable_ensured_domain_atoms(
        program: &dyn CheckingInput,
        callable: SymbolHandle,
        atoms: &mut Vec<DomainAtom>,
    ) {
        let Some(contracts) = callable_signature_contracts(program, callable) else {
            return;
        };
        for contract in contracts {
            if !matches!(contract.kind, SignatureContractKind::Ensures) {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                let Some((owner, _)) =
                    crate::proof_contracts::contract_results::reserved_result_owner(
                        program,
                        membership.value,
                    )
                else {
                    continue;
                };
                if owner != callable
                    && !crate::machine_calls::calls::machine_state_by_symbol(program, callable)
                        .is_some_and(|(machine, _)| owner == machine.symbol)
                {
                    continue;
                }
                append_declared_atom(
                    program,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    !membership.domain_arguments.is_empty(),
                    None,
                    atoms,
                    &mut Vec::new(),
                );
            }
        }
    }

    fn callable_signature_contracts(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> Option<Vec<&typed_trees::signature::SignatureContract>> {
        if let Some((machine, state)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, target)
        {
            return Some(
                program
                    .state_contracts(state)
                    .iter()
                    .chain(program.machine_contracts(machine).iter())
                    .collect(),
            );
        }
        if !target.is_valid() {
            return None;
        }
        let parent = program.symbols.get(target).parent;
        let definition = program.trait_definition_by_symbol(parent)?;
        let signature = program.trait_machine_signature_by_symbol(definition, target)?;
        Some(
            program
                .state_signature_contracts(signature)
                .iter()
                .collect(),
        )
    }

    fn is_deferred_index_compatibility(left: &DomainAtom, right: &DomainAtom) -> bool {
        matches!(
            (left, right),
            (
                DomainAtom::Declared {
                    family: left_family,
                    instance: left_instance,
                    indexed: true,
                    ..
                },
                DomainAtom::Declared {
                    family: right_family,
                    instance: right_instance,
                    indexed: true,
                    ..
                }
            ) if left_family == right_family
                && left_instance.is_valid()
                && right_instance.is_valid()
                && left_instance != right_instance
        )
    }

    fn expression_atoms(
        program: &dyn CheckingInput,
        machine: &Machine,
        state: Option<&State>,
        value: ExpressionHandle,
    ) -> Vec<DomainAtom> {
        let mut atoms = Vec::new();
        match expression_table(program).expression(value) {
            ExpressionNode::Borrow(inner) => {
                return expression_atoms(program, machine, state, inner.target);
            }
            ExpressionNode::Atomic(TableAtomicExpression { value: inner, .. }) => {
                return expression_atoms(program, machine, state, *inner);
            }
            ExpressionNode::Cast(cast) => {
                append_type_atoms(program, cast.target_type, &mut atoms, &mut Vec::new());
                if cast.semantic_domain_symbol.is_valid() {
                    append_declared_atom(
                        program,
                        cast.semantic_domain_symbol,
                        cast.semantic_domain_id,
                        !cast.semantic_domain_arguments.is_empty(),
                        Some(qualification_cast_label(program, cast)),
                        &mut atoms,
                        &mut Vec::new(),
                    );
                }
                if cast.domain != ArithmeticDomain::Exact {
                    push_unique(&mut atoms, DomainAtom::Arithmetic(cast.domain));
                }
                return atoms;
            }
            ExpressionNode::Call(call) => {
                if let Some(return_type) =
                    crate::proof_contracts::arithmetic_domains::call_return_type(program, call)
                {
                    append_type_atoms(program, return_type, &mut atoms, &mut Vec::new());
                }
                return atoms;
            }
            _ => {}
        }

        if let Some(source_type) =
            crate::value_custody::places::declared_place_type_raw(program, machine, state, value)
        {
            append_type_atoms(program, source_type, &mut atoms, &mut Vec::new());
        }
        atoms
    }

    fn append_type_atoms(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
        atoms: &mut Vec<DomainAtom>,
        stack: &mut Vec<SymbolHandle>,
    ) {
        if !type_reference.is_valid() {
            return;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => {
                append_type_atoms(program, *referee, atoms, stack);
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                append_type_atoms(program, *base_type, atoms, stack);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                            append_declared_atom(
                                program,
                                domain.symbol,
                                domain.semantic_id,
                                !domain.arguments.is_empty(),
                                Some(domain_constraint_label(program, domain)),
                                atoms,
                                stack,
                            );
                        }
                        TypeConstraintNode::ArithmeticDomain(domain)
                            if *domain != ArithmeticDomain::Exact =>
                        {
                            push_unique(atoms, DomainAtom::Arithmetic(*domain));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn append_declared_atom(
        program: &dyn CheckingInput,
        symbol: SymbolHandle,
        instance: language_semantics::SemanticDomainId,
        indexed: bool,
        label: Option<String>,
        atoms: &mut Vec<DomainAtom>,
        stack: &mut Vec<SymbolHandle>,
    ) {
        if !symbol.is_valid() || stack.contains(&symbol) {
            return;
        }
        let Some(domain) = program.domain_definition(symbol) else {
            return;
        };
        if let Some(alias) = domain.alias.as_ref() {
            stack.push(symbol);
            for constituent in &alias.constituents {
                append_declared_atom(
                    program,
                    constituent.domain_symbol,
                    language_semantics::SemanticDomainId::NULL,
                    false,
                    None,
                    atoms,
                    stack,
                );
            }
            stack.pop();
        } else {
            push_unique(
                atoms,
                DomainAtom::Declared {
                    family: symbol,
                    instance,
                    indexed,
                    label: label.unwrap_or_else(|| domain.name.to_string()),
                },
            );
            stack.push(symbol);
            for membership in typed_trees::domain::subject_membership_facts(program, domain) {
                append_declared_atom(
                    program,
                    membership.domain_symbol,
                    membership.semantic_domain,
                    !membership.domain_arguments.is_empty(),
                    None,
                    atoms,
                    stack,
                );
            }
            stack.pop();
        }
    }

    fn push_unique(atoms: &mut Vec<DomainAtom>, atom: DomainAtom) {
        if !atoms.contains(&atom) {
            atoms.push(atom);
        }
    }

    fn atom_requires_explicit_removal(program: &dyn CheckingInput, atom: &DomainAtom) -> bool {
        match atom {
            DomainAtom::Arithmetic(_) => true,
            DomainAtom::Declared {
                family: symbol,
                indexed,
                ..
            } => {
                *indexed
                    || program.domain_definition(*symbol).is_some_and(|domain| {
                        !domain.semantic_roles.is_empty() || !domain.establishment_routes.is_empty()
                    })
            }
        }
    }

    fn atoms_equivalent(
        program: &dyn CheckingInput,
        left: &DomainAtom,
        right: &DomainAtom,
    ) -> bool {
        match (left, right) {
            (DomainAtom::Arithmetic(left), DomainAtom::Arithmetic(right)) => left == right,
            (
                DomainAtom::Declared {
                    family: left_family,
                    instance: left_instance,
                    ..
                },
                DomainAtom::Declared {
                    family: right_family,
                    instance: right_instance,
                    ..
                },
            ) => {
                if left_instance.is_valid() && right_instance.is_valid() {
                    return left_instance == right_instance;
                }
                if left_family == right_family {
                    return true;
                }
                let semantic_id = |symbol| {
                    program
                        .domain_definition(symbol)
                        .map(|domain| domain.semantic_id)
                };
                matches!((semantic_id(*left_family), semantic_id(*right_family)), (Some(left), Some(right)) if left.is_valid() && left == right)
            }
            _ => false,
        }
    }

    fn atom_label(atom: &DomainAtom) -> String {
        match atom {
            DomainAtom::Arithmetic(domain) => domain.name().to_owned(),
            DomainAtom::Declared { label, .. } => label.clone(),
        }
    }

    fn domain_argument_display(
        program: &dyn CheckingInput,
        argument: TypeReferenceHandle,
    ) -> String {
        match program.type_reference_table.type_reference(argument) {
            TypeReferenceNode::Named { name, .. } => CanonicalConstValue::from_atom(name.as_str())
                .map_or_else(|| name.to_string(), |value| value.display),
            _ => program.display_type_reference(argument),
        }
    }

    fn domain_constraint_label(program: &dyn CheckingInput, domain: &DomainConstraint) -> String {
        if domain.arguments.is_empty() {
            return domain.name.to_string();
        }

        let arguments = domain
            .arguments
            .iter()
            .map(|argument| domain_argument_display(program, *argument))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}<{arguments}>", domain.name)
    }

    fn qualification_cast_label(program: &dyn CheckingInput, cast: &TableCastExpression) -> String {
        let name = expression_table(program).name_path_string(cast.semantic_domain);
        if cast.semantic_domain_arguments.is_empty() {
            return name;
        }
        let arguments = program
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments)
            .iter()
            .map(|argument| domain_argument_display(program, *argument))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}<{arguments}>")
    }
}

pub mod expression_owners {
    pub(crate) mod call_targets {
        use super::environments::{exact_owner_environments, executable_site_environment};
        use super::inference::{
            infer_expression_type, inferred_nominal_symbol, member_target_in_environment,
        };
        use super::{InferredType, OwnerEnvironmentIndex, OwnerMemberTarget, TypeEnvironment};
        use crate::frozen_program::expression_table;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind;
        use symbols::JoinedPath;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::{ExpressionHandle, TableMemberExpression};
        use typed_trees::types::TypeReferenceHandle;

        pub fn checked_member_target_from_exact_owner(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            expression: ExpressionHandle,
            member: &TableMemberExpression,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<OwnerMemberTarget> {
            let source_span = authored_member_source_span(program, expression)?;
            let mut target = None;
            for environment in
                exact_owner_environments(program, contract_facts, expression, owner_index)?
            {
                let candidate =
                    member_target_in_environment(program, member, &environment, source_span)?;
                retain_consistent(&mut target, candidate)?;
            }
            target
        }

        pub(crate) fn authored_member_source_span(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
        ) -> Option<source::SourceSpan> {
            let mut retained = None;
            for occurrence in expression_table(program).authored_selection_occurrences(expression) {
                let selection = program.authored_declaration_selections().get(occurrence)?;
                if selection.kind() != AuthoredDeclarationSelectionKind::MemberAccess {
                    continue;
                }
                let source_span = selection.source_span();
                if retained.is_some_and(|existing| existing != source_span) {
                    return None;
                }
                retained = Some(source_span);
            }
            retained
        }

        pub fn checked_expression_type_reference_from_exact_owner(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            owner_expression: ExpressionHandle,
            expression: ExpressionHandle,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<TypeReferenceHandle> {
            let mut retained = None;
            for environment in
                exact_owner_environments(program, contract_facts, owner_expression, owner_index)?
            {
                let InferredType::TypeReference(candidate) =
                    infer_expression_type(program, expression, &environment, &mut Vec::new())?
                else {
                    return None;
                };
                if let Some(existing) = retained
                    && program.normalized_type_identity(existing)
                        != program.normalized_type_identity(candidate)
                {
                    return None;
                }
                retained = Some(candidate);
            }
            retained
        }

        pub fn checked_machine_call_target_from_exact_owner(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<SymbolHandle> {
            checked_machine_call_target_from_type_owners(
                program,
                contract_facts,
                expression,
                call,
                owner_index,
            )
            .ok()?
            .or_else(|| {
                checked_machine_call_target_from_executable_owner(
                    program,
                    contract_facts,
                    expression,
                    call,
                    owner_index,
                )
            })
        }

        fn checked_machine_call_target_from_type_owners(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Result<Option<SymbolHandle>, ()> {
            let Some(environments) =
                exact_owner_environments(program, contract_facts, expression, owner_index)
            else {
                return Ok(None);
            };
            let mut target = None;
            let mut complete = true;
            for environment in environments {
                let Some(candidate) = call_target_in_environment(program, call, &environment)
                else {
                    complete = false;
                    continue;
                };
                retain_consistent(&mut target, candidate).ok_or(())?;
            }
            Ok(complete.then_some(target).flatten())
        }

        pub(crate) fn checked_machine_call_target_from_executable_owner(
            program: &dyn crate::CheckingInput,
            _contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            expression: ExpressionHandle,
            call: &typed_trees::expression::TableCallExpression,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<SymbolHandle> {
            let mut target = None;
            if let Some(owner_index) = owner_index {
                for site in owner_index.executable_sites(expression) {
                    let machine = &program.machines()[site.machine];
                    if crate::declarations::selection_resolution::machine_is_template_instance(
                        program, machine,
                    ) {
                        continue;
                    }
                    let state = &program.machine_states(machine)[site.state];
                    let statements = program.statement_table.statements(state.statement_nodes);
                    let candidate = executable_site_call_candidate(
                        program,
                        machine,
                        state,
                        statements,
                        site.statement,
                        call,
                    );
                    if !candidate.is_valid() || target.is_some_and(|retained| retained != candidate)
                    {
                        return None;
                    }
                    target = Some(candidate);
                }
                return target;
            }
            for machine in program.machines() {
                if crate::declarations::selection_resolution::machine_is_template_instance(
                    program, machine,
                ) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    let statements = program.statement_table.statements(state.statement_nodes);
                    for (statement_index, statement) in statements.iter().enumerate() {
                        let mut expressions = Vec::new();
                        crate::queries::expression_traversal::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut expressions,
                        );
                        if !expressions.contains(&expression) {
                            continue;
                        }
                        let candidate = executable_site_call_candidate(
                            program,
                            machine,
                            state,
                            statements,
                            statement_index,
                            call,
                        );
                        if !candidate.is_valid()
                            || target.is_some_and(|retained| retained != candidate)
                        {
                            return None;
                        }
                        target = Some(candidate);
                    }
                }
            }
            target
        }

        pub(crate) fn executable_site_call_candidate(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statements: &[typed_trees::statement::StatementNode],
            statement_index: usize,
            call: &typed_trees::expression::TableCallExpression,
        ) -> SymbolHandle {
            let (receiver_symbol, receiver_path) =
                crate::call_receiver_parts(program, call.receiver);
            let mut candidate = crate::resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            );
            if !candidate.is_valid() && !call.receiver.is_valid() {
                let matching = program
                    .machine_type_parameters(machine)
                    .iter()
                    .filter(|parameter| {
                        parameter.name == call.target
                            && matches!(parameter.kind, TypeParameterKind::Machine { .. })
                    })
                    .map(|parameter| parameter.symbol)
                    .collect::<Vec<_>>();
                if let [selected] = matching.as_slice() {
                    candidate = *selected;
                }
            }
            if !candidate.is_valid() && call.receiver.is_valid() {
                let environment = executable_site_environment(
                    program,
                    machine,
                    state,
                    statements,
                    statement_index,
                );
                if let Some(receiver_type) =
                    infer_expression_type(program, call.receiver, &environment, &mut Vec::new())
                {
                    candidate =
                        attached_machine_target(program, receiver_type, call.target.as_str())
                            .unwrap_or_else(SymbolHandle::invalid);
                }
            }
            candidate
        }

        pub(crate) fn call_target_in_environment(
            program: &dyn crate::CheckingInput,
            call: &typed_trees::expression::TableCallExpression,
            environment: &TypeEnvironment,
        ) -> Option<SymbolHandle> {
            if !call.receiver.is_valid() {
                let state = crate::call_target_state(program, call.target_symbol)?;
                program.machine_holding_state(state.symbol)?;
                return Some(state.symbol);
            }
            let receiver_type = exact_static_data_receiver(program, call.receiver, environment)
                .or_else(|| {
                    infer_expression_type(program, call.receiver, environment, &mut Vec::new())
                })?;
            attached_machine_target(program, receiver_type, call.target.as_str())
        }

        pub(crate) fn exact_static_data_receiver(
            program: &dyn crate::CheckingInput,
            receiver: ExpressionHandle,
            environment: &TypeEnvironment,
        ) -> Option<InferredType> {
            let path = expression_table(program).name_path_node(receiver)?;
            let members = expression_table(program).name_path_members(path.members);
            let selected = crate::resolve_name_path_member_symbol(
                program,
                path,
                members.len().checked_sub(1)?,
            );
            if selected.is_valid()
                && program.symbols.get(selected).kind == symbols::SymbolKind::Data
            {
                return Some(InferredType::Nominal(selected));
            }

            let authored_path = members.joined_path();
            let mut retained = None;
            for inferred in environment
                .self_type
                .into_iter()
                .chain(environment.result_type.map(InferredType::TypeReference))
                .chain(
                    environment
                        .bindings
                        .iter()
                        .map(|binding| InferredType::TypeReference(binding.type_reference)),
                )
            {
                let Some(candidate) = inferred_nominal_symbol(program, inferred) else {
                    continue;
                };
                if program.symbols.get(candidate).kind != symbols::SymbolKind::Data
                    || program.symbols.display_path(candidate, "::") != authored_path
                {
                    continue;
                }
                retain_consistent(&mut retained, candidate)?;
            }
            retained.map(InferredType::Nominal)
        }

        pub(crate) fn attached_machine_target(
            program: &dyn crate::CheckingInput,
            receiver_type: InferredType,
            target_name: &str,
        ) -> Option<SymbolHandle> {
            let attached_data = inferred_nominal_symbol(program, receiver_type)?;
            let mut candidates = program
                .machines()
                .iter()
                .filter(|machine| {
                    !crate::declarations::selection_resolution::machine_is_template_instance(
                        program, machine,
                    )
                })
                .filter(|machine| machine.attached_data_symbol == attached_data)
                .flat_map(|machine| program.machine_states(machine))
                .filter(|state| state.name.as_str() == target_name)
                .map(|state| state.symbol);
            let selected = candidates.next()?;
            candidates.next().is_none().then_some(selected)
        }

        pub(crate) fn retain_consistent<T: Copy + PartialEq>(
            retained: &mut Option<T>,
            candidate: T,
        ) -> Option<()> {
            if retained.is_some_and(|target| target != candidate) {
                return None;
            }
            *retained = Some(candidate);
            Some(())
        }
    }
    pub(crate) mod environments {
        use super::indexing::{
            proof_fact_contains_expression, proof_fact_value_contains_expression,
        };
        use super::inference::type_reference_contains_expression;
        use super::{Binding, InferredType, OwnerEnvironmentIndex, TypeEnvironment};
        use crate::frozen_program::expression_table;
        use checked_trees::ContractProofFactOwner;
        use symbols::SymbolHandle;
        use typed_trees::data::TypeParameterKind;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::signature::StateParameter;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn executable_site_environment(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statements: &[StatementNode],
            statement_index: usize,
        ) -> TypeEnvironment {
            let mut environment = machine_environment(program, machine, Some(state));
            environment
                .bindings
                .extend(
                    statements.iter().take(statement_index).filter_map(
                        |statement| match statement {
                            StatementNode::LocalData(local) if local.type_reference.is_valid() => {
                                Some(Binding {
                                    name: local.name.as_str().to_owned(),
                                    symbol: local.symbol,
                                    type_reference: local.type_reference,
                                })
                            }
                            _ => None,
                        },
                    ),
                );
            environment
        }

        pub(crate) fn contract_owner_environment(
            program: &dyn crate::CheckingInput,
            call_signatures: Option<&arena::Arena<checked_trees::CallContractSignature>>,
            owner: ContractProofFactOwner,
        ) -> Option<TypeEnvironment> {
            match owner {
                ContractProofFactOwner::Machine { machine_symbol } => {
                    let machine = program.machine_by_symbol(machine_symbol)?;
                    let entry = program.machine_states(machine).first();
                    Some(machine_environment(program, machine, entry))
                }
                ContractProofFactOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                } => {
                    let machine = program.machine_by_symbol(machine_symbol)?;
                    let state = program.state_in_machine(machine, state_symbol)?;
                    Some(machine_environment(program, machine, Some(state)))
                }
                ContractProofFactOwner::CallSignature {
                    owner_symbol,
                    state_symbol,
                    application,
                } => {
                    let signatures = call_signatures?;
                    if !signatures.is_valid(application) {
                        return None;
                    }
                    let retained = signatures.get(application);
                    if retained.declaring_trait != owner_symbol
                        || retained.signature.symbol != state_symbol
                    {
                        return None;
                    }
                    Some(environment_from_parameters(
                        state_symbol,
                        program.state_signature_parameters(&retained.signature),
                        retained.signature.return_type,
                        None,
                    ))
                }
                ContractProofFactOwner::StateSignature {
                    owner_symbol,
                    state_symbol,
                } => signature_environment(program, owner_symbol, state_symbol),
                ContractProofFactOwner::OperatorDeclaration { operator_symbol } => {
                    let operator =
                        typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
                    Some(environment_from_parameters(
                        operator_symbol,
                        program.operator_parameters(operator),
                        operator.return_type,
                        None,
                    ))
                }
                ContractProofFactOwner::OperatorUse {
                    operator_symbol, ..
                } => {
                    let operator =
                        typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
                    Some(environment_from_parameters(
                        operator_symbol,
                        program.operator_parameters(operator),
                        operator.return_type,
                        None,
                    ))
                }
                ContractProofFactOwner::Unknown => None,
            }
        }

        pub(crate) fn machine_environment(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: Option<&typed_trees::state::State>,
        ) -> TypeEnvironment {
            let parameters = state.map_or(&[][..], |state| program.state_parameters(state));
            let result_type =
                state.and_then(|state| state.return_type.is_valid().then_some(state.return_type));
            let self_type = machine
                .attached_data_symbol
                .is_valid()
                .then_some(InferredType::Nominal(machine.attached_data_symbol));
            environment_from_parameters(
                machine.symbol,
                parameters,
                result_type.unwrap_or_else(TypeReferenceHandle::invalid),
                self_type,
            )
        }

        pub(crate) fn signature_environment(
            program: &dyn crate::CheckingInput,
            owner_symbol: SymbolHandle,
            state_symbol: SymbolHandle,
        ) -> Option<TypeEnvironment> {
            if let Some(signature) = program.traits().iter().find_map(|definition| {
                (definition.symbol == owner_symbol)
                    .then(|| program.trait_machine_signature_by_symbol(definition, state_symbol))
                    .flatten()
            }) {
                return Some(environment_from_parameters(
                    state_symbol,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    None,
                ));
            }
            // A callable binder keeps its contract under the binder itself. Read
            // it from there: a binder on a bodyless trait requirement has no
            // machine row, so a search through machines never finds it and its
            // nested `requires` and `ensures` clauses are left without types.
            let signature = program
                .retained_static_machine_contract(state_symbol)?
                .signature();
            Some(environment_from_parameters(
                state_symbol,
                program.state_signature_parameters(signature),
                signature.return_type,
                None,
            ))
        }

        pub(crate) fn environment_from_parameters(
            owner: SymbolHandle,
            parameters: &[StateParameter],
            result_type: TypeReferenceHandle,
            self_type: Option<InferredType>,
        ) -> TypeEnvironment {
            TypeEnvironment {
                owner,
                trait_scope: None,
                self_type,
                result_type: result_type.is_valid().then_some(result_type),
                bindings: parameters
                    .iter()
                    .map(|parameter| Binding {
                        name: parameter.name.as_str().to_owned(),
                        symbol: parameter.symbol,
                        type_reference: parameter.type_reference,
                    })
                    .collect(),
            }
        }

        pub(crate) fn exact_owner_environments(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            expression: ExpressionHandle,
            owner_index: Option<&OwnerEnvironmentIndex>,
        ) -> Option<Vec<TypeEnvironment>> {
            if let Some(owner_index) = owner_index {
                return owner_index.environments(expression);
            }
            let mut environments = Vec::new();
            for (_, contract) in contract_facts.iter() {
                if !proof_fact_contains_expression(program, contract.fact, expression) {
                    continue;
                }
                if let Some(environment) = contract_owner_environment(program, None, contract.owner)
                {
                    environments.push(environment);
                }
            }
            for domain in program.domain_definitions() {
                if program
                    .proof_facts(domain)
                    .iter()
                    .any(|fact| proof_fact_value_contains_expression(program, fact, expression))
                {
                    environments.push(TypeEnvironment {
                        owner: domain.symbol,
                        bindings: vec![Binding {
                            name: domain.subject_name.to_string(),
                            symbol: domain.subject_symbol,
                            type_reference: domain.target_type,
                        }],
                        ..Default::default()
                    });
                }
            }
            collect_proposition_environments(program, expression, &mut environments);
            collect_measure_environments(program, expression, &mut environments);
            collect_parameter_constraint_environments(program, expression, &mut environments)?;
            environments.extend(data_type_environments(program).into_iter().filter_map(
                |(reached, environment)| reached.contains(&expression).then_some(environment),
            ));
            for declaration in program.const_declarations() {
                if [
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                ]
                .into_iter()
                .any(|root| {
                    expression_table(program).expression_is_valid(root)
                        && super::expression_contains(program, root, expression)
                }) {
                    environments.push(TypeEnvironment {
                        owner: declaration.symbol,
                        ..Default::default()
                    });
                }
            }
            collect_ranking_environments(program, expression, &mut environments);
            collect_executable_environments(program, expression, &mut environments)?;
            (!environments.is_empty()).then_some(environments)
        }

        pub(crate) fn collect_measure_environments(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) {
            for measure in program.measures() {
                if !expression_table(program)
                    .expression_handles(measure.body)
                    .iter()
                    .any(|root| super::expression_contains(program, *root, expression))
                {
                    continue;
                }
                let Some(parameter) = measure.parameter.as_ref() else {
                    continue;
                };
                environments.push(environment_from_parameters(
                    measure.symbol,
                    std::slice::from_ref(parameter),
                    measure.return_type,
                    None,
                ));
            }
        }

        pub(crate) fn collect_proposition_environments(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) {
            use typed_trees::proposition::{PropositionBody, PropositionFormula};

            for proposition in program.propositions() {
                let PropositionBody::Transparent { proposition: body } = &proposition.body else {
                    continue;
                };
                let contains = match body {
                    PropositionFormula::Application(application) => expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .any(|root| super::expression_contains(program, *root, expression)),
                    PropositionFormula::BooleanExpression(root) => {
                        super::expression_contains(program, *root, expression)
                    }
                };
                if contains {
                    environments.push(environment_from_parameters(
                        proposition.symbol,
                        program.proposition_parameters(proposition),
                        TypeReferenceHandle::invalid(),
                        None,
                    ));
                }
            }
        }

        pub(crate) fn collect_ranking_environments(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) {
            for custody in &program.ranking_expression_custody {
                let roots = custody
                    .subjects
                    .iter()
                    .chain(&custody.view_arguments)
                    .copied()
                    .chain(custody.rank_range);
                if !roots
                    .into_iter()
                    .any(|root| super::expression_contains(program, root, expression))
                {
                    continue;
                }
                let Some(machine) = program.machine_by_symbol(custody.machine) else {
                    continue;
                };
                environments.push(machine_environment(
                    program,
                    machine,
                    program.machine_states(machine).first(),
                ));
            }
        }

        pub(crate) fn collect_parameter_constraint_environments(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) -> Option<()> {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let parameters = program.state_parameters(state);
                    for (index, parameter) in parameters.iter().enumerate() {
                        if !type_reference_contains_expression(
                            program,
                            parameter.type_reference,
                            expression,
                            &mut Vec::new(),
                        ) {
                            continue;
                        }
                        let environment = TypeEnvironment {
                            owner: machine.symbol,
                            self_type: machine
                                .attached_data_symbol
                                .is_valid()
                                .then_some(InferredType::Nominal(machine.attached_data_symbol)),
                            bindings: environment_from_parameters(
                                machine.symbol,
                                &parameters[..index],
                                TypeReferenceHandle::invalid(),
                                None,
                            )
                            .bindings,
                            ..Default::default()
                        };
                        environments.push(environment);
                    }
                }
            }

            for definition in program.traits() {
                for signature in program.trait_machine_signatures(definition) {
                    collect_telescope_constraint_environments(
                        program,
                        signature.symbol,
                        program.state_signature_parameters(signature),
                        expression,
                        environments,
                    )?;
                }
            }
            for operator in program.all_operators() {
                collect_telescope_constraint_environments(
                    program,
                    operator.symbol,
                    program.operator_parameters(operator),
                    expression,
                    environments,
                )?;
            }
            for proposition in program.propositions() {
                collect_telescope_constraint_environments(
                    program,
                    proposition.symbol,
                    program.proposition_parameters(proposition),
                    expression,
                    environments,
                )?;
            }
            Some(())
        }

        pub(crate) fn collect_telescope_constraint_environments(
            program: &dyn crate::CheckingInput,
            owner: SymbolHandle,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) -> Option<()> {
            for (index, parameter) in parameters.iter().enumerate() {
                if !type_reference_contains_expression(
                    program,
                    parameter.type_reference,
                    expression,
                    &mut Vec::new(),
                ) {
                    continue;
                }
                let environment = environment_from_parameters(
                    owner,
                    &parameters[..index],
                    TypeReferenceHandle::invalid(),
                    None,
                );
                environments.push(environment);
            }
            Some(())
        }

        pub(crate) fn collect_executable_environments(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environments: &mut Vec<TypeEnvironment>,
        ) -> Option<()> {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let statements = program.statement_table.statements(state.statement_nodes);
                    for (statement_index, statement) in statements.iter().enumerate() {
                        let mut expressions = Vec::new();
                        crate::queries::expression_traversal::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut expressions,
                        );
                        if !expressions.contains(&expression) {
                            continue;
                        }
                        environments.push(executable_site_environment(
                            program,
                            machine,
                            state,
                            statements,
                            statement_index,
                        ));
                    }
                }
            }
            Some(())
        }

        pub(crate) fn data_type_environments(
            program: &dyn crate::CheckingInput,
        ) -> Vec<(Vec<ExpressionHandle>, TypeEnvironment)> {
            let mut environments = Vec::new();
            for definition in program.data_definitions() {
                let bindings = program
                    .data_type_parameters(definition)
                    .iter()
                    .filter_map(|parameter| {
                        let type_reference = match parameter.kind {
                            TypeParameterKind::Const { type_reference }
                            | TypeParameterKind::Value { type_reference } => type_reference,
                            _ => return None,
                        };
                        Some(Binding {
                            name: parameter.name.as_str().to_owned(),
                            symbol: parameter.symbol,
                            type_reference,
                        })
                    })
                    .collect::<Vec<_>>();
                let mut common = bindings;
                if definition.predicate_subject_symbol.is_valid()
                    && definition.predicate_subject_type.is_valid()
                {
                    common.push(Binding {
                        name: "self".to_owned(),
                        symbol: definition.predicate_subject_symbol,
                        type_reference: definition.predicate_subject_type,
                    });
                }
                common.extend(
                    program
                        .data_members(definition)
                        .iter()
                        .filter_map(|member| {
                            let field = member.as_field()?;
                            Some(Binding {
                                name: field.name.to_string(),
                                symbol: field.symbol,
                                type_reference: field.type_reference,
                            })
                        }),
                );
                let mut reached = Vec::new();
                for fact in program.proof_facts.span_or_empty(definition.requires_facts) {
                    reached.extend(super::indexing::proof_fact_reachable_expressions(
                        program, fact,
                    ));
                }
                for member in program.data_members(definition) {
                    match member {
                        typed_trees::data::DataMember::Field(field) => {
                            reached.extend(super::indexing::type_reference_reachable_expressions(
                                program,
                                field.type_reference,
                            ));
                        }
                        typed_trees::data::DataMember::Variant(variant) => {
                            let mut payload_reached = Vec::new();
                            for fact in program.proof_facts.span_or_empty(variant.requires_facts) {
                                payload_reached.extend(
                                    super::indexing::proof_fact_reachable_expressions(
                                        program, fact,
                                    ),
                                );
                            }
                            let mut payload = common.clone();
                            for field in program.data_payload_fields(variant) {
                                payload.push(Binding {
                                    name: field.name.to_string(),
                                    symbol: field.symbol,
                                    type_reference: field.type_reference,
                                });
                                payload_reached.extend(
                                    super::indexing::type_reference_reachable_expressions(
                                        program,
                                        field.type_reference,
                                    ),
                                );
                            }
                            if !payload_reached.is_empty() {
                                environments.push((
                                    payload_reached,
                                    TypeEnvironment {
                                        owner: definition.symbol,
                                        bindings: payload,
                                        ..Default::default()
                                    },
                                ));
                            }
                        }
                    }
                }
                if !reached.is_empty() {
                    environments.push((
                        reached,
                        TypeEnvironment {
                            owner: definition.symbol,
                            bindings: common,
                            ..Default::default()
                        },
                    ));
                }
            }
            environments
        }
    }
    pub(crate) mod indexing {
        use super::environments::{
            contract_owner_environment, environment_from_parameters, executable_site_environment,
            machine_environment,
        };
        use super::{Binding, ExecutableSite, InferredType, OwnerIndex, TypeEnvironment};
        use crate::CheckingInput;
        use crate::frozen_program::expression_table;
        use typed_trees::domain::ProofFact;
        pub(crate) type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn build_owner_environment_index(
            program: &dyn CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            call_signatures: &arena::Arena<checked_trees::CallContractSignature>,
        ) -> OwnerIndex {
            let mut index = OwnerIndex::default();
            for (handle, contract) in contract_facts.iter() {
                let reached = proof_fact_handle_reachable_expressions(program, contract.fact);
                for expression in &reached {
                    index
                        .entries
                        .entry(*expression)
                        .or_default()
                        .contract_facts
                        .push(handle);
                }
                let Some(environment) =
                    contract_owner_environment(program, Some(call_signatures), contract.owner)
                else {
                    continue;
                };
                index.add_all(reached, || environment);
            }
            for domain in program.domain_definitions() {
                let mut reached = HashSet::default();
                for fact in program.proof_facts(domain) {
                    reached.extend(proof_fact_reachable_expressions(program, fact));
                }
                index.add_all(reached, || TypeEnvironment {
                    owner: domain.symbol,
                    bindings: vec![Binding {
                        name: domain.subject_name.to_string(),
                        symbol: domain.subject_symbol,
                        type_reference: domain.target_type,
                    }],
                    ..Default::default()
                });
            }
            index_proposition_environments(program, &mut index);
            index_measure_environments(program, &mut index);
            index_parameter_constraint_environments(program, &mut index);
            for (reached, environment) in super::environments::data_type_environments(program) {
                index.add_all(reached, || environment);
            }
            for declaration in program.const_declarations() {
                let reached = [
                    declaration.authored_initializer,
                    declaration.materialized_initializer,
                ]
                .into_iter()
                .filter(|root| expression_table(program).expression_is_valid(*root))
                .flat_map(|root| super::reachable_expressions(program, root));
                index.add_all(reached, || TypeEnvironment {
                    owner: declaration.symbol,
                    ..Default::default()
                });
            }
            index_ranking_environments(program, &mut index);
            index_executable_environments(program, &mut index);
            index
        }

        /// Preparation needs the lexical binders of retained contracts, but cannot
        /// mint the checked fact handles installed by the checking pass above.
        pub(crate) fn index_contract_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            let mut add_contracts = |contracts: &[typed_trees::signature::SignatureContract],
                                     environment| {
                let reached = contracts
                    .iter()
                    .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                    .flat_map(|fact| proof_fact_reachable_expressions(program, fact))
                    .collect::<HashSet<_>>();
                index.add_all(reached, || environment);
            };
            for machine in program.machines() {
                add_contracts(
                    program.machine_contracts(machine),
                    machine_environment(program, machine, program.machine_states(machine).first()),
                );
                for state in program.machine_states(machine) {
                    add_contracts(
                        program.state_contracts(state),
                        machine_environment(program, machine, Some(state)),
                    );
                }
            }
            for contract in
                crate::proof_contracts::contract_results::retained_callable_contracts(program)
            {
                let signature = contract.signature;
                let mut environment = environment_from_parameters(
                    signature.symbol,
                    program.state_signature_parameters(signature),
                    signature.return_type,
                    None,
                );
                environment.trait_scope = contract.trait_scope;
                add_contracts(program.state_signature_contracts(signature), environment);
            }
            for operator in program.all_operators() {
                add_contracts(
                    program.operator_contracts(operator),
                    environment_from_parameters(
                        operator.symbol,
                        program.operator_parameters(operator),
                        operator.return_type,
                        None,
                    ),
                );
            }
        }

        pub(crate) fn proof_fact_reachable_expressions(
            program: &dyn CheckingInput,
            fact: &ProofFact,
        ) -> HashSet<ExpressionHandle> {
            let mut reached = HashSet::default();
            match fact {
                ProofFact::Expression(root) => {
                    reached.extend(super::reachable_expressions(program, *root));
                }
                ProofFact::Membership(membership) => {
                    reached.extend(super::reachable_expressions(program, membership.value));
                }
                ProofFact::Proposition(application) => {
                    for root in expression_table(program).expression_handles(application.arguments)
                    {
                        reached.extend(super::reachable_expressions(program, *root));
                    }
                }
            }
            reached
        }

        pub(crate) fn proof_fact_handle_reachable_expressions(
            program: &dyn CheckingInput,
            fact: arena::Handle<ProofFact>,
        ) -> HashSet<ExpressionHandle> {
            proof_fact_reachable_expressions(program, program.proof_facts.get(fact))
        }

        pub(crate) fn type_reference_reachable_expressions(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> HashSet<ExpressionHandle> {
            let mut reached = HashSet::default();
            collect_type_reference_expressions(
                program,
                type_reference,
                &mut Vec::new(),
                &mut reached,
            );
            reached
        }

        pub(crate) fn collect_type_reference_expressions(
            program: &dyn CheckingInput,
            type_reference: TypeReferenceHandle,
            visited: &mut Vec<TypeReferenceHandle>,
            reached: &mut HashSet<ExpressionHandle>,
        ) {
            if !type_reference.is_valid() || visited.contains(&type_reference) {
                return;
            }
            visited.push(type_reference);
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Callable { .. } => {
                    let inputs = typed_trees::references::type_reference_inputs(
                        program.typed_program(),
                        type_reference,
                    );
                    reached.extend(inputs.expressions);
                    visited.extend(inputs.types);
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    collect_type_reference_expressions(program, *referee, visited, reached)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    collect_type_reference_expressions(program, *base_type, visited, reached);
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        match constraint {
                            TypeConstraintNode::Domain(domain) => {
                                for argument in &domain.arguments {
                                    collect_type_reference_expressions(
                                        program, *argument, visited, reached,
                                    );
                                }
                            }
                            TypeConstraintNode::Named(_)
                            | TypeConstraintNode::ArithmeticDomain(_) => {}
                        }
                    }
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    collect_type_reference_expressions(program, *element_type, visited, reached);
                    if let typed_trees::types::FixedArrayLength::Expression(value) = length {
                        collect_type_reference_expressions(program, *value, visited, reached);
                    }
                }
                TypeReferenceNode::Slice { element_type } => {
                    collect_type_reference_expressions(program, *element_type, visited, reached)
                }
                TypeReferenceNode::Generic { arguments, .. } => {
                    for argument in program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                    {
                        collect_type_reference_expressions(program, *argument, visited, reached);
                    }
                }
                TypeReferenceNode::ConstExpression(root) => {
                    reached.extend(super::reachable_expressions(program, *root));
                }
                TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Unit => {}
            }
        }

        pub(crate) fn index_proposition_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            use typed_trees::proposition::{PropositionBody, PropositionFormula};

            for proposition in program.propositions() {
                let PropositionBody::Transparent { proposition: body } = &proposition.body else {
                    continue;
                };
                let mut reached = HashSet::default();
                match body {
                    PropositionFormula::Application(application) => {
                        for root in
                            expression_table(program).expression_handles(application.arguments)
                        {
                            reached.extend(super::reachable_expressions(program, *root));
                        }
                    }
                    PropositionFormula::BooleanExpression(root) => {
                        reached.extend(super::reachable_expressions(program, *root));
                    }
                }
                index.add_all(reached, || {
                    environment_from_parameters(
                        proposition.symbol,
                        program.proposition_parameters(proposition),
                        TypeReferenceHandle::invalid(),
                        None,
                    )
                });
            }
        }

        pub(crate) fn index_measure_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            for measure in program.measures() {
                let Some(parameter) = measure.parameter.as_ref() else {
                    continue;
                };
                let mut reached = HashSet::default();
                for root in expression_table(program).expression_handles(measure.body) {
                    reached.extend(super::reachable_expressions(program, *root));
                }
                index.add_all(reached, || {
                    environment_from_parameters(
                        measure.symbol,
                        std::slice::from_ref(parameter),
                        measure.return_type,
                        None,
                    )
                });
            }
        }

        pub(crate) fn index_parameter_constraint_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let parameters = program.state_parameters(state);
                    for (parameter_index, parameter) in parameters.iter().enumerate() {
                        index.add_all(
                            type_reference_reachable_expressions(program, parameter.type_reference),
                            || TypeEnvironment {
                                owner: machine.symbol,
                                self_type: machine
                                    .attached_data_symbol
                                    .is_valid()
                                    .then_some(InferredType::Nominal(machine.attached_data_symbol)),
                                bindings: environment_from_parameters(
                                    machine.symbol,
                                    &parameters[..parameter_index],
                                    TypeReferenceHandle::invalid(),
                                    None,
                                )
                                .bindings,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            for contract in
                crate::proof_contracts::contract_results::retained_callable_contracts(program)
            {
                index_telescope_parameters_in_scope(
                    program,
                    contract.signature.symbol,
                    program.state_signature_parameters(contract.signature),
                    contract.trait_scope,
                    index,
                );
            }
            for operator in program.all_operators() {
                index_telescope_parameters(
                    program,
                    operator.symbol,
                    program.operator_parameters(operator),
                    index,
                );
            }
            for proposition in program.propositions() {
                index_telescope_parameters(
                    program,
                    proposition.symbol,
                    program.proposition_parameters(proposition),
                    index,
                );
            }
        }

        pub(crate) fn index_telescope_parameters(
            program: &dyn CheckingInput,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            index: &mut OwnerIndex,
        ) {
            index_telescope_parameters_in_scope(program, owner, parameters, None, index);
        }

        fn index_telescope_parameters_in_scope(
            program: &dyn CheckingInput,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
            index: &mut OwnerIndex,
        ) {
            for (parameter_index, parameter) in parameters.iter().enumerate() {
                index.add_all(
                    type_reference_reachable_expressions(program, parameter.type_reference),
                    || {
                        let mut environment = environment_from_parameters(
                            owner,
                            &parameters[..parameter_index],
                            TypeReferenceHandle::invalid(),
                            None,
                        );
                        environment.trait_scope = trait_scope;
                        environment
                    },
                );
            }
        }

        pub(crate) fn index_ranking_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            for custody in &program.ranking_expression_custody {
                let Some(machine) = program.machine_by_symbol(custody.machine) else {
                    continue;
                };
                let mut reached = HashSet::default();
                let roots = custody
                    .subjects
                    .iter()
                    .chain(&custody.view_arguments)
                    .copied()
                    .chain(custody.rank_range);
                for root in roots {
                    reached.extend(super::reachable_expressions(program, root));
                }
                index.add_all(reached, || {
                    machine_environment(program, machine, program.machine_states(machine).first())
                });
            }
        }

        pub(crate) fn index_executable_environments(
            program: &dyn CheckingInput,
            index: &mut OwnerIndex,
        ) {
            let mut expressions = Vec::new();
            for (machine_index, machine) in program.machines().iter().enumerate() {
                for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                    let statements = program.statement_table.statements(state.statement_nodes);
                    for (statement_index, statement) in statements.iter().enumerate() {
                        expressions.clear();
                        crate::queries::expression_traversal::collect_statement_expression_trees(
                            program,
                            statement,
                            &mut expressions,
                        );
                        if expressions.is_empty() {
                            continue;
                        }
                        let environment = index.intern(executable_site_environment(
                            program,
                            machine,
                            state,
                            statements,
                            statement_index,
                        ));
                        for &expression in &expressions {
                            index.add(expression, environment);
                            let entry = index.entries.entry(expression).or_default();
                            entry.executable_sites.push(ExecutableSite {
                                machine: machine_index,
                                state: state_index,
                                statement: statement_index,
                            });
                        }
                    }
                }
            }
        }

        pub fn proof_fact_contains_expression(
            program: &dyn CheckingInput,
            fact: arena::Handle<ProofFact>,
            expression: ExpressionHandle,
        ) -> bool {
            proof_fact_value_contains_expression(program, program.proof_facts.get(fact), expression)
        }

        pub(crate) fn proof_fact_value_contains_expression(
            program: &dyn CheckingInput,
            fact: &ProofFact,
            expression: ExpressionHandle,
        ) -> bool {
            match fact {
                ProofFact::Expression(root) => {
                    super::expression_contains(program, *root, expression)
                }
                ProofFact::Membership(membership) => {
                    super::expression_contains(program, membership.value, expression)
                }
                ProofFact::Proposition(application) => expression_table(program)
                    .expression_handles(application.arguments)
                    .iter()
                    .any(|root| super::expression_contains(program, *root, expression)),
            }
        }
    }
    pub(crate) mod inference {
        /*
        Type inference over authored expressions: the inferred nominal symbol,
        collection view, and member resolution each environment can answer.
        */

        use super::{InferredType, OwnerMemberTarget, TypeEnvironment};
        use crate::collection_operations::MeasureReceiver;
        use language_semantics::declaration_selection::CollectionMeasure;
        /// Handle-keyed indexes and reach sets take the integer hasher.
        use symbols::SymbolHandle;
        use typed_trees::data::DataMember;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMemberExpression};
        use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn type_reference_contains_expression(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            expression: ExpressionHandle,
            visited: &mut Vec<TypeReferenceHandle>,
        ) -> bool {
            if !type_reference.is_valid() || visited.contains(&type_reference) {
                return false;
            }
            visited.push(type_reference);
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Callable { .. } => {
                    typed_trees::references::type_reference_inputs(
                        program.typed_program(),
                        type_reference,
                    )
                    .expressions
                    .contains(&expression)
                }
                TypeReferenceNode::Reference { referee, .. } => {
                    type_reference_contains_expression(program, *referee, expression, visited)
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    type_reference_contains_expression(program, *base_type, expression, visited)
                        || program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .any(|constraint| match constraint {
                                TypeConstraintNode::Domain(domain) => {
                                    domain.arguments.iter().any(|argument| {
                                        type_reference_contains_expression(
                                            program, *argument, expression, visited,
                                        )
                                    })
                                }
                                TypeConstraintNode::Named(_)
                                | TypeConstraintNode::ArithmeticDomain(_) => false,
                            })
                }
                TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } => {
                    type_reference_contains_expression(program, *element_type, expression, visited)
                        || matches!(length, typed_trees::types::FixedArrayLength::Expression(value)
                            if type_reference_contains_expression(program, *value, expression, visited))
                }
                TypeReferenceNode::Slice { element_type } => {
                    type_reference_contains_expression(program, *element_type, expression, visited)
                }
                TypeReferenceNode::Generic { arguments, .. } => program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| {
                        type_reference_contains_expression(program, *argument, expression, visited)
                    }),
                TypeReferenceNode::ConstExpression(root) => {
                    super::expression_contains(program, *root, expression)
                }
                TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Unit => false,
            }
        }

        pub(crate) fn member_target_in_environment(
            program: &dyn crate::CheckingInput,
            member: &TableMemberExpression,
            environment: &TypeEnvironment,
            source_span: source::SourceSpan,
        ) -> Option<OwnerMemberTarget> {
            let receiver_type =
                infer_expression_type(program, member.receiver, environment, &mut Vec::new())?;
            resolve_member_symbol(program, receiver_type, member)
                .or_else(|| {
                    resolve_member_symbol_from_authored_source(
                        program,
                        receiver_type,
                        member,
                        source_span,
                    )
                })
                .map(OwnerMemberTarget::Declaration)
                .or_else(|| {
                    collection_measure_in_environment(program, member, receiver_type)
                        .map(OwnerMemberTarget::CollectionMeasure)
                })
        }

        /// The measure a member selects on its inferred receiver, asked of
        /// [`crate::collection_operations::collection_measure_member`] in that query's
        /// receiver vocabulary. A nominal or compiler-text receiver has no measure.
        pub(crate) fn collection_measure_in_environment(
            program: &dyn crate::CheckingInput,
            member: &TableMemberExpression,
            receiver_type: InferredType,
        ) -> Option<CollectionMeasure> {
            let receiver = match receiver_type {
                InferredType::TypeReference(type_reference) => {
                    MeasureReceiver::Declared(type_reference)
                }
                InferredType::CollectionView(_) => MeasureReceiver::Window,
                InferredType::Nominal(_) | InferredType::CompilerString => return None,
            };
            crate::collection_operations::collection_measure_member(program, member, receiver)
        }

        pub(crate) fn resolve_member_symbol_from_authored_source(
            program: &dyn crate::CheckingInput,
            receiver_type: InferredType,
            member: &TableMemberExpression,
            source_span: source::SourceSpan,
        ) -> Option<SymbolHandle> {
            let InferredType::TypeReference(type_reference) = receiver_type else {
                return None;
            };
            let type_name = unresolved_nominal_type_name(program, type_reference)?;
            let nominal = program
                .symbols
                .find_top_level_by_name_and_kinds_from_source(
                    type_name,
                    &[symbols::SymbolKind::Data],
                    source_span,
                )?;
            resolve_member_symbol(program, InferredType::Nominal(nominal), member)
        }

        pub(crate) fn unresolved_nominal_type_name(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
        ) -> Option<&str> {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => unresolved_nominal_type_name(program, *referee),
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    ..
                } if !base_symbol.is_valid() => Some(base_name.as_str()),
                TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() => {
                    Some(name.as_str())
                }
                _ => None,
            }
        }

        pub(crate) fn infer_expression_type(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environment: &TypeEnvironment,
            visited: &mut Vec<ExpressionHandle>,
        ) -> Option<InferredType> {
            if !expression.is_valid() || visited.contains(&expression) {
                return None;
            }
            visited.push(expression);
            match program.expression_table.expression(expression) {
                ExpressionNode::MachineValue(value) => {
                    Some(InferredType::TypeReference(value.type_reference))
                }
                ExpressionNode::SavedSubject(saved) => {
                    program.saved_value_source(saved).and_then(|subject| {
                        infer_expression_type(program, subject, environment, visited)
                    })
                }
                ExpressionNode::Match(dispatch) => {
                    let mut result = None;
                    for arm in program.expression_table.match_arms(dispatch.arms) {
                        let arm_type = infer_expression_type(
                            program,
                            arm.value,
                            environment,
                            &mut visited.clone(),
                        )?;
                        if result.is_some_and(|previous| previous != arm_type) {
                            return None;
                        }
                        result = Some(arm_type);
                    }
                    result
                }
                ExpressionNode::Atomic(atomic) => {
                    infer_expression_type(program, atomic.value, environment, visited)
                }
                ExpressionNode::Borrow(inner) => {
                    infer_expression_type(program, inner.target, environment, visited)
                }
                ExpressionNode::Cast(cast) => Some(InferredType::TypeReference(cast.target_type)),
                ExpressionNode::Name(path) => {
                    if path.symbol.is_valid()
                        && let Some(type_reference) =
                            crate::value_custody::expression_types::named_value_type_reference(
                                program, path,
                            )
                            .or_else(|| crate::declaration_type_reference(program, path.symbol))
                    {
                        return Some(InferredType::TypeReference(type_reference));
                    }
                    let [name] = program.expression_table.name_path_members(path.members) else {
                        return None;
                    };
                    if !path.symbol.is_valid()
                        && !path.head_symbol.is_valid()
                        && name.is_self_receiver()
                    {
                        return environment.self_type;
                    }
                    if program.expression_table.names_reserved_result(expression) {
                        return environment.result_type.map(InferredType::TypeReference);
                    }
                    let mut matches = environment
                        .bindings
                        .iter()
                        .filter(|binding| {
                            (path.head_symbol.is_valid() && binding.symbol == path.head_symbol)
                                || (path.symbol.is_valid() && binding.symbol == path.symbol)
                                || (!path.head_symbol.is_valid()
                                    && !path.symbol.is_valid()
                                    && binding.name == name.as_str())
                        })
                        .map(|binding| binding.type_reference);
                    let first = matches.next()?;
                    matches
                        .all(|type_reference| type_reference == first)
                        .then_some(InferredType::TypeReference(first))
                }
                ExpressionNode::Member(member) => {
                    let receiver =
                        infer_expression_type(program, member.receiver, environment, visited)?;
                    let field_type = match receiver {
                        InferredType::TypeReference(reference) => {
                            // Chained field lookup needs the actual payload type.
                            // Reopening its generic declaration loses that substitution.
                            crate::value_custody::places::applied_data_member_field(
                                program, reference, member,
                            )
                            .map(|field| field.type_reference)
                        }
                        InferredType::Nominal(_) => {
                            resolve_member_symbol(program, receiver, member).and_then(|symbol| {
                                crate::declaration_type_reference(program, symbol)
                            })
                        }
                        InferredType::CollectionView(_) | InferredType::CompilerString => None,
                    };
                    if let Some(reference) = field_type {
                        return Some(InferredType::TypeReference(reference));
                    }
                    // Measures have no authored field declaration. Their selected
                    // receiver fixes a count carrier in declaration contracts too.
                    collection_measure_in_environment(program, member, receiver)?;
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        symbols::BuiltinTypeAtom::U64,
                    )
                    .map(InferredType::TypeReference)
                }
                ExpressionNode::Call(_)
                | ExpressionNode::Binary(_)
                | ExpressionNode::Unary(_)
                | ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::Integer(_) => {
                    // The environment binds leaves; the ordinary result-type owner
                    // selects computations. A separate call/binary recognizer here
                    // lost proof Int and treated known operands as overload wildcards.
                    // Keep the declaring owner so its trait meanings remain in scope.
                    crate::value_custody::expression_types::declaration_expression_result_type_reference(
                        program,
                        environment.owner,
                        environment.trait_scope,
                        &|leaf| match infer_expression_type(
                            program,
                            leaf,
                            environment,
                            &mut visited.clone(),
                        )? {
                            InferredType::TypeReference(reference) => Some(reference),
                            _ => None,
                        },
                        expression,
                    )
                    .map(InferredType::TypeReference)
                }
                ExpressionNode::Indexed(indexed) => {
                    let collection =
                        infer_expression_type(program, indexed.collection, environment, visited)?;
                    let element_type = collection_element_type(program, collection)?;
                    Some(
                        if matches!(
                            program.expression_table.expression(indexed.index),
                            ExpressionNode::Range(_)
                        ) {
                            InferredType::CollectionView(element_type)
                        } else {
                            InferredType::TypeReference(element_type)
                        },
                    )
                }
                ExpressionNode::StructLiteral(literal) => {
                    if literal.application_type.is_valid() {
                        Some(InferredType::TypeReference(literal.application_type))
                    } else {
                        literal
                            .type_symbol
                            .is_valid()
                            .then_some(InferredType::Nominal(literal.type_symbol))
                    }
                }
                ExpressionNode::String(_) => Some(InferredType::CompilerString),
                ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Range(_)
                | ExpressionNode::TypeExpression(_)
                | ExpressionNode::ZeroValue(_) => None,
            }
        }

        pub(crate) fn resolve_member_symbol(
            program: &dyn crate::CheckingInput,
            receiver_type: InferredType,
            member: &TableMemberExpression,
        ) -> Option<SymbolHandle> {
            let nominal = inferred_nominal_symbol(program, receiver_type)?;
            let data = program.data_definition_by_symbol(nominal)?;

            let mut candidates = Vec::new();
            for data_member in program.data_members(data) {
                match data_member {
                    DataMember::Field(field) if field.name.as_str() == member.member.as_str() => {
                        if !candidates.contains(&field.symbol) {
                            candidates.push(field.symbol);
                        }
                    }
                    DataMember::Variant(variant) => {
                        if member
                            .case_variant
                            .as_ref()
                            .is_some_and(|case| case != &variant.name)
                        {
                            continue;
                        }
                        for field in program
                            .data_payload_fields(variant)
                            .iter()
                            .filter(|field| field.name.as_str() == member.member.as_str())
                        {
                            if !candidates.contains(&field.symbol) {
                                candidates.push(field.symbol);
                            }
                        }
                    }
                    _ => {}
                }
            }
            let [selected] = candidates.as_slice() else {
                return None;
            };
            Some(*selected)
        }

        pub(crate) fn inferred_nominal_symbol(
            program: &dyn crate::CheckingInput,
            inferred: InferredType,
        ) -> Option<SymbolHandle> {
            match inferred {
                InferredType::Nominal(symbol) => symbol.is_valid().then_some(symbol),
                InferredType::TypeReference(type_reference) => {
                    let symbol = program.type_reference_table.type_symbol(type_reference);
                    symbol.is_valid().then_some(symbol)
                }
                InferredType::CollectionView(_) => None,
                InferredType::CompilerString => None,
            }
        }

        pub(crate) fn collection_element_type(
            program: &dyn crate::CheckingInput,
            inferred: InferredType,
        ) -> Option<TypeReferenceHandle> {
            let type_reference = match inferred {
                InferredType::TypeReference(type_reference) => type_reference,
                InferredType::CollectionView(element_type) => return Some(element_type),
                InferredType::Nominal(_) | InferredType::CompilerString => return None,
            };
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. }
                | TypeReferenceNode::Constrained {
                    base_type: referee, ..
                } => collection_element_type(program, InferredType::TypeReference(*referee)),
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => Some(*element_type),
                _ => None,
            }
        }
    }

    pub(crate) mod reachability {
        pub(crate) type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
        use crate::frozen_program::expression_table;
        use typed_trees::expression::ExpressionHandle;

        pub fn expression_contains(
            program: &dyn crate::CheckingInput,
            root: ExpressionHandle,
            target: ExpressionHandle,
        ) -> bool {
            if !root.is_valid() || !target.is_valid() {
                return false;
            }
            if root == target {
                return true;
            }
            if matches!(
                expression_table(program).expression(root),
                typed_trees::literal_expression_pattern!()
            ) {
                return false;
            }
            reaches(program, root, target, &mut HashSet::default())
        }

        pub fn reachable_expressions(
            program: &dyn crate::CheckingInput,
            root: ExpressionHandle,
        ) -> HashSet<ExpressionHandle> {
            let mut reached = HashSet::default();
            reaches(program, root, ExpressionHandle::invalid(), &mut reached);
            reached
        }

        fn reaches(
            program: &dyn crate::CheckingInput,
            root: ExpressionHandle,
            target: ExpressionHandle,
            visited: &mut HashSet<ExpressionHandle>,
        ) -> bool {
            program.subtree_any(root, typed_trees::SubtreeEdges::VALUES, |expression, _| {
                if !visited.insert(expression) {
                    typed_trees::SubtreeVisit::Prune
                } else if expression == target {
                    typed_trees::SubtreeVisit::Found
                } else {
                    typed_trees::SubtreeVisit::Descend
                }
            })
        }
    }
    pub use reachability::{expression_contains, reachable_expressions};

    pub use call_targets::{
        checked_expression_type_reference_from_exact_owner,
        checked_machine_call_target_from_exact_owner, checked_member_target_from_exact_owner,
    };
    use checked_trees::ContractProofFact;
    use indexing::build_owner_environment_index;
    pub use indexing::proof_fact_contains_expression;
    use language_semantics::declaration_selection::CollectionMeasure;
    pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::TypeReferenceHandle;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum OwnerMemberTarget {
        Declaration(SymbolHandle),
        CollectionMeasure(CollectionMeasure),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum InferredType {
        TypeReference(TypeReferenceHandle),
        Nominal(SymbolHandle),
        CollectionView(TypeReferenceHandle),
        CompilerString,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct Binding {
        name: String,
        symbol: SymbolHandle,
        type_reference: TypeReferenceHandle,
    }

    #[derive(Debug, Clone, Default)]
    pub(crate) struct TypeEnvironment {
        owner: SymbolHandle,
        trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
        self_type: Option<InferredType>,
        result_type: Option<TypeReferenceHandle>,
        bindings: Vec<Binding>,
    }

    #[derive(Default)]
    pub(crate) struct ExactOwnerEntry {
        environments: Vec<usize>,
        executable_sites: Vec<ExecutableSite>,
        contract_facts: Vec<arena::Handle<ContractProofFact>>,
    }

    #[derive(Default)]
    pub(crate) struct OwnerIndex {
        environments: Vec<TypeEnvironment>,
        entries: HashMap<ExpressionHandle, ExactOwnerEntry>,
    }

    impl OwnerIndex {
        fn intern(&mut self, environment: TypeEnvironment) -> usize {
            self.environments.push(environment);
            self.environments.len() - 1
        }

        fn add(&mut self, expression: ExpressionHandle, environment: usize) {
            let rows = &mut self.entries.entry(expression).or_default().environments;
            if rows.last() != Some(&environment) {
                rows.push(environment);
            }
        }

        fn add_all(
            &mut self,
            expressions: impl IntoIterator<Item = ExpressionHandle>,
            environment: impl FnOnce() -> TypeEnvironment,
        ) {
            let mut expressions = expressions.into_iter().peekable();
            if expressions.peek().is_none() {
                return;
            }
            let environment = self.intern(environment());
            for expression in expressions {
                self.add(expression, environment);
            }
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ExecutableSite {
        machine: usize,
        state: usize,
        statement: usize,
    }

    pub struct OwnerEnvironmentIndex {
        index: OwnerIndex,
        program_only: std::sync::OnceLock<Box<OwnerEnvironmentIndex>>,
    }

    impl OwnerEnvironmentIndex {
        /// Lexical environments for retained contracts before checking publishes
        /// proof facts. These roots identify binders, never checked evidence.
        pub fn for_preparation(program: &dyn crate::CheckingInput) -> Self {
            let mut owners = Self::new(program, &arena::Arena::default());
            indexing::index_contract_environments(program, &mut owners.index);
            owners
        }

        pub fn new(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<ContractProofFact>,
        ) -> Self {
            Self::with_call_signatures(program, contract_facts, &arena::Arena::default())
        }

        /// Checked invocation signatures retain their complete applied parameter
        /// environment. Preparation uses `for_preparation` to retain source typing
        /// without checked-contract authority.
        pub fn with_call_signatures(
            program: &dyn crate::CheckingInput,
            contract_facts: &arena::Arena<ContractProofFact>,
            call_signatures: &arena::Arena<checked_trees::CallContractSignature>,
        ) -> Self {
            Self {
                index: build_owner_environment_index(program, contract_facts, call_signatures),
                program_only: std::sync::OnceLock::new(),
            }
        }

        pub fn program_only(&self, program: &dyn crate::CheckingInput) -> &OwnerEnvironmentIndex {
            self.program_only
                .get_or_init(|| Box::new(Self::new(program, &arena::Arena::default())))
        }

        pub fn containing_statement_sites(
            &self,
            expression: ExpressionHandle,
        ) -> Vec<(usize, usize, usize)> {
            self.executable_sites(expression)
                .into_iter()
                .map(|site| (site.machine, site.state, site.statement))
                .collect()
        }

        pub fn contains_expression(&self, expression: ExpressionHandle) -> bool {
            self.index.entries.contains_key(&expression)
        }

        fn environments(&self, expression: ExpressionHandle) -> Option<Vec<TypeEnvironment>> {
            let index = &self.index;
            index
                .entries
                .get(&expression)
                .map(|entry| {
                    entry
                        .environments
                        .iter()
                        .map(|environment| index.environments[*environment].clone())
                        .collect::<Vec<_>>()
                })
                .filter(|environments| !environments.is_empty())
        }

        fn executable_sites(&self, expression: ExpressionHandle) -> Vec<ExecutableSite> {
            self.index
                .entries
                .get(&expression)
                .map(|entry| entry.executable_sites.clone())
                .unwrap_or_default()
        }

        pub fn containing_contract_facts(
            &self,
            expression: ExpressionHandle,
        ) -> Vec<arena::Handle<ContractProofFact>> {
            self.index
                .entries
                .get(&expression)
                .map(|entry| entry.contract_facts.clone())
                .unwrap_or_default()
        }
    }

    pub fn source_declaration_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        owners: &OwnerEnvironmentIndex,
    ) -> Option<SymbolHandle> {
        let mut owner =
            crate::proof_contracts::contract_results::source_contract_declaration_owner(
                program, expression,
            )
            .ok()?;
        for (machine, _, _) in owners.containing_statement_sites(expression) {
            let candidate = program.machines().get(machine)?.symbol;
            if !candidate.is_valid() || (owner.is_valid() && owner != candidate) {
                return None;
            }
            owner = candidate;
        }
        owner.is_valid().then_some(owner)
    }

    #[cfg(test)]
    pub(crate) mod index_tests {
        use super::*;
        use arena::{Arena, Handle};
        use checked_trees::{ContractProofFact, ContractProofFactOwner};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            TypedTrees,
            constant::ConstDeclaration,
            data::DataDefinition,
            domain::ProofFact,
            expression::{CallExpression, Expression, ExpressionNode, NamePath},
            machine::Machine,
            name::Identifier,
            signature::StateParameter,
            state::State,
            statement::StatementNode,
            types::TypeReferenceNode,
        };

        fn call(program: &mut TypedTrees) -> ExpressionHandle {
            program
                .expression_table
                .insert_tree(&Expression::Call(Box::new(CallExpression {
                    authored_call_selection: None,
                    authored_call_binding: None,
                    exact_coercion_target: Default::default(),
                    requirement_application: Default::default(),
                    receiver: Some(Box::new(Expression::Name(NamePath::unresolved(vec![
                        Identifier::from("x"),
                    ])))),
                    target_symbol: SymbolHandle::invalid(),
                    target: Identifier::from("go"),
                    arguments: Vec::new().into(),
                    evidence_arguments: Vec::new().into(),
                    operational_acknowledgement: Default::default(),
                })))
        }

        fn fixture(
            flip: bool,
            second_parameter: bool,
        ) -> (
            TypedTrees,
            ExpressionHandle,
            ExpressionHandle,
            [SymbolHandle; 2],
            [SymbolHandle; 2],
        ) {
            let mut symbols = SymbolTableBuilder::default();
            let a = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("A"));
            let b = symbols.insert_root(SymbolKind::Data, SymbolNameRef::Static("B"));
            let first = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("First"));
            let sa = symbols
                .insert_children(first, [(SymbolKind::State, SymbolNameRef::Static("go"))])
                .handle_at(0);
            let pa = symbols
                .insert_children(sa, [(SymbolKind::Parameter, SymbolNameRef::Static("x"))])
                .handle_at(0);
            let second = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("Second"));
            let sb = symbols
                .insert_children(second, [(SymbolKind::State, SymbolNameRef::Static("go"))])
                .handle_at(0);
            let pb = symbols
                .insert_children(sb, [(SymbolKind::Parameter, SymbolNameRef::Static("x"))])
                .handle_at(0);
            let constant = symbols.insert_root(SymbolKind::Const, SymbolNameRef::Static("K"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let ta = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: a,
                    name: Identifier::from("A"),
                });
            let tb = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: b,
                    name: Identifier::from("B"),
                });
            let expression = call(&mut program);
            let repeated = program
                .expression_table
                .insert_expression_handles([expression, expression]);
            let root = program
                .expression_table
                .insert(ExpressionNode::ArrayLiteral(repeated));
            let statements = program
                .statement_table
                .insert_statements([StatementNode::Expression(root)]);
            let mut machine = Machine {
                symbol: first,
                attached_data_symbol: a,
                ..Default::default()
            };
            let mut state = State {
                symbol: sa,
                name: Identifier::from("go"),
                statement_nodes: statements,
                ..Default::default()
            };
            program.push_state_parameter(
                &mut state,
                StateParameter {
                    symbol: pa,
                    name: Identifier::from("x"),
                    type_reference: if flip { tb } else { ta },
                    ..Default::default()
                },
            );
            program.push_machine_state(&mut machine, state);
            program.push_machine(machine);
            let mut machine = Machine {
                symbol: second,
                attached_data_symbol: b,
                ..Default::default()
            };
            let mut state = State {
                symbol: sb,
                name: Identifier::from("go"),
                ..Default::default()
            };
            if second_parameter {
                program.push_state_parameter(
                    &mut state,
                    StateParameter {
                        symbol: pb,
                        name: Identifier::from("x"),
                        type_reference: tb,
                        ..Default::default()
                    },
                );
            }
            program.push_machine_state(&mut machine, state);
            program.push_machine(machine);
            let marker = program
                .expression_table
                .insert(ExpressionNode::Boolean(true));
            let requires_facts = program
                .proof_facts
                .insert_many([ProofFact::Expression(marker), ProofFact::Expression(marker)]);
            program.push_data_definition(DataDefinition {
                symbol: a,
                name: Identifier::from("A"),
                requires_facts,
                ..Default::default()
            });
            program.push_data_definition(DataDefinition {
                symbol: b,
                name: Identifier::from("B"),
                ..Default::default()
            });
            program.push_const_declaration(ConstDeclaration {
                symbol: constant,
                authored_initializer: marker,
                materialized_initializer: marker,
                ..Default::default()
            });
            (program, expression, marker, [first, second], [sa, sb])
        }

        fn contracts(
            program: &mut TypedTrees,
            expression: ExpressionHandle,
            owners: impl IntoIterator<Item = ContractProofFactOwner>,
        ) -> Arena<ContractProofFact> {
            let fact = program
                .proof_facts
                .append(ProofFact::Expression(expression));
            let mut rows = Arena::default();
            for owner in owners {
                rows.append(ContractProofFact {
                    owner,
                    fact,
                    ..Default::default()
                });
            }
            rows
        }

        pub(crate) type Row = (
            SymbolHandle,
            Option<InferredType>,
            Option<TypeReferenceHandle>,
            Vec<(String, SymbolHandle, TypeReferenceHandle)>,
        );
        fn roster(environments: Option<Vec<TypeEnvironment>>) -> Option<Vec<Row>> {
            environments.map(|rows| {
                rows.into_iter()
                    .map(|row| {
                        (
                            row.owner,
                            row.self_type,
                            row.result_type,
                            row.bindings
                                .into_iter()
                                .map(|b| (b.name, b.symbol, b.type_reference))
                                .collect(),
                        )
                    })
                    .collect()
            })
        }

        fn compare(
            program: &TypedTrees,
            facts: &Arena<ContractProofFact>,
            expressions: &[ExpressionHandle],
        ) {
            let index = OwnerEnvironmentIndex::new(program, facts);
            for &expression in expressions {
                assert_eq!(
                    roster(index.environments(expression)),
                    roster(environments::exact_owner_environments(
                        program, facts, expression, None
                    ))
                );
                if let Some(call) = program.expression_table.call_node(expression) {
                    assert_eq!(
                        checked_machine_call_target_from_exact_owner(
                            program,
                            facts,
                            expression,
                            call,
                            Some(&index)
                        ),
                        checked_machine_call_target_from_exact_owner(
                            program, facts, expression, call, None
                        )
                    );
                }
                let mut original = Vec::new();
                for (m, machine) in program.machines().iter().enumerate() {
                    for (s, state) in program.machine_states(machine).iter().enumerate() {
                        for (i, statement) in program
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .enumerate()
                        {
                            let mut reached = Vec::new();
                            crate::queries::expression_traversal::collect_statement_expression_trees(
                        program,
                        statement,
                        &mut reached,
                    );
                            if reached.contains(&expression) {
                                original.push((m, s, i));
                            }
                        }
                    }
                }
                assert_eq!(index.containing_statement_sites(expression), original);
            }
        }

        #[test]
        fn indexed_owners_match_raw_rosters_targets_and_refusals_for_exact_worlds() {
            for flip in [false, true] {
                let (mut program, expression, marker, machines, targets) = fixture(flip, true);
                let owner = ContractProofFactOwner::Machine {
                    machine_symbol: machines[0],
                };
                let equal_owners = contracts(&mut program, expression, [owner, owner]);
                let detached = call(&mut program);
                let other_generation =
                    Handle::from_parts(expression.arena_index(), expression.generation() + 1);
                compare(
                    &program,
                    &equal_owners,
                    &[
                        expression,
                        marker,
                        detached,
                        other_generation,
                        Handle::invalid(),
                    ],
                );
                let index = OwnerEnvironmentIndex::new(&program, &equal_owners);
                assert_eq!(index.environments(expression).unwrap().len(), 3);
                assert_eq!(index.environments(marker).unwrap().len(), 2);
                assert_eq!(
                    index.containing_statement_sites(expression),
                    vec![(0, 0, 0)]
                );
                let node = program.expression_table.call_node(expression).unwrap();
                assert_eq!(
                    checked_machine_call_target_from_exact_owner(
                        &program,
                        &equal_owners,
                        expression,
                        node,
                        Some(&index)
                    ),
                    Some(targets[usize::from(flip)])
                );
                drop(index);
                let conflicting = contracts(
                    &mut program,
                    expression,
                    [
                        owner,
                        ContractProofFactOwner::Machine {
                            machine_symbol: machines[1],
                        },
                    ],
                );
                compare(&program, &conflicting, &[expression, marker]);
                if !flip {
                    let node = program.expression_table.call_node(expression).unwrap();
                    assert_eq!(
                        checked_machine_call_target_from_exact_owner(
                            &program,
                            &conflicting,
                            expression,
                            node,
                            None
                        ),
                        None
                    );
                }
                let signatures = contracts(
                    &mut program,
                    detached,
                    [ContractProofFactOwner::CallSignature {
                        owner_symbol: machines[0],
                        state_symbol: targets[0],
                        application: Handle::from_parts(1, 1),
                    }],
                );
                compare(&program, &signatures, &[detached]);
                let node = program.expression_table.call_node(detached).unwrap();
                assert_eq!(
                    checked_machine_call_target_from_exact_owner(
                        &program,
                        &signatures,
                        detached,
                        node,
                        None
                    ),
                    None
                );
            }
            let (mut program, expression, marker, machines, targets) = fixture(false, false);
            let incomplete = contracts(
                &mut program,
                expression,
                [ContractProofFactOwner::Machine {
                    machine_symbol: machines[1],
                }],
            );
            compare(&program, &incomplete, &[expression, marker]);
            let node = program.expression_table.call_node(expression).unwrap();
            assert_eq!(
                checked_machine_call_target_from_exact_owner(
                    &program,
                    &incomplete,
                    expression,
                    node,
                    None
                ),
                Some(targets[0])
            );
        }
    }
}

pub mod immutable_integer_bounds {
    use crate::frozen_program::expression_table;
    use symbols::SymbolHandle;
    use symbols::SymbolKeyMap as HashMap;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::{StatementHandle, StatementNode, TableLocalData};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    #[derive(Clone)]
    pub struct ImmutableBoundLookup<'program> {
        program: &'program dyn crate::CheckingInput,
        maps: std::sync::OnceLock<std::sync::Arc<BoundMaps>>,
    }

    pub(crate) struct BoundMaps {
        locals_by_symbol: HashMap<SymbolHandle, Option<StatementHandle>>,
        locals_by_name: HashMap<Box<str>, Option<StatementHandle>>,
        parameters_by_symbol: HashMap<SymbolHandle, Option<ParameterPosition>>,
        machine_self_parameters: HashMap<SymbolHandle, ParameterPosition>,
        data_by_symbol: HashMap<SymbolHandle, usize>,
        machines_by_symbol: HashMap<SymbolHandle, usize>,
        data_by_name: HashMap<Box<str>, usize>,
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ParameterPosition {
        machine: u32,
        state: u32,
        parameter: u32,
    }

    impl BoundMaps {
        fn build(program: &dyn crate::CheckingInput) -> Self {
            let mut maps = BoundMaps {
                locals_by_symbol: HashMap::default(),
                locals_by_name: HashMap::default(),
                parameters_by_symbol: HashMap::default(),
                machine_self_parameters: HashMap::default(),
                data_by_symbol: HashMap::default(),
                machines_by_symbol: HashMap::default(),
                data_by_name: HashMap::default(),
            };
            for (position, definition) in program.data_definitions().iter().enumerate() {
                maps.data_by_symbol
                    .entry(definition.symbol)
                    .or_insert(position);
                if !maps.data_by_name.contains_key(definition.name.as_str()) {
                    maps.data_by_name
                        .insert(definition.name.as_str().into(), position);
                }
            }
            for (position, machine) in program.machines().iter().enumerate() {
                maps.machines_by_symbol
                    .entry(machine.symbol)
                    .or_insert(position);
            }
            for (machine_position, machine) in program.machines().iter().enumerate() {
                for (state_position, state) in program.machine_states(machine).iter().enumerate() {
                    for (parameter_position, parameter) in
                        program.state_parameters(state).iter().enumerate()
                    {
                        let position = ParameterPosition {
                            machine: machine_position as u32,
                            state: state_position as u32,
                            parameter: parameter_position as u32,
                        };
                        maps.parameters_by_symbol
                            .entry(parameter.symbol)
                            .and_modify(|entry| *entry = None)
                            .or_insert(Some(position));
                    }
                    for (handle, statement) in program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                    {
                        let Some(local) = statement.as_local_data() else {
                            continue;
                        };
                        maps.locals_by_symbol
                            .entry(local.symbol)
                            .and_modify(|entry| *entry = None)
                            .or_insert(Some(handle));
                        match maps.locals_by_name.get_mut(local.name.as_str()) {
                            Some(entry) => *entry = None,
                            None => {
                                maps.locals_by_name
                                    .insert(local.name.as_str().into(), Some(handle));
                            }
                        }
                    }
                }
            }
            maps
        }
    }

    impl<'program> ImmutableBoundLookup<'program> {
        pub fn new(program: &'program dyn crate::CheckingInput) -> Self {
            ImmutableBoundLookup {
                program,
                maps: std::sync::OnceLock::new(),
            }
        }

        fn maps(&self) -> &BoundMaps {
            self.maps.get_or_init(|| {
                crate::frozen_program::frozen_memo(
                    self.program,
                    |memos| &memos.bound_maps,
                    || BoundMaps::build(self.program),
                )
            })
        }

        pub fn immutable_binding_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
            match self.local_entry(symbol) {
                Some(Some(local)) if !local.is_mutable => Some(local.type_reference),
                Some(_) => None,
                None => match self.parameter_entry(symbol) {
                    Some(Some(parameter)) if !parameter.is_mutable => {
                        Some(parameter.type_reference)
                    }
                    Some(_) => None,
                    None if self.program.symbols.get(symbol).kind == symbols::SymbolKind::Const => {
                        crate::declaration_type_reference(self.program, symbol)
                    }
                    None => None,
                },
            }
        }

        fn local_at(&self, handle: StatementHandle) -> &'program TableLocalData {
            let program: &'program dyn crate::CheckingInput = self.program;
            match program.statement_table.statement(handle) {
                StatementNode::LocalData(local) => local,
                _ => unreachable!("the bound index records local-data statements only"),
            }
        }

        fn parameter_at(&self, position: ParameterPosition) -> &'program StateParameter {
            let program: &'program dyn crate::CheckingInput = self.program;
            let machine = &program.machines()[position.machine as usize];
            let state = &program.machine_states(machine)[position.state as usize];
            &program.state_parameters(state)[position.parameter as usize]
        }

        fn local_entry(&self, symbol: SymbolHandle) -> Option<Option<&'program TableLocalData>> {
            let entry = *self.maps().locals_by_symbol.get(&symbol)?;
            Some(entry.map(|handle| self.local_at(handle)))
        }

        fn local_named(&self, name: &str) -> Option<Option<&'program TableLocalData>> {
            let entry = *self.maps().locals_by_name.get(name)?;
            Some(entry.map(|handle| self.local_at(handle)))
        }

        fn parameter_entry(
            &self,
            symbol: SymbolHandle,
        ) -> Option<Option<&'program StateParameter>> {
            let entry = *self.maps().parameters_by_symbol.get(&symbol)?;
            Some(entry.map(|position| self.parameter_at(position)))
        }

        fn machine_self_parameter(&self, symbol: SymbolHandle) -> Option<&'program StateParameter> {
            let position = *self.maps().machine_self_parameters.get(&symbol)?;
            Some(self.parameter_at(position))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImmutableIntegerBoundOffset {
        pub symbol: SymbolHandle,
        pub offset: i64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ImmutableIntegerBoundSum {
        pub first: SymbolHandle,
        pub second: SymbolHandle,
        pub offset: i64,
    }

    pub(crate) enum LocalLookup<'program> {
        Missing,
        Found(&'program TableLocalData),
        Invalid,
    }

    pub(crate) enum NormalizedBound {
        Expression(ExpressionHandle),
        LocalValue(SymbolHandle),
        MutableValue,
    }

    pub fn normalize_immutable_integer_bound_expression(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        match normalize_bound(program, lookup, expression, &mut Vec::new())? {
            NormalizedBound::Expression(expression) => Some(expression),
            NormalizedBound::LocalValue(_) | NormalizedBound::MutableValue => None,
        }
    }

    pub fn immutable_integer_bound_value_symbol(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        match normalize_bound(program, lookup, expression, &mut Vec::new())? {
            NormalizedBound::LocalValue(symbol) => Some(symbol),
            NormalizedBound::Expression(_) | NormalizedBound::MutableValue => None,
        }
    }

    pub fn immutable_integer_bound_symbol_offset(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<ImmutableIntegerBoundOffset> {
        let mut terms = BoundSumTerms::default();
        collect_bound_terms(program, lookup, expression, &mut terms, 0)?;
        let (Some(symbol), None) = (terms.first, terms.second) else {
            return None;
        };
        Some(ImmutableIntegerBoundOffset {
            symbol,
            offset: terms.offset,
        })
    }

    pub fn immutable_integer_bound_sum(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<ImmutableIntegerBoundSum> {
        let mut terms = BoundSumTerms::default();
        collect_bound_terms(program, lookup, expression, &mut terms, 0)?;
        let (Some(mut first), Some(mut second)) = (terms.first, terms.second) else {
            return None;
        };
        if (first.arena_index(), first.generation()) > (second.arena_index(), second.generation()) {
            std::mem::swap(&mut first, &mut second);
        }
        Some(ImmutableIntegerBoundSum {
            first,
            second,
            offset: terms.offset,
        })
    }

    #[derive(Default, Clone)]
    pub(crate) struct BoundSumTerms {
        first: Option<SymbolHandle>,
        second: Option<SymbolHandle>,
        offset: i64,
    }

    fn exact_integer_type_primitive(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        crate::value_custody::type_shapes::exact_integer_primitive(program, type_reference, false)
    }

    fn collect_bound_terms(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
        terms: &mut BoundSumTerms,
        depth: usize,
    ) -> Option<()> {
        if !expression.is_valid() || depth >= 128 {
            return None;
        }
        if let Some(value) = expression_table(program).constant_integer_value(expression) {
            terms.offset = terms.offset.checked_add(value)?;
            return Some(());
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Binary(binary) => match binary.operator {
                typed_trees::expression::BinaryOperator::Add => {
                    collect_bound_terms(program, lookup, binary.left, terms, depth + 1)?;
                    collect_bound_terms(program, lookup, binary.right, terms, depth + 1)
                }
                typed_trees::expression::BinaryOperator::Subtract => {
                    collect_bound_terms(program, lookup, binary.left, terms, depth + 1)?;
                    let value = expression_table(program).constant_integer_value(binary.right)?;
                    terms.offset = terms.offset.checked_sub(value)?;
                    Some(())
                }
                _ => None,
            },
            _ => {
                let symbol = bound_leaf_symbol(program, lookup, expression)?;
                if let Some(initializer) = bound_leaf_initializer(program, lookup, symbol) {
                    let mut expanded = terms.clone();
                    if collect_bound_terms(program, lookup, initializer, &mut expanded, depth + 1)
                        .is_some()
                    {
                        *terms = expanded;
                        return Some(());
                    }
                }
                if terms.first == Some(symbol) || terms.second == Some(symbol) {
                    return None;
                }
                if terms.first.is_none() {
                    terms.first = Some(symbol);
                } else if terms.second.is_none() {
                    terms.second = Some(symbol);
                } else {
                    return None;
                }
                Some(())
            }
        }
    }

    fn bound_leaf_initializer(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        symbol: SymbolHandle,
    ) -> Option<ExpressionHandle> {
        let LocalLookup::Found(local) = local_by_symbol(lookup, symbol) else {
            return None;
        };
        if local.is_mutable || local.ordinary_value_initializer().is_none() {
            return None;
        }
        let binary = expression_table(program).binary_node(local.initial_value)?;
        let state_symbol = program.symbols.get(local.symbol).parent;
        let (machine, state) = crate::find_state(program, state_symbol)?;
        if !crate::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            local.initial_value,
        ) {
            return None;
        }
        matches!(
            binary.operator,
            typed_trees::expression::BinaryOperator::Add
                | typed_trees::expression::BinaryOperator::Subtract
        )
        .then_some(local.initial_value)
    }

    fn bound_leaf_symbol(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        base: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let path = expression_table(program).name_path_node(base)?;
        let members = expression_table(program).name_path_members(path.members);
        if members.len() != 1 {
            return None;
        }

        let type_reference = if path.symbol.is_valid() && path.head_symbol == path.symbol {
            match local_by_symbol(lookup, path.symbol) {
                LocalLookup::Found(local) => local.type_reference,
                LocalLookup::Missing => parameter_by_symbol(lookup, path.symbol)?.type_reference,
                LocalLookup::Invalid => return None,
            }
        } else {
            match local_by_name(lookup, members[0].as_str()) {
                LocalLookup::Found(local) => local.type_reference,
                LocalLookup::Missing | LocalLookup::Invalid => return None,
            }
        };
        exact_integer_type_primitive(program, type_reference)?;

        match normalize_bound(program, lookup, base, &mut Vec::new())? {
            NormalizedBound::LocalValue(symbol) => Some(symbol),
            NormalizedBound::Expression(expression) => {
                let path = expression_table(program).name_path_node(expression)?;
                let members = expression_table(program).name_path_members(path.members);
                (members.len() == 1 && path.symbol.is_valid() && path.head_symbol == path.symbol)
                    .then_some(path.symbol)
            }
            NormalizedBound::MutableValue => None,
        }
    }

    pub fn mutable_integer_bound_storage_symbol(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<SymbolHandle> {
        let path = expression_table(program).name_path_node(expression)?;
        let members = expression_table(program).name_path_members(path.members);
        let (symbol, is_mutable, type_reference) = single_segment_receiver(lookup, path, members)?;
        if !is_mutable || !symbol.is_valid() {
            return None;
        }
        exact_integer_type_primitive(program, type_reference)?;
        Some(symbol)
    }

    fn projected_integer_bound_field<'program>(
        program: &'program dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, &'program typed_trees::data::DataField, bool)> {
        let member = expression_table(program).member_node(expression)?;
        if member.case_variant.is_some() {
            return None;
        }
        let path = expression_table(program).name_path_node(member.receiver)?;
        let members = expression_table(program).name_path_members(path.members);
        let (symbol, is_mutable, type_reference) = single_segment_receiver(lookup, path, members)?;
        if !symbol.is_valid() {
            return None;
        }
        let receiver =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
        let typed_trees::types::TypeReferenceNode::Named {
            symbol: type_symbol,
            ..
        } = program.type_reference_table.type_reference(receiver)
        else {
            return None;
        };
        if !type_symbol.is_valid() {
            return None;
        }
        let (data, authored_matches_field) = match program.symbols.get(*type_symbol).kind {
            symbols::SymbolKind::Data => (
                lookup
                    .maps()
                    .data_by_symbol
                    .get(type_symbol)
                    .map(|position| &program.data_definitions()[*position])?,
                true,
            ),
            symbols::SymbolKind::Machine => {
                let machine = lookup
                    .maps()
                    .machines_by_symbol
                    .get(type_symbol)
                    .map(|position| &program.machines()[*position])?;
                let attached = machine.attached_data.as_deref()?;
                (
                    lookup
                        .maps()
                        .data_by_name
                        .get(attached)
                        .map(|position| &program.data_definitions()[*position])?,
                    false,
                )
            }
            _ => return None,
        };
        let field = crate::value_custody::places::exact_data_member_field(
            program,
            data,
            if authored_matches_field {
                member.member_symbol
            } else {
                SymbolHandle::invalid()
            },
            member.member.as_str(),
            None,
        )?;
        Some((symbol, field, is_mutable))
    }

    pub fn projected_integer_bound_root(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, SymbolHandle, bool)> {
        let (symbol, field, is_mutable) =
            projected_integer_bound_field(program, lookup, expression)?;
        let primitive = program
            .type_reference_table
            .primitive_type(field.type_reference)?;
        if !primitive.is_fixed_integer()
            || program.arithmetic_domain_for_type_reference(field.type_reference)
                != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return None;
        }
        Some((symbol, field.symbol, is_mutable))
    }

    pub fn indexed_integer_bound_root(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>, bool)> {
        let indexed = expression_table(program).indexed_node(expression)?;
        let (symbol, mut segments, is_mutable, type_reference) =
            match expression_table(program).expression(indexed.collection) {
                ExpressionNode::Name(path) => {
                    let members = expression_table(program).name_path_members(path.members);
                    if members.len() != 1 {
                        return None;
                    }
                    let (symbol, is_mutable, type_reference) =
                        if path.symbol.is_valid() && path.head_symbol == path.symbol {
                            bound_name_receiver(lookup, path)?
                        } else {
                            match local_by_name(lookup, members[0].as_str()) {
                                LocalLookup::Found(local) => {
                                    (local.symbol, local.is_mutable, local.type_reference)
                                }
                                LocalLookup::Missing | LocalLookup::Invalid => return None,
                            }
                        };
                    (symbol, Vec::new(), is_mutable, type_reference)
                }
                ExpressionNode::Member(_) => {
                    let (symbol, field, is_mutable) =
                        projected_integer_bound_field(program, lookup, indexed.collection)?;
                    (
                        symbol,
                        vec![facts::PlaceSegment::Field {
                            symbol: field.symbol,
                        }],
                        is_mutable,
                        field.type_reference,
                    )
                }
                _ => return None,
            };
        if !symbol.is_valid() {
            return None;
        }
        let index = expression_table(program)
            .constant_integer_value(indexed.index)
            .and_then(|value| usize::try_from(value).ok())?;
        segments.push(facts::PlaceSegment::FixedIndex { index });
        let receiver =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
        let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(receiver)
        else {
            return None;
        };
        let primitive = program.type_reference_table.primitive_type(*element_type)?;
        if !primitive.is_fixed_integer()
            || program.arithmetic_domain_for_type_reference(*element_type)
                != numerics::arithmetic::ArithmeticDomain::Exact
        {
            return None;
        }
        Some((symbol, segments, is_mutable))
    }

    pub fn projected_integer_bound_subject_type(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        projected_integer_bound_field(program, lookup, expression)
            .map(|(_, field, _)| field.type_reference)
    }

    pub fn normalize_immutable_integer_bound_to_usize(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<usize> {
        let expression = normalize_immutable_integer_bound_expression(program, lookup, expression)?;
        expression_table(program)
            .constant_integer_value(expression)
            .and_then(|value| usize::try_from(value).ok())
    }

    fn normalize_bound(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
        seen_aliases: &mut Vec<SymbolHandle>,
    ) -> Option<NormalizedBound> {
        if !expression.is_valid() {
            return None;
        }

        match expression_table(program).expression(expression) {
            ExpressionNode::Integer(_) => Some(NormalizedBound::Expression(expression)),
            ExpressionNode::Name(path) => {
                let members = expression_table(program).name_path_members(path.members);
                if members.len() != 1 {
                    return None;
                }
                if path.symbol.is_valid() && path.head_symbol == path.symbol {
                    match local_by_symbol(lookup, path.symbol) {
                        LocalLookup::Missing => {
                            if parameter_mutability(lookup, path.symbol)? {
                                Some(NormalizedBound::MutableValue)
                            } else {
                                Some(NormalizedBound::Expression(expression))
                            }
                        }
                        LocalLookup::Found(local) => {
                            normalize_local(program, lookup, local, seen_aliases)
                        }
                        LocalLookup::Invalid => None,
                    }
                } else {
                    match local_by_name(lookup, members[0].as_str()) {
                        LocalLookup::Found(local) => {
                            normalize_local(program, lookup, local, seen_aliases)
                        }
                        LocalLookup::Missing | LocalLookup::Invalid => None,
                    }
                }
            }
            _ => None,
        }
    }

    fn normalize_local(
        program: &dyn crate::CheckingInput,
        lookup: &ImmutableBoundLookup<'_>,
        local: &TableLocalData,
        seen_aliases: &mut Vec<SymbolHandle>,
    ) -> Option<NormalizedBound> {
        if !local.symbol.is_valid() || seen_aliases.contains(&local.symbol) {
            return None;
        }
        if local.is_mutable {
            return Some(NormalizedBound::MutableValue);
        }
        if !local.role.has_ordinary_initializer() {
            return Some(NormalizedBound::LocalValue(local.symbol));
        }
        seen_aliases.push(local.symbol);
        let normalized = match expression_table(program).expression(local.initial_value) {
            ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
                normalize_bound(program, lookup, local.initial_value, seen_aliases).map(|bound| {
                    match bound {
                        NormalizedBound::MutableValue => NormalizedBound::LocalValue(local.symbol),
                        bound => bound,
                    }
                })
            }
            _ if local.initial_value.is_valid() => Some(NormalizedBound::LocalValue(local.symbol)),
            _ => None,
        };
        seen_aliases.pop();
        normalized
    }

    fn single_segment_receiver(
        lookup: &ImmutableBoundLookup<'_>,
        path: &typed_trees::expression::TableNamePath,
        members: &[typed_trees::name::Identifier],
    ) -> Option<(SymbolHandle, bool, typed_trees::types::TypeReferenceHandle)> {
        if members.len() != 1 {
            return None;
        }
        if path.symbol.is_valid() && path.head_symbol == path.symbol {
            bound_name_receiver(lookup, path)
        } else {
            match local_by_name(lookup, members[0].as_str()) {
                LocalLookup::Found(local) => {
                    Some((local.symbol, local.is_mutable, local.type_reference))
                }
                LocalLookup::Missing | LocalLookup::Invalid => None,
            }
        }
    }

    fn bound_name_receiver(
        lookup: &ImmutableBoundLookup<'_>,
        path: &typed_trees::expression::TableNamePath,
    ) -> Option<(SymbolHandle, bool, typed_trees::types::TypeReferenceHandle)> {
        match local_by_symbol(lookup, path.symbol) {
            LocalLookup::Found(local) => {
                Some((local.symbol, local.is_mutable, local.type_reference))
            }
            LocalLookup::Missing => {
                if let Some(parameter) = parameter_by_symbol(lookup, path.symbol) {
                    Some((path.symbol, parameter.is_mutable, parameter.type_reference))
                } else {
                    let parameter = lookup.machine_self_parameter(path.symbol)?;
                    Some((path.symbol, parameter.is_mutable, parameter.type_reference))
                }
            }
            LocalLookup::Invalid => None,
        }
    }

    fn local_by_symbol<'program>(
        lookup: &ImmutableBoundLookup<'program>,
        symbol: SymbolHandle,
    ) -> LocalLookup<'program> {
        match lookup.local_entry(symbol) {
            Some(Some(local)) => LocalLookup::Found(local),
            Some(None) => LocalLookup::Invalid,
            None => LocalLookup::Missing,
        }
    }

    fn local_by_name<'program>(
        lookup: &ImmutableBoundLookup<'program>,
        name: &str,
    ) -> LocalLookup<'program> {
        match lookup.local_named(name) {
            Some(Some(local)) => LocalLookup::Found(local),
            Some(None) => LocalLookup::Invalid,
            None => LocalLookup::Missing,
        }
    }

    fn parameter_mutability(
        lookup: &ImmutableBoundLookup<'_>,
        symbol: SymbolHandle,
    ) -> Option<bool> {
        match lookup.parameter_entry(symbol) {
            Some(Some(parameter)) => Some(parameter.is_mutable),
            Some(None) => None,
            None => Some(false),
        }
    }

    fn parameter_by_symbol<'program>(
        lookup: &ImmutableBoundLookup<'program>,
        symbol: SymbolHandle,
    ) -> Option<&'program StateParameter> {
        lookup.parameter_entry(symbol).flatten()
    }
}

pub mod instruction_effects {
    use language_core::inline_assembly::AsmRegisterOperation;
    use symbols::SymbolHandle;
    use typed_trees::proposition::PropositionBody;
    use typed_trees::types::PrimitiveType;

    pub fn instruction_effect_model(
        program: &dyn crate::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<facts::InstructionEffectOperation> {
        let declaration = program.proposition_definition_by_symbol(symbol)?;
        if !declaration.is_public || !matches!(declaration.body, PropositionBody::Primitive) {
            return None;
        }
        use language_core::core_vocabulary::{AssemblyEffect, CoreVocabulary};
        let Some(CoreVocabulary::AssemblyEffect(effect)) = program.symbols.core_vocabulary(symbol)
        else {
            return None;
        };
        if effect == AssemblyEffect::AtomicReleaseWritten {
            use typed_trees::proposition::PropositionBinderKind;
            use typed_trees::types::TypeReferenceNode;
            let [cell, value] = program.proposition_binders(declaration) else {
                return None;
            };
            let [destination, stored] = program.proposition_parameters(declaration) else {
                return None;
            };
            if !matches!(cell.kind, PropositionBinderKind::Type)
                || !matches!(value.kind, PropositionBinderKind::Type)
            {
                return None;
            }
            let TypeReferenceNode::Reference {
                referee,
                access: language_semantics::ReferenceAccess::Shared,
                ..
            } = program
                .type_reference_table
                .type_reference(destination.type_reference)
            else {
                return None;
            };
            if !matches!(program.type_reference_table.type_reference(*referee), TypeReferenceNode::Named { symbol, .. } if *symbol == cell.symbol)
                || !matches!(program.type_reference_table.type_reference(stored.type_reference), TypeReferenceNode::Named { symbol, .. } if *symbol == value.symbol)
            {
                return None;
            }
            return Some(facts::InstructionEffectOperation::AtomicReleaseStore);
        }
        if !program.proposition_binders(declaration).is_empty() {
            return None;
        }
        let operation = match effect {
            AssemblyEffect::PortWritten => AsmRegisterOperation::PortWrite,
            AssemblyEffect::ModelSpecificWritten => AsmRegisterOperation::ModelSpecificWrite,
            AssemblyEffect::AtomicReleaseWritten => return None,
        };
        let expected = operation.input_carriers();
        let parameters = program.proposition_parameters(declaration);
        (parameters.len() == expected.len()
            && parameters
                .iter()
                .zip(expected)
                .all(|(parameter, expected)| {
                    program.primitive_type_reference(parameter.type_reference)
                        == PrimitiveType::from_name(expected.name())
                }))
        .then_some(facts::InstructionEffectOperation::Register(operation))
    }

    pub struct InstructionEffectContract<'program> {
        pub operation: facts::InstructionEffectOperation,
        pub applications: Vec<&'program typed_trees::proposition::PropositionApplication>,
    }
    pub fn instruction_effect_contract<'program>(
        program: &'program dyn crate::CheckingInput,
        application: &'program typed_trees::proposition::PropositionApplication,
    ) -> Option<InstructionEffectContract<'program>> {
        let mut applications = Vec::new();
        let mut current = application;
        loop {
            if applications.iter().any(
                |previous: &&typed_trees::proposition::PropositionApplication| {
                    previous.proposition == current.proposition
                },
            ) {
                return None;
            }
            applications.push(current);
            if let Some(operation) = instruction_effect_model(program, current.proposition) {
                return Some(InstructionEffectContract {
                    operation,
                    applications,
                });
            }
            let declaration = program.proposition_definition_by_symbol(current.proposition)?;
            let PropositionBody::Transparent {
                proposition: typed_trees::proposition::PropositionFormula::Application(next),
            } = &declaration.body
            else {
                return None;
            };
            current = next;
        }
    }
}

pub mod parameter_domains {
    use language_semantics::CarryPermission;
    use language_semantics::SemanticDomainId;
    use symbols::SymbolHandle;
    use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

    pub fn parameter_domain_constraints(
        program: &dyn crate::CheckingInput,
        mut type_reference: TypeReferenceHandle,
        mut substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Vec<(
        SymbolHandle,
        String,
        Vec<TypeReferenceHandle>,
        SemanticDomainId,
    )> {
        let mut domains = Vec::new();
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    domains.extend(
                        program
                            .type_reference_table
                            .constraints(*constraints)
                            .iter()
                            .filter_map(|constraint| match constraint {
                                TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                                    program.domain_definition(domain.symbol).map(|definition| {
                                        (
                                            domain.symbol,
                                            definition.name.as_str().to_owned(),
                                            domain.arguments.clone(),
                                            domain.semantic_id,
                                        )
                                    })
                                }
                                TypeConstraintNode::Domain(domain)
                                    if CarryPermission::from_name(domain.name.as_str())
                                        .is_some() =>
                                {
                                    Some((
                                        SymbolHandle::invalid(),
                                        domain.name.as_str().to_owned(),
                                        Vec::new(),
                                        SemanticDomainId::NULL,
                                    ))
                                }
                                _ => None,
                            }),
                    );
                    type_reference = *base_type;
                }
                TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                    let Some((_, selected)) = substitutions
                        .iter()
                        .rev()
                        .find(|(formal, _)| formal == symbol)
                    else {
                        break;
                    };
                    type_reference = *selected;
                    substitutions = &[];
                }
                _ => {
                    if let Some(storage) = program.layout_storage_arguments(type_reference) {
                        type_reference = storage.semantic_type;
                    } else {
                        break;
                    }
                }
            }
        }
        domains
    }
}

pub mod parameter_expression_meaning {
    use language_core::OperatorSpelling;
    use symbols::SymbolHandle;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;

    pub fn has_builtin_parameter_bound_expression_meaning(
        program: &dyn crate::CheckingInput,
        owner_symbol: SymbolHandle,
        parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> bool {
        // Per arena slot: 1 while on the active path, 2 once completed.
        let mut marks: Vec<u8> = Vec::new();
        let mut pending = vec![(expression, false)];
        while let Some((expression, completed)) = pending.pop() {
            let slot = expression.arena_index() as usize;
            if completed {
                marks[slot] = 2;
                continue;
            }
            if !program.expression_table.expression_is_valid(expression) {
                return false;
            }
            if slot >= marks.len() {
                marks.resize(slot + 1, 0);
            }
            match marks[slot] {
                2 => continue,
                1 => return false,
                _ => {}
            }
            marks[slot] = 1;
            pending.push((expression, true));
            match program.expression_table.expression(expression) {
                ExpressionNode::Binary(binary) => {
                    if program.static_type_predicate_operands(expression).is_some() {
                        continue;
                    }
                    if binary.operator == BinaryOperator::CaseMembership {
                        return false;
                    }
                    if let Some(spelling) = spelled_binary_operator(binary.operator) {
                        let operands = [binary.left, binary.right].map(|operand| {
                            crate::parameter_expression_result_type_reference(
                                program,
                                owner_symbol,
                                parameters,
                                operand,
                            )
                        });
                        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                            program,
                            owner_symbol,
                            expression,
                            spelling,
                            &operands,
                        ) {
                            return false;
                        }
                    }
                    pending.extend([(binary.left, false), (binary.right, false)]);
                }
                ExpressionNode::Unary(unary) => pending.push((unary.operand, false)),
                ExpressionNode::Borrow(borrow) => pending.push((borrow.target, false)),
                ExpressionNode::Cast(cast) => pending.push((cast.value, false)),
                ExpressionNode::Atomic(atomic) => pending.push((atomic.value, false)),
                _ => {}
            }
        }
        true
    }

    pub(crate) fn spelled_binary_operator(operator: BinaryOperator) -> Option<OperatorSpelling> {
        match operator {
            BinaryOperator::Equal => Some(OperatorSpelling::Equal),
            BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
            BinaryOperator::Less => Some(OperatorSpelling::Less),
            BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
            BinaryOperator::Greater => Some(OperatorSpelling::Greater),
            BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
            BinaryOperator::Add => Some(OperatorSpelling::Add),
            BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
            BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
            BinaryOperator::Divide => Some(OperatorSpelling::Divide),
            BinaryOperator::Modulo => Some(OperatorSpelling::Modulo),
            BinaryOperator::CaseMembership
            | BinaryOperator::And
            | BinaryOperator::Or
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight => None,
        }
    }
}

pub mod proof_embeddings {
    pub(crate) type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use diagnostics::Diagnostic;
    use numerics::bignum::BigInt;
    use symbols::BuiltinFunction;
    use symbols::BuiltinType;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
    };
    use typed_trees::proof_only::ProofOnlyClassification;
    use typed_trees::proposition::{PropositionBody, PropositionFormula};
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    use crate::queries::expression_traversal::collect_expression_nodes;

    pub(crate) mod calls {
        use std::collections::HashSet;

        use super::{
            Diagnostic, ExpressionHandle, ExpressionNode, PrimitiveType, TypeReferenceHandle,
            TypeReferenceNode, collect_expression_nodes, expression_type_reference,
            is_exact_embed_call, primitive_range,
        };
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ValidatedIntegerEmbeddingCall {
            pub call_expression: ExpressionHandle,
            pub target_machine: symbols::SymbolHandle,
            pub target_state: symbols::SymbolHandle,
        }

        pub(crate) fn validate_integer_embedding_calls(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> Vec<ValidatedIntegerEmbeddingCall> {
            let mut nodes = Vec::new();
            let mut seen = HashSet::new();
            for (_, expression) in program.expression_table.iter_expressions() {
                if let ExpressionNode::Call(call) = expression
                    && is_exact_embed_call(program, call)
                {
                    for argument in program.expression_table.expression_handles(call.arguments) {
                        collect_expression_nodes(program, *argument, &mut nodes, &mut seen);
                    }
                }
            }
            let calls: Vec<_> = nodes
                .into_iter()
                .filter_map(
                    |expression| match program.expression_table.expression(expression) {
                        ExpressionNode::Call(call) if !is_exact_embed_call(program, call) => {
                            Some((expression, call))
                        }
                        _ => None,
                    },
                )
                .collect();
            if calls.is_empty() {
                return Vec::new();
            }
            let operational = crate::infer_operational_may(program);
            let service_reaches = crate::infer_service_reaches(program, &operational);
            let mut admitted = Vec::new();
            for (expression, call) in calls {
                let reject = |reason: &str, diagnostics: &mut Vec<Diagnostic>| {
                    diagnostics.push(Diagnostic::error(format!(
                        "`embed` source call `{}` is not denotational: {reason}",
                        call.target,
                    )));
                };
                if let Some(dispatch) = &call.static_requirement_dispatch {
                    reject(
                        &format!(
                            "static conformance dispatch selected a private realization `{}`; the public requirement `{}` is the contract, not a direct checked value call",
                            program.symbols.name(dispatch.realization_state),
                            program.symbols.name(dispatch.requirement),
                        ),
                        diagnostics,
                    );
                    continue;
                }
                if call.receiver.is_valid()
                    || !call.selects_only_nominal_route()
                    || !call.carries_only_positional_arguments()
                {
                    reject(
                        "the source must select a direct checked value call",
                        diagnostics,
                    );
                    continue;
                }
                if let Some((machine, state)) =
                    crate::machine_calls::fact_call_projections::validate_checked_call_candidate(
                        program,
                        call,
                        &operational,
                        &service_reaches,
                        &reject,
                        diagnostics,
                    )
                {
                    if program
                        .machine_states(machine)
                        .first()
                        .map(|entry| entry.symbol)
                        != Some(state.symbol)
                    {
                        reject(
                            "the source must select the machine's exact entry state",
                            diagnostics,
                        );
                        continue;
                    }
                    let parameters = program.state_parameters(state);
                    let arguments = program.expression_table.expression_handles(call.arguments);
                    if arguments.len() != parameters.len() {
                        reject(
                            &format!(
                                "expected {} value arguments, got {}",
                                parameters.len(),
                                arguments.len()
                            ),
                            diagnostics,
                        );
                        continue;
                    }
                    let mut arguments_match = true;
                    for (argument, parameter) in arguments.iter().zip(parameters) {
                        if !argument_has_exact_type(program, *argument, parameter.type_reference) {
                            reject(
                                &format!(
                                    "argument `{}` must establish its exact declared type `{}`; implicit specialization or narrowing is not admitted here",
                                    parameter.name,
                                    program.display_type_reference_with_constraints(
                                        parameter.type_reference
                                    ),
                                ),
                                diagnostics,
                            );
                            arguments_match = false;
                        }
                    }
                    if !arguments_match {
                        continue;
                    }
                    admitted.push(ValidatedIntegerEmbeddingCall {
                        call_expression: expression,
                        target_machine: machine.symbol,
                        target_state: state.symbol,
                    });
                }
            }
            admitted
        }

        fn argument_has_exact_type(
            program: &dyn crate::CheckingInput,
            argument: ExpressionHandle,
            expected: TypeReferenceHandle,
        ) -> bool {
            if !crate::value_custody::expression_types::argument_matches_type_reference_handle(
                program, argument, expected,
            ) {
                return false;
            }
            if let Some(actual) = expression_type_reference(program, argument) {
                return program.normalized_type_identity(actual)
                    == program.normalized_type_identity(expected);
            }
            if !matches!(
                program.type_reference_table.type_reference(expected),
                TypeReferenceNode::Named { .. }
            ) {
                return false;
            }
            let Some(primitive) = program.primitive_type_reference(expected) else {
                return false;
            };
            match program.expression_table.expression(argument) {
                ExpressionNode::Integer(literal) => {
                    let Some((minimum, maximum)) = primitive_range(primitive) else {
                        return false;
                    };
                    literal
                        .value_bignum()
                        .is_some_and(|value| value >= minimum && value <= maximum)
                }
                ExpressionNode::Boolean(_) => primitive == PrimitiveType::Bool,
                _ => false,
            }
        }
    }
    pub use calls::ValidatedIntegerEmbeddingCall;
    pub(crate) use calls::validate_integer_embedding_calls;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct IntegerEmbedding {
        pub(crate) primitive: PrimitiveType,
        pub(crate) interval: crate::proof_contracts::arithmetic_domains::BigIntInterval,
    }

    pub(crate) fn validate_proof_embeddings(
        program: &dyn CheckingInput,
        classification: &ProofOnlyClassification,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for machine in program.machines() {
            if machine.name.as_str().rsplit("::").next() == Some("embed") {
                diagnostics.push(Diagnostic::error(
                "`embed` is a compiler-owned proof term and cannot be declared or implemented as a machine",
            ));
            }
        }
        let mut proof_nodes = Vec::new();
        let mut runtime_nodes = Vec::new();
        let mut proof_seen = HashSet::default();
        let mut runtime_seen = HashSet::default();
        macro_rules! nodes {
            ($a:expr => $b:expr => $c:expr => $d:expr) => {
                collect_expression_nodes($a, $b, $c, $d)
            };
        }
        for (_, fact) in typed_trees::references::program_proof_facts(program) {
            match fact {
                ProofFact::Expression(expression) => {
                    collect_expression_nodes(
                        program,
                        *expression,
                        &mut proof_nodes,
                        &mut proof_seen,
                    );
                }
                ProofFact::Membership(membership) => {
                    nodes!(program => membership.value => &mut proof_nodes => &mut proof_seen);
                }
                ProofFact::Proposition(application) => {
                    for argument in
                        expression_table(program).expression_handles(application.arguments)
                    {
                        collect_expression_nodes(
                            program,
                            *argument,
                            &mut proof_nodes,
                            &mut proof_seen,
                        );
                    }
                }
            }
        }
        // Preparing a callable can replace a copied signature while its old
        // expression rows remain allocated. Those contract rows still describe
        // proof contexts. This census grants no fact and runtime uses below
        // remain executable demands, even when the same node occurs in a contract.
        for (_, clause) in program.signature_contracts.iter() {
            for fact in program.proof_facts.span_or_empty(clause.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        collect_expression_nodes(
                            program,
                            *expression,
                            &mut proof_nodes,
                            &mut proof_seen,
                        );
                    }
                    ProofFact::Membership(membership) => {
                        nodes!(program => membership.value => &mut proof_nodes => &mut proof_seen);
                    }
                    ProofFact::Proposition(application) => {
                        for argument in
                            expression_table(program).expression_handles(application.arguments)
                        {
                            nodes!(program => *argument => &mut proof_nodes => &mut proof_seen);
                        }
                    }
                }
            }
        }
        for expression in crate::mathematical_applications::expression_nodes(program) {
            collect_expression_nodes(program, expression, &mut proof_nodes, &mut proof_seen);
        }
        for proposition in program.propositions() {
            if let PropositionBody::Transparent { proposition } = &proposition.body {
                match proposition {
                    PropositionFormula::BooleanExpression(expression) => {
                        nodes!(program => *expression => &mut proof_nodes => &mut proof_seen);
                    }
                    PropositionFormula::Application(application) => {
                        for argument in
                            expression_table(program).expression_handles(application.arguments)
                        {
                            nodes!(program => *argument => &mut proof_nodes => &mut proof_seen);
                        }
                    }
                }
            }
        }
        for machine in program.machines() {
            for data in program.machine_owned_data(machine) {
                nodes!(program => data.initial_value => &mut runtime_nodes => &mut runtime_seen);
            }
            if classification.is_proof_machine(program, machine)
                || crate::mathematical_applications::is_indexed_logical_machine(program, machine)
            {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        collect_statement_nodes(
                            program,
                            statement,
                            &mut proof_nodes,
                            &mut proof_seen,
                        );
                    }
                }
            } else if let Some(expression) =
                crate::value_custody::content_projections::content_projection_body_expression(
                    program, machine,
                )
            {
                collect_expression_nodes(program, expression, &mut proof_nodes, &mut proof_seen);
            } else {
                for state in program.machine_states(machine) {
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let Some(fact) = statement.as_assembly_fact() {
                            for expression in program.tables.proof_fact_expression_roots(fact.fact)
                            {
                                nodes!(program => expression => &mut proof_nodes => &mut proof_seen);
                            }
                        } else {
                            collect_statement_nodes(
                                program,
                                statement,
                                &mut runtime_nodes,
                                &mut runtime_seen,
                            );
                        }
                    }
                }
            }
        }

        for expression in crate::mathematical_applications::erased_initializer_nodes(program).iter()
        {
            proof_seen.insert(*expression);
            runtime_seen.remove(expression);
        }

        for (handle, node) in expression_table(program).iter_expressions() {
            let mathematical = match node {
                ExpressionNode::Name(path) => {
                    program.symbols.builtin_type_member(path.symbol).is_some()
                }
                ExpressionNode::Call(call) => {
                    crate::mathematical_applications::is_mathematical_call(program, call)
                }
                _ => false,
            };
            if mathematical && (!proof_seen.contains(&handle) || runtime_seen.contains(&handle)) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "mathematical terms are proof-only and cannot be used as executable values or statements: `{}` {}",
                        expression_table(program).display_name(handle),
                        if runtime_seen.contains(&handle) {
                            "is demanded by executable code"
                        } else {
                            "has no retained proof context"
                        },
                    ))
                    .with_source_span(expression_table(program).source_span(handle)),
                );
            }
            let ExpressionNode::Call(call) = node else {
                continue;
            };
            let exact = is_exact_embed_call(program, call);
            let names_embed = call.target.as_str().rsplit("::").next() == Some("embed");
            if !exact && !names_embed {
                continue;
            }
            if !exact {
                diagnostics.push(Diagnostic::error(
                "`embed` is a compiler-owned proof term; an authored, package-qualified, or same-spelled call cannot replace it",
            ));
            } else if !proof_seen.contains(&handle) || runtime_seen.contains(&handle) {
                diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` is proof-only and cannot be used in an executable value or statement",
            ));
            } else if !has_embedding_shape(call) {
                diagnostics.push(Diagnostic::error(
                "compiler-owned `embed` requires exactly one receiverless value argument and no static or evidence arguments",
            ));
            } else if expression_table(program)
                .expression_handles(call.arguments)
                .first()
                .and_then(|argument| integer_embedding(program, *argument))
                .is_none()
            {
                diagnostics.push(Diagnostic::error(
                "compiler-owned `embed(value)` accepts only a fixed-width integer or address value",
            ));
            }
        }
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(call) = statement.as_call()
                        && program
                            .mathematical_definitions()
                            .iter()
                            .any(|definition| definition.symbol == call.target_symbol)
                    {
                        diagnostics.push(Diagnostic::error("a mathematical definition denotes a proof term, not an executable statement call"));
                    }
                    if let Some(call) = statement.as_call()
                        && (program
                            .symbols
                            .builtin_function_for_symbol(call.target_symbol)
                            == Some(BuiltinFunction::IntegerEmbed)
                            || call.target.as_str().rsplit("::").next() == Some("embed"))
                    {
                        diagnostics.push(Diagnostic::error(
                        "compiler-owned `embed(value)` is a proof value, not an executable statement call",
                    ));
                    }
                }
            }
        }
    }

    fn has_embedding_shape(call: &TableCallExpression) -> bool {
        !call.receiver.is_valid()
            && call.arguments.len() == 1
            && call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && call.static_requirement_dispatch.is_none()
            && call.quotient_operation.is_none()
            && call.private_layout_operation.is_none()
            && call.installed_interrupt_operation.is_none()
    }

    pub(crate) fn is_exact_embed_call(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
    ) -> bool {
        !call.receiver.is_valid()
            && call.target.as_str() == BuiltinFunction::IntegerEmbed.name()
            && program
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(BuiltinFunction::IntegerEmbed)
    }

    pub(crate) fn embedding_result_type_reference(
        program: &dyn CheckingInput,
        call: &TableCallExpression,
    ) -> Option<TypeReferenceHandle> {
        (is_exact_embed_call(program, call) && has_embedding_shape(call))
            .then(|| proof_int_type_reference(program))
            .flatten()
    }

    pub(crate) fn integer_embedding(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<IntegerEmbedding> {
        let primitive = expression_primitive(program, expression)?;
        let (minimum, maximum) = if primitive == PrimitiveType::Addr {
            (BigInt::zero(), None)
        } else {
            let (minimum, maximum) = primitive_range(primitive)?;
            (minimum, Some(maximum))
        };
        Some(IntegerEmbedding {
            primitive,
            interval: crate::proof_contracts::arithmetic_domains::BigIntInterval {
                low: Some(minimum),
                high: maximum,
            },
        })
    }

    pub fn integer_embedding_argument(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<(PrimitiveType, ExpressionHandle)> {
        let call = expression_table(program).call_node(expression)?;
        if !is_exact_embed_call(program, call) || !has_embedding_shape(call) {
            return None;
        }
        let [argument] = expression_table(program).expression_handles(call.arguments) else {
            return None;
        };
        Some((integer_embedding(program, *argument)?.primitive, *argument))
    }

    pub(crate) fn proof_int_type_reference(
        program: &dyn CheckingInput,
    ) -> Option<TypeReferenceHandle> {
        let symbol = program.symbols.builtin_type_symbol(BuiltinType::Int)?;
        program
            .type_reference_table
            .find_named_type_reference(symbol)
    }

    fn expression_primitive(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<PrimitiveType> {
        if let Some(type_reference) = expression_type_reference(program, expression) {
            return program.primitive_type_reference(type_reference);
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::Integer(literal) => Some(PrimitiveType::from_landed_integer(
                literal.landing()?.landed_type,
            )),
            ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::CaseMembership => Some(PrimitiveType::Bool),
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or => Some(PrimitiveType::Bool),
                BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
                    let count_is_integer = expression_primitive(program, binary.right)
                        .is_some_and(|primitive| primitive_range(primitive).is_some())
                        || is_unlanded_integer(program, binary.right);
                    count_is_integer
                        .then(|| expression_primitive(program, binary.left))
                        .flatten()
                }
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor => {
                    let left = expression_primitive(program, binary.left);
                    let right = expression_primitive(program, binary.right);
                    match (left, right) {
                        (Some(left), Some(right)) if left == right => Some(left),
                        (Some(primitive), None) if is_unlanded_integer(program, binary.right) => {
                            Some(primitive)
                        }
                        (None, Some(primitive)) if is_unlanded_integer(program, binary.left) => {
                            Some(primitive)
                        }
                        _ => None,
                    }
                }
            },
            ExpressionNode::Unary(unary) => match unary.operator {
                UnaryOperator::LogicalNot => Some(PrimitiveType::Bool),
                UnaryOperator::BitwiseNot => expression_primitive(program, unary.operand),
            },
            _ => None,
        }
    }

    fn is_unlanded_integer(program: &dyn CheckingInput, expression: ExpressionHandle) -> bool {
        matches!(expression_table(program).expression(expression),
        ExpressionNode::Integer(literal) if literal.landing().is_none())
    }

    fn collection_measure_receiver(
        program: &dyn CheckingInput,
        member: &typed_trees::expression::TableMemberExpression,
    ) -> Option<crate::collection_operations::MeasureReceiver> {
        let mut receiver = member.receiver;
        while let Some(borrow) = expression_table(program).borrow_expression(receiver) {
            receiver = borrow.target;
        }
        if let ExpressionNode::Indexed(indexed) = expression_table(program).expression(receiver)
            && expression_table(program).indexed_on_range(indexed)
        {
            return Some(crate::collection_operations::MeasureReceiver::Window);
        }
        expression_type_reference(program, receiver)
            .map(crate::collection_operations::MeasureReceiver::Declared)
    }

    pub(crate) fn expression_type_reference(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(path) => {
                crate::value_custody::expression_types::named_value_type_reference(program, path)
                    .or_else(|| {
                        crate::proof_contracts::contract_results::type_reference(
                            program, expression,
                        )
                    })
            }
            ExpressionNode::Member(member) => {
                if let ExpressionNode::Name(receiver) =
                    expression_table(program).expression(member.receiver)
                    && let Some(machine) = program.machine(receiver.symbol)
                    && let Some(field) =
                        crate::value_custody::places::exact_self_field(program, machine, expression)
                {
                    return Some(field.type_reference);
                }
                if let Some(receiver) = expression_type_reference(program, member.receiver)
                    && let Some(data) =
                        crate::value_custody::places::data_definition_for_type(program, receiver)
                    && let Some(field) = program.data_field_named(data, member.member.as_str())
                {
                    return Some(field.type_reference);
                }
                if let Some(measure_receiver) = collection_measure_receiver(program, member)
                    && crate::collection_operations::collection_measure_member(
                        program,
                        member,
                        measure_receiver,
                    )
                    .is_some()
                {
                    return crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    program,
                    symbols::BuiltinTypeAtom::U64,
                );
                }
                crate::declarations::data_queries::data_field_type_by_symbol(
                    program,
                    member.member_symbol,
                )
            }
            ExpressionNode::Cast(cast) if !cast.form.is_recast() => Some(cast.target_type),
            ExpressionNode::Call(call) => {
                if let Some(reference) = embedding_result_type_reference(program, call) {
                    return Some(reference);
                }
                if !call.target_symbol.is_valid() {
                    return None;
                }
                if let Some(operator) =
                    typed_trees::operator::resolve_named_expression_call(program, call)
                {
                    return operator
                        .return_type
                        .is_valid()
                        .then_some(operator.return_type);
                }
                program.machines().iter().find_map(|machine| {
                    let states = program.machine_states(machine);
                    let state = if machine.symbol == call.target_symbol {
                        states.first()
                    } else {
                        typed_trees::state::state_by_symbol(states, call.target_symbol)
                    }?;
                    state.return_type.is_valid().then_some(state.return_type)
                })
            }
            ExpressionNode::Indexed(indexed) => {
                let collection = expression_type_reference(program, indexed.collection)?;
                let collection =
                    crate::value_custody::places::unwrapped_type_reference(program, collection)?;
                crate::value_custody::places::collection_element_type(program, collection)
            }
            _ => None,
        }
    }

    pub(crate) fn primitive_range(primitive: PrimitiveType) -> Option<(BigInt, BigInt)> {
        primitive
            .integer_bounds()
            .map(|(minimum, maximum)| (BigInt::from_i128(minimum), BigInt::from_i128(maximum)))
    }

    fn collect_statement_nodes(
        program: &dyn CheckingInput,
        statement: &StatementNode,
        nodes: &mut Vec<ExpressionHandle>,
        seen: &mut HashSet<ExpressionHandle>,
    ) {
        for (_, expression) in program.tables.expression_roots(statement) {
            collect_expression_nodes(program, expression, nodes, seen);
        }
    }
}

pub mod proof_facts {
    use crate::CheckingInput;
    use crate::StateSignatureOwner;
    use crate::frozen_program::expression_table;
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use facts::FactPlan;
    use std::fmt;
    use symbols::JoinedPath;
    use symbols::SymbolHandle;
    use typed_trees::data::DataMember;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::name::Identifier;
    use typed_trees::trait_definition::TraitDefinition;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod proposition_parameters {
        use super::{ExpressionHandle, ProofFactOwner, TypeReferenceHandle};
        use typed_trees::proposition::{
            PropositionApplication, PropositionBinderArgumentKind, PropositionBinderKind,
            PropositionDefinition,
        };
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::TypeReferenceNode;

        pub(crate) fn argument_matches(
            program: &dyn crate::CheckingInput,
            argument: ExpressionHandle,
            parameter_type: TypeReferenceHandle,
            declaration: Option<&PropositionDefinition>,
            application: &PropositionApplication,
            owner: ProofFactOwner<'_>,
        ) -> bool {
            let mut substitutions = Vec::new();
            if let Some(declaration) = declaration {
                for (index, binder) in program.proposition_binders(declaration).iter().enumerate() {
                    let Some(selected) = application.binder_arguments.get(index) else {
                        return false;
                    };
                    let expected_kind = match binder.kind {
                        PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                        PropositionBinderKind::Const { .. } => PropositionBinderArgumentKind::Const,
                        PropositionBinderKind::Machine => PropositionBinderArgumentKind::Machine,
                    };
                    if selected.kind != expected_kind {
                        return false;
                    }
                    let reference = if let Some(literal) = &selected.const_literal {
                        program.type_reference_table.type_reference_rows().find_map(
                            |(handle, node)| {
                                let TypeReferenceNode::ConstExpression(expression) = node else {
                                    return None;
                                };
                                matches!(program.expression_table.expression(*expression),
                        typed_trees::expression::ExpressionNode::Integer(value) if value == literal
                    ).then_some(handle)
                            },
                        )
                    } else if selected.symbol.is_valid() {
                        program
                            .type_reference_table
                            .find_named_type_reference(selected.symbol)
                    } else {
                        None
                    };
                    let Some(reference) = reference else {
                        return false;
                    };
                    substitutions.push((binder.symbol, reference));
                }
            }
            carrier_matches(program, argument, parameter_type, &substitutions, owner)
        }

        fn carrier_matches(
            program: &dyn crate::CheckingInput,
            argument: ExpressionHandle,
            parameter_type: TypeReferenceHandle,
            substitutions: &[(symbols::SymbolHandle, TypeReferenceHandle)],
            owner: ProofFactOwner<'_>,
        ) -> bool {
            if let Some(borrow) = program.expression_table.borrow_expression(argument) {
                let TypeReferenceNode::Reference {
                    referee, access, ..
                } = program.type_reference_table.type_reference(parameter_type)
                else {
                    return false;
                };
                return borrow.access == *access
                    && carrier_matches(program, borrow.target, *referee, substitutions, owner);
            }
            if let Some(actual) = super::proof_argument_declared_type(program, argument, owner) {
                return program.type_identity(TypeIdentityRequest::ordinary(actual))
                    == program.type_identity_with_simultaneous_substitutions(
                        TypeIdentityRequest {
                            substitutions,
                            ..TypeIdentityRequest::ordinary(parameter_type)
                        },
                    );
            }
            crate::value_custody::expression_types::argument_matches_type_reference_with_substitutions(
        program,
        argument,
        parameter_type,
        substitutions,
            )
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum ProofFactOwner<'program> {
        DataDefaultDomain(&'program str),
        Domain(&'program str),
        MachineContract {
            machine: &'program typed_trees::machine::Machine,
            kind: &'static str,
        },
        StatementContract {
            machine: &'program typed_trees::machine::Machine,
            state: &'program typed_trees::state::State,
            statement_index: usize,
            kind: &'static str,
        },
        StateSignatureContract {
            owner: StateSignatureOwner<'program>,
            state: &'program str,
            kind: &'static str,
        },
    }

    fn unknown_membership_domain_diagnostic(
        program: &dyn CheckingInput,
        owner: ProofFactOwner<'_>,
        domain: HandleSpan<Identifier>,
        domain_symbol: symbols::SymbolHandle,
    ) -> Option<Diagnostic> {
        if domain_symbol.is_valid()
            || is_implicit_case_domain(program, domain)
            || carry_permission(program, domain).is_some()
        {
            return None;
        }
        Some(Diagnostic::error(format!(
            "{owner} references unknown domain `{}`",
            domain_path_label(program, domain)
        )))
    }

    pub(crate) fn validate_proposition_definitions(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for proposition in program.propositions() {
            let mut visiting = Vec::new();
            if transparent_proposition_cycle(program, proposition.symbol, &mut visiting) {
                diagnostics.push(Diagnostic::error(format!(
                    "transparent proposition `{}` participates in an alias cycle",
                    proposition.name.as_str()
                )));
            }
            if let typed_trees::proposition::PropositionBody::Witness { evidence } =
                proposition.body
            {
                validate_witness_evidence_interface(
                    program,
                    proposition.name.as_str(),
                    evidence,
                    diagnostics,
                );
            }
        }
    }

    fn validate_witness_evidence_interface(
        program: &dyn CheckingInput,
        proposition: &str,
        evidence: TypeReferenceHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !evidence.is_valid() {
            diagnostics.push(Diagnostic::error(format!(
                "witness-bearing proposition `{proposition}` has no resolved evidence interface"
            )));
            return;
        }

        let (trait_symbol, trait_name, lifetime_argument_count, arguments) = match program
            .type_reference_table
            .type_reference(evidence)
        {
            TypeReferenceNode::Named { symbol, name } => (*symbol, name.as_str(), 0, &[][..]),
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            } => (
                *base_symbol,
                base_name.as_str(),
                lifetime_arguments.len(),
                program
                    .type_reference_table
                    .type_reference_handles(*arguments),
            ),
            TypeReferenceNode::DynamicTrait { name, .. } => {
                diagnostics.push(Diagnostic::error(format!(
                    "witness-bearing proposition `{proposition}` names selected dynamic evidence `dyn {name}`; name the trait interface directly"
                )));
                return;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "witness-bearing proposition `{proposition}` evidence `{}` must name one proof evidence trait interface",
                    program.display_type_reference(evidence)
                )));
                return;
            }
        };

        let Some(trait_definition) = program.traits().iter().find(|candidate| {
            (trait_symbol.is_valid() && candidate.symbol == trait_symbol)
                || candidate.name.as_str() == trait_name
        }) else {
            diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence `{}` is not a trait interface",
            program.display_type_reference(evidence)
        )));
            return;
        };

        if lifetime_argument_count != trait_definition.lifetime_parameters.len() {
            diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` expects {} lifetime argument(s), but the interface supplies {lifetime_argument_count}",
            trait_definition.lifetime_parameters.len()
        )));
        }

        let parameter_count = program.trait_type_parameters(trait_definition).len();
        if arguments.len() != parameter_count {
            diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` expects {parameter_count} generic argument(s), but the interface supplies {}",
            arguments.len()
        )));
        }
        for argument in arguments {
            if !evidence_argument_is_resolved(program, *argument) {
                diagnostics.push(Diagnostic::error(format!(
                "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` has unresolved generic argument `{}`",
                program.display_type_reference(*argument)
            )));
            }
        }

        let mut visiting = Vec::new();
        if let Some(reason) = evidence_trait_violation(program, trait_definition, &mut visiting) {
            diagnostics.push(Diagnostic::error(format!(
            "witness-bearing proposition `{proposition}` evidence trait `{trait_name}` is not a proof evidence interface: {reason}"
        )));
        }
    }

    fn evidence_argument_is_resolved(
        program: &dyn CheckingInput,
        argument: TypeReferenceHandle,
    ) -> bool {
        if !argument.is_valid() {
            return false;
        }
        match program.type_reference_table.type_reference(argument) {
            TypeReferenceNode::Callable { .. } => program
                .runtime_callable_type_identity(
                    typed_trees::type_identity::TypeIdentityRequest::ordinary(argument),
                )
                .is_ok(),
            TypeReferenceNode::Named { symbol, .. } => symbol.is_valid(),
            TypeReferenceNode::Reference { referee, .. } => {
                evidence_argument_is_resolved(program, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                evidence_argument_is_resolved(program, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                evidence_argument_is_resolved(program, *element_type)
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                base_symbol.is_valid()
                    && program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .all(|argument| evidence_argument_is_resolved(program, *argument))
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => true,
            TypeReferenceNode::DynamicTrait { symbol, .. } => symbol.is_valid(),
        }
    }

    fn evidence_trait_violation(
        program: &dyn CheckingInput,
        trait_definition: &TraitDefinition,
        visiting: &mut Vec<SymbolHandle>,
    ) -> Option<String> {
        if trait_definition.is_boundary {
            return Some("boundary traits describe externally executed services".to_owned());
        }
        if visiting.contains(&trait_definition.symbol) {
            return None;
        }
        visiting.push(trait_definition.symbol);
        for parent in program.trait_requirements(trait_definition) {
            let Some(parent_definition) = program.trait_definition_by_symbol(parent.symbol) else {
                continue;
            };
            if let Some(reason) = evidence_trait_violation(program, parent_definition, visiting) {
                visiting.pop();
                return Some(format!(
                    "parent interface `{}` is not a proof evidence interface: {reason}",
                    parent.name
                ));
            }
        }
        visiting.pop();
        None
    }

    fn transparent_proposition_cycle(
        program: &dyn CheckingInput,
        symbol: symbols::SymbolHandle,
        visiting: &mut Vec<symbols::SymbolHandle>,
    ) -> bool {
        if visiting.contains(&symbol) {
            return true;
        }
        let Some(proposition) = program.proposition_definition_by_symbol(symbol) else {
            return false;
        };
        let typed_trees::proposition::PropositionBody::Transparent {
            proposition: typed_trees::proposition::PropositionFormula::Application(application),
        } = &proposition.body
        else {
            return false;
        };
        visiting.push(symbol);
        let cyclic = transparent_proposition_cycle(program, application.proposition, visiting);
        visiting.pop();
        cyclic
    }

    impl fmt::Display for ProofFactOwner<'_> {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::DataDefaultDomain(data) => write!(formatter, "data `{data}` default domain"),
                Self::Domain(domain) => write!(formatter, "domain `{domain}`"),
                Self::MachineContract { machine, kind } => {
                    write!(formatter, "machine `{}` {kind} contract", machine.name)
                }
                Self::StatementContract {
                    machine,
                    state,
                    kind,
                    ..
                } => write!(
                    formatter,
                    "machine `{}` state `{}` {kind} contract",
                    machine.name, state.name
                ),
                Self::StateSignatureContract { owner, state, kind } => {
                    write!(formatter, "{owner} state `{state}` {kind} contract")
                }
            }
        }
    }

    pub(crate) fn validate_domain_fact_payloads(
        program: &dyn CheckingInput,
        fact_plan: &FactPlan,
        domain: &typed_trees::domain::DomainDefinition,
        diagnostics: &mut Vec<Diagnostic>,
        owner: ProofFactOwner<'_>,
    ) {
        for fact in fact_plan.boolean_facts_for_symbol(domain.symbol) {
            if !is_boolean_fact_expression(program, fact.expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} proof fact `{}` is not boolean-shaped",
                    expression_table(program).display_name(fact.expression)
                )));
            } else if !is_boolean_domain_fact_expression(program, domain, fact.expression) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} proof fact `{}` must have type `bool`",
                    expression_table(program).display_name(fact.expression)
                )));
            }
        }

        for membership in fact_plan.domain_memberships_for_symbol(domain.symbol) {
            if let Some(diagnostic) = unknown_membership_domain_diagnostic(
                program,
                owner,
                membership.domain,
                membership.domain_symbol,
            ) {
                diagnostics.push(diagnostic);
            }
        }
    }

    pub(crate) fn validate_proof_facts(
        program: &dyn CheckingInput,
        facts: &[ProofFact],
        diagnostics: &mut Vec<Diagnostic>,
        owner: ProofFactOwner<'_>,
    ) {
        for fact in facts {
            if matches!(
                owner,
                ProofFactOwner::StateSignatureContract {
                    owner: StateSignatureOwner::Trait(_),
                    ..
                }
            ) && let Some(member) = abstract_trait_self_member_in_fact(program, fact)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references undeclared structural member `{member}`; abstract `Self` \
                 has no field namespace -- use an explicit proposition parameter or a declared \
                 accessor requirement"
                )));
            }
            match fact {
                ProofFact::Expression(expression) => {
                    if !is_boolean_owned_fact_expression(program, *expression, owner) {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} proof fact `{}` is not boolean-shaped",
                            expression_table(program).display_name(*expression)
                        )));
                    }
                }
                ProofFact::Membership(membership) => {
                    if let Some(diagnostic) = unknown_membership_domain_diagnostic(
                        program,
                        owner,
                        membership.domain,
                        membership.domain_symbol,
                    ) {
                        diagnostics.push(diagnostic);
                    }
                }
                ProofFact::Proposition(application) => {
                    let declaration =
                        program.proposition_definition_by_symbol(application.proposition);
                    let parameter_signature = program
                        .data_type_parameters
                        .iter()
                        .map(|(_, parameter)| parameter)
                        .find_map(|parameter| match &parameter.kind {
                            typed_trees::data::TypeParameterKind::Proposition { contract }
                                if parameter.symbol == application.proposition =>
                            {
                                Some(contract)
                            }
                            _ => None,
                        });
                    if declaration.is_none() && parameter_signature.is_none() {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} references an unknown proposition `{}`",
                            application.name.as_str()
                        )));
                        continue;
                    }
                    let parameters = if let Some(declaration) = declaration {
                        program.proposition_parameters(declaration)
                    } else if let Some(signature) = parameter_signature {
                        program.state_parameters.span_or_empty(signature.parameters)
                    } else {
                        &[]
                    };
                    for (index, (argument, parameter)) in expression_table(program)
                        .expression_handles(application.arguments)
                        .iter()
                        .zip(parameters)
                        .enumerate()
                    {
                        let matches = proposition_parameters::argument_matches(
                            program,
                            *argument,
                            parameter.type_reference,
                            declaration,
                            application,
                            owner,
                        );
                        if !matches {
                            let expected_name =
                                program.display_type_reference(parameter.type_reference);
                            diagnostics.push(Diagnostic::error(format!(
                            "{owner} proposition `{}` argument {} does not match parameter `{}` type `{}`",
                            application.name.as_str(),
                            index + 1,
                            parameter.name.as_str(),
                            expected_name,
                        )));
                        }
                    }
                }
            }
        }
    }

    fn abstract_trait_self_member_in_fact(
        program: &dyn CheckingInput,
        fact: &ProofFact,
    ) -> Option<String> {
        let mut visited = Vec::new();
        match fact {
            ProofFact::Expression(expression) => {
                abstract_trait_self_member_in_expression(program, *expression, &mut visited)
            }
            ProofFact::Membership(membership) => {
                abstract_trait_self_member_in_expression(program, membership.value, &mut visited)
            }
            ProofFact::Proposition(application) => expression_table(program)
                .expression_handles(application.arguments)
                .iter()
                .find_map(|argument| {
                    abstract_trait_self_member_in_expression(program, *argument, &mut visited)
                }),
        }
    }

    fn abstract_trait_self_member_in_expression(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        visited: &mut Vec<ExpressionHandle>,
    ) -> Option<String> {
        if !expression.is_valid() || visited.contains(&expression) {
            return None;
        }
        visited.push(expression);
        if let Some(member) = self_member_name(program, expression) {
            return Some(member.to_owned());
        }
        match expression_table(program).expression(expression) {
            ExpressionNode::MachineValue(_) => None,
            ExpressionNode::Match(dispatch) => {
                crate::queries::expression_traversal::match_children(program, *dispatch).find_map(
                    |child| abstract_trait_self_member_in_expression(program, child, visited),
                )
            }
            ExpressionNode::ArrayLiteral(values) => expression_table(program)
                .expression_handles(*values)
                .iter()
                .find_map(|value| {
                    abstract_trait_self_member_in_expression(program, *value, visited)
                }),
            ExpressionNode::Atomic(atomic) => {
                abstract_trait_self_member_in_expression(program, atomic.value, visited).or_else(
                    || abstract_trait_self_member_in_expression(program, atomic.result, visited),
                )
            }
            ExpressionNode::Binary(binary) => {
                abstract_trait_self_member_in_expression(program, binary.left, visited).or_else(
                    || abstract_trait_self_member_in_expression(program, binary.right, visited),
                )
            }
            ExpressionNode::Cast(cast) => {
                abstract_trait_self_member_in_expression(program, cast.value, visited)
            }
            ExpressionNode::Call(call) => abstract_trait_self_member_in_expression(
                program,
                call.receiver,
                visited,
            )
            .or_else(|| {
                expression_table(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .find_map(|argument| {
                        abstract_trait_self_member_in_expression(program, *argument, visited)
                    })
            }),
            ExpressionNode::Indexed(indexed) => {
                abstract_trait_self_member_in_expression(program, indexed.collection, visited)
                    .or_else(|| {
                        abstract_trait_self_member_in_expression(program, indexed.index, visited)
                    })
            }
            ExpressionNode::Member(member) => {
                abstract_trait_self_member_in_expression(program, member.receiver, visited)
            }
            ExpressionNode::Borrow(value) => {
                abstract_trait_self_member_in_expression(program, value.target, visited)
            }
            ExpressionNode::Range(range) => {
                abstract_trait_self_member_in_expression(program, range.start, visited).or_else(
                    || abstract_trait_self_member_in_expression(program, range.end, visited),
                )
            }
            ExpressionNode::StructLiteral(literal) => expression_table(program)
                .struct_fields(literal.fields)
                .iter()
                .find_map(|field| {
                    abstract_trait_self_member_in_expression(program, field.value, visited)
                }),
            ExpressionNode::Unary(unary) => {
                abstract_trait_self_member_in_expression(program, unary.operand, visited)
            }
            ExpressionNode::TypeExpression(_)
            | ExpressionNode::SavedSubject(_)
            | typed_trees::literal_expression_pattern!() => None,
        }
    }

    fn proof_argument_declared_type(
        program: &dyn CheckingInput,
        argument: ExpressionHandle,
        owner: ProofFactOwner<'_>,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if let ProofFactOwner::StatementContract {
            state,
            statement_index,
            ..
        } = owner
        {
            return crate::expression_type_reference_in_state(
                program,
                state.symbol,
                statement_index,
                argument,
            );
        }
        if let Some(member_name) = self_member_name(program, argument)
            && let Some(machine_name) = proof_fact_machine_name(owner)
            && let Some(attached_data) = program
                .machine_by_name_str(machine_name)
                .and_then(|machine| machine.attached_data.as_ref())
            && let Some(definition) = program.data_definition_by_name(attached_data)
            && let Some(type_reference) =
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|field| match field {
                        DataMember::Field(field) if field.name.as_str() == member_name => {
                            Some(field.type_reference)
                        }
                        _ => None,
                    })
        {
            return Some(type_reference);
        }
        let symbol = match expression_table(program).expression(argument) {
            ExpressionNode::Name(path) => path.symbol,
            ExpressionNode::Member(member) => member.member_symbol,
            _ => return None,
        };
        if !symbol.is_valid() {
            return None;
        }
        program
            .state_parameters
            .iter()
            .map(|(_, parameter)| parameter)
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .field_owner(symbol)
                    .map(|(_, field)| field.type_reference)
            })
    }

    fn proof_fact_machine_name(owner: ProofFactOwner<'_>) -> Option<&str> {
        match owner {
            ProofFactOwner::MachineContract { machine, .. } => Some(machine.name.as_str()),
            ProofFactOwner::StatementContract { machine, .. } => Some(machine.name.as_str()),
            ProofFactOwner::StateSignatureContract {
                owner: StateSignatureOwner::Machine(machine),
                ..
            } => Some(machine.name.as_str()),
            ProofFactOwner::DataDefaultDomain(_)
            | ProofFactOwner::Domain(_)
            | ProofFactOwner::StateSignatureContract { .. } => None,
        }
    }

    fn self_member_name(program: &dyn CheckingInput, expression: ExpressionHandle) -> Option<&str> {
        match expression_table(program).expression(expression) {
            ExpressionNode::Member(member) => {
                let path = expression_table(program).name_path_node(member.receiver)?;
                matches!(
                    expression_table(program).name_path_members(path.members),
                    [name] if name.is_self_receiver()
                )
                .then_some(member.member.as_str())
            }
            ExpressionNode::Name(path) => {
                match expression_table(program).name_path_members(path.members) {
                    [root, member] if root.is_self_receiver() => Some(member.as_str()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(crate) fn carry_permission(
        program: &dyn CheckingInput,
        domain: HandleSpan<Identifier>,
    ) -> Option<language_semantics::CarryPermission> {
        let name = domain_path_label(program, domain);
        language_semantics::CarryPermission::from_name(&name)
    }

    fn is_implicit_case_domain(
        program: &dyn CheckingInput,
        domain: HandleSpan<Identifier>,
    ) -> bool {
        let [type_name, case_name] = program.domain_path_members(domain) else {
            return false;
        };
        program.data_definitions().iter().any(|definition| {
            definition.name.as_str() == type_name.as_str()
                && program.data_members(definition).iter().any(|member| {
                    matches!(
                        member,
                        DataMember::Variant(variant)
                            if variant.name.as_str() == case_name.as_str()
                    )
                })
        })
    }

    fn is_boolean_domain_fact_expression(
        program: &dyn CheckingInput,
        domain: &typed_trees::domain::DomainDefinition,
        expression: ExpressionHandle,
    ) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                crate::value_custody::expression_types::domain_expression_result_type_reference(
                    program, domain, expression,
                )
                .and_then(|reference| program.primitive_type_reference(reference))
                    == Some(typed_trees::types::PrimitiveType::Bool)
            }
            ExpressionNode::Atomic(atomic) => {
                is_boolean_domain_fact_expression(program, domain, atomic.value)
            }
            ExpressionNode::Unary(unary) => {
                unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
                    && is_boolean_domain_fact_expression(program, domain, unary.operand)
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                is_boolean_domain_fact_expression(program, domain, binary.left)
                    && is_boolean_domain_fact_expression(program, domain, binary.right)
            }
            ExpressionNode::Borrow(_) => false,
            _ => is_boolean_fact_expression(program, expression),
        }
    }

    fn is_boolean_owned_fact_expression(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        owner: ProofFactOwner<'_>,
    ) -> bool {
        if !matches!(
            expression_table(program).expression(expression),
            ExpressionNode::Match(_)
        ) {
            return is_boolean_fact_expression(program, expression);
        }
        match owner {
            ProofFactOwner::MachineContract { machine, .. } => {
                let Some(state) = program.machine_states(machine).first() else {
                    return false;
                };
                boolean_match_shape(program, expression, |expression| {
                    crate::parameter_expression_result_type_reference(
                        program,
                        machine.symbol,
                        program.state_parameters(state),
                        expression,
                    )
                })
            }
            ProofFactOwner::StatementContract { machine, state, .. } => {
                boolean_match_shape(program, expression, |expression| {
                    crate::expression_result_type_reference(program, machine, state, expression)
                })
            }
            _ => false,
        }
    }

    fn boolean_match_shape(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        type_of: impl Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
    ) -> bool {
        let ExpressionNode::Match(_) = expression_table(program).expression(expression) else {
            return false;
        };
        type_of(expression).and_then(|reference| program.primitive_type_reference(reference))
            == Some(typed_trees::types::PrimitiveType::Bool)
            && crate::expression_has_closed_value_captures(program, expression)
    }

    fn is_boolean_fact_expression(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::MachineValue(_) => false,
            ExpressionNode::SavedSubject(_) | ExpressionNode::Match(_) => false,
            ExpressionNode::Atomic(atomic) => is_boolean_fact_expression(program, atomic.value),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::CaseMembership => true,
                BinaryOperator::And
                | BinaryOperator::Equal
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::NotEqual
                | BinaryOperator::Or => true,
                BinaryOperator::Add
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => false,
            },
            ExpressionNode::Boolean(_)
            | ExpressionNode::Call(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Name(_) => true,
            ExpressionNode::Range(_) => false,
            ExpressionNode::Borrow(inner) => is_boolean_fact_expression(program, inner.target),
            ExpressionNode::Unary(unary) => is_boolean_fact_expression(program, unary.operand),
            ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => false,
        }
    }

    pub(crate) fn is_boolean_asm_fact_expression(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
    ) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::Match(_) => {
                let Some(state) = state.or_else(|| program.machine_states(machine).first()) else {
                    return false;
                };
                boolean_match_shape(program, expression, |expression| {
                    crate::expression_result_type_reference(program, machine, state, expression)
                })
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                crate::value_custody::places::declared_place_type_raw(
                    program, machine, state, expression,
                )
                .and_then(|handle| program.primitive_type_reference(handle))
                    == Some(typed_trees::types::PrimitiveType::Bool)
            }
            ExpressionNode::Borrow(inner) => {
                is_boolean_asm_fact_expression(program, machine, state, inner.target)
            }
            _ => is_boolean_fact_expression(program, expression),
        }
    }

    fn domain_path_label(program: &dyn CheckingInput, domain: HandleSpan<Identifier>) -> String {
        let path = program.domain_path_members(domain);
        if path.is_empty() {
            return "<unknown>".to_owned();
        }

        path.joined_path()
    }

    pub(crate) fn string_literal_grants_domain(
        program: &dyn CheckingInput,
        expression: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        let Some(literal) = typed_trees::byte_predicates::literal_bytes(program, expression) else {
            return false;
        };
        typed_trees::byte_predicates::domain_byte_predicate(program, domain_symbol)
            .is_some_and(|predicate| predicate.holds_for(&literal))
    }

    pub(crate) fn domain_admits_zero_bytes(
        program: &dyn CheckingInput,
        domain_symbol: SymbolHandle,
    ) -> bool {
        program
            .domain_definition(domain_symbol)
            .is_some_and(|domain| {
                crate::value_custody::field_domains::domain_admits_zero_byte_carrier(
                    program,
                    domain_symbol,
                    domain.target_type,
                )
            })
    }

    pub(crate) fn ensures_expression_facts<'a>(
        program: &'a dyn CheckingInput,
        contracts: &'a [typed_trees::signature::SignatureContract],
    ) -> impl Iterator<Item = ExpressionHandle> + 'a {
        ensures_facts(program, contracts).filter_map(|fact| {
            let expression = fact.as_expression()?;
            Some(*expression)
        })
    }

    pub(crate) fn ensures_facts<'a>(
        program: &'a dyn CheckingInput,
        contracts: &'a [typed_trees::signature::SignatureContract],
    ) -> impl Iterator<Item = &'a ProofFact> + 'a {
        contracts
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
    }

    pub(crate) fn contract_expression_facts<'a>(
        program: &'a dyn CheckingInput,
        contracts: impl Iterator<Item = &'a typed_trees::signature::SignatureContract> + 'a,
        kind: typed_trees::signature::SignatureContractKind,
    ) -> impl Iterator<Item = ExpressionHandle> + 'a {
        contracts
            .filter(move |contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| {
                let expression = fact.as_expression()?;
                Some(*expression)
            })
    }
}

pub mod proof_only_faces {
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::proof_only::ProofOnlyClassification;

    pub(crate) fn validate_proof_only_consumption(
        program: &dyn crate::CheckingInput,
        classification: &ProofOnlyClassification,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for definition in program.data_definitions() {
            let has_machine_parameter = program
                .data_type_parameters(definition)
                .iter()
                .any(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }));
            if has_machine_parameter
                && definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
                && !classification.is_proof_only(definition.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` is parameterized by a static machine but has a runtime layout; `<machine ...>` on data is reserved for proof-only families (recursive data has no layout)",
                definition.name
            )));
            }
        }

        for definition in program.data_definitions() {
            let properties = definition.properties;
            if !(properties.multiplicity != Multiplicity::Affine || properties.carry.is_some()) {
                continue;
            }
            if let Some(reason) =
                classification.describe(definition.name.as_str(), definition.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "data `{}` declares runtime properties, but {reason}; properties speak \
                 about values in memory, which proof-only data never becomes",
                    definition.name
                )));
            }
        }

        for machine in program.machines() {
            if crate::value_custody::content_projections::is_content_projection_machine(
                program, machine,
            ) {
                continue;
            }
            if classification.is_proof_machine(program, machine) {
                continue;
            }

            for owned_data in program.machine_owned_data(machine) {
                if let Some(held) =
                    classification.proof_only_mention(program, owned_data.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` owned data `{}` holds proof-only `{held}`, which has no \
                     runtime layout",
                        machine.name, owned_data.name
                    )));
                }
            }

            for state in program.machine_states(machine) {
                for parameter in program.state_parameters(state) {
                    if parameter.relevance.is_erased() {
                        continue;
                    }
                    if let Some(held) =
                        classification.proof_only_mention(program, parameter.type_reference)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{}` parameter `{}` mentions proof-only \
                         `{held}`, which has no runtime layout",
                            machine.name, state.name, parameter.name
                        )));
                    }
                }

                if state.return_type.is_valid()
                    && let Some(held) =
                        classification.proof_only_mention(program, state.return_type)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` returns proof-only `{held}`, which has no \
                     runtime layout",
                        machine.name, state.name
                    )));
                }

                for statement in program.statement_table.statements(state.statement_nodes) {
                    let Some(local_data) = statement.as_local_data() else {
                        continue;
                    };
                    if local_data.relevance.is_erased() {
                        continue;
                    }
                    if let Some(held) =
                        classification.proof_only_mention(program, local_data.type_reference)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "machine `{}` state `{}` local `{}` mentions proof-only `{held}`, \
                         which has no runtime layout",
                            machine.name, state.name, local_data.name
                        )));
                    }
                }
            }
        }

        for schema in program.wire_schemas() {
            for member in program.wire_members(schema.members) {
                let typed_trees::wire::WireMember::Field(field) = member else {
                    continue;
                };
                if field.relevance.is_erased() {
                    continue;
                }
                if let Some(held) = classification.proof_only_mention(program, field.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "wire data `{}` field `{}` mentions proof-only `{held}`, which has no \
                     wire encoding",
                        schema.name, field.name
                    )));
                }
            }
        }

        for definition in program.data_definitions() {
            if classification.is_proof_only(definition.symbol) {
                continue;
            }
            for field in program.data_member_fields(program.data_members(definition)) {
                if field.relevance == language_core::BindingRelevance::Erased {
                    continue;
                }
                if let Some(held) = classification.proof_only_mention(program, field.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` field `{}` views proof-only `{held}` through an \
                     indirection, but proof-only values never materialize at runtime \
                     (there is nothing to point at)",
                        definition.name, field.name
                    )));
                }
            }
        }
    }
}

pub mod properties {
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::data::{DataDefinition, TypeParameter};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DeclaredPropertyRequirement {
        Copy,
        Linear,
        Carry(language_semantics::CarryPolicy),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub enum OpaqueDataPropertyReceiptKind {
        Copy,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct OpaqueDataPropertyReceipt {
        declaration: symbols::SymbolHandle,
        kind: OpaqueDataPropertyReceiptKind,
    }

    impl OpaqueDataPropertyReceipt {
        pub const fn copy(declaration: symbols::SymbolHandle) -> Self {
            Self {
                declaration,
                kind: OpaqueDataPropertyReceiptKind::Copy,
            }
        }

        pub const fn declaration(self) -> symbols::SymbolHandle {
            self.declaration
        }

        pub const fn kind(self) -> OpaqueDataPropertyReceiptKind {
            self.kind
        }
    }

    impl std::fmt::Display for DeclaredPropertyRequirement {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Copy => formatter.write_str("copy"),
                Self::Linear => formatter.write_str("linear"),
                Self::Carry(policy) => write!(formatter, "{policy}"),
            }
        }
    }

    pub(crate) fn validate_data_properties(
        program: &dyn crate::CheckingInput,
        opaque_property_receipts: &[OpaqueDataPropertyReceipt],
        allow_pending_opaque_copy: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_opaque_property_receipts(program, opaque_property_receipts, diagnostics);
        for data_definition in program.data_definitions() {
            let properties = data_definition.properties;

            if data_definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque {
                validate_opaque_data_properties(
                    data_definition,
                    opaque_property_receipts,
                    allow_pending_opaque_copy,
                    diagnostics,
                );
                continue;
            }

            if properties.multiplicity == Multiplicity::Unrestricted {
                validate_copy_property(program, data_definition, diagnostics);
            }
            if let Some(carry) = properties.carry {
                validate_carry_policy(program, data_definition, carry, diagnostics);
            }
        }
    }

    fn validate_copy_property(
        program: &dyn crate::CheckingInput,
        data_definition: &DataDefinition,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let property = "copy";
        let type_parameters = program.data_type_parameters(data_definition);
        for_each_stored_field(
            program,
            data_definition,
            &mut |field, case: Option<&str>| {
                if type_satisfies_declared_property(
                    program,
                    type_parameters,
                    field.type_reference,
                    DeclaredPropertyRequirement::Copy,
                ) {
                    return;
                }
                let place = match case {
                    Some(case) => format!("case `{case}` payload field `{}`", field.name),
                    None => format!("field `{}`", field.name),
                };
                if let Some(parameter) =
                    referenced_type_parameter(program, type_parameters, field.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares `[{property}]` but {place} type parameter `{name}` does not declare `[{property}]` — add `{name} [{property}]`",
                data_definition.name,
                name = parameter.name
            )));
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
            "data `{}` declares `[{property}]` but {place} is not `{property}`: only primitives, shared references and data types declaring `[{property}]` qualify",
            data_definition.name
        )));
            },
        );
    }

    fn validate_carry_policy(
        program: &dyn crate::CheckingInput,
        data_definition: &DataDefinition,
        required: language_semantics::CarryPolicy,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let type_parameters = program.data_type_parameters(data_definition);
        for_each_stored_field(
            program,
            data_definition,
            &mut |field, case: Option<&str>| {
                let actual =
                    CarryDerivation::new(program, type_parameters).derive(field.type_reference);
                if actual.permits(required) {
                    return;
                }
                let place = match case {
                    Some(case) => format!("case `{case}` payload field `{}`", field.name),
                    None => format!("field `{}`", field.name),
                };
                if let Some(parameter) =
                    referenced_type_parameter(program, type_parameters, field.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} type parameter `{}` has only `{actual}`; add a compatible `[carry(...)]` bound",
                data_definition.name, parameter.name
            )));
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} permits only `{actual}`",
                data_definition.name
            )));
            },
        );
    }

    pub(crate) mod carry_derivation {
        use crate::CheckingInput;
        use symbols::SymbolHandle;
        use typed_trees::data::{DataDefinition, DataField, DataMember, TypeParameter};
        use typed_trees::types::TypeReferenceHandle;
        use typed_trees::types::TypeReferenceNode;

        pub(crate) struct CarryDerivation<'program> {
            program: &'program dyn CheckingInput,
            parameters: Vec<(SymbolHandle, String, language_semantics::CarryPolicy)>,
            substitutions: Vec<(SymbolHandle, String, TypeReferenceHandle)>,
            visiting: Vec<SymbolHandle>,
            requires_borrow_preservation: bool,
            borrow_independent: language_semantics::CarryPolicy,
        }

        impl<'program> CarryDerivation<'program> {
            pub(crate) fn new(
                program: &'program dyn CheckingInput,
                parameters: &[TypeParameter],
            ) -> Self {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.symbol,
                            parameter.name.as_str().to_owned(),
                            parameter
                                .bounds
                                .carry
                                .unwrap_or(language_semantics::CarryPolicy::STRICT),
                        )
                    })
                    .collect();
                Self {
                    program,
                    parameters,
                    substitutions: Vec::new(),
                    visiting: Vec::new(),
                    requires_borrow_preservation: false,
                    borrow_independent: language_semantics::CarryPolicy::PERMISSIVE,
                }
            }

            pub(crate) fn derive_requirements(
                mut self,
                type_reference: TypeReferenceHandle,
            ) -> super::TypeCarryRequirements {
                let structural = self.derive(type_reference);
                super::TypeCarryRequirements {
                    structural,
                    borrow_independent: self.borrow_independent,
                    requires_borrow_preservation: self.requires_borrow_preservation,
                }
            }

            pub(crate) fn derive(
                &mut self,
                type_reference: TypeReferenceHandle,
            ) -> language_semantics::CarryPolicy {
                use language_semantics::CarryPolicy;

                if let Some(storage) = self.program.layout_storage_arguments(type_reference) {
                    return self.derive(storage.semantic_type);
                }
                if self
                    .program
                    .type_reference_table
                    .primitive_type(type_reference)
                    .is_some()
                {
                    return CarryPolicy::PERMISSIVE;
                }

                match self
                    .program
                    .type_reference_table
                    .type_reference(type_reference)
                    .clone()
                {
                    TypeReferenceNode::Named { symbol, name } => {
                        if let Some((_, _, argument)) = self.substitutions.iter().rev().find(
                            |(candidate, candidate_name, _)| {
                                (*candidate == symbol && symbol.is_valid())
                                    || (!symbol.is_valid() && candidate_name == name.as_str())
                            },
                        ) {
                            let argument = *argument;
                            return self.derive(argument);
                        }
                        if let Some((_, _, policy)) =
                            self.parameters
                                .iter()
                                .rev()
                                .find(|(candidate, candidate_name, _)| {
                                    (*candidate == symbol && symbol.is_valid())
                                        || (!symbol.is_valid() && candidate_name == name.as_str())
                                })
                        {
                            if policy.suspension == language_semantics::CarrySuspension::Forbidden {
                                self.requires_borrow_preservation = true;
                            }
                            self.borrow_independent = self.borrow_independent.intersect(*policy);
                            return *policy;
                        }
                        self.derive_named_data(symbol, name.as_str(), None)
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => self.derive(base_type),
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    } => {
                        if length == typed_trees::types::FixedArrayLength::Literal(0) {
                            CarryPolicy::PERMISSIVE
                        } else {
                            self.derive(element_type)
                        }
                    }
                    TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        arguments,
                        ..
                    } => {
                        let arguments = self
                            .program
                            .type_reference_table
                            .type_reference_handles(arguments)
                            .to_vec();
                        self.derive_named_data(base_symbol, base_name.as_str(), Some(&arguments))
                    }
                    TypeReferenceNode::Callable { .. }
                    | TypeReferenceNode::ConstExpression(_)
                    | TypeReferenceNode::Unit => CarryPolicy::PERMISSIVE,
                    TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                        self.requires_borrow_preservation = true;
                        CarryPolicy::STRICT
                    }
                    TypeReferenceNode::DynamicTrait { .. } => {
                        self.borrow_independent =
                            self.borrow_independent.intersect(CarryPolicy::STRICT);
                        CarryPolicy::STRICT
                    }
                }
            }

            pub(crate) fn derive_named_data(
                &mut self,
                symbol: SymbolHandle,
                name: &str,
                arguments: Option<&[TypeReferenceHandle]>,
            ) -> language_semantics::CarryPolicy {
                use language_semantics::CarryPolicy;

                let Some(definition) = self.program.data_definitions().iter().find(|definition| {
                    (symbol.is_valid() && definition.symbol == symbol)
                        || (!symbol.is_valid() && definition.name.as_str() == name)
                }) else {
                    self.borrow_independent =
                        self.borrow_independent.intersect(CarryPolicy::STRICT);
                    return CarryPolicy::STRICT;
                };
                if definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
                    || self.visiting.contains(&definition.symbol)
                {
                    self.borrow_independent =
                        self.borrow_independent.intersect(CarryPolicy::STRICT);
                    return CarryPolicy::STRICT;
                }

                self.visiting.push(definition.symbol);
                let parameter_len = self.parameters.len();
                let substitution_len = self.substitutions.len();
                let definition_parameters = self.program.data_type_parameters(definition);
                for parameter in definition_parameters {
                    self.parameters.push((
                        parameter.symbol,
                        parameter.name.as_str().to_owned(),
                        parameter.bounds.carry.unwrap_or(CarryPolicy::STRICT),
                    ));
                }
                if let Some(arguments) = arguments {
                    self.substitutions
                        .extend(definition_parameters.iter().zip(arguments).map(
                            |(parameter, argument)| {
                                (
                                    parameter.symbol,
                                    parameter.name.as_str().to_owned(),
                                    *argument,
                                )
                            },
                        ));
                }

                let mut field_types = Vec::new();
                for_each_stored_field(self.program, definition, &mut |field, _| {
                    field_types.push(field.type_reference);
                });
                let mut effective = definition
                    .properties
                    .carry
                    .unwrap_or(CarryPolicy::PERMISSIVE);
                self.borrow_independent = self.borrow_independent.intersect(effective);
                for field_type in field_types {
                    effective = effective.intersect(self.derive(field_type));
                }

                self.substitutions.truncate(substitution_len);
                self.parameters.truncate(parameter_len);
                self.visiting.pop();
                effective
            }
        }

        pub(crate) fn for_each_stored_field(
            program: &dyn CheckingInput,
            data_definition: &DataDefinition,
            visit: &mut impl FnMut(&DataField, Option<&str>),
        ) {
            for member in program.data_members(data_definition) {
                match member {
                    DataMember::Field(field) => visit(field, None),
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            visit(field, Some(variant.name.as_str()));
                        }
                    }
                }
            }
        }
    }
    use carry_derivation::{CarryDerivation, for_each_stored_field};

    fn validate_opaque_data_properties(
        data_definition: &DataDefinition,
        receipts: &[OpaqueDataPropertyReceipt],
        allow_pending_copy: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let properties = data_definition.properties;
        if properties.multiplicity == Multiplicity::Unrestricted
            && !allow_pending_copy
            && !receipts.iter().any(|receipt| {
                receipt.declaration == data_definition.symbol
                    && receipt.kind == OpaqueDataPropertyReceiptKind::Copy
            })
        {
            diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot claim `[copy]` without an admitted property receipt",
            data_definition.name
        )));
        }
        if properties
            .carry
            .is_some_and(|policy| policy != language_semantics::CarryPolicy::STRICT)
        {
            diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot relax its carry policy without an admitted property receipt",
            data_definition.name
        )));
        }
    }

    fn validate_opaque_property_receipts(
        program: &dyn crate::CheckingInput,
        receipts: &[OpaqueDataPropertyReceipt],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (index, receipt) in receipts.iter().enumerate() {
            if receipts[..index].contains(receipt) {
                diagnostics.push(Diagnostic::error(
                    "opaque data property receipts repeat one exact declaration and property",
                ));
                continue;
            }
            let definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == receipt.declaration)
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "opaque data property receipt maps to {} declarations; expected one",
                    definitions.len(),
                )));
                continue;
            };
            if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque {
                diagnostics.push(Diagnostic::error(format!(
                    "opaque data property receipt targets non-opaque declaration `{}`",
                    definition.name,
                )));
                continue;
            }
            match receipt.kind {
            OpaqueDataPropertyReceiptKind::Copy
                if definition.properties.multiplicity
                    == Multiplicity::Unrestricted => {}
            OpaqueDataPropertyReceiptKind::Copy => diagnostics.push(Diagnostic::error(format!(
                "opaque data property receipt grants `[copy]` to `{}`, but that declaration does not claim `[copy]`",
                definition.name,
            ))),
        }
        }
    }

    pub fn effective_data_carry_policy(
        program: &dyn crate::CheckingInput,
        data_definition: &DataDefinition,
    ) -> language_semantics::CarryPolicy {
        CarryDerivation::new(program, &[]).derive_named_data(
            data_definition.symbol,
            data_definition.name.as_str(),
            None,
        )
    }

    pub fn effective_type_carry_policy(
        program: &dyn crate::CheckingInput,
        type_parameters: &[TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> language_semantics::CarryPolicy {
        effective_type_carry_requirements(program, type_parameters, type_reference).structural
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TypeCarryRequirements {
        pub structural: language_semantics::CarryPolicy,
        pub borrow_independent: language_semantics::CarryPolicy,
        pub requires_borrow_preservation: bool,
    }

    pub fn effective_type_carry_requirements(
        program: &dyn crate::CheckingInput,
        type_parameters: &[TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> TypeCarryRequirements {
        CarryDerivation::new(program, type_parameters).derive_requirements(type_reference)
    }

    pub fn type_satisfies_declared_property(
        program: &dyn crate::CheckingInput,
        type_parameters: &[TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
        property: DeclaredPropertyRequirement,
    ) -> bool {
        match property {
            DeclaredPropertyRequirement::Carry(required) => {
                CarryDerivation::new(program, type_parameters)
                    .derive(type_reference)
                    .permits(required)
            }
            DeclaredPropertyRequirement::Copy => type_satisfies_structural_property(
                program,
                type_parameters,
                type_reference,
                &[],
                "copy",
            ),
            DeclaredPropertyRequirement::Linear => type_satisfies_structural_property(
                program,
                type_parameters,
                type_reference,
                &[],
                "linear",
            ),
        }
    }

    pub fn substituted_type_satisfies_copy(
        program: &dyn crate::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> bool {
        type_satisfies_structural_property(program, &[], type_reference, substitutions, "copy")
    }

    fn type_satisfies_structural_property(
        program: &dyn crate::CheckingInput,
        type_parameters: &[TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        property: &str,
    ) -> bool {
        if let Some(storage) = program.layout_storage_arguments(type_reference) {
            return type_satisfies_structural_property(
                program,
                type_parameters,
                storage.semantic_type,
                substitutions,
                property,
            );
        }
        if program
            .type_reference_table
            .primitive_type(type_reference)
            .is_some()
        {
            return property != "linear";
        }

        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, name } => {
                if symbol.is_valid()
                    && let Some(position) = substitutions
                        .iter()
                        .rposition(|(formal, actual)| formal == symbol && *actual != type_reference)
                {
                    let remaining = substitutions
                        .iter()
                        .filter(|(formal, _)| formal != symbol)
                        .copied()
                        .collect::<Vec<_>>();
                    return type_satisfies_structural_property(
                        program,
                        type_parameters,
                        substitutions[position].1,
                        &remaining,
                        property,
                    );
                }
                if let Some(parameter) =
                    type_parameter_for_reference(type_parameters, *symbol, name.as_str())
                {
                    return type_parameter_declares_property(parameter, property);
                }
                named_type_declares_property(program, *symbol, name.as_str(), property)
            }
            TypeReferenceNode::Reference { access, .. } => {
                property == "copy" && *access == language_core::ReferenceAccess::Shared
            }
            TypeReferenceNode::Constrained { base_type, .. } => type_satisfies_structural_property(
                program,
                type_parameters,
                *base_type,
                substitutions,
                property,
            ),
            TypeReferenceNode::FixedArray { element_type, .. } => {
                type_satisfies_structural_property(
                    program,
                    type_parameters,
                    *element_type,
                    substitutions,
                    property,
                )
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                ..
            } => named_type_declares_property(program, *base_symbol, base_name.as_str(), property),
            _ => false,
        }
    }

    pub fn declared_property_requirements(
        properties: &typed_trees::data::DataProperties,
    ) -> Vec<DeclaredPropertyRequirement> {
        let mut names = Vec::new();
        if properties.multiplicity == Multiplicity::Unrestricted {
            names.push(DeclaredPropertyRequirement::Copy);
        }
        if properties.multiplicity == Multiplicity::Linear {
            names.push(DeclaredPropertyRequirement::Linear);
        }
        if let Some(carry) = properties.carry {
            names.push(DeclaredPropertyRequirement::Carry(carry));
        }
        names
    }

    fn type_parameter_for_reference<'program>(
        type_parameters: &'program [TypeParameter],
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Option<&'program TypeParameter> {
        if symbol.is_valid() {
            return type_parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol);
        }
        type_parameters
            .iter()
            .find(|parameter| parameter.name.as_str() == name)
    }

    fn type_parameter_declares_property(parameter: &TypeParameter, property: &str) -> bool {
        match property {
            "copy" => parameter.bounds.multiplicity == Multiplicity::Unrestricted,
            "linear" => parameter.bounds.multiplicity == Multiplicity::Linear,
            _ => false,
        }
    }

    pub(crate) fn referenced_type_parameter<'program>(
        program: &dyn crate::CheckingInput,
        type_parameters: &'program [TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> Option<&'program TypeParameter> {
        if !type_reference.is_valid() {
            return None;
        }
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Named { symbol, name } => {
                type_parameter_for_reference(type_parameters, *symbol, name.as_str())
            }
            TypeReferenceNode::Reference { referee, .. } => {
                referenced_type_parameter(program, type_parameters, *referee)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                referenced_type_parameter(program, type_parameters, *base_type)
            }
            TypeReferenceNode::FixedArray { element_type, .. } => {
                referenced_type_parameter(program, type_parameters, *element_type)
            }
            _ => None,
        }
    }

    fn named_type_declares_property(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        name: &str,
        property: &str,
    ) -> bool {
        program
            .data_definitions()
            .iter()
            .find(|definition| {
                if symbol.is_valid() {
                    definition.symbol == symbol
                } else {
                    definition.name.as_str() == name
                }
            })
            .is_some_and(|definition| match property {
                "copy" => definition.properties.multiplicity == Multiplicity::Unrestricted,
                "linear" => definition.properties.multiplicity == Multiplicity::Linear,
                _ => false,
            })
    }
}

pub mod proposition_entailment {
    use crate::frozen_program::expression_table;
    use std::collections::{BTreeSet, HashSet};
    use typed_trees::expression::ExpressionNode;
    use typed_trees::proposition::ProofSubstitutions;
    use typed_trees::trait_definition::Conformance;

    use diagnostics::Diagnostic;
    use typed_trees::domain::ProofFact;
    use typed_trees::machine::Machine;
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::statement::TableCall;

    pub(crate) fn validate_proposition_entailment(
        program: &dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut call_targets_with_proposition_requires = HashSet::new();
        for callee in program.machines() {
            let has_proposition_requires =
                program.machine_contracts(callee).iter().any(|contract| {
                    contract.kind == SignatureContractKind::Requires
                        && program
                            .proof_facts
                            .span_or_empty(contract.facts)
                            .iter()
                            .any(|fact| matches!(fact, ProofFact::Proposition(_)))
                });
            if has_proposition_requires {
                call_targets_with_proposition_requires.insert(callee.symbol);
                for state in program.machine_states(callee) {
                    call_targets_with_proposition_requires.insert(state.symbol);
                }
            }
        }
        let any_machine_has_proposition_requires =
            !call_targets_with_proposition_requires.is_empty();
        for machine in program.machines() {
            if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
                continue;
            }
            let machine_ensures = proposition_labels(
                program,
                program.machine_contracts(machine),
                SignatureContractKind::Ensures,
                &[],
                false,
            );
            let produced_evidence = produced_evidence_labels(program, machine, diagnostics);
            if machine_ensures.is_empty()
                && !any_machine_has_proposition_requires
                && !program.machine_states(machine).iter().any(|state| {
                    program.state_contracts(state).iter().any(|contract| {
                        contract.kind == SignatureContractKind::Ensures
                            && program
                                .proof_facts
                                .span_or_empty(contract.facts)
                                .iter()
                                .any(|fact| matches!(fact, ProofFact::Proposition(_)))
                    })
                })
            {
                continue;
            }
            let mut machine_requires: Option<Vec<String>> = None;
            let has_proof_output_calls = program
                .proof_output_calls
                .iter()
                .any(|package| package.machine_symbol == machine.symbol);
            for state in program.machine_states(machine) {
                let mut required = machine_ensures.clone();
                required.extend(proposition_labels(
                    program,
                    program.state_contracts(state),
                    SignatureContractKind::Ensures,
                    &[],
                    false,
                ));
                let statements = program.statement_table.statements(state.statement_nodes);
                let last_proposition_requires_call_index = if any_machine_has_proposition_requires {
                    statements.iter().rposition(|statement| {
                        statement.as_call().is_some_and(|call| {
                            call_targets_with_proposition_requires.contains(&call.target_symbol)
                        })
                    })
                } else {
                    None
                };
                if !produced_evidence.is_empty() {
                    required.retain(|goal| !produced_evidence.contains(goal));
                }
                if required.is_empty() && last_proposition_requires_call_index.is_none() {
                    continue;
                }
                let machine_requires = machine_requires.get_or_insert_with(|| {
                    proposition_labels(
                        program,
                        program.machine_contracts(machine),
                        SignatureContractKind::Requires,
                        &[],
                        true,
                    )
                });
                let mut known = machine_requires.iter().cloned().collect::<BTreeSet<_>>();
                known.extend(proposition_labels(
                    program,
                    program.state_contracts(state),
                    SignatureContractKind::Requires,
                    &[],
                    true,
                ));
                required.retain(|goal| !known.contains(goal));
                if required.is_empty() && last_proposition_requires_call_index.is_none() {
                    continue;
                }
                for (statement_index, statement) in statements.iter().enumerate() {
                    let need_ensures = !required.is_empty()
                        || last_proposition_requires_call_index
                            .is_some_and(|last_index| statement_index <= last_index);
                    if has_proof_output_calls && need_ensures {
                        intake_proof_output_propositions(
                            program,
                            machine,
                            state.symbol,
                            statement_index,
                            &mut known,
                        );
                    }
                    if let Some(call) = statement.as_call() {
                        let call_has_proposition_requires =
                            call_targets_with_proposition_requires.contains(&call.target_symbol);
                        let call_need_ensures = !required.is_empty()
                            || last_proposition_requires_call_index
                                .is_some_and(|last_index| statement_index < last_index);
                        if call_has_proposition_requires || call_need_ensures {
                            intake_call_propositions(
                                program,
                                machine,
                                state,
                                call,
                                call_has_proposition_requires,
                                call_need_ensures,
                                &mut known,
                                diagnostics,
                            );
                        }
                    }
                }
                if has_proof_output_calls && !required.is_empty() {
                    intake_proof_output_propositions(
                        program,
                        machine,
                        state.symbol,
                        statements.len(),
                        &mut known,
                    );
                }
                required.retain(|goal| !known.contains(goal));
                if required.is_empty() {
                    continue;
                }

                let structural_proposition_goals = program
                    .machine_contracts(machine)
                    .iter()
                    .chain(program.state_contracts(state))
                    .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                    .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                    .filter_map(|fact| match fact {
                        ProofFact::Proposition(application) => program
                            .normalize_proposition_application(application, None)
                            .map(|formula| (formula.identity_label(), application)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                for goal in required {
                    if known.contains(&goal) || produced_evidence.contains(&goal) {
                        continue;
                    }
                    let instruction_flow_obligation =
                    structural_proposition_goals
                        .iter()
                        .any(|(label, application)| {
                            label == &goal
                                && crate::proof_contracts::instruction_effects::instruction_effect_contract(
                                    program,
                                    application,
                                )
                                .is_some()
                        });
                    if instruction_flow_obligation {
                        continue;
                    }
                    let structurally_entailed =
                        structural_proposition_goals
                            .iter()
                            .any(|(label, application)| {
                                label == &goal
                                    && crate::transparent_proposition_application_entailed(
                                        program,
                                        machine,
                                        state,
                                        application,
                                        None,
                                    )
                            });
                    if !structurally_entailed {
                        diagnostics.push(Diagnostic::error(format!(
                        "checked machine `{}` cannot establish proposition ensure `{goal}` in state `{}`; require it, cite a checked/accepted proof that ensures it, or supply its declared evidence",
                        machine.name.as_str(),
                        state.name.as_str(),
                    )));
                    }
                }
            }
        }
    }

    fn intake_proof_output_propositions(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        known: &mut BTreeSet<String>,
    ) {
        for package in &program.proof_output_calls {
            if package.machine_symbol != caller.symbol
                || package.state_symbol != state_symbol
                || package.statement_index != statement_index
            {
                continue;
            }
            let ExpressionNode::Call(call) = expression_table(program).expression(package.call)
            else {
                continue;
            };
            let Some(callee) = program.machine_by_own_or_state_symbol(call.target_symbol) else {
                continue;
            };
            known.extend(proposition_labels(
                program,
                program.machine_contracts(callee),
                SignatureContractKind::Ensures,
                &[],
                true,
            ));
        }
    }

    fn produced_evidence_labels(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Vec<String> {
        if program.evidence_forwardings.is_empty() {
            return Vec::new();
        }
        let mut labels = Vec::new();
        for assignment in &program.evidence_forwardings {
            if assignment.machine_symbol != machine.symbol {
                continue;
            }
            let Some(conformance) = assignment.source_conformance else {
                continue;
            };
            let Some(contract) = program.machine_contracts(machine).iter().find(|contract| {
                contract.kind == SignatureContractKind::Ensures
                    && contract
                        .binding
                        .as_ref()
                        .is_some_and(|binding| binding == &assignment.target)
            }) else {
                continue;
            };
            let Some((expected_interface_label, expected_interface)) =
                program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
                    .find_map(|fact| {
                        let application = fact.as_proposition()?;
                        let normalized =
                            program.normalize_nominal_proposition_application(application, None)?;
                        match normalized.classification {
                    typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => Some((evidence, interface)),
                    typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
                }
                    })
            else {
                continue;
            };
            let Some(expected_interface) = expected_interface else {
                diagnostics.push(Diagnostic::error(format!(
                "named implementation `{}` cannot provide unresolved generic evidence interface `{expected_interface_label}` required by `{}`",
                assignment.source, assignment.target
            )));
                continue;
            };
            if select_named_evidence_implementation(
                program,
                conformance,
                assignment.source.as_str(),
                &expected_interface,
            )
            .is_none()
            {
                diagnostics.push(Diagnostic::error(format!(
                "named implementation `{}` does not provide the exact `{expected_interface_label}` evidence interface required by `{}`",
                assignment.source, assignment.target
            )));
                continue;
            }
            labels.extend(proposition_labels(
                program,
                std::slice::from_ref(contract),
                SignatureContractKind::Ensures,
                &[],
                false,
            ));
        }
        labels
    }

    pub fn select_named_evidence_implementation<'program>(
        program: &'program dyn crate::CheckingInput,
        conformance_symbol: symbols::SymbolHandle,
        source_name: &str,
        expected_interface: &typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
    ) -> Option<(&'program Conformance, symbols::SymbolHandle)> {
        let conformance = program.conformance_by_symbol(conformance_symbol)?;
        if conformance.alias.as_ref()?.as_str() != source_name {
            return None;
        }
        let evidence_trait = program
            .traits()
            .iter()
            .find(|candidate| candidate.name == conformance.trait_name)?;
        let selected_interface = typed_trees::proposition::NormalizedEvidenceInterfaceIdentity {
            trait_symbol: evidence_trait.symbol,
            arguments: program
                .type_reference_table
                .type_reference_handles(conformance.arguments)
                .iter()
                .map(|argument| program.normalized_type_identity(*argument))
                .collect(),
            requirements: expected_interface.requirements.clone(),
        };
        (&selected_interface == expected_interface).then_some((conformance, evidence_trait.symbol))
    }

    fn intake_call_propositions(
        program: &dyn crate::CheckingInput,
        caller: &Machine,
        caller_state: &typed_trees::state::State,
        call: &TableCall,
        has_proposition_requires: bool,
        need_ensures: bool,
        known: &mut BTreeSet<String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some((callee, callee_state)) = call_target(program, call) else {
            return;
        };
        let callee_contracts = program.machine_contracts(callee);
        let has_relevant_ensures = need_ensures
            && callee_contracts.iter().any(|contract| {
                contract.kind == SignatureContractKind::Ensures
                    && program
                        .proof_facts
                        .span_or_empty(contract.facts)
                        .iter()
                        .any(|fact| {
                            matches!(fact, ProofFact::Proposition(_) | ProofFact::Expression(_))
                        })
            });
        if !has_proposition_requires && !has_relevant_ensures {
            return;
        }
        let substitutions =
            call_parameter_substitutions(program, call, program.state_parameters(callee_state));
        if has_proposition_requires {
            let arguments = program.statement_table.expression_handles(call.arguments);
            let missing = callee_contracts
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Requires)
                .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                .filter_map(ProofFact::as_proposition)
                .filter_map(|application| {
                    Some((
                        application,
                        application_label(program, application, &substitutions)?,
                    ))
                })
                .find(|(application, label)| {
                    !known.contains(label)
                        && !crate::transparent_call_requirement_entailed(
                            program,
                            caller,
                            caller_state,
                            callee,
                            callee_state,
                            arguments,
                            application,
                        )
                });
            if let Some((_, missing)) = missing {
                diagnostics.push(Diagnostic::error(format!(
                "checked machine `{}` cannot cite `{}`: proposition requirement `{missing}` is not established at the call",
                caller.name.as_str(),
                callee.name.as_str(),
            )));
                return;
            }
        }
        if has_relevant_ensures {
            known.extend(proposition_labels(
                program,
                callee_contracts,
                SignatureContractKind::Ensures,
                &substitutions,
                true,
            ));
        }
    }

    fn call_target<'a>(
        program: &'a dyn crate::CheckingInput,
        call: &TableCall,
    ) -> Option<(&'a Machine, &'a typed_trees::state::State)> {
        if let Some(machine) = program.machine_by_symbol(call.target_symbol) {
            return Some((machine, program.machine_states(machine).first()?));
        }
        let machine = program.machine_holding_state(call.target_symbol)?;
        let state = program.state_in_machine(machine, call.target_symbol)?;
        Some((machine, state))
    }

    fn call_parameter_substitutions(
        program: &dyn crate::CheckingInput,
        call: &TableCall,
        parameters: &[StateParameter],
    ) -> Vec<(symbols::SymbolHandle, String, String)> {
        let arguments = program.statement_table.expression_handles(call.arguments);
        let mut argument_index = 0usize;
        parameters
            .iter()
            .map(|parameter| {
                let label = {
                    let label = arguments
                        .get(argument_index)
                        .map(|argument| expression_table(program).display_name(*argument))
                        .unwrap_or_else(|| parameter.name.as_str().to_owned());
                    argument_index = argument_index.saturating_add(1);
                    label
                };
                (parameter.symbol, parameter.name.as_str().to_owned(), label)
            })
            .collect()
    }

    fn proposition_labels(
        program: &dyn crate::CheckingInput,
        contracts: &[SignatureContract],
        kind: SignatureContractKind,
        substitutions: &[(symbols::SymbolHandle, String, String)],
        include_boolean_expressions: bool,
    ) -> Vec<String> {
        contracts
            .iter()
            .filter(|contract| contract.kind == kind)
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .filter_map(|fact| {
                let Some(application) = fact.as_proposition() else {
                    return match fact {
                        ProofFact::Expression(expression) if include_boolean_expressions => {
                            Some(format!(
                                "boolean:{}",
                                program.render_proof_expression(
                                    *expression,
                                    ProofSubstitutions::ByParameter(substitutions)
                                )
                            ))
                        }
                        ProofFact::Expression(_)
                        | ProofFact::Membership(_)
                        | ProofFact::Proposition(_) => None,
                    };
                };
                application_label(program, application, substitutions)
            })
            .collect()
    }

    fn application_label(
        program: &dyn crate::CheckingInput,
        application: &typed_trees::proposition::PropositionApplication,
        substitutions: &[(symbols::SymbolHandle, String, String)],
    ) -> Option<String> {
        let binder_labels = application
            .binder_arguments
            .iter()
            .map(|argument| {
                substitutions
                    .iter()
                    .find(|(symbol, _, _)| *symbol == argument.symbol)
                    .map(|(_, _, label)| label.clone())
                    .unwrap_or_else(|| argument.display_name())
            })
            .collect::<Vec<_>>();
        let argument_labels = expression_table(program)
            .expression_handles(application.arguments)
            .iter()
            .map(|argument| {
                program.render_proof_expression(
                    *argument,
                    ProofSubstitutions::ByParameter(substitutions),
                )
            })
            .collect::<Vec<_>>();
        program.normalized_identity_label(application, &binder_labels, &argument_labels)
    }
}

pub mod qualification_evidence {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use crate::value_custody::places::unwrapped_type_reference;
    use crate::value_custody::type_references::type_references_match;
    use checked_trees::BoundaryQualificationAuthorization;
    use diagnostics::Diagnostic;
    use language_semantics::DomainEstablishmentRoute;
    use language_semantics::MachineSupplyMode;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::signature::StateSignature;
    use typed_trees::types::TypeReferenceHandle;

    pub(crate) mod calls {
        use checked_trees::BoundaryQualificationAuthorization;
        use language_semantics::MachineSupplyMode;
        use symbols::SymbolHandle;
        use typed_trees::machine::Machine;
        use typed_trees::signature::{SignatureContractKind, StateSignature};

        pub enum CallQualificationSource<'program> {
            CheckedBody(&'program Machine),
            AdmittedRequirement(BoundaryQualificationAuthorization),
            Propagated,
        }

        pub fn call_qualification_source(
            program: &dyn crate::CheckingInput,
            target: SymbolHandle,
            authorization: Option<BoundaryQualificationAuthorization>,
        ) -> Option<CallQualificationSource<'_>> {
            CallQualificationTarget::resolve(program, target).fact_source(authorization)
        }

        pub(crate) enum CallQualificationTarget<'program> {
            CheckedBody(&'program Machine),
            AdmittedRequirement,
            BoundaryRequirement {
                invoked: SymbolHandle,
                declaration: SymbolHandle,
                signature: SymbolHandle,
            },
            Propagated,
        }

        impl<'program> CallQualificationTarget<'program> {
            fn resolve(program: &'program dyn crate::CheckingInput, target: SymbolHandle) -> Self {
                if let Some(machine) = program.machine_by_own_or_state_symbol(target) {
                    return if machine.body_is_present
                        && matches!(
                            machine.supply_mode,
                            MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
                        ) {
                        Self::CheckedBody(machine)
                    } else {
                        Self::AdmittedRequirement
                    };
                }
                if let Some((declaration, signature)) =
                    crate::selected_requirement_signature(program, target)
                        .filter(|(declaration, _)| declaration.is_boundary)
                {
                    return Self::BoundaryRequirement {
                        invoked: target,
                        declaration: declaration.symbol,
                        signature: signature.symbol,
                    };
                }
                if program
                    .trait_definition_by_symbol(target)
                    .is_some_and(|definition| definition.is_boundary)
                {
                    return Self::AdmittedRequirement;
                }
                Self::Propagated
            }

            fn fact_source(
                &self,
                authorization: Option<BoundaryQualificationAuthorization>,
            ) -> Option<CallQualificationSource<'program>> {
                match self {
                    Self::CheckedBody(machine) => authorization
                        .is_none()
                        .then_some(CallQualificationSource::CheckedBody(machine)),
                    Self::AdmittedRequirement | Self::BoundaryRequirement { .. } => {
                        authorization.map(CallQualificationSource::AdmittedRequirement)
                    }
                    Self::Propagated => Some(CallQualificationSource::Propagated),
                }
            }
        }

        pub struct CallQualificationContract<'program> {
            program: &'program dyn crate::CheckingInput,
            target: CallQualificationTarget<'program>,
            clauses: CallQualificationClauses<'program>,
        }

        pub(crate) enum CallQualificationClauses<'program> {
            ActualBody,
            Requirement {
                declaration: SymbolHandle,
                signature: &'program StateSignature,
            },
            UnresolvedRequirement,
        }

        impl<'program> CallQualificationContract<'program> {
            pub fn prepare(
                program: &'program dyn crate::CheckingInput,
                target: SymbolHandle,
                selected: Option<&'program StateSignature>,
            ) -> Self {
                let clauses = match selected {
                    None => match crate::selected_requirement_signature(program, target) {
                        Some((declaration, signature)) => CallQualificationClauses::Requirement {
                            declaration: declaration.symbol,
                            signature,
                        },
                        None => CallQualificationClauses::ActualBody,
                    },
                    Some(signature) => {
                        match crate::selected_requirement_signature(program, signature.symbol) {
                            Some((declaration, _)) => CallQualificationClauses::Requirement {
                                declaration: declaration.symbol,
                                signature,
                            },
                            None => CallQualificationClauses::UnresolvedRequirement,
                        }
                    }
                };
                Self {
                    program,
                    target: CallQualificationTarget::resolve(program, target),
                    clauses,
                }
            }

            pub fn declares_membership_obligation(
                &self,
                kind: SignatureContractKind,
                fact: arena::Handle<typed_trees::domain::ProofFact>,
            ) -> bool {
                let (
                    CallQualificationTarget::BoundaryRequirement {
                        invoked,
                        declaration: actual_declaration,
                        signature: actual_signature,
                    },
                    CallQualificationClauses::Requirement {
                        declaration,
                        signature,
                    },
                ) = (&self.target, &self.clauses)
                else {
                    return false;
                };
                if kind != SignatureContractKind::Ensures
                    || actual_declaration != declaration
                    || !self.program.proof_facts.is_valid(fact)
                    || self.program.proof_facts.get(fact).as_membership().is_none()
                {
                    return false;
                }
                if self
                    .program
                    .closed_requirement_application(*invoked)
                    .is_some()
                    && *actual_signature != signature.symbol
                {
                    return false;
                }
                if let (Some(actual), Some(selected)) = (
                    self.program
                        .closed_requirement_application(*actual_signature),
                    self.program
                        .closed_requirement_application(signature.symbol),
                ) && actual.trait_application_identity != selected.trait_application_identity
                {
                    return false;
                }
                let declaration_signature = |selected| {
                    self.program
                        .closed_requirement_application(selected)
                        .map_or(Some(selected), |application| {
                            (application.declaring_trait == *declaration)
                                .then_some(application.template)
                        })
                };
                if declaration_signature(*actual_signature).is_none()
                    || declaration_signature(*actual_signature)
                        != declaration_signature(signature.symbol)
                {
                    return false;
                }
                self.program
                    .state_signature_contracts(signature)
                    .iter()
                    .any(|contract| {
                        contract.kind == kind && contract.facts.handles().any(|owned| owned == fact)
                    })
            }

            pub fn fact_source(
                &self,
                kind: SignatureContractKind,
                fact: &typed_trees::domain::ProofFact,
            ) -> Option<CallQualificationSource<'program>> {
                let authorization = match &self.clauses {
                    CallQualificationClauses::ActualBody => None,
                    CallQualificationClauses::Requirement {
                        declaration,
                        signature,
                    } => super::boundary_qualification_authorization(
                        self.program,
                        *declaration,
                        signature,
                        kind,
                        fact,
                    ),
                    CallQualificationClauses::UnresolvedRequirement => return None,
                };
                self.target.fact_source(authorization)
            }
        }
    }
    pub use calls::{
        CallQualificationContract, CallQualificationSource, call_qualification_source,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BoundaryQualificationObligation {
        pub declaring_trait: SymbolHandle,
        pub requirement: SymbolHandle,
        pub realization_state: SymbolHandle,
        pub domain: SymbolHandle,
        pub semantic_domain: language_semantics::SemanticDomainId,
        pub parameter: SymbolHandle,
    }

    pub fn boundary_provider_qualification_obligations(
        program: &dyn CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<BoundaryQualificationObligation> {
        use typed_trees::trait_definition::AppliedTraitScope;
        let mut obligations = Vec::new();
        let mut append = |application: &AppliedTraitScope<'_>,
                          requirement: &StateSignature,
                          state: &typed_trees::state::State| {
            let definition = application.declaration;
            if !definition.is_boundary {
                return;
            }
            let requirement =
                match program.applied_callable_signature(application, requirement.symbol) {
                    Some(applied) => applied,
                    None if requirement.type_parameters.is_empty()
                        && application.substitutions.is_empty()
                        && definition.lifetime_parameters.is_empty() =>
                    {
                        requirement
                    }
                    None => return,
                };
            let Some(actual) =
                crate::inherited_satisfier_parameters(program, definition, requirement, state)
            else {
                return;
            };
            let formal = program.state_signature_parameters(requirement);
            for contract in program.state_signature_contracts(requirement) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if boundary_qualification_authorization(
                        program,
                        definition.symbol,
                        requirement,
                        contract.kind.clone(),
                        fact,
                    )
                    .is_none()
                    {
                        continue;
                    }
                    let Some(membership) = fact.as_membership() else {
                        continue;
                    };
                    if !membership.domain_symbol.is_valid() {
                        continue;
                    }
                    let parameter = if expression_table(program)
                        .names_reserved_result(membership.value)
                    {
                        SymbolHandle::invalid()
                    } else {
                        let Some(position) = formal.iter().position(|parameter| {
                            expression_table(program).names_parameter(membership.value, parameter)
                        }) else {
                            continue;
                        };
                        let Some(parameter) = actual.get(position) else {
                            continue;
                        };
                        parameter.symbol
                    };
                    obligations.push(BoundaryQualificationObligation {
                        declaring_trait: definition.symbol,
                        requirement: requirement.symbol,
                        realization_state: state.symbol,
                        domain: membership.domain_symbol,
                        semantic_domain: membership.semantic_domain,
                        parameter,
                    });
                }
            }
        };
        for conformance in program.conformances() {
            if !conformance
                .rows
                .iter()
                .any(|row| row.realization_machine == machine.symbol)
            {
                continue;
            }
            let Some(root) =
                AppliedTraitScope::from_conformance(program.typed_program(), conformance)
            else {
                continue;
            };
            for row in conformance
                .rows
                .iter()
                .filter(|row| row.realization_machine == machine.symbol)
            {
                let Some(application) =
                    root.declaring_scope(program.typed_program(), row.declaring_trait)
                else {
                    continue;
                };
                let Some(requirement) = program
                    .trait_machine_signature_by_symbol(application.declaration, row.requirement)
                else {
                    continue;
                };
                let Some(state) = program.state_in_machine(machine, row.realization_state) else {
                    continue;
                };
                append(&application, requirement, state);
            }
        }
        if let Some(entry) = program.machine_states(machine).first() {
            for conformance in program.machine_trait_conformances(machine) {
                let Some(application) = AppliedTraitScope::from_machine_conformance(
                    program.typed_program(),
                    machine,
                    conformance,
                ) else {
                    continue;
                };
                for requirement in program.trait_machine_signatures(application.declaration) {
                    if conformance.requirement.as_ref() == Some(&requirement.name) {
                        append(&application, requirement, entry);
                    }
                }
            }
        }
        obligations
    }

    pub(crate) fn validate_qualification_authorization(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        validate_boundary_requirements(program, diagnostics);
        validate_external_machine_claims(program, diagnostics);
    }

    fn validate_boundary_requirements(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for trait_definition in program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
        {
            for signature in program.trait_machine_signatures(trait_definition) {
                for contract in program
                    .state_signature_contracts(signature)
                    .iter()
                    .filter(|contract| contract.kind == SignatureContractKind::Ensures)
                {
                    for fact in program.proof_facts.span_or_empty(contract.facts) {
                        let Some(membership) = fact.as_membership() else {
                            continue;
                        };
                        if let Some(permission) =
                            crate::proof_contracts::proof_facts::carry_permission(
                                program,
                                membership.domain,
                            )
                        {
                            if !expression_table(program).names_reserved_result(membership.value)
                                && mutable_parameter(program, signature, membership.value).is_none()
                            {
                                diagnostics.push(Diagnostic::error(format!(
                                "boundary requirement `{}::{}` may admit carry permission `{permission}` only for its exact `result` or an exact mutable parameter",
                                trait_definition.name, signature.name,
                            )));
                            }
                            continue;
                        }
                        if !membership.domain_symbol.is_valid() {
                            continue;
                        }
                        let Some(domain) = program.domain_definition(membership.domain_symbol)
                        else {
                            continue;
                        };

                        let subject =
                            if expression_table(program).names_reserved_result(membership.value) {
                                Some((signature.return_type, "result"))
                            } else {
                                mutable_parameter(program, signature, membership.value).map(
                                    |parameter| (parameter.type_reference, parameter.name.as_str()),
                                )
                            };
                        let Some((subject, subject_name)) = subject else {
                            diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` may admit domain `{}` only for its exact `result` or an exact mutable parameter; move the membership guarantee to `ensures result in {}` or name a mutable parameter",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            domain.name,
                        )));
                            continue;
                        };

                        let subject_carrier = unwrapped_type_reference(program, subject);
                        let domain_carrier = unwrapped_type_reference(program, domain.target_type);
                        if !matches!(
                            (subject_carrier, domain_carrier),
                            (Some(subject_carrier), Some(domain_carrier))
                                if type_references_match(program, subject_carrier, domain_carrier)
                                    || lifetime_erased_nominal_carriers_match(
                                        program,
                                        subject_carrier,
                                        domain_carrier,
                                    )
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` cannot admit `{subject_name} in {}`: {} carrier `{}` does not match domain target `{}`",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            if language_core::is_reserved_result(subject_name) {
                                "result"
                            } else {
                                "parameter"
                            },
                            program.display_type_reference_with_constraints(subject),
                            program.display_type_reference_with_constraints(domain.target_type),
                        )));
                        }
                    }
                }
            }
        }
    }

    fn lifetime_erased_nominal_carriers_match(
        program: &dyn CheckingInput,
        application: typed_trees::types::TypeReferenceHandle,
        nominal: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program.type_reference_table.type_reference(application)
        else {
            return false;
        };
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(nominal)
        else {
            return false;
        };
        *base_symbol == *symbol
            && symbol.is_valid()
            && program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty()
            && crate::declarations::data_queries::data_definition_by_symbol(program, *base_symbol)
                .is_some_and(|definition| {
                    !definition.lifetime_parameters.is_empty()
                        && definition.lifetime_parameters.len() == lifetime_arguments.len()
                })
    }

    fn validate_external_machine_claims(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for machine in program.machines().iter().filter(|machine| {
            matches!(
                machine.supply_mode,
                MachineSupplyMode::AdmissionClaim
                    | MachineSupplyMode::Requirement
                    | MachineSupplyMode::ExternalRealization { .. }
            )
        }) {
            for contract in program
                .machine_contracts(machine)
                .iter()
                .chain(
                    program
                        .machine_states(machine)
                        .iter()
                        .flat_map(|state| program.state_contracts(state)),
                )
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let Some(membership) = fact.as_membership() else {
                        continue;
                    };
                    if let Some(permission) = crate::proof_contracts::proof_facts::carry_permission(
                        program,
                        membership.domain,
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                        "external machine `{}` cannot directly admit carry permission `{permission}`; publish it on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                        machine.name,
                    )));
                        continue;
                    }
                    if !membership.domain_symbol.is_valid() {
                        continue;
                    }
                    let domain_name = program
                        .domain_definition(membership.domain_symbol)
                        .map(|domain| domain.name.as_str())
                        .unwrap_or("<unknown>");
                    diagnostics.push(Diagnostic::error(format!(
                    "external machine `{}` cannot directly admit domain membership `{}`; publish `ensures result in {}` on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                    machine.name, domain_name, domain_name,
                )));
                }
            }
        }
    }

    fn mutable_parameter<'a>(
        program: &'a dyn CheckingInput,
        signature: &'a typed_trees::signature::StateSignature,
        expression: ExpressionHandle,
    ) -> Option<&'a typed_trees::signature::StateParameter> {
        program
            .state_signature_parameters(signature)
            .iter()
            .find(|parameter| {
                is_mutable_reference(program, parameter.type_reference)
                    && expression_table(program).names_parameter(expression, parameter)
            })
    }

    fn is_mutable_reference(
        program: &dyn CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                *access == language_core::ReferenceAccess::Mutable
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                is_mutable_reference(program, *base_type)
            }
            _ => false,
        }
    }

    pub fn boundary_qualification_authorization(
        program: &dyn CheckingInput,
        owner_symbol: SymbolHandle,
        signature: &StateSignature,
        contract_kind: SignatureContractKind,
        fact: &typed_trees::domain::ProofFact,
    ) -> Option<BoundaryQualificationAuthorization> {
        if contract_kind != SignatureContractKind::Ensures
            || !program
                .traits()
                .iter()
                .any(|definition| definition.symbol == owner_symbol && definition.is_boundary)
        {
            return None;
        }

        let membership = fact.as_membership()?;
        let subject_carrier = if expression_table(program).names_reserved_result(membership.value) {
            signature.return_type
        } else {
            mutable_parameter(program, signature, membership.value)?.type_reference
        };
        if crate::proof_contracts::proof_facts::carry_permission(program, membership.domain)
            .is_some()
        {
            return Some(BoundaryQualificationAuthorization {
                requirement_symbol: owner_symbol,
                signature_symbol: signature.symbol,
            });
        }
        if !membership.domain_symbol.is_valid() {
            return None;
        }
        let domain = program.domain_definition(membership.domain_symbol)?;
        if membership.authored_domain_selection.is_none()
            || membership.domain_arguments.len() != domain.type_parameters.len()
        {
            return None;
        }
        let authority_signature = program
            .closed_requirement_application(signature.symbol)
            .filter(|application| application.declaring_trait == owner_symbol)
            .map_or(signature.symbol, |application| application.template);
        if !domain.establishment_routes.iter().any(|route| {
            matches!(
                route,
                DomainEstablishmentRoute::BoundaryRequirement {
                    boundary_trait,
                    requirement,
                } if *boundary_trait == owner_symbol && *requirement == authority_signature
            )
        }) {
            return None;
        }
        if !return_type_matches_domain_target(program, subject_carrier, domain.target_type) {
            return None;
        }

        Some(BoundaryQualificationAuthorization {
            requirement_symbol: owner_symbol,
            signature_symbol: signature.symbol,
        })
    }

    fn return_type_matches_domain_target(
        program: &dyn CheckingInput,
        return_type: TypeReferenceHandle,
        domain_target: TypeReferenceHandle,
    ) -> bool {
        if program
            .unwrapped_type_reference(return_type)
            .zip(program.unwrapped_type_reference(domain_target))
            .is_some_and(|(left, right)| {
                program.normalized_type_identity(left) == program.normalized_type_identity(right)
            })
        {
            return true;
        }
        nominal_head_symbol(program, return_type)
            .zip(nominal_head_symbol(program, domain_target))
            .is_some_and(|(left, right)| left == right)
    }

    fn nominal_head_symbol(
        program: &dyn CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<SymbolHandle> {
        let unwrapped = program.unwrapped_type_reference(type_reference)?;
        program
            .type_reference_table
            .closed_nominal_symbol(unwrapped)
    }
}

pub mod relevance {
    //! Runtime noninterference and fail-closed fences for occurrence-level
    //! `[erased]` bindings: data fields, case payload fields, signature
    //! parameters, and `let` locals.
    //!
    //! The executable slice supports transparent records and sums, including
    //! constructors joined to their exact closed receiving application, and closed
    //! plain data whose attached machines are ordinary checked bodies. The full
    //! semantic tree remains intact for proofs and ownership; native lowering later strips erased literal fields from its
    //! private runtime expression graph and attached-machine storage/topology.
    //!
    //! Every binding occurrence shares one contract
    //! (wiki/spec/proofs/contracts.md#explicit-erased-bindings): the binding may
    //! supply proof computation but cannot determine runtime data or control.
    //! Contracts, invariants, and proof machines are `Context::Proof` and never
    //! walk here as runtime reads. An erased binding's own initializer -- a
    //! field initializer in a struct literal, a `let x [erased]` initializer, or
    //! the argument supplied to an erased parameter position -- is
    //! `Context::ErasedInitializer`: it may read erased bindings but cannot call
    //! a runtime machine or perform an atomic operation, because nothing at
    //! runtime will ever observe the result. Runtime reads of erased parameters
    //! and locals are resolved by symbol against the current state's own
    //! parameters and `let` statements; parameter positions align arguments with
    //! the callee's non-`self` parameters. A statement call's receiver is its
    //! callee's `self` operand -- a runtime place, not a proof position -- so a
    //! runtime call through an erased binding or erased field projection rejects
    //! with the same diagnostic as a direct read.

    use crate::CheckingInput;
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Context {
        Runtime,
        ErasedInitializer,
        Proof,
    }

    pub(crate) fn validate_relevance(
        program: &dyn CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let proof_only = crate::proof_only_classification(program);
        validate_erased_field_shapes(program, diagnostics);

        for machine in program.machines() {
            let machine_context = if proof_only.is_proof_machine(program, machine)
                || crate::mathematical_applications::is_indexed_logical_machine(program, machine)
                || crate::value_custody::content_projections::is_content_projection_machine(
                    program, machine,
                ) {
                Context::Proof
            } else {
                Context::Runtime
            };
            for state in program.machine_states(machine) {
                macro_rules! relevance_child {
                    ($expression:expr, $context:expr $(,)?) => {
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            $expression,
                            $context,
                            diagnostics,
                        )
                    };
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    match statement {
                        StatementNode::Assembly(_) => {}
                        StatementNode::AssemblyFact(fact) => {
                            for expression in program.tables.proof_fact_expression_roots(fact.fact)
                            {
                                relevance_child!(expression, Context::Proof);
                            }
                        }
                        StatementNode::Assignment(assignment) => {
                            relevance_child!(assignment.target, machine_context);
                            relevance_child!(assignment.value, machine_context);
                        }
                        StatementNode::Call(call) => {
                            let argument_context = if machine_context == Context::Proof
                                || call_targets_proof_machine(
                                    program,
                                    &proof_only,
                                    call.target_symbol,
                                ) {
                                Context::Proof
                            } else {
                                Context::Runtime
                            };
                            if argument_context == Context::Runtime {
                                runtime_uses::validate_statement_call_receiver(
                                    program,
                                    machine,
                                    state,
                                    call,
                                    diagnostics,
                                );
                            }
                            let callee_parameters = callee_parameters(program, call.target_symbol);
                            for (position, argument) in program
                                .statement_table
                                .expression_handles(call.arguments)
                                .iter()
                                .enumerate()
                            {
                                relevance_child!(
                                    *argument,
                                    argument_position_context(
                                        argument_context,
                                        callee_parameters,
                                        position,
                                    ),
                                );
                            }
                        }
                        StatementNode::Expression(expression) => {
                            relevance_child!(*expression, machine_context)
                        }
                        StatementNode::LocalData(local) => relevance_child!(
                            local.initial_value,
                            if machine_context == Context::Runtime && local.relevance.is_erased() {
                                Context::ErasedInitializer
                            } else {
                                machine_context
                            },
                        ),
                        StatementNode::Transition(transition) => {
                            if let TransitionGuardNode::When(guard) = transition.guard {
                                relevance_child!(guard, machine_context);
                            }
                            for target in transition.targets() {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } => {
                                        let target_parameters =
                                            callee_parameters(program, path.symbol);
                                        for (position, argument) in program
                                            .statement_table
                                            .expression_handles(*arguments)
                                            .iter()
                                            .enumerate()
                                        {
                                            let position_context = argument_position_context(
                                                machine_context,
                                                target_parameters,
                                                position,
                                            );
                                            relevance_child!(*argument, position_context);
                                        }
                                    }
                                    TransitionTargetNode::Value(value) => {
                                        relevance_child!(*value, machine_context)
                                    }
                                    TransitionTargetNode::Terminal => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn dispatch_subject_of_erased_binding(
        program: &dyn CheckingInput,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> bool {
        if !local.name.as_str().starts_with("__transition_subject")
            || !local.initial_value.is_valid()
        {
            return false;
        }
        let mut expression = local.initial_value;
        let mut symbol = symbols::SymbolHandle::invalid();
        for _ in 0..8 {
            match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                    expression = borrow.target;
                }
                typed_trees::expression::ExpressionNode::Name(path) => {
                    symbol = path.symbol;
                    break;
                }
                _ => return false,
            }
        }
        if !symbol.is_valid() {
            return false;
        }
        program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter.symbol == symbol && parameter.relevance.is_erased())
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    statement.as_local_data().is_some_and(|binding| {
                        binding.symbol == symbol && binding.relevance.is_erased()
                    })
                })
    }

    pub(crate) mod shape_admission {
        use diagnostics::Diagnostic;
        use language_core::BindingRelevance;
        use language_semantics::DataSupplyMode;
        use typed_trees::data::{DataDefinition, DataField, DataMember, DataShapeKind};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn validate_erased_field_shapes(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for definition in program.data_definitions() {
                let erased = erased_fields(program, definition);
                if erased.is_empty() {
                    continue;
                }
                let field_names = erased
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if definition.supply_mode == DataSupplyMode::BoundaryOpaque {
                    diagnostics.push(unsupported(
                        definition,
                        &field_names,
                        "boundary-opaque ABI data",
                    ));
                }
                if program
                    .access_view_plans
                    .iter()
                    .any(|plan| plan.data_symbol == definition.symbol)
                {
                    diagnostics.push(unsupported(definition, &field_names, "access-view data"));
                }
                let has_attached_machines = program
                    .machines()
                    .iter()
                    .any(|machine| machine.attached_data.as_ref() == Some(&definition.name));
                if has_attached_machines
                    && program.data_type_parameters(definition).is_empty()
                    && !erased_attached_machine_record_is_admitted(program, definition)
                {
                    diagnostics.push(unsupported(
                        definition,
                        &field_names,
                        "data with attached machines",
                    ));
                }
            }

            validate_unresolved_erased_generic_uses(program, diagnostics);
            validate_erased_binding_qualifiers(program, diagnostics);
            validate_erased_runtime_scalar_formals(program, diagnostics);
            crate::erased_actuals::validate(program, diagnostics);
        }

        /// Whether an expression clause of the machine's contracts names
        /// exactly the field path `fields` below `parameter`.
        fn contract_names_field(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            parameter: symbols::SymbolHandle,
            fields: &[symbols::SymbolHandle],
        ) -> bool {
            let mut expressions = Vec::new();
            crate::queries::expression_traversal::collect_expression_trees(
                program,
                program
                    .machine_contracts(machine)
                    .iter()
                    .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                    .filter_map(|fact| fact.as_expression().copied()),
                &mut expressions,
            );
            expressions.iter().any(|expression| {
                crate::canonical_place_from_expression(program, *expression).is_some_and(|place| {
                    place.root.as_symbol() == Some(parameter)
                        && place.segments.len() == fields.len()
                        && place.segments.iter().zip(fields).all(|(segment, field)| {
                            matches!(segment, facts::PlaceSegment::Field { symbol } if symbol == field)
                        })
                })
            })
        }

        fn validate_erased_runtime_scalar_formals(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let proof_only = crate::proof_only_classification(program);
            for machine in program.machines() {
                if proof_only.is_proof_machine(program, machine) {
                    continue;
                }
                for state in program.machine_states(machine) {
                    for parameter in program.state_parameters(state) {
                        if !parameter.relevance.is_erased() || !parameter.type_reference.is_valid()
                        {
                            continue;
                        }
                        let lanes = program.erased_scalar_lanes(parameter.type_reference);
                        // A contract over a record's `bool` field is read by
                        // the Boolean vocabulary, which names a formal's
                        // structural slot; an erased record has none. A
                        // `bool` field no contract names rides its lane like
                        // any other field.
                        if let Some(lane) = lanes.iter().find(|lane| {
                            !lane.fields.is_empty()
                                && lane.primitive_type == typed_trees::types::PrimitiveType::Bool
                                && contract_names_field(
                                    program,
                                    machine,
                                    parameter.symbol,
                                    &lane.fields,
                                )
                        }) {
                            diagnostics.push(Diagnostic::error(format!(
                                "machine `{}::{}` erased parameter `{}` is a record with `bool` field `{}`; an erased record carries integer and float fields only, so pass that field as its own erased `bool` parameter",
                                machine.name,
                                state.name,
                                parameter.name,
                                lane.fields
                                    .iter()
                                    .map(|field| program.symbols.name(*field))
                                    .collect::<Vec<_>>()
                                    .join("."),
                            )));
                            continue;
                        }
                        if !lanes.is_empty() {
                            continue;
                        }
                        if proof_only.contract_term_carrier(program, parameter.type_reference)
                            || crate::mathematical_applications::is_source_evidence_carrier(
                                program,
                                parameter.type_reference,
                            )
                        {
                            continue;
                        }
                        diagnostics.push(Diagnostic::error(format!(
                    "machine `{}::{}` erased parameter `{}` is not scalar; an erased formal on a runtime machine needs a scalar lane or a contract term carrier",
                    machine.name, state.name, parameter.name,
                )));
                    }
                }
            }
        }

        fn validate_erased_binding_qualifiers(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    for parameter in program.state_parameters(state) {
                        let Some(qualifier) = erased_qualifier(
                            parameter.relevance,
                            parameter.is_const,
                            parameter.is_mutable,
                        ) else {
                            continue;
                        };
                        diagnostics.push(Diagnostic::error(format!(
                    "machine `{}::{}` erased parameter `{}` cannot be `{qualifier}`; an erased binding owns no runtime storage or custody",
                    machine.name, state.name, parameter.name,
                )));
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some(local) = statement.as_local_data() else {
                            continue;
                        };
                        if local.relevance.is_erased() && local.is_mutable {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` erased local `{}` cannot be `mut`; an erased binding owns no runtime storage or custody",
                        machine.name, state.name, local.name,
                    )));
                        }
                    }
                }
            }
            for definition in program.traits() {
                for signature in program.trait_machine_signatures(definition) {
                    for parameter in program.state_signature_parameters(signature) {
                        let Some(qualifier) = erased_qualifier(
                            parameter.relevance,
                            parameter.is_const,
                            parameter.is_mutable,
                        ) else {
                            continue;
                        };
                        diagnostics.push(Diagnostic::error(format!(
                    "requirement `{}` erased parameter `{}` cannot be `{qualifier}`; an erased binding owns no runtime storage or custody",
                    signature.name, parameter.name,
                )));
                    }
                }
            }
        }

        fn erased_qualifier(
            relevance: BindingRelevance,
            is_const: bool,
            is_mutable: bool,
        ) -> Option<&'static str> {
            if !relevance.is_erased() {
                return None;
            }
            if is_const {
                Some("const")
            } else if is_mutable {
                Some("mut")
            } else {
                None
            }
        }

        fn erased_attached_machine_record_is_admitted(
            program: &dyn crate::CheckingInput,
            definition: &DataDefinition,
        ) -> bool {
            if definition.supply_mode != DataSupplyMode::CheckedShape
                || !program.data_type_parameters(definition).is_empty()
                || !matches!(
                    DataDefinition::shape_kind_from_members(program.data_members(definition)),
                    DataShapeKind::Record | DataShapeKind::Enum | DataShapeKind::Mixed
                )
                || program.plan_laid_layout_for_data(definition).is_some()
                || program
                    .access_view_plans
                    .iter()
                    .any(|plan| plan.data_symbol == definition.symbol)
                || program
                    .wire_schemas()
                    .iter()
                    .any(|schema| schema.name == definition.name)
            {
                return false;
            }

            let attached = program
                .machines()
                .iter()
                .filter(|machine| machine.attached_data.as_ref() == Some(&definition.name));
            let mut found = false;
            for machine in attached {
                found = true;
                if !machine.supply_mode.is_checked_body()
                    || !program.machine_type_parameters(machine).is_empty()
                {
                    return false;
                }
            }
            found
        }

        fn validate_unresolved_erased_generic_uses(
            program: &dyn crate::CheckingInput,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            for definition in program
                .data_definitions()
                .iter()
                .filter(|definition| definition.type_parameters.is_empty())
            {
                for member in program.data_members(definition) {
                    let fields = program.data_member_fields(std::slice::from_ref(member));
                    for field in fields {
                        if field.relevance.is_erased()
                            && crate::mathematical_applications::is_source_evidence_carrier(
                                program,
                                field.type_reference,
                            )
                        {
                            continue;
                        }
                        if let Some(base) =
                            unresolved_erased_generic_base(program, field.type_reference)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "data `{}` field `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        definition.name, field.name
                    )));
                        }
                    }
                }
            }

            for machine in program.machines().iter().filter(|machine| {
                program.machine_type_parameters(machine).is_empty()
                    && !crate::mathematical_applications::is_indexed_logical_machine(
                        program, machine,
                    )
            }) {
                for state in program.machine_states(machine) {
                    for parameter in program.state_parameters(state) {
                        if parameter.relevance.is_erased()
                            && crate::mathematical_applications::is_source_evidence_carrier(
                                program,
                                parameter.type_reference,
                            )
                        {
                            continue;
                        }
                        if let Some(base) =
                            unresolved_erased_generic_base(program, parameter.type_reference)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` parameter `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        machine.name, state.name, parameter.name
                    )));
                        }
                    }
                    if let Some(base) = unresolved_erased_generic_base(program, state.return_type) {
                        diagnostics.push(Diagnostic::error(format!(
                    "machine `{}::{}` result uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                    machine.name, state.name
                )));
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        let Some(local) = statement.as_local_data() else {
                            continue;
                        };
                        if (local.relevance.is_erased()
                            && crate::mathematical_applications::is_source_evidence_carrier(
                                program,
                                local.type_reference,
                            ))
                            || super::dispatch_subject_of_erased_binding(program, state, local)
                        {
                            continue;
                        }
                        if let Some(base) =
                            unresolved_erased_generic_base(program, local.type_reference)
                        {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` local `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized instance",
                        machine.name, state.name, local.name
                    )));
                        }
                    }
                }
            }
        }

        fn unsupported(definition: &DataDefinition, fields: &str, shape: &str) -> Diagnostic {
            Diagnostic::error(format!(
                "data `{}` has erased field(s) `{fields}`, but erased-stripped runtime support for {shape} is not implemented yet",
                definition.name
            ))
        }

        pub(crate) fn erased_fields<'program>(
            program: &'program dyn crate::CheckingInput,
            definition: &'program DataDefinition,
        ) -> Vec<&'program DataField> {
            let mut fields = Vec::new();
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) if field.relevance == BindingRelevance::Erased => {
                        fields.push(field)
                    }
                    DataMember::Variant(variant) => fields.extend(
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .filter(|field| field.relevance == BindingRelevance::Erased),
                    ),
                    DataMember::Field(_) => {}
                }
            }
            fields
        }

        fn unresolved_erased_generic_base(
            program: &dyn crate::CheckingInput,
            handle: TypeReferenceHandle,
        ) -> Option<String> {
            if !handle.is_valid() {
                return None;
            }
            match program.type_reference_table.type_reference(handle) {
                TypeReferenceNode::Callable { .. } => None,
                TypeReferenceNode::Reference { referee, .. } => {
                    unresolved_erased_generic_base(program, *referee)
                }
                TypeReferenceNode::Constrained { base_type, .. } => {
                    unresolved_erased_generic_base(program, *base_type)
                }
                TypeReferenceNode::FixedArray { element_type, .. }
                | TypeReferenceNode::Slice { element_type } => {
                    unresolved_erased_generic_base(program, *element_type)
                }
                TypeReferenceNode::Generic {
                    base_symbol,
                    base_name,
                    arguments,
                    ..
                } => {
                    let base = program.data_definitions().iter().find(|definition| {
                        if base_symbol.is_valid() {
                            definition.symbol == *base_symbol
                        } else {
                            definition.name == *base_name
                        }
                    });
                    if base.is_some_and(|definition| {
                        !definition.type_parameters.is_empty()
                            && !erased_fields(program, definition).is_empty()
                    }) {
                        return Some(base_name.as_str().to_owned());
                    }
                    program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .iter()
                        .find_map(|argument| unresolved_erased_generic_base(program, *argument))
                }
                TypeReferenceNode::Named { .. }
                | TypeReferenceNode::ConstExpression(_)
                | TypeReferenceNode::DynamicTrait { .. }
                | TypeReferenceNode::Unit => None,
            }
        }
    }
    pub(crate) use shape_admission::erased_fields;
    use shape_admission::validate_erased_field_shapes;

    pub(crate) mod runtime_uses {

        use super::{
            Context, argument_position_context, call_targets_proof_machine, callee_parameters,
            erased_fields,
        };
        use crate::frozen_program::expression_table;
        use diagnostics::Diagnostic;
        use language_core::BindingRelevance;
        use symbols::SymbolHandle;
        use typed_trees::data::{DataDefinition, DataField, DataMember};
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::TableCall;
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn validate_expression(
            program: &dyn crate::CheckingInput,
            proof_only: &typed_trees::proof_only::ProofOnlyClassification,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            context: Context,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if !expression.is_valid() {
                return;
            }
            macro_rules! visit_expression {
                ($child:expr, $context:expr) => {
                    validate_expression(
                        program,
                        proof_only,
                        machine,
                        state,
                        $child,
                        $context,
                        diagnostics,
                    )
                };
            }
            match expression_table(program).expression(expression) {
                ExpressionNode::Name(path) => {
                    if context == Context::Runtime {
                        let mut reported = false;
                        for symbol in
                            expression_table(program).name_path_member_symbols(path.member_symbols)
                        {
                            reported |= report_runtime_erased_field(program, *symbol, diagnostics)
                                || report_runtime_erased_binding(
                                    program,
                                    state,
                                    *symbol,
                                    diagnostics,
                                );
                        }
                        let members = expression_table(program).name_path_members(path.members);
                        if !reported && members.len() == 2 && members[0].is_self_receiver() {
                            report_runtime_erased_attached_field(
                                program,
                                machine,
                                members[1].as_str(),
                                diagnostics,
                            );
                        }
                    }
                }
                ExpressionNode::Member(member) => {
                    if context == Context::Runtime
                        && !report_runtime_erased_field(program, member.member_symbol, diagnostics)
                    {
                        if crate::value_custody::places::direct_self_field_member(
                            program, expression,
                        ) == Some(member.member.as_str())
                        {
                            report_runtime_erased_attached_field(
                                program,
                                machine,
                                member.member.as_str(),
                                diagnostics,
                            );
                        } else {
                            report_runtime_erased_member_by_receiver(
                                program,
                                machine,
                                state,
                                member.receiver,
                                member.member.as_str(),
                                member.case_variant.as_ref().map(|variant| variant.as_str()),
                                diagnostics,
                            );
                        }
                    }
                    visit_expression!(member.receiver, context);
                }
                ExpressionNode::StructLiteral(literal) => validate_struct_literal(
                    program,
                    proof_only,
                    machine,
                    state,
                    expression,
                    literal,
                    context,
                    diagnostics,
                ),
                ExpressionNode::Call(call) => {
                    if context == Context::Runtime
                        && call_targets_proof_machine(program, proof_only, call.target_symbol)
                    {
                        diagnostics.push(Diagnostic::error(format!(
                    "call to proof machine `{}` has no runtime result; use it only in a proof or erased context, or as a statement citation",
                    call.target
                )));
                    }
                    if context == Context::ErasedInitializer
                        && !call_targets_proof_machine(program, proof_only, call.target_symbol)
                        && !(crate::mathematical_applications::is_mathematical_call(program, call)
                            && program
                                .prepared_mathematics()
                                .is_some_and(|view| view.application(expression).is_some()))
                    {
                        diagnostics.push(Diagnostic::error(format!(
                    "erased binding initializer calls runtime machine `{}`; erased initialization cannot perform runtime effects or computation",
                    call.target
                )));
                    }
                    if call.receiver.is_valid() {
                        visit_expression!(call.receiver, context);
                    }
                    let parameters = callee_parameters(program, call.target_symbol);
                    for (position, argument) in expression_table(program)
                        .expression_handles(call.arguments)
                        .iter()
                        .enumerate()
                    {
                        visit_expression!(
                            *argument,
                            argument_position_context(context, parameters, position)
                        );
                    }
                }
                ExpressionNode::Atomic(atomic) => {
                    if context == Context::ErasedInitializer {
                        diagnostics.push(Diagnostic::error(
                            "erased binding initializer cannot perform an atomic runtime operation",
                        ));
                    }
                    visit_expression!(atomic.value, context);
                    visit_expression!(atomic.result, context);
                }
                other => crate::queries::expression_traversal::children(program, other, |child| {
                    visit_expression!(child, context);
                }),
            }
        }

        fn validate_struct_literal(
            program: &dyn crate::CheckingInput,
            proof_only: &typed_trees::proof_only::ProofOnlyClassification,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            literal: &TableStructLiteral,
            context: Context,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            macro_rules! visit_expression {
                ($child:expr, $context:expr) => {
                    validate_expression(
                        program,
                        proof_only,
                        machine,
                        state,
                        $child,
                        $context,
                        diagnostics,
                    )
                };
            }
            let Some(definition) = program.data_definition_by_symbol(literal.type_symbol) else {
                for field in expression_table(program).struct_fields(literal.fields) {
                    visit_expression!(field.value, context);
                }
                return;
            };
            let checked_source_evidence = context != Context::Runtime
                && program.prepared_mathematics().is_some_and(|view| {
                    view.source_evidence(expression).is_some_and(|evidence| {
                        evidence.type_identity.is_valid()
                            && evidence.type_identity
                                == program.normalized_type_identity(evidence.source_type)
                    }) || view
                        .source_constructor(expression)
                        .is_some_and(|constructor| {
                            constructor.type_identity.is_valid()
                                && constructor.type_identity
                                    == program.normalized_type_identity(constructor.source_type)
                                && constructor.machine == machine.symbol
                                && constructor.state == state.symbol
                        })
                });
            // Receiving application custody establishes correspondence, not evidence
            // or permission to read an erased field at runtime. Its selected fields
            // still require explicit terms (including checked nullary elaboration),
            // and those terms retain their ErasedInitializer context below.
            let applied = program.closed_data_constructor_definition(
                crate::value_custody::struct_literals::construction_application(
                    program, expression,
                ),
                literal.type_symbol,
            );
            let unsupported_bare_generic = !definition.type_parameters.is_empty()
                && !erased_fields(program, definition).is_empty()
                && applied.is_none()
                && !checked_source_evidence;
            if unsupported_bare_generic {
                diagnostics.push(Diagnostic::error(format!(
            "construction of erased generic data `{}` is unsupported in this context; use a closed generic record in an explicitly typed local initializer",
            definition.name
        )));
            }
            let declared_erased = literal_fields(
                program,
                applied.unwrap_or(definition),
                literal.case_name.as_ref(),
            )
            .filter(|field| field.relevance.is_erased())
            .collect::<Vec<_>>();
            let authored = expression_table(program).struct_fields(literal.fields);
            for erased in &declared_erased {
                if !unsupported_bare_generic
                    && !authored.iter().any(|field| field.name == erased.name)
                {
                    diagnostics.push(Diagnostic::error(format!(
                "construction of `{}` omits erased field `{}`; supply an explicit proof term because no unique accessible nullary constructor determines this binding",
                definition.name, erased.name
            )));
                }
            }
            for field in authored {
                let field_context = if declared_erased
                    .iter()
                    .any(|declared| declared.name == field.name)
                {
                    Context::ErasedInitializer
                } else {
                    context
                };
                visit_expression!(field.value, field_context);
            }
        }

        pub(crate) fn validate_statement_call_receiver(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            call: &TableCall,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if report_runtime_erased_binding(program, state, call.receiver_root_symbol, diagnostics)
                || report_runtime_erased_binding(program, state, call.receiver_symbol, diagnostics)
                || report_runtime_erased_field(program, call.receiver_root_symbol, diagnostics)
                || report_runtime_erased_field(program, call.receiver_symbol, diagnostics)
            {
                return;
            }
            let members = program.statement_table.name_path_members(call.receiver);
            let Some((first, projected)) = members.split_first() else {
                return;
            };
            let mut owner = if first.is_self_receiver() {
                crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    machine.attached_data_symbol,
                )
            } else {
                receiver_root_type(program, state, call.receiver_root_symbol).and_then(
                    |type_reference| {
                        crate::value_custody::places::data_definition_for_type(
                            program,
                            type_reference,
                        )
                    },
                )
            };
            for member in projected {
                let Some(definition) = owner else {
                    return;
                };
                let Some(field) = crate::value_custody::places::exact_data_member_field(
                    program,
                    definition,
                    SymbolHandle::invalid(),
                    member.as_str(),
                    None,
                ) else {
                    return;
                };
                if field.relevance.is_erased() {
                    diagnostics.push(Diagnostic::error(format!(
                "erased field `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs or another erased binding",
                field.name
            )));
                    return;
                }
                owner = crate::value_custody::places::data_definition_for_type(
                    program,
                    field.type_reference,
                );
            }
        }

        fn receiver_root_type(
            program: &dyn crate::CheckingInput,
            state: &State,
            root: SymbolHandle,
        ) -> Option<TypeReferenceHandle> {
            if !root.is_valid() {
                return None;
            }
            crate::state_root_type(program, state, root)
                .or_else(|| field_by_symbol(program, root).map(|field| field.type_reference))
        }

        fn report_runtime_erased_field(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if !symbol.is_valid() {
                return false;
            }
            if let Some(field) = field_by_symbol(program, symbol)
                && field.relevance == BindingRelevance::Erased
            {
                diagnostics.push(Diagnostic::error(format!(
            "erased field `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs or another erased binding",
            field.name
        )));
                return true;
            }
            false
        }

        fn report_runtime_erased_binding(
            program: &dyn crate::CheckingInput,
            state: &State,
            symbol: SymbolHandle,
            diagnostics: &mut Vec<Diagnostic>,
        ) -> bool {
            if !symbol.is_valid() {
                return false;
            }
            let erased_parameter = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == symbol && parameter.relevance.is_erased());
            if let Some(parameter) = erased_parameter {
                diagnostics.push(Diagnostic::error(format!(
            "erased parameter `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs, contracts, or another erased binding",
            parameter.name
        )));
                return true;
            }
            let erased_local = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == symbol && local.relevance.is_erased() =>
                    {
                        Some(local)
                    }
                    _ => None,
                });
            if let Some(local) = erased_local {
                diagnostics.push(Diagnostic::error(format!(
            "erased local `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs, contracts, or another erased binding",
            local.name
        )));
                return true;
            }
            false
        }

        fn report_runtime_erased_attached_field(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            name: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let Some(attached) = machine.attached_data.as_ref() else {
                return;
            };
            let Some(definition) = program.data_definition_by_name(attached) else {
                return;
            };
            if let Some(field) = program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.name.as_str() == name).then_some(field)
            }) {
                report_runtime_erased_field(program, field.symbol, diagnostics);
            }
        }

        fn report_runtime_erased_member_by_receiver(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            receiver: ExpressionHandle,
            name: &str,
            case_variant: Option<&str>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let receiver_type = crate::value_custody::places::declared_place_type(
                program,
                machine,
                Some(state),
                receiver,
            )
            .or_else(|| {
                crate::value_custody::places::declared_indexed_projection_type(
                    program,
                    machine,
                    Some(state),
                    receiver,
                )
            });
            let Some(definition) = receiver_type.and_then(|type_reference| {
                crate::value_custody::places::data_definition_for_type(program, type_reference)
            }) else {
                return;
            };
            let field = if let Some(case_variant) = case_variant {
                program.data_members(definition).iter().find_map(|member| {
                    let variant = member.as_variant()?;
                    (variant.name.as_str() == case_variant).then(|| {
                        program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.name.as_str() == name)
                    })?
                })
            } else {
                let common = program.data_members(definition).iter().find_map(|member| {
                    let field = member.as_field()?;
                    (field.name.as_str() == name).then_some(field)
                });
                common.or_else(|| {
                    program.data_members(definition).iter().find_map(|member| {
                        let variant = member.as_variant()?;
                        program.data_payload_fields(variant).iter().find(|field| {
                            field.name.as_str() == name && field.relevance.is_erased()
                        })
                    })
                })
            };
            if let Some(field) = field {
                report_runtime_erased_field(program, field.symbol, diagnostics);
            }
        }

        fn literal_fields<'program>(
            program: &'program dyn crate::CheckingInput,
            definition: &'program DataDefinition,
            case_name: Option<&typed_trees::name::Identifier>,
        ) -> impl Iterator<Item = &'program DataField> {
            let mut fields = Vec::new();
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => fields.push(field),
                    DataMember::Variant(variant)
                        if case_name.is_some_and(|case_name| *case_name == variant.name) =>
                    {
                        fields.extend(program.data_payload_fields(variant));
                    }
                    DataMember::Variant(_) => {}
                }
            }
            fields.into_iter()
        }

        fn field_by_symbol(
            program: &dyn crate::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&DataField> {
            program.any_field_owner(symbol).map(|(_, field)| field)
        }
    }
    use runtime_uses::validate_expression;

    pub(crate) fn callee_parameters(
        program: &dyn CheckingInput,
        target: SymbolHandle,
    ) -> Option<&[typed_trees::signature::StateParameter]> {
        program
            .state_by_symbol(target)
            .or_else(|| {
                program
                    .machine_by_symbol(target)
                    .and_then(|machine| program.machine_states(machine).first())
            })
            .map(|state| program.state_parameters(state))
    }

    pub(crate) fn argument_position_context(
        context: Context,
        callee_parameters: Option<&[typed_trees::signature::StateParameter]>,
        position: usize,
    ) -> Context {
        if context != Context::Runtime {
            return context;
        }
        let erased_position = callee_parameters.is_some_and(|parameters| {
            parameters
                .get(position)
                .is_some_and(|parameter| parameter.relevance.is_erased())
        });
        if erased_position {
            Context::ErasedInitializer
        } else {
            Context::Runtime
        }
    }

    pub(crate) fn call_targets_proof_machine(
        program: &dyn CheckingInput,
        proof_only: &typed_trees::proof_only::ProofOnlyClassification,
        target: SymbolHandle,
    ) -> bool {
        program
            .machine_holding_state(target)
            .or_else(|| program.machine_by_symbol(target))
            .is_some_and(|machine| {
                proof_only.is_proof_machine(program, machine)
                    || crate::mathematical_applications::is_indexed_logical_machine(
                        program, machine,
                    )
            })
    }
}

pub mod scalar_boundary_guarantees {
    use checked_trees::{
        CheckedBooleanExpression as Boolean, CheckedBoundaryScalarGuarantee as Guarantee,
        CheckedFloatProjectionSource as Source, CheckedFloatSemanticApplicationOperand as Operand,
        CheckedIeeeFloatComparisonKind as Compare, CheckedScalarExpression as Scalar,
        ClosedScalarContractValue,
    };
    use numerics::float_semantics_catalog::FloatSemanticOperation;
    use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};
    use typed_trees::{machine::Machine, types::PrimitiveType};

    pub fn derive_scalar_boundary_guarantees(
        program: &dyn crate::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &Machine,
        contract: &checked_trees::MachineContractPlan,
    ) -> Vec<Guarantee> {
        let mut guarantees = Vec::new();
        for clause in contract.closed_scalar_values.ensures().iter().flatten() {
            match clause {
                ClosedScalarContractValue::Predicate(predicate) => {
                    guarantees.push(Guarantee::Predicate(predicate.clone()));
                }
                ClosedScalarContractValue::SourceCheckedGuarantee {
                    facts: source_facts,
                    ..
                } => {
                    for fact in program.proof_facts.span_or_empty(*source_facts) {
                        if let Some(expression) = fact.as_expression()
                            && let Some(guarantee) =
                                meaning_consequence(program, facts, machine, *expression)
                        {
                            guarantees.push(guarantee);
                        }
                    }
                }
                _ => {}
            }
        }
        guarantees
    }

    fn meaning_consequence(
        program: &dyn crate::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &Machine,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<Guarantee> {
        let mut equalities = facts
            .proof
            .float_meaning_equalities
            .iter()
            .filter(|row| row.use_site.is_none() && row.source_expression == expression);
        let equality = equalities.next()?;
        if equalities.next().is_some() {
            return None;
        }
        let projections = &facts.proof.float_meaning_projections;
        let projection = |id| projections.iter().find(|row| row.result.id == id);
        let (result, application) = [
            (equality.left, equality.right),
            (equality.right, equality.left),
        ]
        .into_iter()
        .find_map(|(result, application)| {
            let Source::DirectMachineResult(result) = projection(result)?.source else {
                return None;
            };
            if result.owner_machine != machine.symbol {
                return None;
            }
            let application = facts
                .proof
                .float_semantic_applications
                .iter()
                .find(|row| row.result == application)?;
            Some((result, application))
        })?;
        application.validate().ok()?;
        if !source_equation_matches(program, facts, machine, expression, application) {
            return None;
        }
        let row = FloatSemanticOperation::for_contract_identity(&application.contract)?;
        let division = match row.name {
            "add" | "subtract" | "multiply" => false,
            "divide" => true,
            _ => return None,
        };
        let primitive_type = match application.format {
            IeeeFloatFormat::Binary32 => PrimitiveType::F32,
            IeeeFloatFormat::Binary64 => PrimitiveType::F64,
        };
        if result.fallback.primitive != primitive_type {
            return None;
        }
        let [state] = program.machine_states(machine) else {
            return None;
        };
        if program.primitive_type_reference(state.return_type) != Some(primitive_type)
            || !language_core::signature_reserves_result(
                program
                    .state_parameters(state)
                    .iter()
                    .map(|parameter| program.symbols.name(parameter.symbol)),
            )
        {
            return None;
        }
        let parameters = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
            })
            .collect::<Vec<_>>();
        let [
            Operand::Format(format),
            Operand::Meaning(left),
            Operand::Meaning(right),
        ] = application.operands.as_slice()
        else {
            return None;
        };
        if *format != application.format {
            return None;
        }
        let formal = |id| {
            let Source::DirectMachineParameter(source) = projection(id)?.source else {
                return None;
            };
            if source.owner_machine != machine.symbol || source.fallback.primitive != primitive_type
            {
                return None;
            }
            let position = parameters.iter().position(|parameter| {
                parameter.symbol == source.parameter
                    && program.primitive_type_reference(parameter.type_reference)
                        == Some(primitive_type)
            })?;
            Some(Scalar::Parameter {
                position,
                primitive_type,
            })
        };
        let left = formal(*left)?;
        let right = formal(*right)?;
        let result = Scalar::Parameter {
            position: parameters.len(),
            primitive_type,
        };
        let mut condition = and(
            finite(left, application.format),
            finite(right.clone(), application.format),
        );
        if division {
            let zero = Scalar::IeeeFloatLiteral {
                value: match application.format {
                    IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
                    IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
                },
            };
            condition = and(
                condition,
                Boolean::Not(Box::new(compare(Compare::Equal, right, zero))),
            );
        }
        Some(Guarantee::Implication {
            premise: condition,
            conclusion: compare(Compare::Equal, result.clone(), result),
        })
    }

    fn source_equation_matches(
        program: &dyn crate::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &Machine,
        expression: typed_trees::expression::ExpressionHandle,
        application: &checked_trees::CheckedFloatSemanticApplication,
    ) -> bool {
        use typed_trees::expression::BinaryOperator;
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        if binary.operator != BinaryOperator::Equal {
            return false;
        }
        let projection_source = |expression, expected: Option<symbols::SymbolHandle>| {
            let Some(call) = program.expression_table.call_node(expression) else {
                return false;
            };
            let Some((_, primitive, _, _)) =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program, call,
                )
            else {
                return false;
            };
            if primitive
                != match application.format {
                    IeeeFloatFormat::Binary32 => PrimitiveType::F32,
                    IeeeFloatFormat::Binary64 => PrimitiveType::F64,
                }
            {
                return false;
            }
            let [operand] = program.expression_table.expression_handles(call.arguments) else {
                return false;
            };
            let Some(path) = program.expression_table.name_path_node(*operand) else {
                return false;
            };
            match expected {
                Some(symbol) => path.symbol == symbol,
                None => matches!(program.expression_table.name_path_members(path.members), [name]
                if name.as_str() == "result"),
            }
        };
        [(binary.left, binary.right), (binary.right, binary.left)]
            .into_iter()
            .any(|(result, meaning)| {
                if !projection_source(result, None) {
                    return false;
                }
                let Some(call) = program.expression_table.call_node(meaning) else {
                    return false;
                };
                let Some(operator) =
                    typed_trees::operator::resolve_named_expression_call(program, call)
                else {
                    return false;
                };
                let Some((_, contract)) =
                    crate::exact_toolchain_float_semantic_contract(program, operator)
                else {
                    return false;
                };
                if contract != application.contract {
                    return false;
                }
                let arguments = program.expression_table.expression_handles(call.arguments);
                if arguments.len() != application.operands.len() {
                    return false;
                }
                arguments
                    .iter()
                    .zip(&application.operands)
                    .all(|(expression, operand)| match operand {
                        Operand::Format(format) => {
                            crate::exact_toolchain_float_format_const(program, *expression)
                                == Some(*format)
                        }
                        Operand::Meaning(value) => {
                            let Some(projection) = facts
                                .proof
                                .float_meaning_projections
                                .iter()
                                .find(|projection| projection.result.id == *value)
                            else {
                                return false;
                            };
                            let Source::DirectMachineParameter(source) = projection.source else {
                                return false;
                            };
                            source.owner_machine == machine.symbol
                                && projection_source(*expression, Some(source.parameter))
                        }
                    })
            })
    }

    fn compare(kind: Compare, left: Scalar, right: Scalar) -> Boolean {
        Boolean::ScalarIeeeFloatComparison {
            kind,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn and(left: Boolean, right: Boolean) -> Boolean {
        Boolean::And {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn finite(value: Scalar, format: IeeeFloatFormat) -> Boolean {
        let (minimum, maximum) = match format {
            IeeeFloatFormat::Binary32 => (
                IeeeFloatValue::Binary32(0xff7f_ffff),
                IeeeFloatValue::Binary32(0x7f7f_ffff),
            ),
            IeeeFloatFormat::Binary64 => (
                IeeeFloatValue::Binary64(0xffef_ffff_ffff_ffff),
                IeeeFloatValue::Binary64(0x7fef_ffff_ffff_ffff),
            ),
        };
        and(
            compare(
                Compare::LessOrEqual,
                Scalar::IeeeFloatLiteral { value: minimum },
                value.clone(),
            ),
            compare(
                Compare::LessOrEqual,
                value,
                Scalar::IeeeFloatLiteral { value: maximum },
            ),
        )
    }
}

pub mod signature_ensures {
    use crate::{CallWriteOrigins, LocalWriteSource};
    use facts::PlaceRoot;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};
    use typed_trees::signature::{SignatureContractKind, StateSignature};
    use typed_trees::state::State;

    pub fn ensures_contract_expressions<'program>(
        program: &'program dyn crate::CheckingInput,
        signature: &'program StateSignature,
    ) -> impl Iterator<Item = ExpressionHandle> + 'program {
        program
            .signature_contracts
            .span_or_empty(signature.contracts)
            .iter()
            .filter(|contract| matches!(contract.kind, SignatureContractKind::Ensures))
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts).iter())
            .filter_map(|fact| match fact {
                ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
    }

    pub fn collect_parameter_bound_comparisons(
        program: &dyn crate::CheckingInput,
        state: &State,
        signature: &StateSignature,
        origins: &CallWriteOrigins,
        conjunct: ExpressionHandle,
        comparisons: &mut Vec<(String, BinaryOperator, i64)>,
    ) {
        let Some(comparison) = program.expression_table.binary_node(conjunct) else {
            return;
        };
        if comparison.operator == BinaryOperator::And {
            collect_parameter_bound_comparisons(
                program,
                state,
                signature,
                origins,
                comparison.left,
                comparisons,
            );
            collect_parameter_bound_comparisons(
                program,
                state,
                signature,
                origins,
                comparison.right,
                comparisons,
            );
            return;
        }
        let Some(place) = crate::canonical_place_from_expression(program, comparison.left) else {
            return;
        };
        let parameters = program.state_signature_parameters(signature);
        let Some(position) = parameters
            .iter()
            .position(|parameter| place.root == PlaceRoot::Symbol(parameter.symbol))
        else {
            return;
        };
        let parameter = &parameters[position];
        let Some(reference) = crate::reference_shell(program, parameter.type_reference) else {
            return;
        };
        let Some(leaf) = crate::project_type_reference_from_segments(
            program,
            reference.referee,
            &place.segments,
        ) else {
            return;
        };
        let Some(primitive) = program.primitive_type_reference(
            crate::value_custody::places::assignment_value_type(program, leaf),
        ) else {
            return;
        };
        if primitive.integer_sign_bits().is_none() {
            return;
        }
        let spelling = match comparison.operator {
            BinaryOperator::Less => language_core::OperatorSpelling::Less,
            BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
            BinaryOperator::Greater => language_core::OperatorSpelling::Greater,
            BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
            _ => return,
        };
        let operand_types = [comparison.left, comparison.right].map(|operand| {
            crate::parameter_expression_result_type_reference(
                program,
                signature.symbol,
                parameters,
                operand,
            )
        });
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            signature.symbol,
            conjunct,
            spelling,
            &operand_types,
        ) {
            return;
        }
        let Some(literal) = program.expression_table.integer_node(comparison.right) else {
            return;
        };
        let Some(value) = literal.value_i64() else {
            return;
        };
        let Some(sources) = origins
            .arguments
            .get(position)
            .and_then(|origins| origins.as_ref())
            .filter(|origins| !origins.is_empty())
        else {
            return;
        };
        let relative = LocalWriteSource {
            invocation_output: None,
            invocation_result: None,
            is_image_storage: false,
            root: parameter.symbol,
            segments: place.segments,
            representation: None,
            recasts: Vec::new(),
            path: String::new(),
            collection_coarse: false,
        };
        let mut coordinate: Option<crate::ScalarStorageCoordinate> = None;
        for source in sources {
            let Some(projected) = source
                .substitute_storage(program, &relative)
                .and_then(|source| source.scalar_storage_coordinate(program, state))
                .filter(|coordinate| {
                    coordinate.preserves_scalar_value
                        && coordinate.source_primitive == primitive
                        && coordinate.view_primitive == primitive
                })
            else {
                return;
            };
            if coordinate
                .as_ref()
                .is_some_and(|existing| existing.place != projected.place)
            {
                return;
            }
            coordinate = Some(projected);
        }
        let Some(coordinate) = coordinate else {
            return;
        };
        let Some(path) = coordinate.label(program) else {
            return;
        };
        comparisons.push((path, comparison.operator, value));
    }
}

pub mod slice_ranking {
    use crate::frozen_program::expression_table;
    use crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup;
    use symbols::SymbolHandle;

    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::types::{PrimitiveType, TypeReferenceNode};

    pub fn slice_tail_strictly_decreases<'p>(
        program: &'p dyn crate::CheckingInput,
        guard: ExpressionHandle,
        argument: ExpressionHandle,
        parameter: &StateParameter,
        bound_lookup: &mut Option<ImmutableBoundLookup<'p>>,
    ) -> bool {
        if !parameter.symbol.is_valid() || !parameter_is_slice(program, parameter) {
            return false;
        }
        let argument = expression_table(program)
            .borrow_expression(argument)
            .map_or(argument, |borrow| borrow.target);
        let Some(indexed) = expression_table(program).indexed_node(argument) else {
            return false;
        };
        let Some(range) = expression_table(program).range_node(indexed.index) else {
            return false;
        };
        if !names_parameter(program, indexed.collection, parameter)
            || range.end.is_valid()
            || range.end_inclusive
        {
            return false;
        }
        let Some(start) = tail_bound(
            program,
            bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
            range.start,
        ) else {
            return false;
        };
        if !tail_bound_is_positive(program, start) {
            return false;
        }
        let guard = match expression_table(program).expression(guard) {
            ExpressionNode::Binary(binary)
                if binary.operator == BinaryOperator::Equal
                    && matches!(
                        expression_table(program).expression(binary.right),
                        ExpressionNode::Boolean(true)
                    ) =>
            {
                binary.left
            }
            _ => guard,
        };
        let Some(binary) = expression_table(program).binary_node(guard) else {
            return false;
        };
        let Some(length) = crate::length_member(program, binary.left) else {
            return false;
        };
        let bonus = match binary.operator {
            BinaryOperator::Greater => 1,
            BinaryOperator::GreaterOrEqual => 0,
            _ => return false,
        };
        names_parameter(program, length.receiver, parameter)
            && tail_bound(
                program,
                bound_lookup.get_or_insert_with(|| ImmutableBoundLookup::new(program)),
                binary.right,
            )
            .is_some_and(|bound| bound_ordering_at_least(program, bound, start, bonus))
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum TailBound {
        Literal(usize),
        Symbol { symbol: SymbolHandle, offset: i64 },
    }

    fn tail_bound(
        program: &dyn crate::CheckingInput,
        bound_lookup: &crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'_>,
        expression: ExpressionHandle,
    ) -> Option<TailBound> {
        if let Some(value) =
            crate::normalize_immutable_integer_bound_to_usize(program, bound_lookup, expression)
        {
            return Some(TailBound::Literal(value));
        }
        if let Some(bound) =
            crate::immutable_integer_bound_symbol_offset(program, bound_lookup, expression)
        {
            return Some(TailBound::Symbol {
                symbol: bound.symbol,
                offset: bound.offset,
            });
        }
        if let Some(leaf) =
            crate::normalize_immutable_integer_bound_expression(program, bound_lookup, expression)
        {
            return match expression_table(program).expression(leaf) {
                ExpressionNode::Name(path) if crate::is_single_segment_name_path(program, path) => {
                    Some(TailBound::Symbol {
                        symbol: path.symbol,
                        offset: 0,
                    })
                }
                _ => None,
            };
        }
        crate::immutable_integer_bound_value_symbol(program, bound_lookup, expression)
            .map(|symbol| TailBound::Symbol { symbol, offset: 0 })
    }

    fn bound_ordering_at_least(
        program: &dyn crate::CheckingInput,
        bound: TailBound,
        start: TailBound,
        bonus: i64,
    ) -> bool {
        match (bound, start) {
            (TailBound::Literal(bound), TailBound::Literal(start)) => {
                bound as i128 + bonus as i128 >= start as i128
            }
            (
                TailBound::Symbol {
                    symbol: bound_symbol,
                    offset: bound_offset,
                },
                TailBound::Symbol {
                    symbol: start_symbol,
                    offset: start_offset,
                },
            ) => {
                bound_symbol == start_symbol
                    && bound_offset
                        .checked_add(bonus)
                        .is_some_and(|bound| bound >= start_offset)
            }
            (TailBound::Symbol { symbol, offset }, TailBound::Literal(start)) => {
                i64::try_from(start).ok().is_some_and(|start| {
                    bound_symbol_floor(program, symbol)
                        .and_then(|floor| floor.checked_add(offset))
                        .and_then(|low| low.checked_add(bonus))
                        .is_some_and(|low| low >= start)
                })
            }
            (TailBound::Literal(_), TailBound::Symbol { .. }) => false,
        }
    }

    fn tail_bound_is_positive(program: &dyn crate::CheckingInput, start: TailBound) -> bool {
        match start {
            TailBound::Literal(value) => value >= 1,
            TailBound::Symbol { symbol, offset } => bound_symbol_floor(program, symbol)
                .and_then(|floor| floor.checked_add(offset))
                .is_some_and(|floor| floor >= 1),
        }
    }

    fn bound_symbol_floor(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> Option<i64> {
        let type_reference =
            crate::value_custody::places::bound_symbol_declared_type(program, symbol)?;
        let carrier = match program
            .type_reference_table
            .primitive_type(type_reference)?
        {
            PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
            | PrimitiveType::Addr => 0,
            PrimitiveType::I8 => i8::MIN as i64,
            PrimitiveType::I16 => i16::MIN as i64,
            PrimitiveType::I32 => i32::MIN as i64,
            PrimitiveType::I64 => i64::MIN,
            PrimitiveType::Selector
            | PrimitiveType::Bool
            | PrimitiveType::F32
            | PrimitiveType::F64 => {
                return None;
            }
        };
        let declared = crate::proof_contracts::arithmetic_domains::enforced_declared_range(
            program,
            type_reference,
        )
        .and_then(|interval| interval.low);
        Some(carrier.max(declared.unwrap_or(i64::MIN)))
    }

    fn names_parameter(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        parameter: &StateParameter,
    ) -> bool {
        matches!(
            expression_table(program).expression(expression),
            ExpressionNode::Name(path)
                if path.symbol == parameter.symbol
                    && path.head_symbol == parameter.symbol
                    && program.expression_table.name_path_members(path.members).len() == 1
        )
    }

    fn parameter_is_slice(program: &dyn crate::CheckingInput, parameter: &StateParameter) -> bool {
        let mut reference = parameter.type_reference;
        let mut visited = Vec::new();
        while reference.is_valid() && !visited.contains(&reference) {
            visited.push(reference);
            reference = match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Slice { .. } => return true,
                TypeReferenceNode::Reference { referee, .. } => *referee,
                TypeReferenceNode::Constrained { base_type, .. } => *base_type,
                _ => return false,
            };
        }
        false
    }
}

pub use argument_subjects::contract_argument_root;
