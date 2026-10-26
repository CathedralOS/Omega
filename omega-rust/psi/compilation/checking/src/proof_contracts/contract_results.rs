//! Rejoin reserved `result` occurrences to their exact authored contract owner.
//! Integer embeddings and nominal tag predicates need the same result scope:
//! spelling or an equal carrier on another machine cannot authorize a result.

use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::types::TypeReferenceHandle;

/// Temporary interpretation of an exact authored result occurrence and its
/// declaration-owned field coordinates, without inventing a storage symbol.
#[derive(Debug, Clone)]
pub struct ReservedResultPlace {
    /// Exact callable declaration: a machine or a requirement signature, not
    /// the trait containing that requirement.
    pub machine_symbol: symbols::SymbolHandle,
    pub root: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub segments: Vec<facts::PlaceSegment>,
}

/// Resolve declared fields and literal fixed indices rooted at the contract result.
/// Case payloads keep their nominal Case/Field coordinates; resolving a path
/// supplies neither membership nor read authority.
/// The result name and the complete projection share one owning ensures when
/// the occurrence index lists both. An intermediate field node that the index
/// does not list separately still belongs to that result; a different owner
/// for the same nodes does not.
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
        match program.expression_table.expression(root) {
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
    for projection in projections.into_iter().rev() {
        if let ExpressionNode::Indexed(indexed) = program.expression_table.expression(projection) {
            // Storage spelling does not select [] meaning. Authenticate the
            // exact declaration telescope and retained operator occurrence;
            // an authored operator may return an entirely different value.
            // Only an immutable literal selector enters this path. Computed
            // selectors need their own completed-capture evidence, even when
            // a later storage observation could give them the same number.
            let literal = program.expression_table.integer_node(indexed.index)?;
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
            let mut occurrences = program
                .expression_table
                .authored_selection_occurrences(projection);
            let occurrence = occurrences.next()?;
            let selection = program.authored_declaration_selections().get(occurrence)?;
            // Stage 04 settles ordinary operator occurrences after proving
            // contracts. The late binding is source custody, not a builtin
            // premise: the exact owner/operand query below must independently
            // reconstruct builtin meaning before the coordinate is usable.
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
            let receiver =
                crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
            let typed_trees::types::TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(extent),
            } = program.type_reference_table.type_reference(receiver)
            else {
                return None;
            };
            if index >= *extent
                || !program
                    .type_reference_table
                    .contains_type_reference(*element_type)
            {
                return None;
            }
            segments.push(facts::PlaceSegment::FixedIndex { index });
            type_reference = *element_type;
            continue;
        }
        let ExpressionNode::Member(member) = program.expression_table.expression(projection) else {
            return None;
        };
        let receiver =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
        // Resolve the complete closed application, not just a named node or
        // a generic base. The type owner retains concrete specializations and
        // lifetime-only applications without manufacturing a field telescope.
        let data = program.closed_data_definition(receiver)?;
        if !data.symbol.is_valid()
            || program.symbols.get(data.symbol).kind != symbols::SymbolKind::Data
        {
            return None;
        }
        // Reuse the applied member join: prepared fields may translate the
        // retained template selector only through this exact receiver. A case
        // qualifier chooses its declared payload namespace, never another
        // case's same-spelled field or evidence that the case currently holds.
        let field =
            crate::value_custody::places::applied_data_member_field(program, receiver, member)?;
        let declaration = program.symbols.get(field.symbol);
        if declaration.kind != symbols::SymbolKind::Field
            || program.symbols.name(field.symbol) != field.name.as_str()
            || !program
                .type_reference_table
                .contains_type_reference(field.type_reference)
        {
            return None;
        }
        if declaration.parent != data.symbol {
            let mut cases = program.data_members(data).iter().filter_map(|member| {
                let typed_trees::data::DataMember::Variant(case) = member else {
                    return None;
                };
                (case.symbol == declaration.parent).then_some(case)
            });
            let case = cases.next()?;
            if cases.next().is_some()
                || program.symbols.get(case.symbol).kind != symbols::SymbolKind::Variant
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
        }
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
    })
}

pub(crate) fn type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    reserved_result_owner(program, expression).map(|(_, type_reference)| type_reference)
}

/// Identify the exact callable owning a reserved result occurrence. Matching
/// carrier types do not establish ownership, and an authored parameter named
/// `result` takes precedence over the reserved contract form.
pub fn reserved_result_owner(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    if !program.expression_table.names_reserved_result(expression) {
        return None;
    }

    contract_occurrence_owner(program, expression, expression)
}

/// Result dependence belongs to the authored contract, even when a selected
/// operation has no first-order structural term. This classifies functional
/// guarantees; it grants no proof of them and preserves formal shadowing.
pub(crate) fn expression_uses_reserved_result(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::<_, symbols::BuildSymbolHasher>::default();
    crate::expression_traversal::collect_expression_nodes(
        program, expression, &mut nodes, &mut seen,
    );
    nodes
        .into_iter()
        .any(|node| reserved_result_owner(program, node).is_some())
}

/// Proof correspondence may use an indexed expression as an element
/// coordinate only under its authored declaration's operator meaning. A
/// provider's equal carrier and same syntax cannot supply that premise.
/// Range operations retain their separate existing interpretation.
pub(crate) fn expression_has_source_builtin_index_meaning(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::<_, symbols::BuildSymbolHasher>::default();
    crate::expression_traversal::collect_expression_nodes(
        program, expression, &mut nodes, &mut seen,
    );
    nodes.into_iter().all(|node| {
        let Some(indexed) = program.expression_table.indexed_node(node) else {
            return true;
        };
        if program.expression_table.indexed_on_range(indexed) {
            return true;
        }
        let Ok(owner) = source_contract_declaration_owner(program, node) else {
            return false;
        };
        let Some(parameters) = source_contract_parameters(program, owner, node) else {
            return false;
        };
        let operands = [indexed.collection, indexed.index].map(|operand| {
            crate::parameter_expression_result_type_reference(program, owner, parameters, operand)
        });
        // Raw contract literals need not have a runtime integer carrier.
        // Keep the known collection and pass an unresolved selector through
        // the shared query's conservative overload matching; do not guess
        // its type or let its absence eliminate an authored candidate.
        operands[0].is_some()
            && typed_trees::operator::builtin_index_read_is_spelled(program, owner, node, &operands)
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
                crate::expression_traversal::collect_type_expressions(
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

/// Inspect transparent formulas before any direct-edge proof deferral, not
/// only when correspondence happens to expand them. Alias bodies retain their
/// authored source scope; actual arguments cannot turn an unsupported body
/// operation into the provider's builtin operation.
fn proposition_has_source_builtin_index_meaning(
    program: &dyn crate::CheckingInput,
    application: &typed_trees::proposition::PropositionApplication,
    expanding: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    use typed_trees::proposition::{PropositionBody, PropositionFormula};

    if !program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .all(|argument| expression_has_source_builtin_index_meaning(program, *argument))
    {
        return false;
    }
    let Some(declaration) = program.proposition_definition_by_symbol(application.proposition)
    else {
        // Opaque proposition binders retain their existing declaration-based
        // correspondence. This query supplies no interpretation of them.
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

/// Exact declaration owning a raw authored contract expression. This query
/// grants source scope only; it does not assume the contract was proved and
/// does not require the expression to use the reserved result form.
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
    // A frozen program answers every occurrence from one pass over its
    // contracts; any other program rescans them per query.
    if let Some(memos) = crate::frozen_program::frozen_program_memos(program) {
        return memos
            .contract_occurrences
            .get_or_init(|| std::sync::Arc::new(ContractOccurrenceIndex::of(program)))
            .owner(root, expression);
    }
    let mut owner = None;
    for (callable, return_type, parameters, contract) in callable_contract_scopes(program) {
        let seen = contract_nodes(program, contract);
        if !seen.contains(&expression) || !seen.contains(&root) {
            continue;
        }
        // Spelling is only the reserved-form discriminator. The full
        // expression handle must belong to this owning ensures clause;
        // another callable with an equal result type is not an owner.
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

/// Applied facts keep their declaration symbol but have fresh formal rows.
/// Rejoin the exact occurrence to that retained roster, not the template's
/// same-symbol signature or a type guessed from its substituted proof value.
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
        &SignatureContract,
    ),
> + '_ {
    let machines = program.machines().iter().filter_map(|machine| {
        let entry = program.machine_states(machine).first()?;
        Some((
            machine.symbol,
            entry.return_type,
            entry.parameters,
            program.machine_contracts(machine),
        ))
    });
    let requirements = program
        .traits()
        .iter()
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .chain(program.prepared_callable_signatures())
        .chain(
            machine_parameter_contract_signatures(program)
                .into_iter()
                .map(|(_, signature)| signature),
        )
        .map(|signature| {
            (
                signature.symbol,
                signature.return_type,
                signature.parameters,
                program.state_signature_contracts(signature),
            )
        });
    let operators = program.operators().iter().map(|operator| {
        (
            operator.symbol,
            operator.return_type,
            operator.parameters,
            program.operator_contracts(operator),
        )
    });
    machines.chain(operators).chain(requirements).flat_map(
        |(callable, return_type, parameters, contracts)| {
            contracts
                .iter()
                .map(move |contract| (callable, return_type, parameters, contract))
        },
    )
}

/// Follow machine-parameter contracts from live declaration telescopes. Backing
/// arenas also retain private applications, so arena membership is not a scope.
pub(crate) fn machine_parameter_contract_signatures(
    program: &dyn crate::CheckingInput,
) -> Vec<(
    &typed_trees::data::TypeParameter,
    &typed_trees::signature::StateSignature,
)> {
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
        .flat_map(|definition| program.trait_machine_signatures(definition))
        .chain(program.prepared_callable_signatures())
    {
        parameters.extend(program.state_signature_type_parameters(signature));
    }
    let mut signatures = Vec::<(
        &typed_trees::data::TypeParameter,
        &typed_trees::signature::StateSignature,
    )>::new();
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

/// Every expression node one contract's facts reach.
fn contract_nodes(
    program: &dyn crate::CheckingInput,
    contract: &SignatureContract,
) -> std::collections::HashSet<ExpressionHandle, symbols::BuildSymbolHasher> {
    let mut nodes = Vec::new();
    let mut seen = std::collections::HashSet::default();
    for fact in program.proof_facts.span_or_empty(contract.facts) {
        match fact {
            ProofFact::Expression(root) => {
                crate::expression_traversal::collect_expression_nodes(
                    program, *root, &mut nodes, &mut seen,
                );
            }
            ProofFact::Membership(membership) => {
                crate::expression_traversal::collect_expression_nodes(
                    program,
                    membership.value,
                    &mut nodes,
                    &mut seen,
                );
            }
            ProofFact::Proposition(application) => {
                for argument in program
                    .expression_table
                    .expression_handles(application.arguments)
                {
                    crate::expression_traversal::collect_expression_nodes(
                        program, *argument, &mut nodes, &mut seen,
                    );
                }
            }
        }
    }
    seen
}

/// Whether a contract reaching a reserved `result` occurrence can own it: an
/// `ensures` of a valid callable with a result type and no parameter that
/// shadows the reserved name.
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

/// The callable contracts of one program, in `callable_contract_scopes` order, and
/// for every expression node the ascending positions of the contracts that
/// reach it.
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
        for (callable, return_type, parameters, contract) in callable_contract_scopes(program) {
            let position =
                u32::try_from(contracts.len()).expect("callable contract position overflow");
            for node in contract_nodes(program, contract) {
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

    fn declaration_owner(&self, expression: ExpressionHandle) -> Result<symbols::SymbolHandle, ()> {
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

    /// The scan's verdict: the contracts reaching both nodes, in order.
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
