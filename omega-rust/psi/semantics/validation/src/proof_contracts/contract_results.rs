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
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(root) => {
                        crate::value_custody::expression_types::collect_expression_nodes(
                            program, *root, &mut nodes,
                        );
                    }
                    ProofFact::Membership(membership) => {
                        crate::value_custody::expression_types::collect_expression_nodes(
                            program,
                            membership.value,
                            &mut nodes,
                        );
                    }
                    ProofFact::Proposition(application) => {
                        for argument in program
                            .expression_table
                            .expression_handles(application.arguments)
                        {
                            crate::value_custody::expression_types::collect_expression_nodes(
                                program, *argument, &mut nodes,
                            );
                        }
                    }
                }
            }
            if !nodes.contains(&expression) || !nodes.contains(&root) {
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
        reserved_result_place, type_reference,
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
    fn nested_result_fields_retain_exact_root_owner_and_coordinates() {
        let program = typed(PROJECTED_RESULT);
        let expression = projected_occurrence(&program);
        let place = reserved_result_place(&program, expression).unwrap();
        assert_eq!(place.machine_symbol, program.machines()[0].symbol);
        assert_eq!(place.segments.len(), 2);
        let mut expected_owner = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Wrapper")
            .unwrap()
            .symbol;
        for segment in &place.segments {
            let facts::PlaceSegment::Field { symbol } = segment else {
                panic!("ordinary field");
            };
            assert_eq!(program.symbols.get(*symbol).parent, expected_owner);
            let field = program
                .data_members
                .iter()
                .find_map(|(_, member)| match member {
                    typed_trees::data::DataMember::Field(field) if field.symbol == *symbol => {
                        Some(field)
                    }
                    _ => None,
                })
                .unwrap();
            expected_owner = program
                .type_reference_table
                .type_symbol(field.type_reference);
        }
        assert_eq!(program.symbols.name(expected_owner), "Message");
        assert_eq!(
            program
                .type_reference_table
                .type_symbol(place.type_reference),
            expected_owner
        );
        assert_eq!(reserved_result_owner(&program, expression), None);
        let root = reserved_result_place(&program, place.root).unwrap();
        assert!(root.segments.is_empty());
        assert_ne!(root.type_reference, place.type_reference);
    }

    #[test]
    fn projected_result_rejects_unauthored_graft_and_foreign_selection() {
        let program = typed(PROJECTED_RESULT);
        let expression = projected_occurrence(&program);
        let mut grafted = program.clone();
        let forged = grafted
            .expression_table
            .insert(program.expression_table.expression(expression).clone());
        assert!(reserved_result_place(&grafted, forged).is_none());
        let foreign = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Foreign")
            .unwrap();
        let typed_trees::data::DataMember::Field(foreign_field) = &program.data_members(foreign)[0]
        else {
            panic!("foreign field");
        };
        for symbol in [foreign_field.symbol, program.machines()[0].symbol] {
            let mut altered = program.clone();
            let ExpressionNode::Member(member) =
                altered.expression_table.expression_mut(expression)
            else {
                panic!("member");
            };
            member.member_symbol = symbol;
            assert!(reserved_result_place(&altered, expression).is_none());
        }
    }

    #[test]
    fn projected_result_rejects_forged_field_row_owner_and_stale_type() {
        let program = typed(PROJECTED_RESULT);
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

    #[test]
    fn projected_result_respects_shadowing_and_ensures_scope() {
        for source in [
            "data Wrapper { message: u8; } machine value(result: Wrapper) -> Wrapper ensures result.message == 1; { result }",
            "data Wrapper { message: u8; } machine value() -> Wrapper requires result.message == 1; { Wrapper { message: 1 } }",
        ] {
            let program = typed(source);
            assert!(reserved_result_place(&program, projected_occurrence(&program)).is_none());
        }
    }

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
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
    fn requirement_result_scalars_keep_exact_signature_owners() {
        let program = typed(REQUIREMENT_RESULTS);
        let signatures = program.trait_machine_signatures(&program.traits()[0]);
        let occurrences = result_occurrences(&program);
        assert_eq!(occurrences.len(), 2);
        let owners = occurrences
            .iter()
            .map(|expression| {
                let (owner, reference) =
                    reserved_result_owner(&program, *expression).expect("requirement result owner");
                assert_eq!(
                    program.primitive_type_reference(reference),
                    Some(typed_trees::types::PrimitiveType::U64)
                );
                assert!(signatures.iter().any(|signature| signature.symbol == owner));
                assert_ne!(owner, program.traits()[0].symbol);
                owner
            })
            .collect::<Vec<_>>();
        assert_ne!(owners[0], owners[1]);
    }

    #[test]
    fn requirement_result_nested_fields_support_integer_embedding() {
        let program = typed(
            "data Range { length: u64; }
            data Packet { range: Range; }
            data Foreign { length: u64; }
            boundary trait Geometry {
                machine make(input: u64) -> Packet
                ensures embed(result.range.length) == embed(input);
            }",
        );
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| {
                matches!(node, ExpressionNode::Member(member) if member.member.as_str() == "length")
                    .then_some(handle)
            })
            .expect("nested length occurrence");
        let place =
            reserved_result_place(&program, expression).expect("signature result projection");
        assert_eq!(place.segments.len(), 2);
        assert_eq!(
            place.machine_symbol,
            program.trait_machine_signatures(&program.traits()[0])[0].symbol
        );
        assert_eq!(
            program.primitive_type_reference(place.type_reference),
            Some(typed_trees::types::PrimitiveType::U64)
        );
        assert!(
            program
                .expression_table
                .iter_expressions()
                .any(|(handle, _)| {
                    crate::proof_contracts::proof_embeddings::integer_embedding_argument(
                        &program, handle,
                    ) == Some((typed_trees::types::PrimitiveType::U64, expression))
                })
        );
        let mut grafted = program.clone();
        let unauthored = grafted
            .expression_table
            .insert(program.expression_table.expression(expression).clone());
        assert!(reserved_result_place(&grafted, unauthored).is_none());
        let foreign = program
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Foreign")
            .expect("foreign data");
        let typed_trees::data::DataMember::Field(field) = &program.data_members(foreign)[0] else {
            panic!("foreign length field");
        };
        let mut substituted = program.clone();
        let ExpressionNode::Member(member) =
            substituted.expression_table.expression_mut(expression)
        else {
            panic!("length projection");
        };
        member.member_symbol = field.symbol;
        assert!(reserved_result_place(&substituted, expression).is_none());
    }

    #[test]
    fn requirement_result_rejects_shadowing_and_requires_occurrences() {
        for source in [
            "boundary trait Geometry { machine make(result: u64) -> u64 ensures result == 1; }",
            "boundary trait Geometry { machine make(input: u64) -> u64 requires result == input; }",
        ] {
            let program = typed(source);
            let occurrences = result_occurrences(&program);
            assert!(!occurrences.is_empty());
            for expression in occurrences {
                assert!(reserved_result_owner(&program, expression).is_none());
            }
        }
    }

    #[test]
    fn requirement_result_rejects_ambiguous_owner_and_invalid_result_type() {
        let program = typed(REQUIREMENT_RESULTS);
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

    #[test]
    fn equal_result_carriers_keep_distinct_exact_contract_owners() {
        let program = typed(
            "machine first(input: u16) -> u16 ensures result == input { input }
             machine second(input: u16) -> u16 ensures result == input { input }",
        );
        let occurrences = result_occurrences(&program);
        assert_eq!(occurrences.len(), 2);
        let owners = occurrences
            .iter()
            .map(|expression| {
                let owner = reserved_result_owner(&program, *expression).unwrap();
                assert_eq!(type_reference(&program, *expression), Some(owner.1));
                assert_eq!(
                    program.primitive_type_reference(owner.1),
                    Some(typed_trees::types::PrimitiveType::U16)
                );
                owner.0
            })
            .collect::<Vec<_>>();
        assert_ne!(owners[0], owners[1]);
        for machine in program.machines() {
            assert!(owners.contains(&machine.symbol));
        }
    }

    #[test]
    fn authored_result_parameter_shadows_the_reserved_form() {
        let program =
            typed("machine identity(result: u16) -> u16 ensures result == result { result }");
        let occurrences = result_occurrences(&program);
        assert!(!occurrences.is_empty());
        for expression in occurrences {
            assert_eq!(reserved_result_owner(&program, expression), None);
            assert_eq!(type_reference(&program, expression), None);
        }
    }

    #[test]
    fn result_spelling_outside_ensures_has_no_reserved_owner() {
        let program =
            typed("machine invalid(input: u16) -> u16 requires result == input { input }");
        let occurrences = result_occurrences(&program);
        assert_eq!(occurrences.len(), 1);
        assert_eq!(reserved_result_owner(&program, occurrences[0]), None);
    }

    #[test]
    fn resolved_symbol_cannot_impersonate_reserved_result() {
        let program = typed("machine value(input: u16) -> u16 ensures result == input { input }");
        let expression = result_occurrences(&program)[0];
        assert!(reserved_result_owner(&program, expression).is_some());
        let symbol = program.machines()[0].symbol;
        for component in 0..3 {
            let mut altered = program.clone();
            let ExpressionNode::Name(mut path) = *altered.expression_table.expression(expression)
            else {
                panic!("result")
            };
            match component {
                0 => path.symbol = symbol,
                1 => path.head_symbol = symbol,
                _ => {
                    path.member_symbols = arena::HandleSpan::empty();
                    altered
                        .expression_table
                        .push_name_path_member_symbol(&mut path.member_symbols, symbol);
                }
            }
            *altered.expression_table.expression_mut(expression) = ExpressionNode::Name(path);
            assert_eq!(reserved_result_owner(&altered, expression), None);
        }
    }
}
