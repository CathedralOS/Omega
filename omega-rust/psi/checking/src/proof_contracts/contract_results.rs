//! Rejoin reserved `result` occurrences to their exact authored contract owner.
//! Integer embeddings and nominal tag predicates need the same result scope:
//! spelling or an equal carrier on another machine cannot authorize a result.

use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
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
/// Both the root and complete projection must occur in the same owning ensures.
pub fn reserved_result_place(
    program: &TypedTrees,
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
        && contract_occurrence_owner(program, root, expression)? != (machine_symbol, type_reference)
    {
        return None;
    }
    let mut segments = Vec::with_capacity(members.len());
    for member in members.into_iter().rev() {
        let receiver =
            crate::value_custody::places::unwrapped_type_reference(program, type_reference)?;
        // A generic base alone does not instantiate its field telescope. Named
        // specializations already carry the producer's concrete declaration.
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(receiver)
        else {
            return None;
        };
        if !symbol.is_valid() || program.symbols.get(*symbol).kind != symbols::SymbolKind::Data {
            return None;
        }
        let mut declarations = program
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == *symbol);
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
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    reserved_result_owner(program, expression).map(|(_, type_reference)| type_reference)
}

/// Identify the exact callable owning a reserved result occurrence. Matching
/// carrier types do not establish ownership, and an authored parameter named
/// `result` takes precedence over the reserved contract form.
pub fn reserved_result_owner(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if path.symbol.is_valid()
        || path.head_symbol.is_valid()
        || program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            .iter()
            .any(|symbol| symbol.is_valid())
        || !matches!(program.expression_table.name_path_members(path.members),
        [name] if name.as_str() == "result")
    {
        return None;
    }

    contract_occurrence_owner(program, expression, expression)
}

fn contract_occurrence_owner(
    program: &TypedTrees,
    root: ExpressionHandle,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let mut owner = None;
    let machines = program.machines().iter().filter_map(|machine| {
        let entry = program.machine_states(machine).first()?;
        Some((
            machine.symbol,
            entry.return_type,
            program.state_parameters(entry),
            program.machine_contracts(machine),
        ))
    });
    let requirements = program.traits().iter().flat_map(|definition| {
        program
            .trait_machine_signatures(definition)
            .iter()
            .map(|signature| {
                (
                    signature.symbol,
                    signature.return_type,
                    program.state_signature_parameters(signature),
                    program.state_signature_contracts(signature),
                )
            })
    });
    // The same occurrence-membership check serves both declarations. A trait
    // symbol cannot distinguish sibling requirements, and manufacturing a
    // machine for a bodyless signature would give it the wrong result owner.
    for (callable, return_type, parameters, contracts) in machines.chain(requirements) {
        for contract in contracts {
            let mut nodes = Vec::new();
            let mut seen = std::collections::HashSet::new();
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
            if !seen.contains(&expression) || !seen.contains(&root) {
                continue;
            }
            // Spelling is only the reserved-form discriminator. The full
            // expression handle must belong to this owning ensures clause;
            // another callable with an equal result type is not an owner.
            if contract.kind != SignatureContractKind::Ensures
                || !callable.is_valid()
                || !program
                    .type_reference_table
                    .contains_type_reference(return_type)
                || parameters
                    .iter()
                    .any(|parameter| parameter.name.as_str() == "result")
            {
                return None;
            }
            if owner.is_some_and(|candidate| candidate != (callable, return_type)) {
                return None;
            }
            owner = Some((callable, return_type));
        }
    }
    owner
}

#[cfg(test)]
mod tests {
    use super::{
        ExpressionHandle, ExpressionNode, TypeReferenceHandle, TypedTrees, reserved_result_owner,
        reserved_result_place,
    };

    const PROJECTED_RESULT: &str = "data Message { case Empty; case Data(value: u8); }
        data Inner { message: Message; }
        data Wrapper { inner: Inner; }
        data Foreign { message: Message; }
        machine make() -> Wrapper ensures result.inner.message in Message::Data;
        { Wrapper { inner: Inner { message: Message::Data { value: 1 } } } }";

    fn projected_occurrence(program: &TypedTrees) -> ExpressionHandle {
        program.expression_table.iter_expressions().find_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Member(member) if member.member.as_str() == "message")
                .then_some(handle)
        }).expect("authored message projection")
    }

    #[test]
    fn projected_result_rejects_forged_field_row_owner_and_stale_type() {
        let program = crate::front_end::typed_program(PROJECTED_RESULT);
        let expression = projected_occurrence(&program);
        let inner = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Inner")
            .unwrap();
        let foreign = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Foreign")
            .unwrap();
        let typed_trees::data::DataMember::Field(foreign_field) = &program.data_members(foreign)[0]
        else {
            panic!("foreign field");
        };
        for corrupt_owner in [true, false] {
            let mut altered = program.clone();
            let ExpressionNode::Member(member) =
                altered.expression_table.expression_mut(expression)
            else {
                panic!("member");
            };
            member.member_symbol = symbols::SymbolHandle::invalid();
            let typed_trees::data::DataMember::Field(field) =
                altered.data_members.get_mut(inner.members.start())
            else {
                panic!("inner field");
            };
            if corrupt_owner {
                field.symbol = foreign_field.symbol;
            } else {
                field.type_reference = TypeReferenceHandle::invalid();
            }
            assert!(reserved_result_place(&altered, expression).is_none());
        }
    }

    fn result_occurrences(program: &TypedTrees) -> Vec<ExpressionHandle> {
        program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Name(path)
                if matches!(program.expression_table.name_path_members(path.members),
                    [name] if name.as_str() == "result"))
                .then_some(handle)
            })
            .collect()
    }

    const REQUIREMENT_RESULTS: &str = "boundary trait Geometry {
        machine first(input: u64) -> u64 ensures result == input;
        machine second(input: u64) -> u64 ensures result == input;
    }";

    #[test]
    fn requirement_result_rejects_ambiguous_owner_and_invalid_result_type() {
        let program = crate::front_end::typed_program(REQUIREMENT_RESULTS);
        let signatures = program.traits()[0].machines;
        let first = signatures.start();
        let second = arena::Handle::from_parts(first.arena_index() + 1, first.generation());
        let first_contracts = program.trait_machine_signatures.get(first).contracts;
        let root = result_occurrences(&program)[0];
        let mut ambiguous = program.clone();
        ambiguous.trait_machine_signatures.get_mut(second).contracts = first_contracts;
        assert!(reserved_result_owner(&ambiguous, root).is_none());
        for reference in [
            TypeReferenceHandle::invalid(),
            arena::Handle::from_parts(u32::MAX, 1),
        ] {
            let mut invalid = program.clone();
            invalid.trait_machine_signatures.get_mut(first).return_type = reference;
            assert!(reserved_result_owner(&invalid, root).is_none());
        }
    }
}
