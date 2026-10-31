use super::*;
use crate::SpelledDeclarationTable;
use crate::domain::DomainDefinition;
use crate::name::Identifier;
use crate::signature::StateParameter;
use arena::{Arena, Handle, HandleSpan};
use std::sync::Arc;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn rows<T: Default>(values: Vec<T>, generation: u32) -> Arena<T> {
    Arena::from_storage(T::default(), values, None, generation, true).unwrap()
}
fn span<T>(start: u32, count: u32, generation: u32) -> HandleSpan<T> {
    HandleSpan::from_parts(Handle::from_parts(start, generation), count)
}
fn rebase<T>(old: HandleSpan<T>, generation: u32) -> HandleSpan<T> {
    if old.is_empty() {
        old
    } else {
        span(old.start().arena_index(), old.count(), generation)
    }
}

fn fixture() -> (TypedTrees, TypeReferenceHandle, TypeReferenceHandle) {
    let mut builder = SymbolTableBuilder::default();
    let u64_symbol = builder.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("u64"));
    let i64_symbol = builder.insert_root(SymbolKind::BuiltinType, SymbolNameRef::Static("i64"));
    let domains = [
        builder.insert_root(SymbolKind::Domain, SymbolNameRef::Static("First")),
        builder.insert_root(SymbolKind::Domain, SymbolNameRef::Static("Second")),
    ];
    let names = [
        "RootAdd",
        "RootMultiply",
        "NoToken",
        "MachineAdd",
        "DomainAdd",
        "DomainSubtract",
        "DomainI64",
    ];
    let symbols =
        names.map(|name| builder.insert_root(SymbolKind::Machine, SymbolNameRef::Static(name)));
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let u64_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: u64_symbol,
            name: Identifier::from("u64"),
        });
    let i64_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: i64_symbol,
            name: Identifier::from("i64"),
        });
    let mut parameters = |reference| {
        program.state_parameters.insert_many([
            StateParameter {
                type_reference: reference,
                ..Default::default()
            },
            StateParameter {
                type_reference: reference,
                ..Default::default()
            },
        ])
    };
    let unsigned_parameters = parameters(u64_type);
    let signed_parameters = parameters(i64_type);
    let spellings = [
        Some(OperatorSpelling::Add),
        Some(OperatorSpelling::Multiply),
        None,
        Some(OperatorSpelling::Add),
        Some(OperatorSpelling::Add),
        Some(OperatorSpelling::Subtract),
        Some(OperatorSpelling::Add),
    ];
    let operators = symbols
        .into_iter()
        .enumerate()
        .map(|(position, symbol)| OperatorDefinition {
            symbol,
            spelling: spellings[position],
            parameters: if position == 6 {
                signed_parameters
            } else {
                unsigned_parameters
            },
            home_domain: if position == 3 {
                domains[0]
            } else {
                SymbolHandle::invalid()
            },
            return_type: u64_type,
            ..Default::default()
        })
        .collect();
    program.tables.operators = rows(operators, 1).into();
    program.roots.operators = span(1, 3, 1);
    program.roots.machine_token_bindings = span(4, 1, 1);
    program.tables.domain_definitions = rows(
        vec![
            DomainDefinition {
                symbol: domains[0],
                name: Identifier::from("First"),
                target_type: u64_type,
                operators: span(5, 2, 1),
                ..Default::default()
            },
            DomainDefinition {
                symbol: domains[1],
                name: Identifier::from("Second"),
                target_type: i64_type,
                operators: span(7, 1, 1),
                ..Default::default()
            },
        ],
        1,
    )
    .into();
    program.roots.domain_definitions = span(1, 2, 1);
    program.tables.machines = rows(
        vec![crate::machine::Machine {
            symbol: symbols[3],
            name: Identifier::from("MachineAdd"),
            spelling: Some(OperatorSpelling::Add),
            ..Default::default()
        }],
        1,
    );
    program.roots.machines = span(1, 1, 1);
    (program, u64_type, i64_type)
}

// The literal pre-index discovery/filter owner. The tests compare public APIs
// against this independent whole-roster walk, including every row occurrence.
fn original<'a>(
    program: &'a TypedTrees,
    spelling: OperatorSpelling,
    receiver: Option<TypeReferenceHandle>,
    use_site: source::SourceSpan,
) -> Vec<SpelledOperator<'a>> {
    let root = program
        .operators()
        .iter()
        .chain(program.machine_token_bindings())
        .filter(|operator| operator.spelling == Some(spelling))
        .map(|operator| SpelledOperator {
            operator,
            domain: operator
                .home_domain
                .is_valid()
                .then(|| crate::domain::domain_by_symbol(program, operator.home_domain))
                .flatten(),
        });
    let domains = program.domain_definitions().iter().flat_map(|domain| {
        program
            .domain_operators(domain)
            .iter()
            .filter(move |operator| operator.spelling == Some(spelling))
            .map(move |operator| SpelledOperator {
                operator,
                domain: Some(domain),
            })
    });
    root.chain(domains)
        .filter(|candidate| {
            program
                .symbols
                .source_reference_may_select_symbol(use_site, candidate.operator.symbol)
                || (program
                    .symbols
                    .source_reference_can_see_symbol(use_site, candidate.operator.symbol)
                    && super::super::primitive_float::is_default_primitive_operator(
                        program,
                        candidate.operator,
                    ))
        })
        .filter(|candidate| {
            receiver.is_none_or(|receiver| {
                operator_matches_receiver(program, candidate.operator, receiver)
            })
        })
        .collect()
}
fn identities(rows: &[SpelledOperator<'_>]) -> Vec<(usize, Option<usize>)> {
    rows.iter()
        .map(|row| {
            (
                std::ptr::from_ref(row.operator).addr(),
                row.domain.map(|domain| std::ptr::from_ref(domain).addr()),
            )
        })
        .collect()
}
fn compare(program: &TypedTrees, unsigned: TypeReferenceHandle, signed: TypeReferenceHandle) {
    let site = source::SourceSpan::default();
    // The root machine view resolves its home through the same domain owner.
    // Compare that association independently of the operator discovery oracle.
    for symbol in program
        .domain_definitions()
        .iter()
        .map(|domain| domain.symbol)
        .chain(
            program
                .machine_token_bindings()
                .iter()
                .map(|row| row.home_domain),
        )
    {
        let expected = symbol
            .is_valid()
            .then(|| {
                program
                    .domain_definitions()
                    .iter()
                    .position(|row| row.symbol == symbol)
            })
            .flatten();
        assert_eq!(program.domain_position_by_symbol(symbol), expected);
    }
    let names = program.with_domain_candidate_index(
        |index| {
            index.qualified.extend(
                program
                    .domain_definitions()
                    .iter()
                    .map(|row| row.name.as_str().into()),
            );
        },
        |index| index.qualified.clone(),
    );
    assert_eq!(
        names,
        program
            .domain_definitions()
            .iter()
            .map(|row| Box::<str>::from(row.name.as_str()))
            .collect::<Vec<_>>()
    );
    for spelling in OperatorSpelling::ALL {
        for receiver in [None, Some(unsigned), Some(signed)] {
            assert_eq!(
                identities(&resolve_spelling(program, spelling, receiver, site)),
                identities(&original(program, spelling, receiver, site))
            );
        }
        for operands in [
            [OperandType::Unknown, OperandType::Unknown],
            [
                OperandType::Reference(unsigned),
                OperandType::Reference(unsigned),
            ],
            [
                OperandType::Reference(signed),
                OperandType::Reference(signed),
            ],
            [
                OperandType::Primitive(crate::types::PrimitiveType::U64),
                OperandType::Unknown,
            ],
        ] {
            let expected = original(program, spelling, None, site)
                .into_iter()
                .filter(|candidate| {
                    operator_matches_operands(program, candidate.operator, &operands)
                })
                .collect::<Vec<_>>();
            let actual = resolve_spelling_for_operand_types(program, spelling, &operands, site);
            assert_eq!(identities(&actual), identities(&expected));
            assert_eq!(
                spelling_for_operands_exists(program, spelling, &operands, site),
                !expected.is_empty()
            );
        }
    }
}

#[test]
fn token_rosters_match_original_discovery_across_owned_mutations_and_recovery() {
    let (mut program, unsigned, signed) = fixture();
    compare(&program, unsigned, signed);
    let first = program
        .operator_spelling_rows(OperatorSpelling::Add)
        .unwrap();
    assert_eq!(first.len(), 4);
    assert!(
        Arc::ptr_eq(
            &first,
            &program
                .operator_spelling_rows(OperatorSpelling::Add)
                .unwrap()
        ),
        "use-site queries share only raw rows"
    );
    // Root membership changes without any arena mutation or token change.
    let roots = program.roots.operators;
    program.roots.operators = HandleSpan::empty();
    compare(&program, unsigned, signed);
    program.roots.operators = roots;
    compare(&program, unsigned, signed);
    let domains = program.roots.domain_definitions;
    program.roots.domain_definitions = span(2, 1, 1);
    compare(&program, unsigned, signed);
    program.roots.domain_definitions = domains;
    compare(&program, unsigned, signed);
    let first = program
        .operator_spelling_rows(OperatorSpelling::Add)
        .unwrap();
    let site = source::SourceSpan::default();
    assert_ne!(
        identities(&resolve_spelling_for_operand_types(
            &program,
            OperatorSpelling::Add,
            &[OperandType::Unknown; 2],
            site
        )),
        identities(&resolve_spelling_for_operand_types(
            &program,
            OperatorSpelling::Add,
            &[OperandType::Reference(unsigned); 2],
            site
        ))
    );

    // The machine leg is the retained operator-signature view, not machines().
    // Replacing the physical machine table alone changes no raw membership;
    // the original scanner agrees. Publishing a changed view below does.
    let mut machine = program.machines()[0].clone();
    machine.name = Identifier::from("DifferentBodyOwner");
    machine.spelling = Some(OperatorSpelling::Divide);
    program.tables.machines = rows(vec![machine], 1);
    compare(&program, unsigned, signed);
    assert!(Arc::ptr_eq(
        &first,
        &program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
    ));
    let name = program
        .operator_path_members
        .insert_many([Identifier::from("RenamedView")]);
    program
        .tables
        .operators
        .get_mut(Handle::from_parts(4, 1))
        .name = name;
    compare(&program, unsigned, signed);
    assert!(!Arc::ptr_eq(
        &first,
        &program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
    ));

    // Repointing a full machine-view root span retains raw duplicates/order.
    program.roots.machine_token_bindings = program.roots.operators;
    compare(&program, unsigned, signed);
    assert_eq!(
        program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
            .len(),
        4
    );
    let domain = program.roots.domain_definitions.start();
    program.tables.domain_definitions.get_mut(domain).operators = span(5, 1, 1);
    compare(&program, unsigned, signed);

    // A branch keeps the old raw roster while a mutable escape invalidates
    // only the changed table's identity; all use-site matching still reruns.
    let mut branch = program.clone();
    branch
        .tables
        .operators
        .get_mut(Handle::from_parts(1, 1))
        .spelling = Some(OperatorSpelling::Divide);
    compare(&branch, unsigned, signed);
    compare(&program, unsigned, signed);
    assert_ne!(
        branch
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
            .len(),
        program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
            .len()
    );

    // Both unrelated replacements have revision0 and identical counts/spans.
    // The strong owner identities, not revision sums or recycled addresses,
    // distinguish their raw candidate and home-domain associations.
    let mut values = program.tables.operators.storage_view().items.to_vec();
    values[0].spelling = Some(OperatorSpelling::Divide);
    program.tables.operators = rows(values, 1).into();
    assert_eq!(program.tables.operators.revision(), 0);
    compare(&program, unsigned, signed);
    let revision = program.tables.operators.revision();
    let prior_rows = program
        .operator_spelling_rows(OperatorSpelling::Add)
        .unwrap();
    let mut replacement = program.tables.operators.storage_view().items.to_vec();
    replacement[0].spelling = Some(OperatorSpelling::Add);
    program.tables.operators = rows(replacement, 1).into();
    assert_eq!(program.tables.operators.revision(), revision);
    compare(&program, unsigned, signed);
    assert!(!Arc::ptr_eq(
        &prior_rows,
        &program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
    ));
    let mut domains = program
        .tables
        .domain_definitions
        .storage_view()
        .items
        .to_vec();
    domains[0].symbol = domains[1].symbol;
    domains[0].name = Identifier::from("Replacement");
    program.tables.domain_definitions = rows(domains, 1).into();
    assert_eq!(program.tables.domain_definitions.revision(), 0);
    compare(&program, unsigned, signed);
    let revision = program.tables.domain_definitions.revision();
    let prior_rows = program
        .operator_spelling_rows(OperatorSpelling::Add)
        .unwrap();
    let mut replacement = program
        .tables
        .domain_definitions
        .storage_view()
        .items
        .to_vec();
    replacement[0].operators = span(6, 1, 1);
    replacement[0].name = Identifier::from("AnotherReplacement");
    program.tables.domain_definitions = rows(replacement, 1).into();
    assert_eq!(program.tables.domain_definitions.revision(), revision);
    compare(&program, unsigned, signed);
    assert!(!Arc::ptr_eq(
        &prior_rows,
        &program
            .operator_spelling_rows(OperatorSpelling::Add)
            .unwrap()
    ));
    let machine = program.tables.operators.get(Handle::from_parts(4, 1));
    assert!(
        crate::domain::domain_by_symbol(&program, machine.home_domain).is_none(),
        "another row at the old position cannot become that owner"
    );

    // Full generational spans matter even when start indexes/counts match.
    let values = program.tables.operators.storage_view().items.to_vec();
    program.tables.operators = rows(values, 2).into();
    compare(&program, unsigned, signed);
    assert!(resolve_spelling(&program, OperatorSpelling::Add, None, site).is_empty());
    program.roots.operators = rebase(program.roots.operators, 2);
    program.roots.machine_token_bindings = rebase(program.roots.machine_token_bindings, 2);
    let root = program.roots.domain_definitions;
    for domain in program.tables.domain_definitions.span_mut_or_empty(root) {
        domain.operators = rebase(domain.operators, 2);
    }
    compare(&program, unsigned, signed);
    assert!(!resolve_spelling(&program, OperatorSpelling::Add, None, site).is_empty());
    let values = program
        .tables
        .domain_definitions
        .storage_view()
        .items
        .to_vec();
    program.tables.domain_definitions = rows(values, 2).into();
    compare(&program, unsigned, signed);
    program.roots.domain_definitions = rebase(program.roots.domain_definitions, 2);
    compare(&program, unsigned, signed);

    // Storage encoding and content equality do not include owner tokens.
    // Decode always starts a new owner, never a retained index authority.
    let arena = Arena::clone(&*program.tables.operators);
    let bytes =
        compiler_storage::encode(&arena, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(
        bytes,
        compiler_storage::encode(
            &program.tables.operators,
            compiler_storage::RecordLimits::default()
        )
        .unwrap()
    );
    let restored: SpelledDeclarationTable<OperatorDefinition> =
        compiler_storage::decode(&bytes, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(restored, program.tables.operators);
    assert!(restored.storage_identity() != program.tables.operators.storage_identity());
    let domain_arena = Arena::clone(&*program.tables.domain_definitions);
    let domain_bytes =
        compiler_storage::encode(&domain_arena, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(
        domain_bytes,
        compiler_storage::encode(
            &program.tables.domain_definitions,
            compiler_storage::RecordLimits::default()
        )
        .unwrap()
    );
    let domain_restored: SpelledDeclarationTable<DomainDefinition> =
        compiler_storage::decode(&domain_bytes, compiler_storage::RecordLimits::default()).unwrap();
    assert_eq!(domain_restored, program.tables.domain_definitions);
    assert!(
        domain_restored.storage_identity() != program.tables.domain_definitions.storage_identity()
    );
    let mut cold_program = program.clone();
    cold_program.declaration_indexes = Default::default();
    let cold =
        compiler_storage::encode(&cold_program, compiler_storage::RecordLimits::default()).unwrap();
    compare(&cold_program, unsigned, signed);
    assert_eq!(
        cold,
        compiler_storage::encode(&cold_program, compiler_storage::RecordLimits::default()).unwrap()
    );
    let mut recovered: TypedTrees =
        compiler_storage::decode(&cold, compiler_storage::RecordLimits::default()).unwrap();
    compare(&recovered, unsigned, signed);
    let replacement = rows(Vec::<OperatorDefinition>::new(), 1);
    recovered.tables.operators = replacement.into();
    compare(&recovered, unsigned, signed);
    assert!(resolve_spelling(&recovered, OperatorSpelling::Add, None, site).is_empty());
}
