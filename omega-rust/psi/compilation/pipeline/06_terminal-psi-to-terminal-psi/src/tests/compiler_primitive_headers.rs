//! A catalog identity must arrive through real bundled source and retain its
//! complete generic header. These controls do not claim application checking.

use language_core::sealed_intrinsics::{CollectionPrimitive, CompilerPrimitive};
use std::sync::Arc;

const INDEXING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/indexing.omg"
));

fn typed(
    text: &str,
    origin: source::SourceOrigin,
) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    typed_at(text, "indexing.omg", origin)
}

fn typed_at(
    text: &str,
    path: &str,
    origin: source::SourceOrigin,
) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            format!("toolchain/{path}").into(),
            text.to_owned(),
            "toolchain".into(),
            None,
            origin,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text)
        .tokenize()
        .expect("catalog source tokenizes");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens)
        .expect("catalog source parses");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: vec![],
            retained_base: None,
        },
    )?;
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|error| vec![error])
}

#[test]
fn actual_generic_catalog_source_produces_complete_typed_schemas() {
    let program = typed(INDEXING, source::SourceOrigin::Toolchain).unwrap();
    assert_eq!(program.machines().len(), 3);
    assert!(
        program.operators().is_empty(),
        "no independent partial header"
    );
    for (name, operation, generics, parameters, contracts) in [
        ("read", CollectionPrimitive::Read, 1, 2, 1),
        ("range", CollectionPrimitive::Range, 1, 3, 2),
        ("view", CollectionPrimitive::ArrayView, 2, 1, 1),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        assert_eq!(program.machine_type_parameters(machine).len(), generics);
        assert_eq!(machine.lifetime_parameters.len(), 1);
        let [entry] = program.machine_states(machine) else {
            panic!("one real entry signature")
        };
        assert_eq!(program.state_parameters(entry).len(), parameters);
        assert_eq!(
            program.machine_contracts(machine).len() + program.state_contracts(entry).len(),
            contracts
        );
        let (actual, schema) = program
            .compiler_primitive_schema(machine.symbol)
            .unwrap_or_else(|error| {
                panic!(
                    "{name}: {error:?}; header={machine:#?}; inputs={:?}; output={:?}",
                    program
                        .state_parameters(entry)
                        .iter()
                        .map(|parameter| (
                            parameter.symbol,
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            program
                                .type_reference_table
                                .operand_lifetime_owner(parameter.type_reference),
                            program
                                .type_reference_table
                                .operand_lifetimes(parameter.type_reference)
                        ))
                        .collect::<Vec<_>>(),
                    program
                        .type_reference_table
                        .type_reference(entry.return_type)
                )
            })
            .expect("exact source custody");
        assert_eq!(actual, CompilerPrimitive::Collection(operation));
        assert!(program.type_identity_text(schema).is_some());
        if let Some(view) = program.machine_token_binding_by_symbol(machine.symbol) {
            assert_eq!(view.parameters, entry.parameters);
            assert_eq!(view.contracts, machine.contracts);
        }
    }
}

#[test]
fn real_byte_observation_headers_publish_complete_terminating_schemas() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/byte_observation.omg"
    ));
    use language_core::byte_predicates::ByteSequencePredicate;
    let program = typed_at(
        SOURCE,
        "byte_observation.omg",
        source::SourceOrigin::Toolchain,
    )
    .expect("the real observation source must resolve and type");
    assert_eq!(program.machines().len(), 4);
    for (name, predicate) in [
        ("valid_utf8", ByteSequencePredicate::ValidUtf8),
        ("no_nul", ByteSequencePredicate::NoNul),
        ("ascii_only", ByteSequencePredicate::AsciiOnly),
        ("non_empty", ByteSequencePredicate::NonEmpty),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let (operation, identity) = program
            .compiler_primitive_schema(machine.symbol)
            .expect("the complete promise must normalize")
            .expect("exact compiler custody");
        assert_eq!(operation, CompilerPrimitive::ByteObservation(predicate));
        assert_eq!(machine.lifetime_parameters.len(), 1);
        assert!(matches!(machine.termination_plan.interface,
            language_semantics::TerminationInterface::Published(
                language_semantics::TerminationGuarantee::Terminates { ref premises }
            ) if premises.is_empty()));
        assert!(program.type_identity_text(identity).is_some());
    }
}

#[test]
fn a_copied_or_locally_edited_generic_catalog_cannot_supply_primitives() {
    let edited = format!("{INDEXING}\n// local edit\n");
    for (text, origin) in [
        (INDEXING, source::SourceOrigin::User),
        (edited.as_str(), source::SourceOrigin::Toolchain),
    ] {
        let errors =
            typed(text, origin).expect_err("a copied signature supplies no catalog authority");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("has no body")
                    || error.message.contains("sealed toolchain")),
            "{errors:?}"
        );
    }
}

#[test]
fn returned_fields_and_collection_measures_keep_their_exact_roles() {
    use typed_trees::expression::{CollectionMeasure, ExpressionNode, MemberSelection};
    let program = typed(
        r#"
data Counter { len: u64 }
machine counter() -> Counter ensures result.len == 3 { Counter { len: 3 } }
machine same<'backing>(items: &'backing [u8]) -> &'backing [u8]
ensures result.len == items.len { items }
"#,
        source::SourceOrigin::User,
    )
    .unwrap();
    let field = program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_member_fields(program.data_members(definition)))
        .find(|field| field.name.as_str() == "len")
        .unwrap()
        .symbol;
    let result_members = program
        .expression_table
        .expression_entries()
        .filter_map(|(_, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            program
                .expression_table
                .names_reserved_result(member.receiver)
                .then_some(member.selection)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        result_members,
        [
            MemberSelection::Field(field),
            MemberSelection::CollectionMeasure(CollectionMeasure::Length)
        ]
    );
}

fn observed_program(text: &str) -> Vec<Result<crate::RouteVerdict, crate::CompilationRefusal>> {
    let typed = super::build_purpose::frontier_with_core(
        text,
        "machine build_entry() {}",
        &[(
            "byte_observation.omg",
            "byte_observation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../source/library/core/byte_observation.omg"
            )),
        )],
    );
    let entry = super::build_purpose::symbol(&typed, "main");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    )
}

#[test]
fn selected_byte_observation_executes_the_existing_operation_and_proves_equivalence() {
    let verdicts = observed_program(
        r#"
use omega::language::core::byte_observation;
public machine checked<'input>(bytes: &'input [u8]) -> bool
reaches none crashes none terminates
ensures result == byte_observation::ascii_only(bytes)
{ byte_observation::ascii_only(bytes) }
machine main() crashes Trap {
    let valid: bool = checked("Room A1");
    transition valid { true -> {} false -> bad() }
    machine bad() crashes Trap { crash Trap; }
}
"#,
    );
    for verdict in verdicts {
        let Ok(crate::RouteVerdict::Accepted(accepted)) = verdict else {
            panic!(
                "an authenticated observation and its ordinary equivalence must be checked: {verdict:?}"
            );
        };
        let module = &accepted.elaboration.module;
        assert!(module.operations().any(|operation| matches!(
            operation.kind,
            terminal_psi::OperationKind::ByteSequencePredicate { .. }
        )));
        accepted
            .publish_invocation()
            .expect("ordinary receiving validation");
    }
}

#[test]
fn a_constant_byte_wrapper_cannot_claim_the_observers_equivalence() {
    for result in ["true", "false"] {
        let source = format!(
            r#"
use omega::language::core::byte_observation;
public machine checked<'input>(bytes: &'input [u8]) -> bool
reaches none crashes none terminates
ensures result == byte_observation::ascii_only(bytes)
{{ {result} }}
machine main() {{ let valid: bool = checked("Room A1"); }}
"#
        );
        let verdicts = observed_program(&source);
        assert!(
            verdicts
                .iter()
                .any(|verdict| matches!(verdict, Ok(crate::RouteVerdict::Rejected { .. }))),
            "neither constant result proves exact membership: {verdicts:?}"
        );
    }
}
