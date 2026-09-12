use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn input(program: &TypedTrees) -> TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    program.state_parameters(&program.machine_states(machine)[0])[0].type_reference
}

#[test]
fn empty_fixed_arrays_preserve_their_element_contents_classification() {
    for carrier in ["[u8; 0]", "Empty", "Holder<[u8; 0]>"] {
        let program = typed(&format!(
            "data Empty {{ bytes: [u8; 0]; }} data Holder<T> {{ value: T; }} machine inspect(value: {carrier}) {{}}"
        ));
        assert!(
            has_plain_owned_contents(&program, input(&program)),
            "{carrier}"
        );
        assert!(
            has_plain_owned_contents_with_numeric_constraints(&program, input(&program)),
            "{carrier}"
        );
        assert!(
            has_stable_observable_contents(&program, input(&program)),
            "{carrier}"
        );
    }
    for carrier in [
        "[Borrowed; 0]",
        "[Resource; 0]",
        "[Dropped; 0]",
        "[u8; Width]",
    ] {
        let program = typed(&format!(
            "data Borrowed {{ value: &u8; }} data Resource [linear] {{ value: u8; }} data Dropped {{ value: u8; }} machine Dropped::drop(&mut self) {{}} machine inspect<const Width: u64>(value: {carrier}) {{}}"
        ));
        assert!(
            !has_plain_owned_contents(&program, input(&program)),
            "{carrier}"
        );
        assert!(
            !has_plain_owned_contents_with_numeric_constraints(&program, input(&program)),
            "{carrier}"
        );
    }
}

#[test]
fn primitive_ranges_inside_owned_records_do_not_introduce_cleanup() {
    let source =
        "data Limits { limit: u64; divisor: u64 [3..=5]; } machine inspect(limits: Limits) {}";
    let program = typed(source);
    assert!(!has_plain_owned_contents(&program, input(&program)));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &program,
        input(&program)
    ));
    let nested = typed(
        "data Inner { divisor: u64 [3..=5]; } data Outer { inner: Inner; } machine inspect(outer: Outer) {}",
    );
    assert!(!has_plain_owned_contents(&nested, input(&nested)));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &nested,
        input(&nested)
    ));
    for source in [
        format!("{source} machine Limits::drop(&mut self) {{}}"),
        "data Inner { divisor: u64 [3..=5]; } data Outer { inner: Inner; } machine inspect(outer: Outer) {} machine Inner::drop(&mut self) {}".to_owned(),
    ] {
        let program = typed(&source);
        assert!(!has_plain_owned_contents(&program, input(&program)));
        assert!(!has_plain_owned_contents_with_numeric_constraints(&program, input(&program)));
    }
}

#[test]
fn primitive_range_admission_does_not_erase_other_constraints() {
    let mut program = typed("machine inspect(value: u64 [3..=5]) {}");
    let reference = input(&program);
    assert!(!has_plain_owned_contents(&program, reference));
    assert!(has_plain_owned_contents_with_numeric_constraints(
        &program, reference
    ));
    let TypeReferenceNode::Constrained { base_type, .. } =
        *program.type_reference_table.type_reference(reference)
    else {
        panic!("range constraint");
    };
    let constraints = program.type_reference_table.insert_constraints([
        typed_trees::types::TypeConstraintNode::Named("atomic".into()),
    ]);
    let qualified = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints,
        });
    assert!(!has_plain_owned_contents(&program, qualified));
    assert!(!has_plain_owned_contents_with_numeric_constraints(
        &program, qualified
    ));
    let empty = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints: arena::HandleSpan::empty(),
        });
    assert!(!has_plain_owned_contents(&program, empty));
    assert!(!has_plain_owned_contents_with_numeric_constraints(
        &program, empty
    ));
}

#[test]
fn stable_observation_composes_shared_contents_and_pure_value_domains() {
    for carrier in [
        "&[u8] in Text",
        "[u8; 8] in Text",
        "Holder<Holder<TextView>>",
        "&Holder<Holder<Words>>",
        "u64 in Positive",
        "Holder<PositiveField>",
        "PositiveField in Marked",
    ] {
        let program = typed(&format!(
            "domain [u8]::Text requires valid_utf8(self);
             domain [u8; 8]::Text requires valid_utf8(self);
             domain u64::Positive requires self > 0;
             data Holder<T> {{ value: T; }}
             data TextView {{ value: &[u8] in Text; }}
             data Words {{ value: &[u16]; }}
             data PositiveField {{ value: u64 in Positive; }}
             domain PositiveField::Marked;
             machine inspect(value: {carrier}) {{}}"
        ));
        let reference = input(&program);
        assert!(
            has_stable_observable_contents(&program, reference),
            "{carrier}"
        );
        assert!(!has_plain_owned_contents(&program, reference), "{carrier}");
        assert!(
            !has_plain_owned_contents_with_numeric_constraints(&program, reference),
            "{carrier}"
        );
    }
}

#[test]
fn stable_observation_rejects_nested_mutation_authority_and_unknown_qualifiers() {
    for carrier in [
        "&mut [u8]",
        "&write [u8]",
        "Holder<Holder<Mutable>>",
        "&Holder<WriteOnly>",
    ] {
        let program = typed(&format!(
            "data Holder<T> {{ value: T; }}
             data Mutable {{ value: &mut [u8]; }}
             data WriteOnly {{ value: &write u64; }}
             machine inspect(value: {carrier}) {{}}"
        ));
        assert!(
            !has_stable_observable_contents(&program, input(&program)),
            "{carrier}"
        );
    }

    // Unknown retained qualifications must not disappear through the scalar
    // classifier even though their old authored bracket surface is retired.
    for carrier in ["u64", "Holder<Scalar>", "&[u8]", "&Holder<Bytes>"] {
        for qualifier in ["atomic", "unknown"] {
            let mut program = typed(&format!(
                "data Holder<T> {{ value: T; }}
                 data Scalar {{ value: u64; }} data Bytes {{ value: &[u8]; }}
                 machine inspect(value: {carrier}) {{}}"
            ));
            assert!(has_stable_observable_contents(&program, input(&program)));
            let primitives = program
                .type_reference_table
                .named_references()
                .filter_map(|(reference, _, name)| {
                    matches!(name, "u64" | "u8").then_some(reference)
                })
                .collect::<Vec<_>>();
            for reference in primitives {
                let base = program
                    .type_reference_table
                    .type_reference(reference)
                    .clone();
                let base_type = program.type_reference_table.insert(base);
                let constraints = program.type_reference_table.insert_constraints([
                    typed_trees::types::TypeConstraintNode::Named(qualifier.into()),
                ]);
                program.type_reference_table.substitute_node(
                    reference,
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    },
                );
            }
            assert!(
                !has_stable_observable_contents(&program, input(&program)),
                "{carrier}, {qualifier}"
            );
        }
    }
}

#[test]
fn stable_observation_preserves_recursive_and_cleanup_exclusions() {
    for source in [
        "data Node { next: &Node; } machine inspect(value: Node) {}",
        "data Holder<T> { value: T; } data Node<T> { next: &Node<Holder<T>>; } machine inspect(value: Node<u64>) {}",
        "data Resource [linear] { value: u64; } machine inspect(value: &Resource) {}",
        "data Resource { value: u64; } machine Resource::drop(&mut self) {} machine inspect(value: &Resource) {}",
    ] {
        let program = typed(source);
        assert!(
            !has_stable_observable_contents(&program, input(&program)),
            "{source}"
        );
    }
}

#[test]
fn stable_observation_does_not_erase_domain_authority_metadata() {
    let original =
        typed("domain u64::Positive requires self > 0; machine inspect(value: u64 in Positive) {}");
    let reference = input(&original);
    assert!(has_stable_observable_contents(&original, reference));
    let TypeReferenceNode::Constrained { constraints, .. } =
        *original.type_reference_table.type_reference(reference)
    else {
        panic!("qualified scalar");
    };
    let [typed_trees::types::TypeConstraintNode::Domain(domain)] =
        original.type_reference_table.constraints(constraints)
    else {
        panic!("one exact domain");
    };
    let domain = domain.clone();
    let route = language_semantics::DomainEstablishmentRoute::CheckedRequirement {
        trait_definition: domain.symbol,
        requirement: domain.symbol,
    };
    for mutate_declaration in [false, true] {
        for change in ["route", "role", "classification", "alias_or_unresolved"] {
            let mut program = original.clone();
            if mutate_declaration {
                let (handle, _) = program
                    .tables
                    .domain_definitions
                    .iter()
                    .find(|(_, declaration)| declaration.symbol == domain.symbol)
                    .unwrap();
                let declaration = program.tables.domain_definitions.get_mut(handle);
                match change {
                    "route" => declaration.establishment_routes.push(route),
                    "classification" => {
                        declaration.classification =
                            Some(language_semantics::DomainClassification::ProgressProfile)
                    }
                    "role" => {
                        declaration.semantic_roles.denotation_dimension = Some(domain.semantic_id)
                    }
                    _ => {
                        declaration.alias =
                            Some(typed_trees::domain::DomainAliasDefinition::default())
                    }
                }
            } else {
                let mut changed = domain.clone();
                match change {
                    "route" => changed.establishment_routes.push(route),
                    "classification" => {
                        changed.classification =
                            Some(language_semantics::DomainClassification::ProgressProfile)
                    }
                    "role" => {
                        changed.semantic_roles.denotation_dimension = Some(domain.semantic_id)
                    }
                    _ => changed.symbol = symbols::SymbolHandle::invalid(),
                }
                program.type_reference_table.set_constraint_at_offset(
                    constraints,
                    0,
                    typed_trees::types::TypeConstraintNode::Domain(changed),
                );
            }
            assert!(
                !has_stable_observable_contents(&program, reference),
                "declaration={mutate_declaration}, {change}"
            );
        }
    }
}
