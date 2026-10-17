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

/// Resolve ordinary field projections rooted at the reserved contract result.
/// The result name and the complete projection share one owning ensures when
/// the occurrence index lists both. An intermediate field node that the index
/// does not list separately still belongs to that result; a different owner
/// for the same nodes does not.
pub fn reserved_result_place(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ReservedResultPlace> {
    let mut root = expression;
    let mut members = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(root) || members.len() >= 128 {
            return None;
        }
        match program.expression_table.expression(root) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                members.push(member);
                root = member.receiver;
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
    let mut segments = Vec::with_capacity(members.len());
    for member in members.into_iter().rev() {
        let receiver =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
        // A lifetime application such as `Held<'row>` keeps the base data's
        // fields. An open type argument is not that application: its field
        // types are not a concrete telescope, so this query still stops.
        let symbol = match program.type_reference_table.type_reference(receiver) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.is_empty()
                && base_symbol.is_valid()
                && program.symbols.get(*base_symbol).kind == symbols::SymbolKind::Data =>
            {
                *base_symbol
            }
            _ => return None,
        };
        if !symbol.is_valid() || program.symbols.get(symbol).kind != symbols::SymbolKind::Data {
            return None;
        }
        let mut declarations = program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == symbol);
        let data = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let field = crate::value_custody::places::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            None,
        )?;
        let declaration = program.symbols.get(field.symbol);
        if declaration.kind != symbols::SymbolKind::Field
            || declaration.parent != data.symbol
            || program.symbols.name(field.symbol) != field.name.as_str()
            || !program
                .type_reference_table
                .contains_type_reference(field.type_reference)
        {
            return None;
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
    for (callable, return_type, parameters, contract) in callable_contracts(program) {
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
/// machines with an entry state, operators, then trait requirement signatures.
/// Operators own their result guarantees even when their boundary body is
/// absent; ignoring them leaves the result projection without an owner. A trait
/// symbol cannot distinguish sibling requirements, and manufacturing a machine
/// for a bodyless signature would give it the wrong result owner.
fn callable_contracts(
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

/// The callable contracts of one program, in `callable_contracts` order, and
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
        for (callable, return_type, parameters, contract) in callable_contracts(program) {
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
