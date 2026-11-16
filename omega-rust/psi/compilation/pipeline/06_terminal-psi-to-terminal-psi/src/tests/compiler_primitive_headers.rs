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
        let Some([entry]) = program.machine_states(machine).as_array() else {
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
        assert!(
            machine
                .termination_plan
                .interface
                .published()
                .is_some_and(|guarantee| guarantee.promises_termination())
        );
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

#[test]
fn copied_or_edited_byte_observation_headers_cannot_acquire_catalog_supply() {
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/byte_observation.omg"
    ));
    let edited = SOURCE.replace("reaches none", "reaches none\nsuspends true");
    for (source, origin) in [
        (SOURCE, source::SourceOrigin::User),
        (edited.as_str(), source::SourceOrigin::Toolchain),
    ] {
        assert!(
            typed_at(source, "byte_observation.omg", origin).is_err(),
            "a copied or changed header cannot nominate compiler supply"
        );
    }
}

fn observed_typed(text: &str) -> typed_trees::TypedTrees {
    super::build_purpose::frontier_with_core(
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
    )
}

fn judge_observed(
    typed: &typed_trees::TypedTrees,
) -> Vec<Result<crate::RouteVerdict, crate::CompilationRefusal>> {
    let entry = super::build_purpose::symbol(typed, "main");
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            typed,
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

fn observed_program(text: &str) -> Vec<Result<crate::RouteVerdict, crate::CompilationRefusal>> {
    judge_observed(&observed_typed(text))
}

fn execute_observed(accepted: Box<crate::AcceptedClosure>) {
    let publication = accepted
        .publish_invocation()
        .expect("ordinary receiving validation");
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert_eq!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(100_000))
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
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
    assert!(!verdicts.is_empty(), "the entry needs a checker verdict");
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
        execute_observed(accepted);
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
        assert!(!verdicts.is_empty(), "the entry needs a checker verdict");
        for verdict in verdicts {
            // A symbolic byte predicate can leave the guarantee unproved.
            // Pin that authored guarantee, including an incomplete derivation.
            let unproved = match verdict {
                Ok(crate::RouteVerdict::Incomplete(unproved)) => unproved,
                Ok(crate::RouteVerdict::Rejected {
                    formations,
                    violations,
                    unproved,
                    unestablished,
                    conflicts,
                    exceeded,
                }) => {
                    assert!(formations.is_empty() && violations.is_empty());
                    assert!(
                        unestablished.is_empty() && conflicts.is_empty() && exceeded.is_empty()
                    );
                    unproved
                }
                other => panic!("a constant must not publish the equivalence: {other:?}"),
            };
            assert!(!unproved.is_empty());
            assert!(
                unproved.iter().all(|failure| {
                    matches!(
                        failure.kind,
                        terminal_check_tables::obligations::ObligationKind::NormalGuarantee { .. }
                    ) && failure
                        .statement
                        .contains("result == byte_observation::ascii_only(bytes)")
                }),
                "{unproved:?}"
            );
        }
    }
}

#[test]
fn authenticated_byte_calls_still_require_their_complete_actual_header() {
    for mutation in 0..5 {
        let mut typed = observed_typed(
            r#"
use omega::language::core::byte_observation;
machine main() { let valid: bool = byte_observation::ascii_only("Room A1"); }
"#,
        );
        let index = typed
            .machines()
            .iter()
            .position(|machine| machine.name.as_str() == "ascii_only")
            .unwrap();
        let symbol = typed.machines()[index].symbol;
        // Keep the exact authenticated source, symbol and signature types.
        // These are different actual headers, not copied catalog spellings.
        match mutation {
            0 => typed.machine_mut(index).is_public = false,
            1 => typed.machine_mut(index).admitted_same_stack_bytes = 1,
            2 => typed
                .machine_mut(index)
                .lifetime_parameters
                .push(typed_trees::name::Identifier::generated("extra")),
            3 => {
                let foreign = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == "valid_utf8")
                    .unwrap()
                    .symbol;
                let machine = typed.machine_by_symbol(symbol).unwrap();
                let Some([entry]) = typed.machine_states(machine).as_array() else {
                    panic!("one actual entry");
                };
                let reference = typed.state_parameters(entry)[0].type_reference;
                typed
                    .type_reference_table
                    .retain_operand_lifetimes(
                        reference,
                        [typed_trees::type_identity::LifetimeParameterIdentity {
                            declaration: foreign,
                            ordinal: 0,
                        }],
                    )
                    .unwrap();
            }
            4 => {
                typed.machine_mut(index).name =
                    typed_trees::name::Identifier::generated("valid_utf8")
            }
            _ => unreachable!(),
        }
        assert!(typed.compiler_primitive_schema(symbol).is_err());
        let verdicts = judge_observed(&typed);
        assert!(
            verdicts
                .iter()
                .any(|verdict| !matches!(verdict, Ok(crate::RouteVerdict::Accepted(_)))),
            "catalog custody must not erase actual header differences: {verdicts:?}"
        );
    }
}

#[test]
fn byte_domains_form_an_explicit_shared_whole_observation_of_self() {
    for (argument, accepted) in [("&self[..]", true), ("self", false)] {
        let text = format!(
            r#"
use omega::language::core::byte_observation;
domain [u8; 2]::Ascii requires byte_observation::ascii_only({argument});
data Holder {{ bytes: [u8; 2] in Ascii; }}
machine main() {{ let plain: [u8; 2] = [65u8, 66u8]; let holder: Holder = Holder {{ bytes: plain as +Ascii }}; }}
"#
        );
        let verdicts = observed_program(&text);
        assert_eq!(
            verdicts
                .iter()
                .all(|verdict| matches!(verdict, Ok(crate::RouteVerdict::Accepted(_)))),
            accepted,
            "a logical subject grants no implicit loan: {argument}: {verdicts:?}"
        );
    }
}

#[test]
fn a_utf8_array_element_write_must_preserve_the_whole_sequence() {
    // Both starting arrays are UTF-8. The identical write keeps AB valid,
    // but turns C3 A9 into 41 A9, whose continuation byte has no lead.
    let positive_source = r#"
use omega::language::core::byte_observation;
domain [u8; 2]::Utf8 requires byte_observation::valid_utf8(&self[..]);
machine main() crashes Trap {
    let plain: [u8; 2] = [65u8, 66u8];
    let mut bytes: [u8; 2] in Utf8 = plain as +Utf8;
    bytes[0] = 65u8;
    let valid: bool = byte_observation::valid_utf8(&bytes[..]);
    transition valid { true -> {} false -> bad() }
    machine bad() crashes Trap { crash Trap; }
}
"#;
    let negative_source = positive_source.replace("[65u8, 66u8]", "[195u8, 169u8]");
    assert_ne!(negative_source, positive_source);
    let mut positive = judge_observed(&observed_typed(positive_source));
    let mut negative = judge_observed(&observed_typed(&negative_source));
    // Record both actual verdicts before either assertion, so an unserved
    // cast or observation is visible rather than counted as a store refusal.
    for (label, verdicts) in [("positive", &positive), ("negative", &negative)] {
        for verdict in verdicts {
            if matches!(verdict, Ok(crate::RouteVerdict::Accepted(_))) {
                eprintln!("UTF-8 store {label}: accepted");
            } else {
                eprintln!("UTF-8 store {label}: {verdict:?}");
            }
        }
    }
    assert_eq!(positive.len(), 1, "one ordinary positive entry verdict");
    assert_eq!(negative.len(), 1, "one ordinary negative entry verdict");
    let accepted = match positive.remove(0) {
        Ok(crate::RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the same write over an ASCII UTF-8 array must check: {other:?}"),
    };
    execute_observed(accepted);
    match negative.remove(0) {
        Ok(crate::RouteVerdict::Rejected {
            formations,
            violations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
        }) => {
            assert!(formations.is_empty() && violations.is_empty());
            assert!(unestablished.is_empty() && conflicts.is_empty() && exceeded.is_empty());
            assert_eq!(unproved.len(), 1, "only the whole-array invariant is owed");
            let failure = &unproved[0];
            assert!(matches!(
                failure.kind,
                terminal_check_tables::obligations::ObligationKind::DataDomain { clause: 0, .. }
            ));
            assert!(
                failure.consumed.is_some(),
                "the write opens an actual invariant window"
            );
            let write_start = negative_source.find("bytes[0] = 65u8;").unwrap();
            let write_end = write_start + "bytes[0] = 65u8;".len();
            assert!(
                matches!(
                    failure.site,
                    terminal_check_tables::program::SourceOrigin::Authored(site)
                        if site.span.start >= write_start && site.span.end <= write_end
                ),
                "only the authored byte write may owe this restoration: {failure:?}"
            );
        }
        other => panic!(
            "a UTF-8-destroying write must fail its whole-array invariant, \
             not parsing, type selection, cast formation, or an unserved observer: {other:?}"
        ),
    }
}

#[test]
fn bounded_concat_keeps_its_original_room_a1_execution_oracle() {
    // Use the actual customer body. Only the harness entry spelling changes;
    // both copies, every bound, and the Room A1/length-seven check stay intact.
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../samples/cli/probes/bounded_concat/main.omg"
    ))
    .replace("machine entry_start()", "machine main()");
    let typed = observed_typed(&source);
    let mut verdicts = judge_observed(&typed);
    let accepted = match verdicts.remove(0) {
        Ok(crate::RouteVerdict::Accepted(accepted)) => accepted,
        other => {
            // Keep the failed original program diagnosable without reducing
            // it to a different buffer example or erasing its requirements.
            let owners = typed_trees::references::collect_program_operand_owners(&typed);
            let unselected: Vec<_> = typed
                .expression_table
                .expression_entries()
                .filter_map(|(handle, node)| {
                    let typed_trees::expression::ExpressionNode::Call(call) = node else {
                        return None;
                    };
                    if call.target_symbol.is_valid() {
                        return None;
                    }
                    Some((
                        call.target.as_str(),
                        typed.expression_table.source_span(handle),
                        typed.symbols.top_level_candidates_from_source(
                            call.target.as_str(),
                            &[symbols::SymbolKind::Machine],
                            typed.expression_table.source_span(handle),
                        ),
                        owners
                            .expression_owners(handle)
                            .map(|owner| {
                                if typed.symbols.symbols().nodes().is_valid(owner) {
                                    typed.symbols.display_path(owner, "::")
                                } else {
                                    format!("invalid owner {owner:?}")
                                }
                            })
                            .collect::<Vec<_>>(),
                        typed
                            .expression_table
                            .expression_handles(call.arguments)
                            .iter()
                            .map(|argument| {
                                (
                                    typed.expression_table.expression(*argument),
                                    typed
                                        .expression_table
                                        .constructed_value_type_reference(*argument),
                                )
                            })
                            .collect::<Vec<_>>(),
                    ))
                })
                .collect();
            panic!(
                "the kept buffer program must be checked: {other:?}; unselected calls: {unselected:#?}"
            );
        }
    };
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(crate::RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
    execute_observed(accepted);
}
