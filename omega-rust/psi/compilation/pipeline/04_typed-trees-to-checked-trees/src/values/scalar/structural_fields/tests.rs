use super::{exact_self_parameter, structural_parameter_field_path};
use crate::tests::front_end::typed_program;
use checked_trees::CheckedStructuralPredicatePathSegment;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceNode;

#[test]
fn local_record_array_guard_retains_exact_nested_storage_path() {
    let typed = typed_program(
        r#"
        data Values { items: [i32; 4]; other: [i32; 4]; }
        data Context { values: Values; }
        machine verify() {
            let mut context: Context;
            context.values.items[1] = 7;
            transition context.values.items[1] == 7 {
                true -> passed()
                false -> failed()
            }
            state passed() {}
            state failed() {}
        }
    "#,
    );
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "verify")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let statements = checked.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(local) = &statements[0] else {
        panic!("local context");
    };
    let typed_trees::statement::StatementNode::Assignment(assignment) = &statements[1] else {
        panic!("indexed write");
    };
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(assignment.target)
    else {
        panic!("array read");
    };
    let collection = indexed.collection;
    let parameters = checked.state_parameters(state);
    let (root, path, carrier) = super::owned_local_place(&checked, parameters, collection)
        .expect("nested local collection");
    assert_eq!(root, local.symbol);
    assert_eq!(path.len(), 2);
    assert!(matches!(
        checked.type_reference_table.type_reference(carrier),
        TypeReferenceNode::FixedArray { .. }
    ));
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .is_some(),
        "indexed guard and indexed write compose"
    );

    let owner = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Values")
        .unwrap();
    let foreign = checked
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == "other" => Some(field.symbol),
            _ => None,
        })
        .unwrap();
    let mut corrupted = checked.typed.clone();
    let ExpressionNode::Member(member) = corrupted.expression_table.expression_mut(collection)
    else {
        panic!("array member");
    };
    member.member_symbol = foreign;
    assert!(
        super::owned_local_place(&corrupted, parameters, collection).is_none(),
        "a same-typed sibling field must not replace the selected array"
    );
}

#[test]
fn named_state_owned_byte_guard_observes_live_length_after_indexed_writes() {
    let typed = typed_program(
        r#"
        domain [u8; 8]::Utf8 requires valid_utf8(self);
        machine verify() {
            transition { _ -> render() }
            state render() {
                let original: [u8; 8] in Utf8 = "ab";
                let mut line: [u8; 8] in Utf8 = "ab";
                line[0] = 0;
                line[1] = 65;
                transition line.len == 2 && line[0] == 0 && line[1] == 65
                    && original[0] == 97 && original[1] == 98 {
                    true -> finish(70)
                    _ -> finish(71)
                }
            }
            state finish(code: u64) {}
        }
    "#,
    );
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "verify")
        .unwrap();
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "render")
        .unwrap();
    let statements = checked.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(local) = &statements[1] else {
        unreachable!()
    };
    let (ordinal, guard) = statements
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                return None;
            };
            let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
                return None;
            };
            Some((ordinal, guard))
        })
        .unwrap();
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                u32::try_from(ordinal).unwrap(),
                checked_trees::CheckedScalarExpressionRole::Guard,
            )
            .is_some()
            || checked
                .facts
                .values
                .scalar_computations
                .root_at(
                    state.symbol,
                    u32::try_from(ordinal).unwrap(),
                    checked_trees::CheckedScalarExpressionRole::Guard,
                )
                .is_some(),
        "the entire authored guard retains its scalar observation plan"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .is_some(),
        "the observed guard composes with the two indexed writes"
    );
    let mut length = guard;
    while let ExpressionNode::Binary(binary) = checked.expression_table.expression(length) {
        length = binary.left;
    }
    let parameters = checked.state_parameters(state);
    assert!(
        matches!(super::structural_sequence_length(&checked, parameters, length),
        Some(checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
            root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol }, path,
        }) if symbol == local.symbol && path.is_empty())
    );

    let TypeReferenceNode::Constrained { base_type, .. } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        unreachable!()
    };
    let mut raw = checked.typed.clone();
    let typed_trees::statement::StatementNode::LocalData(raw_local) =
        &mut raw.statement_table.statements_mut(state.statement_nodes)[1]
    else {
        unreachable!()
    };
    raw_local.type_reference = *base_type;
    assert!(
        super::structural_sequence_length(&raw, parameters, length).is_none(),
        "raw fixed capacity must not be treated as a stored live byte extent"
    );
    let mut foreign_member = checked.typed.clone();
    let ExpressionNode::Member(member) = foreign_member.expression_table.expression_mut(length)
    else {
        panic!("leftmost guard operand is line.len");
    };
    member.member_symbol = local.symbol;
    assert!(
        super::structural_sequence_length(&foreign_member, parameters, length).is_none(),
        "a resolved foreign member cannot impersonate the builtin length observer"
    );
}

#[test]
fn named_state_owned_byte_index_read_retains_qualified_local_identity() {
    let typed = typed_program(
        "domain [u8; 10]::Utf8 requires valid_utf8(self);
         domain [u8; 4]::Utf8 requires valid_utf8(self);
         machine render(index: u64) {
             transition { _ -> write(index) }
             state write(index: u64) {
                 let digits: [u8; 10] in Utf8 = \"0123456789\";
                 let mut line: [u8; 4] in Utf8 = \"abcd\";
                 let selected: u64 = index % 10;
                 line[1] = digits[selected];
             }
         }",
    );
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "render")
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "write")
        .unwrap();
    let (ordinal, assignment) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| match statement {
            typed_trees::statement::StatementNode::Assignment(assignment) => {
                Some((ordinal, assignment))
            }
            _ => None,
        })
        .unwrap();
    let (binding, value) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state.symbol,
            u32::try_from(ordinal).unwrap(),
            checked_trees::CheckedScalarExpressionRole::AssignmentValue,
        )
        .expect("an indexed qualified byte local retains the ordinary assignment value");
    assert_eq!(binding.expression, assignment.value);
    let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
        root: checked_trees::CheckedStorageRoot::OwnedLocal { symbol },
        primitive_type,
        path,
        ..
    } = value
    else {
        panic!("exact owned indexed source: {value:?}");
    };
    assert_eq!(*primitive_type, typed_trees::types::PrimitiveType::U8);
    assert!(path.is_empty());
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(assignment.value)
    else {
        panic!("indexed source");
    };
    let (root, declared) =
        super::owned_local_root(program, program.state_parameters(state), indexed.collection)
            .expect("whole owned byte local");
    assert_eq!(root, *symbol);
    assert!(
        matches!(
            program.type_reference_table.type_reference(declared),
            TypeReferenceNode::Constrained { .. }
        ),
        "retaining the declared qualification keeps live-length carrier selection possible"
    );
    let mut borrowed = program.clone();
    let borrowed_type = borrowed
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            lifetime: None,
            referee: declared,
        });
    let local = borrowed
        .statement_table
        .statements_mut(state.statement_nodes)
        .iter_mut()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) if local.symbol == root => {
                Some(local)
            }
            _ => None,
        })
        .unwrap();
    local.type_reference = borrowed_type;
    assert!(
        super::owned_local_root(
            &borrowed,
            program.state_parameters(state),
            indexed.collection
        )
        .is_none(),
        "the same qualified referent behind a borrowed shell is not owned storage"
    );
}

#[test]
fn closed_generic_getters_retain_scalar_bindings_after_setters() {
    let typed = crate::tests::front_end::typed_program_with_generic_data_result(
        "data Cell<T> { value: T; touched: i32 in Wrapping; }
         machine Cell::set<T>(&mut self, value: T) {
             self.value = value;
             self.touched = self.touched + 1;
         }
         machine Cell::touch_count<T>(cell: &Cell<T>) -> i32 in Wrapping {
             transition { _ -> (cell.touched) }
         }
         data Pair { first: Cell<i32>; second: Cell<bool>; }
         machine exercise(pair: &mut Pair) -> i32 in Wrapping {
             pair.first.set(7);
             pair.second.set(true);
             let first: i32 in Wrapping = Cell::touch_count(&pair.first);
             let second: i32 in Wrapping = Cell::touch_count(&pair.second);
             first + second
         }",
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let program = &checked.typed;
    let mut observed_owners = Vec::new();
    for machine in program.machines().iter().filter(|machine| {
        machine.name.as_str().contains("touch_count")
            && program.machine_type_parameters(machine).is_empty()
    }) {
        let state = &program.machine_states(machine)[0];
        let parameters = program.state_parameters(state);
        let Some(owner) = super::structural_data(program, parameters[0].type_reference) else {
            continue;
        };
        if owner.generic_instance.is_none() {
            continue;
        }
        let (binding, _) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.symbol,
                0,
                checked_trees::CheckedScalarExpressionRole::Return,
            )
            .expect("closed getter retains its return expression and exact source binding");
        assert!(
            super::lower_structural_parameter_field(program, parameters, binding.expression)
                .is_some()
        );
        let ExpressionNode::Member(member) =
            program.expression_table.expression(binding.expression)
        else {
            panic!("getter returns one projected field");
        };
        let other = program
            .data_definitions()
            .iter()
            .find(|candidate| {
                candidate.symbol != owner.symbol && candidate.generic_instance.is_some()
            })
            .expect("other closed application");
        let foreign = program
            .data_members(other)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "touched" => Some(field.symbol),
                _ => None,
            })
            .unwrap();
        let mut invalid = program.clone();
        let mut wrong = member.clone();
        wrong.member_symbol = foreign;
        let expression = invalid
            .expression_table
            .insert(ExpressionNode::Member(wrong));
        assert!(
            super::lower_structural_parameter_field(&invalid, parameters, expression).is_none()
        );
        // Derived helpers and call specializations may share the same closed
        // owner. Every body still needs evidence, but those generated copies
        // do not introduce another application identity.
        if !observed_owners.contains(&owner.symbol) {
            observed_owners.push(owner.symbol);
        }
    }
    assert_eq!(observed_owners.len(), 2);
}

fn byte_length_fixture(source: &str) -> (TypedTrees, ExpressionHandle) {
    let program = typed_program(source);
    let ExpressionNode::Binary(relation) =
        program.expression_table.expression(requirement(&program))
    else {
        panic!("length requirement");
    };
    let length = relation.left;
    (program, length)
}

#[test]
fn byte_field_length_retains_exact_path_and_rejects_wrong_members() {
    let (program, length) = byte_length_fixture(
        "domain [u8; 16]::Utf8 requires valid_utf8(self);
         data Input { text: [u8; 16] in Utf8; }
         data Other { text: [u8; 16] in Utf8; }
         machine Input::measure(&mut self) -> u64
         requires self.text.len == 0 { 0 }",
    );
    let parameters = program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
    assert!(matches!(
        super::structural_sequence_length(&program, parameters, length),
        Some(checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
            root: checked_trees::CheckedStorageRoot::Parameter { index: 0 }, path,
        }) if path == [CheckedStructuralPredicatePathSegment::Field("text".into())]
    ));
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign text field");
    };
    let ExpressionNode::Member(member) = program.expression_table.expression(length) else {
        panic!("length member");
    };
    let receiver = member.receiver;
    for target in [length, receiver] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(target) else {
            panic!("selected member");
        };
        member.member_symbol = foreign.symbol;
        assert!(super::structural_sequence_length(&invalid, parameters, length).is_none());
    }
    let mut wrong_spelling = program.clone();
    let ExpressionNode::Member(member) = wrong_spelling.expression_table.expression_mut(length)
    else {
        panic!("length member");
    };
    member.member = "capacity".into();
    assert!(super::structural_sequence_length(&wrong_spelling, parameters, length).is_none());
}

/// A borrowed whole bounded owner observes its live length, not its capacity:
/// `text.len` of `&[u8; 8] in Utf8` is the parameter's runtime extent. A raw
/// borrowed `[u8; 8]` keeps its declared extent, and an owned bounded carrier
/// has no borrowed observation root.
#[test]
fn whole_bounded_owner_length_is_live_and_raw_array_length_is_declared() {
    let length_of = |source: &str| {
        let (program, length) = byte_length_fixture(source);
        let parameters =
            program.state_parameters(&program.machine_states(&program.machines()[0])[0]);
        super::structural_sequence_length(&program, parameters, length)
    };
    for access in ["&", "&mut "] {
        let source = format!(
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
             machine measure(text: {access}[u8; 8] in Utf8) -> u64
             requires text.len == 0 {{ 0 }}"
        );
        assert!(
            matches!(
                length_of(&source),
                Some(checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
                    root: checked_trees::CheckedStorageRoot::Parameter { index: 0 },
                    path,
                }) if path.is_empty()
            ),
            "{access} bounded owner observes its live length"
        );
    }
    assert!(matches!(
        length_of(
            "machine measure(bytes: &[u8; 8]) -> u64
             requires bytes.len == 8 { 0 }"
        ),
        Some(checked_trees::CheckedScalarExpression::IntegerLiteral { .. })
    ));
    assert!(
        length_of(
            "domain [u8; 8]::Utf8 requires valid_utf8(self);
             machine measure(text: [u8; 8] in Utf8) -> u64
             requires text.len == 0 { 0 }"
        )
        .is_none()
    );
}

fn fixture() -> TypedTrees {
    let source = "data Input { allowed: bool; }
        data Other { allowed: bool; }
        machine Input::read(&self, fallback: bool) -> bool
        requires self.allowed
        { transition { _ -> finish(fallback) }
          state finish(other: bool) -> bool { other } }
        machine Other::read(&self) -> bool { true }";
    typed_program(source)
}

fn requirement(program: &TypedTrees) -> ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("self requirement")
}

fn path(program: &TypedTrees) -> Option<Vec<CheckedStructuralPredicatePathSegment>> {
    let entry = &program.machine_states(&program.machines()[0])[0];
    let mut path = Vec::new();
    let position = structural_parameter_field_path(
        program,
        program.state_parameters(entry),
        requirement(program),
        &mut path,
    )?;
    assert_eq!(position, 0);
    Some(path)
}

#[test]
fn runtime_self_alias_preserves_identity_and_rejects_missing_or_foreign_fields() {
    let program = fixture();
    assert_eq!(
        path(&program),
        Some(vec![CheckedStructuralPredicatePathSegment::Field(
            "allowed".into()
        )])
    );
    let DataMember::Field(foreign) = &program.data_members(&program.data_definitions()[1])[0]
    else {
        panic!("foreign field");
    };
    let root = requirement(&program);
    let ExpressionNode::Member(member) = program.expression_table.expression(root) else {
        panic!("self field");
    };
    let stale = SymbolHandle::from_parts(
        member.member_symbol.arena_index(),
        member.member_symbol.generation() + 1,
    );
    for selected in [SymbolHandle::invalid(), foreign.symbol, stale] {
        let mut invalid = program.clone();
        let ExpressionNode::Member(member) = invalid.expression_table.expression_mut(root) else {
            panic!("self field");
        };
        member.member_symbol = selected;
        assert!(path(&invalid).is_none());
    }
    let mut numbered = program.clone();
    let members = numbered.data_definitions()[0].members;
    let DataMember::Field(field) = &mut numbered.tables.data_members.span_mut_or_empty(members)[0]
    else {
        panic!("original field");
    };
    field.identity = Some(7);
    assert_eq!(
        path(&numbered),
        Some(vec![CheckedStructuralPredicatePathSegment::Field(
            "#7".into()
        )])
    );
}

#[test]
fn runtime_self_alias_requires_the_exact_first_state_telescope() {
    let program = fixture();
    let states = program.machine_states(&program.machines()[0]);
    let parameters = program.state_parameters(&states[0]);
    let ExpressionNode::Member(member) = program.expression_table.expression(requirement(&program))
    else {
        panic!("self field");
    };
    let receiver = member.receiver;
    assert!(exact_self_parameter(&program, parameters, receiver).is_some());
    assert!(
        exact_self_parameter(&program, program.state_parameters(&states[1]), receiver).is_none()
    );
    let mut duplicate = parameters.to_vec();
    duplicate.push(parameters[0].clone());
    assert!(exact_self_parameter(&program, &duplicate, receiver).is_none());
    let mut reordered = parameters.to_vec();
    reordered.reverse();
    assert!(exact_self_parameter(&program, &reordered, receiver).is_none());
    let mut invalid = program.clone();
    let foreign_parameter =
        program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(states[0].parameters)[0]
        .symbol = foreign_parameter;
    assert!(path(&invalid).is_none());
}

#[test]
fn runtime_self_alias_rejects_wrong_owner_types_and_stale_or_excessive_wrappers() {
    let program = fixture();
    let parameters = program.machine_states(&program.machines()[0])[0].parameters;
    for (symbol, name) in [
        (program.data_definitions()[1].symbol, "Other"),
        (program.data_definitions()[0].symbol, "Self"),
        (SymbolHandle::invalid(), "Self"),
    ] {
        let mut invalid = program.clone();
        let reference = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: name.into(),
            });
        invalid
            .tables
            .state_parameters
            .span_mut_or_empty(parameters)[0]
            .type_reference = reference;
        assert!(path(&invalid).is_none());
    }
    let mut invalid = program.clone();
    let original = invalid.tables.state_parameters.span_or_empty(parameters)[0].type_reference;
    let stale = typed_trees::types::TypeReferenceHandle::from_parts(
        original.arena_index(),
        original.generation() + 1,
    );
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(parameters)[0]
        .type_reference = stale;
    assert!(path(&invalid).is_none());
    let mut invalid = program.clone();
    let mut reference = original;
    for _ in 0..65 {
        reference = invalid
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: reference,
                constraints: arena::HandleSpan::empty(),
            });
    }
    invalid
        .tables
        .state_parameters
        .span_mut_or_empty(parameters)[0]
        .type_reference = reference;
    assert!(path(&invalid).is_none());
}
