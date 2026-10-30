use super::*;
use arena::HandleSpan;
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::{
    TypedTrees,
    domain::{ProofFact, ProofMembershipFact},
    name::Identifier,
};

fn graph(
    edges: &[Vec<usize>],
    own_facts: &[bool],
    bad: Option<usize>,
) -> (TypedTrees, Vec<SymbolHandle>, Vec<TypeReferenceHandle>) {
    let names = ["Root", "D", "B", "Alias", "Leaf", "Tail", "Other", "Last"];
    let mut builder = SymbolTableBuilder::default();
    let mut symbols = Vec::new();
    let mut fields = Vec::new();
    for name in names.iter().take(edges.len()) {
        let symbol = builder.insert_root(SymbolKind::Data, SymbolNameRef::Static(name));
        fields.push(builder.insert_children(
            symbol,
            [
                (SymbolKind::Field, SymbolNameRef::Static("left")),
                (SymbolKind::Field, SymbolNameRef::Static("right")),
            ],
        ));
        symbols.push(symbol);
    }
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let references = symbols
        .iter()
        .zip(names)
        .map(|(&symbol, name)| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::from(name),
                })
        })
        .collect::<Vec<_>>();
    let truth = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(truth)]);
    let unsupported = program
        .proof_facts
        .insert_many([ProofFact::Membership(ProofMembershipFact::default())]);
    for (position, children) in edges.iter().enumerate() {
        let members = program
            .data_members
            .insert_many(children.iter().enumerate().map(|(i, &child)| {
                DataMember::Field(DataField {
                    symbol: fields[position].handle_at(i as u32),
                    name: Identifier::from(if i == 0 { "left" } else { "right" }),
                    type_reference: references[child],
                    ..Default::default()
                })
            }));
        program.push_data_definition(DataDefinition {
            symbol: symbols[position],
            name: Identifier::from(names[position]),
            members,
            where_facts: if bad == Some(position) {
                unsupported
            } else if own_facts[position] {
                facts
            } else {
                HandleSpan::empty()
            },
            ..Default::default()
        });
    }
    (program, symbols, references)
}

fn compare(program: &TypedTrees, definition: &DataDefinition, expected: bool) -> (usize, usize) {
    let operators = CheckedOperatorFacts::default();
    let mut own_evaluations = 0;
    assert_eq!(
        original_definition_storage(
            program,
            &operators,
            definition,
            &mut Vec::new(),
            &mut own_evaluations,
        ),
        expected
    );
    let mut visited = Vec::new();
    let mut checked_own = Vec::new();
    assert_eq!(
        complete_definition_storage(
            program,
            &operators,
            definition,
            &mut visited,
            &mut checked_own,
        ),
        expected
    );
    assert!(visited.is_empty());
    assert_eq!(
        declaration_facts_carried(program, &operators, definition),
        expected
    );
    (own_evaluations, checked_own.len())
}

#[test]
fn own_fact_reuse_preserves_original_storage_walks_and_alias_cycles() {
    // The original walk proves each of these shared own-fact rows again on
    // every path. The production walk still follows every edge, but each
    // successful nonempty own check is represented once in its local roster.
    let diamond = vec![vec![1, 2], vec![3, 3], vec![3, 3], vec![4, 4], vec![]];
    let (program, symbols, _) = graph(&diamond, &[true; 5], None);
    let root = program
        .unique_data_definition_by_symbol(symbols[0])
        .unwrap();
    let (old, once) = compare(&program, root, true);
    assert_eq!(old, 15);
    assert_eq!(once, 5);

    // Both genuine inline cycles and the coarser active-identity alias
    // counterexample must still traverse their children and refuse.
    let (program, symbols, _) = graph(&[vec![1], vec![0]], &[true; 2], None);
    compare(
        &program,
        program
            .unique_data_definition_by_symbol(symbols[0])
            .unwrap(),
        false,
    );
    let (mut program, symbols, references) =
        graph(&[vec![1, 3], vec![2], vec![], vec![1]], &[true; 4], None);
    let mut alias = program
        .unique_data_definition_by_symbol(symbols[3])
        .unwrap()
        .clone();
    alias.symbol = symbols[2]; // B1 and B2: same active identity, different rows.
    program.retain_data_application_definition(references[3], alias, true);
    assert!(program.data_definitions().iter().all(|definition| {
        definition_own_facts_carried(&program, &CheckedOperatorFacts::default(), definition)
    }));
    assert!(definition_own_facts_carried(
        &program,
        &CheckedOperatorFacts::default(),
        program.data_application_definition(references[3]).unwrap(),
    ));
    compare(
        &program,
        program
            .unique_data_definition_by_symbol(symbols[0])
            .unwrap(),
        false,
    );

    // Full generations remain significant to the old active fence. The
    // corresponding distinct borrowed view still owns its own field graph.
    let mut alias = program
        .data_application_definition(references[3])
        .unwrap()
        .clone();
    alias.symbol = arena::Handle::from_parts(symbols[2].arena_index(), symbols[2].generation() + 1);
    program.retain_data_application_definition(references[3], alias, true);
    compare(
        &program,
        program
            .unique_data_definition_by_symbol(symbols[0])
            .unwrap(),
        true,
    );

    // Same symbol/application does not grant another presentation's own
    // where proof. The earlier B1 succeeds; B2's distinct row still fails.
    let mut alias = program
        .data_application_definition(references[3])
        .unwrap()
        .clone();
    alias.symbol = symbols[2];
    alias.members = HandleSpan::empty(); // Failure must come from this row's own fact.
    alias.where_facts = program
        .proof_facts
        .insert_many([ProofFact::Membership(ProofMembershipFact::default())]);
    program.retain_data_application_definition(references[3], alias, true);
    compare(
        &program,
        program
            .unique_data_definition_by_symbol(symbols[0])
            .unwrap(),
        false,
    );

    // An unsupported own fact still short-circuits before the next field.
    let (program, symbols, _) = graph(&[vec![1, 2], vec![], vec![]], &[true; 3], Some(1));
    let root = program
        .unique_data_definition_by_symbol(symbols[0])
        .unwrap();
    let operators = CheckedOperatorFacts::default();
    let mut checked = Vec::new();
    assert!(!complete_definition_storage(
        &program,
        &operators,
        root,
        &mut Vec::new(),
        &mut checked
    ));
    assert_eq!(checked.len(), 1);
    assert!(std::ptr::eq(checked[0], root));
    compare(&program, root, false);

    for bad in [false, true] {
        let (mut program, symbols, references) =
            graph(&[vec![1], vec![]], &[true; 2], bad.then_some(1));
        let mut root = program
            .unique_data_definition_by_symbol(symbols[0])
            .unwrap()
            .clone();
        let original_field = program.data_members(&root)[0].as_field().unwrap().clone();
        for length in [
            FixedArrayLength::Literal(0),
            FixedArrayLength::Literal(2),
            FixedArrayLength::ConstParameter {
                symbol: symbols[0],
                name: Identifier::from("N"),
            },
        ] {
            let is_zero = matches!(length, FixedArrayLength::Literal(0));
            let is_literal = matches!(length, FixedArrayLength::Literal(_));
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: references[1],
                    length,
                });
            root.members = program
                .data_members
                .insert_many([DataMember::Field(DataField {
                    type_reference: reference,
                    ..original_field.clone()
                })]);
            compare(&program, &root, is_zero || (is_literal && !bad));
        }
        // A borrowed referent's own facts do not belong to this storage.
        for access in [
            ReferenceAccess::Shared,
            ReferenceAccess::Mutable,
            ReferenceAccess::WriteOnly,
        ] {
            let reference = program
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: references[1],
                    access,
                    lifetime: None,
                });
            root.members = program
                .data_members
                .insert_many([DataMember::Field(DataField {
                    type_reference: reference,
                    ..original_field.clone()
                })]);
            compare(&program, &root, true);
        }
    }
    // An unresolved extent may omit row-free elements, under the unchanged
    // storage_carries_rows rule; a nonempty own-fact element above cannot.
    let (mut program, symbols, references) = graph(&[vec![1], vec![]], &[true, false], None);
    let mut root = program
        .unique_data_definition_by_symbol(symbols[0])
        .unwrap()
        .clone();
    let reference = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: references[1],
            length: FixedArrayLength::ConstParameter {
                symbol: symbols[0],
                name: Identifier::from("N"),
            },
        });
    let mut field = program.data_members(&root)[0].as_field().unwrap().clone();
    field.type_reference = reference;
    root.members = program.data_members.insert_many([DataMember::Field(field)]);
    compare(&program, &root, true);
}

// Literal pre-edit DFS, with only test names and own-check counting added.
fn original_definition_storage(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    definition: &DataDefinition,
    visited: &mut Vec<StorageDefinitionIdentity>,
    own_evaluations: &mut usize,
) -> bool {
    // Emission cannot revisit an inline recursive owner. Borrowed referents
    // and empty arrays are excluded by the same storage traversal below.
    if visited.contains(&StorageDefinitionIdentity::of(definition))
        || !{
            *own_evaluations += usize::from(!definition.where_facts.is_empty());
            definition_own_facts_carried(program, operators, definition)
        }
    {
        return false;
    }
    visited.push(StorageDefinitionIdentity::of(definition));
    let complete = owned_storage_fields(program, definition).all(|(_, field)| {
        original_owned_storage(
            program,
            operators,
            field.type_reference,
            visited,
            own_evaluations,
        )
    });
    visited.pop();
    complete
}

fn original_owned_storage(
    program: &dyn crate::CheckingInput,
    operators: &CheckedOperatorFacts,
    reference: TypeReferenceHandle,
    visited: &mut Vec<StorageDefinitionIdentity>,
    own_evaluations: &mut usize,
) -> bool {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(reference);
    match program.type_reference_table.type_reference(carrier) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(0),
            ..
        } => true,
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => original_owned_storage(program, operators, *element_type, visited, own_evaluations),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            // The roster can enumerate only an exact retained literal extent.
            // Never publish a partial element predicate list for another form.
            !storage_carries_rows(program, operators, *element_type, &mut Vec::new())
        }
        TypeReferenceNode::Reference { .. } => true,
        _ => owned_data_definition(program, carrier).is_none_or(|definition| {
            original_definition_storage(program, operators, definition, visited, own_evaluations)
        }),
    }
}
