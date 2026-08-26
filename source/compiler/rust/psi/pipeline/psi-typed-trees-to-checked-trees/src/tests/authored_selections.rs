use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionIntrinsic, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelectionTarget,
};

#[test]
fn successful_checking_finalizes_authored_call_occurrences() {
    let source = r#"
        machine identity(value: u32) -> u32 { value }
        machine compare(value: u32) -> bool { identity(value) == value }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let selections = checked.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::Call
            && matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(
        selections.iter().any(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::Operator
                && selection.target()
                    == AuthoredDeclarationSelectionTarget::Intrinsic(
                        AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                    )
        }),
        "selections={selections:#?}; operators={:#?}",
        checked.facts.operators
    );
    assert!(selections.all_finalized());
}

#[test]
fn successful_checking_finalizes_declared_operator_occurrences() {
    let source = r#"
        data Quantity { value: i32; }

        domain Quantity::Additive
        requires
            self.value >= 0;

        operator + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity;

        data Main {}

        machine Main::combine(&self, left: Quantity, right: Quantity)
        requires
            left in Quantity::Additive
        {
            let sum: Quantity = left + right;
        }

        machine Main::main(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    assert!(
        resolved
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::Operator
                    && matches!(
                        selection.target(),
                        AuthoredDeclarationSelectionTarget::LateBound(_)
                    )
            })
    );

    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let selections = checked.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::Operator
            && matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
}

#[test]
fn successful_checking_finalizes_inferred_field_members_and_primitive_operators() {
    let source = r#"
        data Build { freestanding: bool; }

        machine Build::configure(&mut self) {
            self.freestanding = false;
            let unchanged: bool = self.freestanding == false;
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let selections = checked.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess
            && matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::Operator
            && selection.target()
                == AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
                )
    }));
    assert!(selections.all_finalized(), "selections={selections:#?}");
}

#[test]
fn successful_checking_finalizes_nested_intrinsic_logical_operators() {
    let source = r#"
        data Reading { value: i64; minimum: i64; maximum: i64; }

        machine within_calibration(reading: Reading) -> bool {
            reading.value >= reading.minimum && reading.value <= reading.maximum
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let operators = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();

    assert_eq!(operators.len(), 3);
    assert!(operators.iter().all(|selection| {
        selection.target()
            == AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            )
    }));
    let indexed = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(
                node,
                psi_typed_trees::expression::ExpressionNode::Indexed(_)
            )
            .then_some(expression)
        })
        .expect("checked program retains indexed expression");
    assert!(crate::authored_selections::typed_operator_is_definitely_intrinsic(&checked, indexed));
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_finalizes_index_and_range_operator_occurrences() {
    let source = r#"
        proposition selected(value: i32);
        proposition window(values: &[i32]);
        machine inspect(values: [i32; 2])
        requires
            selected(values[0]),
            window(values[0..1])
        { }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let operators = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();

    assert_eq!(operators.len(), 2, "selections={operators:#?}");
    assert!(operators.iter().all(|selection| {
        selection.target()
            == AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::BuiltinOperator,
            )
    }));
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_retains_inferred_generic_call_conformance() {
    let source = r#"
        trait Marker { }
        data Good { }
        GoodMarker: Good satisfies Marker;

        machine accepts<T>(value: T) -> bool
        where T satisfies Marker
        {
            true
        }

        machine caller(value: Good) -> bool { accepts(value) }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let selected = resolved
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "GoodMarker")
        })
        .expect("GoodMarker conformance")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    assert!(
        checked
            .authored_declaration_selections()
            .iter()
            .any(|selection| {
                selection.kind() == AuthoredDeclarationSelectionKind::Conformance
                    && matches!(
                        selection.target(),
                        AuthoredDeclarationSelectionTarget::Resolved(target)
                            if target.selected_symbol() == selected
                    )
            }),
        "selections={:#?}; specializations={:#?}",
        checked.authored_declaration_selections(),
        checked.machine_specializations,
    );
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.inferred_conformance_arguments == [selected])
        .expect("specialization retains its exact inferred conformance");
    assert!(
        specialization.conformance_arguments.is_empty(),
        "inferred conformance must not impersonate an explicit evidence argument"
    );
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_retains_inferred_statement_call_conformance() {
    let source = r#"
        trait Marker { }
        data Good { }
        GoodMarker: Good satisfies Marker;

        machine accepts<T>(value: T)
        where T satisfies Marker
        {
        }

        machine caller(value: Good) { accepts(value); }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let selected = resolved
        .conformances
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|alias| alias.as_str() == "GoodMarker")
        })
        .expect("GoodMarker conformance")
        .symbol;
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let call = checked
        .authored_declaration_selections()
        .iter()
        .find(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
        .expect("statement call selection");
    let inferred = checked
        .authored_declaration_selections()
        .iter()
        .find(|selection| {
            selection.kind() == AuthoredDeclarationSelectionKind::Conformance
                && selection.source_span() == call.source_span()
                && matches!(
                    selection.target(),
                    AuthoredDeclarationSelectionTarget::Resolved(target)
                        if target.selected_symbol() == selected
                )
        })
        .expect("inferred statement-call conformance selection");

    assert_eq!(inferred.exposure(), call.exposure());
    assert!(checked.authored_declaration_selections().all_finalized());
}

#[test]
fn successful_checking_finalizes_attached_calls_through_parameter_fields() {
    let source = r#"
        domain [u8]::Path requires no_nul(self);
        data BuildSource {}
        data Build { source: BuildSource; }
        machine BuildSource::resolve<'path>(
            &self,
            relative: &'path [u8] in Path
        ) -> &'path [u8] in Path {
            relative
        }
        machine build(builder: &mut Build) {
            let resolved: &[u8] in Path = builder.source.resolve("input.txt");
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    assert!(
        checked
            .authored_declaration_selections()
            .iter()
            .any(|selection| matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Intrinsic(
                    AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                        psi_language_semantics::byte_predicates::ByteSequencePredicate::NoNul,
                    )
                )
            ))
    );
    assert!(
        checked.authored_declaration_selections().all_finalized(),
        "selections={:#?}",
        checked.authored_declaration_selections()
    );
}

#[test]
fn declared_call_wins_over_byte_predicate_intrinsic_spelling() {
    let source = r#"
        machine no_nul(value: &[u8]) -> bool { true }
        domain [u8]::Path requires no_nul(self);
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let selections = checked.authored_declaration_selections();

    assert!(selections.iter().any(|selection| {
        selection.kind() == AuthoredDeclarationSelectionKind::Call
            && matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(_)
            )
    }));
    assert!(!selections.iter().any(|selection| {
        selection.target()
            == AuthoredDeclarationSelectionTarget::Intrinsic(
                AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(
                    psi_language_semantics::byte_predicates::ByteSequencePredicate::NoNul,
                ),
            )
    }));
    assert!(selections.all_finalized());
}

#[test]
fn successful_checking_binds_boundary_calls_through_parameter_fields() {
    let source = r#"
        boundary trait FilesystemHost {
            machine open(&self, path: &[u8], flags: i32) -> i32
            reaches FilesystemHost;
        }
        data Build { filesystem: FilesystemHost; }
        machine build(builder: &mut Build)
        reaches FilesystemHost
        {
            let descriptor: i32 = builder.filesystem.open("input.txt", 0);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::Call(call)
                if call.target.as_str() == "open" =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("boundary call");
    let requirement = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "FilesystemHost")
        .and_then(|definition| checked.trait_machine_signatures(definition).first())
        .expect("filesystem requirement");
    assert_eq!(call.target_symbol, requirement.symbol);
}

#[test]
fn successful_checking_canonicalizes_local_selections_across_specializations() {
    let source = r#"
        data Light [copy] { weight: i32; }
        data Main { light: Light; number: i32; }
        machine Main::pick<T [copy]>(&self, value: &T) -> i32 {
            let selected: i32 = 7;
            transition { _ -> selected }
        }
        machine Main::use_both(&mut self) {
            let from_light: i32 = self.pick(&self.light);
            let from_number: i32 = self.pick(&self.number);
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    assert!(
        checked.authored_declaration_selections().all_finalized(),
        "selections={:#?}",
        checked.authored_declaration_selections()
    );
}

#[test]
fn public_conformance_rejects_private_header_declarations() {
    let source = r#"
        trait Shape {}
        data Circle {}
        pub CircleShape: Circle satisfies Shape;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("private header must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("public conformance `CircleShape` exposes private data `Circle`"));
    assert!(rendered.contains("public conformance `CircleShape` exposes private trait `Shape`"));
}
