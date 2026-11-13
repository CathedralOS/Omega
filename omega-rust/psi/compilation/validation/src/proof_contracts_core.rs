/*
Proof contract structure, immutable integer bounds, properties, qualification
evidence, relevance, proof embeddings, and slice ranking (`proof_contracts`).
*/
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
        let (machine_symbol, mut type_reference) = reserved_result_owner(program, root)?;
        if expression != root
            && let Some(owner) = contract_occurrence_owner(program, root, expression)
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
                let parameters = source_contract_parameters(program, machine_symbol, root)?;
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
        if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
            return memos
                .contract_occurrences
                .get_or_init(|| std::sync::Arc::new(ContractOccurrenceIndex::of(program)))
                .owner(root, expression);
        }
        let mut owner = None;
        let (mut nodes, mut seen) = (Vec::new(), ContractNodeSet::default());
        for (callable, return_type, parameters, _, contract) in callable_contract_scopes(program) {
            contract_nodes(program, contract, &mut nodes, &mut seen);
            if !seen.contains(&expression) || !seen.contains(&root) {
                continue;
            }
            if !owns_result(
                program,
                callable,
                return_type,
                program.state_parameters.span_or_empty(parameters),
                contract,
            ) {
                return None;
            }
            if owner.is_some_and(|candidate| candidate != (callable, return_type)) {
                return None;
            }
            owner = Some((callable, return_type));
        }
        owner
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
        contract.kind == SignatureContractKind::Ensures
            && callable.is_valid()
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
            for position in self.containing.get(&expression)? {
                let (callable, _, _, candidate) = self.contracts[*position as usize];
                if callable != owner || parameters.is_some_and(|prior| prior != candidate) {
                    return None;
                }
                parameters = Some(candidate);
            }
            parameters
        }

        fn of(program: &dyn crate::CheckingInput) -> Self {
            let mut contracts = Vec::new();
            let mut containing = symbols::SymbolKeyMap::<ExpressionHandle, Vec<u32>>::default();
            let (mut nodes, mut seen) = (Vec::new(), ContractNodeSet::default());
            for (callable, return_type, parameters, _, contract) in
                callable_contract_scopes(program)
            {
                let position =
                    u32::try_from(contracts.len()).expect("callable contract position overflow");
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
                ));
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
            for position in self.containing.get(&expression).into_iter().flatten() {
                let (callable, _, _, _) = self.contracts[*position as usize];
                if !callable.is_valid() || (owner.is_valid() && owner != callable) {
                    return Err(());
                }
                owner = callable;
            }
            Ok(owner)
        }

        fn owner(
            &self,
            root: ExpressionHandle,
            expression: ExpressionHandle,
        ) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
            let with_expression = self.containing.get(&expression)?;
            let with_root = self.containing.get(&root)?;
            let mut owner = None;
            for position in with_expression
                .iter()
                .filter(|position| with_root.binary_search(position).is_ok())
            {
                let (callable, return_type, owns_result, _) = self.contracts[*position as usize];
                if !owns_result {
                    return None;
                }
                if owner.is_some_and(|candidate| candidate != (callable, return_type)) {
                    return None;
                }
                owner = Some((callable, return_type));
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
                for site in owner_index.executable_sites(expression).ok()? {
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
                runtime_contract: None,
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
            environments.extend(
                data_type_environments(program)
                    .ok()?
                    .into_iter()
                    .filter_map(|(reached, environment)| {
                        reached.contains(&expression).then_some(environment)
                    }),
            );
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
            environments.extend(
                typed_trees::source_expression_types::callable_environments(
                    program.typed_program(),
                )
                .ok()?
                .into_iter()
                .filter_map(|(reached, environment)| {
                    reached.contains(&expression).then_some(environment)
                }),
            );
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
                        )? {
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
                )? {
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
        ) -> Result<Vec<(Vec<ExpressionHandle>, TypeEnvironment)>, &'static str> {
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
                            )?);
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
                                    )?,
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
            Ok(environments)
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
        use typed_trees::types::TypeReferenceHandle;

        pub(crate) fn build_owner_environment_index(
            program: &dyn CheckingInput,
            contract_facts: &arena::Arena<checked_trees::ContractProofFact>,
            call_signatures: &arena::Arena<checked_trees::CallContractSignature>,
        ) -> Result<OwnerIndex, &'static str> {
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
            index_parameter_constraint_environments(program, &mut index)?;
            for (reached, environment) in super::environments::data_type_environments(program)? {
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
            for (reached, environment) in
                typed_trees::source_expression_types::callable_environments(
                    program.typed_program(),
                )?
            {
                index.add_all(reached, || environment);
            }
            index_ranking_environments(program, &mut index);
            index_executable_environments(program, &mut index);
            Ok(index)
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
        ) -> Result<HashSet<ExpressionHandle>, &'static str> {
            typed_trees::source_expression_types::type_inputs(
                program.typed_program(),
                type_reference,
            )
            .map(|inputs| inputs.expressions.into_iter().collect())
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
        ) -> Result<(), &'static str> {
            for machine in program.machines() {
                for state in program.machine_states(machine) {
                    let parameters = program.state_parameters(state);
                    for (parameter_index, parameter) in parameters.iter().enumerate() {
                        index.add_all(
                            type_reference_reachable_expressions(
                                program,
                                parameter.type_reference,
                            )?,
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
                )?;
            }
            for operator in program.all_operators() {
                index_telescope_parameters(
                    program,
                    operator.symbol,
                    program.operator_parameters(operator),
                    index,
                )?;
            }
            for proposition in program.propositions() {
                index_telescope_parameters(
                    program,
                    proposition.symbol,
                    program.proposition_parameters(proposition),
                    index,
                )?;
            }
            Ok(())
        }

        pub(crate) fn index_telescope_parameters(
            program: &dyn CheckingInput,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            index: &mut OwnerIndex,
        ) -> Result<(), &'static str> {
            index_telescope_parameters_in_scope(program, owner, parameters, None, index)
        }

        fn index_telescope_parameters_in_scope(
            program: &dyn CheckingInput,
            owner: symbols::SymbolHandle,
            parameters: &[StateParameter],
            trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
            index: &mut OwnerIndex,
        ) -> Result<(), &'static str> {
            for (parameter_index, parameter) in parameters.iter().enumerate() {
                index.add_all(
                    type_reference_reachable_expressions(program, parameter.type_reference)?,
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
            Ok(())
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
        use typed_trees::expression::{ExpressionHandle, TableMemberExpression};
        use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

        pub(crate) fn type_reference_contains_expression(
            program: &dyn crate::CheckingInput,
            type_reference: TypeReferenceHandle,
            expression: ExpressionHandle,
            _visited: &mut Vec<TypeReferenceHandle>,
        ) -> Option<bool> {
            typed_trees::source_expression_types::type_inputs(
                program.typed_program(),
                type_reference,
            )
            .ok()
            .map(|inputs| inputs.expressions.contains(&expression))
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

        struct SourceTypes<'program>(&'program dyn crate::CheckingInput);
        impl typed_trees::source_expression_types::ExpressionTypes for SourceTypes<'_> {
            fn typed(&self) -> &typed_trees::TypedTrees {
                self.0.typed_program()
            }
            fn declaration_type(&self, symbol: SymbolHandle) -> Option<TypeReferenceHandle> {
                crate::declaration_type_reference(self.0, symbol)
            }
            fn member_type(
                &self,
                receiver: InferredType,
                member: &TableMemberExpression,
            ) -> Option<TypeReferenceHandle> {
                match receiver {
                    InferredType::TypeReference(reference) => {
                        crate::value_custody::places::applied_data_member_field(
                            self.0, reference, member,
                        )
                        .map(|field| field.type_reference)
                    }
                    InferredType::Nominal(_) => resolve_member_symbol(self.0, receiver, member)
                        .and_then(|symbol| crate::declaration_type_reference(self.0, symbol)),
                    InferredType::CollectionView(_) | InferredType::CompilerString => None,
                }
            }
            fn collection_measure_type(
                &self,
                receiver: InferredType,
                member: &TableMemberExpression,
            ) -> Option<TypeReferenceHandle> {
                collection_measure_in_environment(self.0, member, receiver)?;
                crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                    self.0,
                    symbols::BuiltinTypeAtom::U64,
                )
            }
            fn computation_type(
                &self,
                environment: &TypeEnvironment,
                expression: ExpressionHandle,
                leaf_type: &dyn Fn(ExpressionHandle) -> Option<TypeReferenceHandle>,
            ) -> Option<TypeReferenceHandle> {
                crate::value_custody::expression_types::declaration_expression_result_type_reference(
                    self.0,
                    environment.owner,
                    environment.trait_scope,
                    leaf_type,
                    expression,
                )
            }
        }
        pub(crate) fn infer_expression_type(
            program: &dyn crate::CheckingInput,
            expression: ExpressionHandle,
            environment: &TypeEnvironment,
            visited: &mut Vec<ExpressionHandle>,
        ) -> Option<InferredType> {
            typed_trees::source_expression_types::infer_expression_type(
                &SourceTypes(program),
                expression,
                environment,
                visited,
            )
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
            typed_trees::source_expression_types::collection_element_type(
                program.typed_program(),
                inferred,
            )
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

    pub(crate) use typed_trees::source_expression_types::{Binding, InferredType, TypeEnvironment};

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
        index: Result<OwnerIndex, &'static str>,
        program_only: std::sync::OnceLock<Box<OwnerEnvironmentIndex>>,
    }

    impl OwnerEnvironmentIndex {
        /// Lexical environments for retained contracts before checking publishes
        /// proof facts. These roots identify binders, never checked evidence.
        pub fn for_preparation(program: &dyn crate::CheckingInput) -> Self {
            let mut owners = Self::new(program, &arena::Arena::default());
            if let Ok(index) = &mut owners.index {
                indexing::index_contract_environments(program, index);
            }
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
        ) -> Result<Vec<(usize, usize, usize)>, &'static str> {
            Ok(self
                .executable_sites(expression)?
                .into_iter()
                .map(|site| (site.machine, site.state, site.statement))
                .collect())
        }

        pub fn contains_expression(
            &self,
            expression: ExpressionHandle,
        ) -> Result<bool, &'static str> {
            Ok(self
                .index
                .as_ref()
                .map_err(|reason| *reason)?
                .entries
                .contains_key(&expression))
        }

        pub fn validate_source_inputs(&self) -> Result<(), &'static str> {
            self.index.as_ref().map(|_| ()).map_err(|reason| *reason)
        }

        fn environments(&self, expression: ExpressionHandle) -> Option<Vec<TypeEnvironment>> {
            let index = self.index.as_ref().ok()?;
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

        fn executable_sites(
            &self,
            expression: ExpressionHandle,
        ) -> Result<Vec<ExecutableSite>, &'static str> {
            Ok(self
                .index
                .as_ref()
                .map_err(|reason| *reason)?
                .entries
                .get(&expression)
                .map(|entry| entry.executable_sites.clone())
                .unwrap_or_default())
        }

        pub fn containing_contract_facts(
            &self,
            expression: ExpressionHandle,
        ) -> Result<Vec<arena::Handle<ContractProofFact>>, &'static str> {
            Ok(self
                .index
                .as_ref()
                .map_err(|reason| *reason)?
                .entries
                .get(&expression)
                .map(|entry| entry.contract_facts.clone())
                .unwrap_or_default())
        }
    }

    pub fn source_declaration_owner(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        owners: &OwnerEnvironmentIndex,
    ) -> Option<SymbolHandle> {
        owners.validate_source_inputs().ok()?;
        let mut owner =
            crate::proof_contracts::contract_results::source_contract_declaration_owner(
                program, expression,
            )
            .ok()?;
        if let Some(environments) = owners.environments(expression) {
            for environment in environments {
                if let Some(contract) = environment.runtime_contract {
                    let row = program.typed_program().runtime_callable_contract(contract);
                    let candidate = row.lexical_owner;
                    if candidate != environment.owner
                        || !candidate.is_valid()
                        || (owner.is_valid() && owner != candidate)
                    {
                        return None;
                    }
                    owner = candidate;
                }
            }
        }
        for (machine, _, _) in owners.containing_statement_sites(expression).ok()? {
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
                assert_eq!(
                    index.containing_statement_sites(expression).unwrap(),
                    original
                );
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
                    index.containing_statement_sites(expression).unwrap(),
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
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use numerics::bignum::BigInt;
    use symbols::{BuiltinFunction, BuiltinType};
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
    };
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct IntegerEmbedding {
        pub(crate) primitive: PrimitiveType,
        pub(crate) interval: crate::proof_contracts::arithmetic_domains::BigIntInterval,
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
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::name::Identifier;
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
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let mut member = None;
        program.subtree_any(
            expression,
            typed_trees::SubtreeEdges::STRUCTURAL,
            |expression, _| {
                if visited.contains(&expression) {
                    return typed_trees::SubtreeVisit::Prune;
                }
                visited.push(expression);
                if let Some(name) = self_member_name(program, expression) {
                    member = Some(name.to_owned());
                    typed_trees::SubtreeVisit::Found
                } else {
                    typed_trees::SubtreeVisit::Descend
                }
            },
        );
        member
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
        typed_trees::source_expression_types::is_boolean_fact_expression(
            program.typed_program(),
            expression,
        )
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

pub mod properties {
    use diagnostics::Diagnostic;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::data::{DataDefinition, TypeParameter};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub use typed_trees::source_properties::DeclaredPropertyRequirement;

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
            program.typed_program(),
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
            program.typed_program(),
            data_definition,
            &mut |field, case: Option<&str>| {
                let actual = CarryDerivation::new(program.typed_program(), type_parameters)
                    .derive(field.type_reference);
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

    use typed_trees::source_carry::{CarryDerivation, for_each_stored_field};

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
        CarryDerivation::new(program.typed_program(), &[]).derive_named_data(
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

    pub use typed_trees::source_carry::TypeCarryRequirements;

    pub fn effective_type_carry_requirements(
        program: &dyn crate::CheckingInput,
        type_parameters: &[TypeParameter],
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> TypeCarryRequirements {
        CarryDerivation::new(program.typed_program(), type_parameters)
            .derive_requirements(type_reference)
    }

    pub use typed_trees::source_properties::{
        referenced_type_parameter, substituted_type_satisfies_copy,
        type_satisfies_declared_property,
    };

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
}

pub mod proposition_entailment {
    use typed_trees::trait_definition::Conformance;

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
        let evidence_trait = program.trait_definition_by_symbol(conformance.trait_symbol)?;
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
}

pub mod qualification_evidence {
    use crate::CheckingInput;
    use crate::frozen_program::expression_table;
    use language_semantics::DomainEstablishmentRoute;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::SignatureContractKind;
    use typed_trees::signature::StateSignature;
    use typed_trees::types::TypeReferenceHandle;

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
                    if !boundary_declares_qualification(
                        program,
                        definition.symbol,
                        requirement,
                        contract.kind.clone(),
                        fact,
                    ) {
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

    fn boundary_declares_qualification(
        program: &dyn CheckingInput,
        owner_symbol: SymbolHandle,
        signature: &StateSignature,
        contract_kind: SignatureContractKind,
        fact: &typed_trees::domain::ProofFact,
    ) -> bool {
        if contract_kind != SignatureContractKind::Ensures
            || !program
                .traits()
                .iter()
                .any(|definition| definition.symbol == owner_symbol && definition.is_boundary)
        {
            return false;
        }

        let Some(membership) = fact.as_membership() else {
            return false;
        };
        let subject_carrier = if expression_table(program).names_reserved_result(membership.value) {
            signature.return_type
        } else {
            let Some(parameter) = mutable_parameter(program, signature, membership.value) else {
                return false;
            };
            parameter.type_reference
        };
        if crate::proof_contracts::proof_facts::carry_permission(program, membership.domain)
            .is_some()
        {
            return true;
        }
        if !membership.domain_symbol.is_valid() {
            return false;
        }
        let Some(domain) = program.domain_definition(membership.domain_symbol) else {
            return false;
        };
        if membership.authored_domain_selection.is_none()
            || membership.domain_arguments.len() != domain.type_parameters.len()
        {
            return false;
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
            return false;
        }
        if !return_type_matches_domain_target(program, subject_carrier, domain.target_type) {
            return false;
        }

        true
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
