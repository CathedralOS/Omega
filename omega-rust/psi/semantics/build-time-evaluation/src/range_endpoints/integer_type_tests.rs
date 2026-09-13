use super::integer_type::IntegerPosition;
use super::*;
use numerics::bignum::BigInt;
use typed_trees::types::TypeReferenceNode;

fn typed(text: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn parameter_type(program: &TypedTrees, ordinal: usize) -> TypeReferenceHandle {
    let machine = &program.machines()[0];
    let entry = &program.machine_states(machine)[0];
    program.state_parameters(entry)[ordinal].type_reference
}

#[test]
fn closed_range_positions_keep_full_width_and_empty_boundaries() {
    for (parameter, accepted, rejected) in [
        (
            "u64[0..=18446744073709551615u64]",
            vec![BigInt::from_u64(u64::MAX)],
            vec![
                BigInt::from_i64(-1),
                BigInt::from_u64(u64::MAX).add(&BigInt::from_i64(1)),
            ],
        ),
        (
            "i64[-9223372036854775808i64..=-9223372036854775808i64]",
            vec![BigInt::from_i64(i64::MIN)],
            vec![BigInt::from_i64(i64::MIN + 1)],
        ),
        (
            "i64[-9223372036854775808i64..-9223372036854775808i64]",
            vec![],
            vec![BigInt::from_i64(i64::MIN), BigInt::from_i64(0)],
        ),
        (
            "u64[0..0]",
            vec![],
            vec![BigInt::from_i64(0), BigInt::from_u64(u64::MAX)],
        ),
    ] {
        let program = typed(&format!("machine endpoint(value: {parameter}) {{}}"));
        let position =
            IntegerPosition::prepare(&program, &program, parameter_type(&program, 0), None)
                .unwrap_or_else(|error| panic!("{parameter}: {error}"));
        for value in accepted {
            position
                .require_value(&value)
                .unwrap_or_else(|error| panic!("{parameter} <- {value}: {error}"));
        }
        for value in rejected {
            assert!(
                position.require_value(&value).is_err(),
                "{parameter} <- {value}"
            );
        }
    }
}

#[test]
fn every_range_shell_contributes_to_value_admission() {
    let mut program = typed("machine endpoint(first: u64[0..=5], second: u64[3..7]) {}");
    let first = parameter_type(&program, 0);
    let second = parameter_type(&program, 1);
    let TypeReferenceNode::Constrained { constraints, .. } =
        *program.type_reference_table.type_reference(second)
    else {
        panic!("second authored range");
    };
    let combined = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: first,
            constraints,
        });
    let position = IntegerPosition::prepare(&program, &program, combined, None).unwrap();
    for value in [3, 5] {
        position.require_value(&BigInt::from_i64(value)).unwrap();
    }
    for value in [0, 2, 6, 7, 10] {
        assert!(
            position.require_value(&BigInt::from_i64(value)).is_err(),
            "{value}"
        );
    }
}

#[test]
fn nominal_policy_and_invalid_type_handles_do_not_become_integer_positions() {
    for parameter in ["Token", "u64 in Wrapping"] {
        let program = typed(&format!(
            "data Token {{}} machine endpoint(value: {parameter}) {{}}"
        ));
        assert!(
            IntegerPosition::prepare(&program, &program, parameter_type(&program, 0), None,)
                .is_err(),
            "{parameter}"
        );
    }

    let mut program = typed("machine endpoint(value: u64[0..=8]) {}");
    let reference = parameter_type(&program, 0);
    let stale =
        TypeReferenceHandle::from_parts(reference.arena_index(), reference.generation() + 1);
    for invalid in [TypeReferenceHandle::invalid(), stale] {
        assert!(IntegerPosition::prepare(&program, &program, invalid, None).is_err());
    }
    let TypeReferenceNode::Constrained { constraints, .. } =
        *program.type_reference_table.type_reference(reference)
    else {
        panic!("authored range");
    };
    program.type_reference_table.substitute_node(
        reference,
        TypeReferenceNode::Constrained {
            base_type: reference,
            constraints,
        },
    );
    assert!(IntegerPosition::prepare(&program, &program, reference, None).is_err());
}

#[test]
fn retained_constant_bound_selection_is_admitted_before_the_body() {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    };
    use semantic_vocabulary::PackageKeyIdentity;
    use std::{path::PathBuf, sync::Arc};

    struct Selection(PackageKeyIdentity);
    impl crate::BuildTimeSelectionAuthority for Selection {
        fn allows_declaration_selection(
            &self,
            requester: PackageKeyIdentity,
            _: PackageKeyIdentity,
        ) -> bool {
            requester != self.0
        }

        fn package_label(&self, package: PackageKeyIdentity) -> String {
            if package == self.0 {
                "retained-bound-requester".to_owned()
            } else {
                "endpoint-owner".to_owned()
            }
        }
    }

    for signature in [
        "machine endpoint() -> u64[0..=BOUND] {1u64 / 0}",
        "machine endpoint(ignored: u64[0..=BOUND]) -> u64 {1u64 / 0}",
    ] {
        let argument = if signature.contains("ignored") {
            "8"
        } else {
            ""
        };
        let text = format!(
            "const BOUND: u64 = 8; {signature}
             machine bounded(value: u64[0..=endpoint({argument})]) {{}}"
        );
        let owner = PackageKeyIdentity::from_digest([0x73; 32]).unwrap();
        let retained_requester = PackageKeyIdentity::from_digest([0x74; 32]).unwrap();
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("owner/main.omg"),
                text.clone(),
                PathBuf::from("owner"),
                Some(owner),
                source::SourceOrigin::User,
            )
            .source_id;
        let retained_source = sources
            .add_with_metadata(
                PathBuf::from("requester/bound.omg"),
                "BOUND".to_owned(),
                PathBuf::from("requester"),
                Some(retained_requester),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(&text)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            Arc::new(sources),
        )
        .unwrap();
        let mut program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "endpoint")
            .unwrap();
        let entry = &program.machine_states(machine)[0];
        let reference = if argument.is_empty() {
            entry.return_type
        } else {
            program.state_parameters(entry)[0].type_reference
        };
        let TypeReferenceNode::Constrained { constraints, .. } =
            *program.type_reference_table.type_reference(reference)
        else {
            panic!("authored range position");
        };
        let TypeConstraintNode::Range { maximum, .. } =
            program.type_reference_table.constraints(constraints)[0]
        else {
            panic!("authored bound");
        };
        assert!(matches!(
            program.expression_table.expression(maximum),
            ExpressionNode::Integer(_)
        ));
        let bound_symbol = program.const_declarations()[0].symbol;
        let occurrence = program
            .record_resolved_authored_declaration_selection_once(
                source::SourceSpan::new(retained_source, source::Span::new(0, 5)),
                AuthoredDeclarationSelectionExposure::PrivateImplementation,
                AuthoredDeclarationSelectionKind::StaticPathSegment,
                bound_symbol,
            )
            .unwrap();
        program
            .expression_table
            .attach_authored_selection_occurrences(maximum, [occurrence]);
        let errors = evaluate_const_range_endpoints_with_authority(
            &mut program,
            Some(Arc::new(Selection(retained_requester))),
        )
        .expect_err("a folded bound retains the original requester's authority");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].message.contains("retained-bound-requester")
                && errors[0]
                    .message
                    .contains("without direct dependency authority"),
            "{signature}: {errors:?}"
        );
    }
}
