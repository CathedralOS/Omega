//! A catalog identity must arrive through real bundled source and retain its
//! complete generic header. These controls do not claim application checking.

use language_core::sealed_intrinsics::{CollectionPrimitive, CompilerPrimitive};
use std::sync::Arc;

const INDEXING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/indexing/indexing.omg"
));

fn typed(
    text: &str,
    origin: source_admission::SourceOrigin,
) -> Result<crate::source_state::TypedTrees, Vec<diagnostics::Diagnostic>> {
    typed_at(text, "indexing/indexing.omg", origin)
}

fn typed_at(
    text: &str,
    path: &str,
    origin: source_admission::SourceOrigin,
) -> Result<crate::source_state::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = crate::source_inputs::SourceMap::default();
    let source = sources
        .add_checked_instance(
            format!("toolchain/{path}").into(),
            text.to_owned(),
            "toolchain".into(),
            None,
            origin,
            source_admission::SourceResolutionStratum::Base,
            source_provenance::DependencyScope::Product,
        )
        .source_id;
    let tokens = crate::frontend_fixture::Lexer::new(text)
        .tokenize()
        .expect("catalog source tokenizes");
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
        .expect("catalog source parses");
    let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: vec![],
        retained_base: None,
    })?;
    crate::typing::lower_symbol_resolved_trees(&resolved).map_err(|error| vec![error])
}

#[test]
fn actual_generic_catalog_source_produces_complete_typed_schemas() {
    let program = typed(INDEXING, source_admission::SourceOrigin::Toolchain).unwrap();
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
        "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
    ));
    use language_core::byte_predicates::ByteSequencePredicate;
    let program = typed_at(
        SOURCE,
        "byte_observation/byte_observation.omg",
        source_admission::SourceOrigin::Toolchain,
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
        (INDEXING, source_admission::SourceOrigin::User),
        (edited.as_str(), source_admission::SourceOrigin::Toolchain),
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
    use crate::source_state::expression::{CollectionMeasure, ExpressionNode, MemberSelection};
    let program = typed(
        r#"
data Counter { length: u64 }
machine counter() -> Counter ensures result.length == 3 { Counter { length: 3 } }
machine same<'backing>(items: @'backing [u8]) -> @'backing [u8]
ensures result.length == items.length { items }
"#,
        source_admission::SourceOrigin::User,
    )
    .unwrap();
    let field = program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_member_fields(program.data_members(definition)))
        .find(|field| field.name.as_str() == "length")
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
        "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
    ));
    let edited = SOURCE.replace("reaches none", "reaches none\nsuspends true");
    for (source, origin) in [
        (SOURCE, source_admission::SourceOrigin::User),
        (edited.as_str(), source_admission::SourceOrigin::Toolchain),
    ] {
        assert!(
            typed_at(source, "byte_observation/byte_observation.omg", origin).is_err(),
            "a copied or changed header cannot nominate compiler supply"
        );
    }
}

fn observed_typed(text: &str) -> crate::source_state::TypedTrees {
    super::build_purpose::frontier_with_core(
        text,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[(
            "byte_observation/byte_observation.omg",
            "byte_observation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
            )),
        )],
    )
}

fn judge_observed(
    typed: &crate::source_state::TypedTrees,
) -> Vec<Result<crate::RouteVerdict, crate::CompilationRefusal>> {
    let typed = &crate::prepare_fixture(typed.clone());
    let entry = super::build_purpose::symbol(typed.trees(), "main");
    crate::judge_compilation(
        crate::lowering::prepare_compilation(
            typed,
            super::build_purpose::roster(
                &[entry],
                &[Some(super::build_purpose::package())],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
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

const PACKED_INTEGER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/packed_integer/packed_integer.omg"
));

#[test]
fn packed_add_requires_the_exact_core_declaration() {
    let path = "packed_integer/packed_integer.omg";
    let program = typed_at(
        PACKED_INTEGER,
        path,
        source_admission::SourceOrigin::Toolchain,
    )
    .unwrap();
    let declaration = &program.machines()[0];
    let (primitive, _) = program
        .compiler_primitive_schema(declaration.symbol)
        .unwrap()
        .unwrap();
    assert_eq!(
        primitive,
        CompilerPrimitive::PackedInteger(
            language_core::sealed_intrinsics::PackedIntegerPrimitive::WrappingAddI16x8
        )
    );
    assert!(typed_at(PACKED_INTEGER, path, source_admission::SourceOrigin::User).is_err());
    let short = PACKED_INTEGER.replace("[i16; 8]", "[i16; 7]");
    assert!(typed_at(&short, path, source_admission::SourceOrigin::Toolchain).is_err());
}

#[test]
fn packed_add_reads_both_inputs_and_establishes_an_owned_array() {
    let typed = super::build_purpose::frontier_with_core(
        r#"
use omega\language\core\packed_integer;
machine main() crashes Trap {
    let a: [i16; 8] = [32767i16, -32768i16, 1i16, 2i16, 3i16, 4i16, 5i16, 6i16];
    let b: [i16; 8] = [1i16, -1i16, 0i16, 0i16, 0i16, 0i16, 0i16, 2i16];
    let c: [i16; 8] = packed_integer\wrapping_add_i16x8(@a, @b);
    transition c[0] == -32768i16 && c[1] == 32767i16 && c[7] == 8i16 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}"#,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[(
            "packed_integer/packed_integer.omg",
            "packed_integer",
            PACKED_INTEGER,
        )],
    );
    let verdicts = judge_observed(&typed);
    assert!(!verdicts.is_empty());
    for verdict in verdicts {
        let Ok(crate::RouteVerdict::Accepted(accepted)) = verdict else {
            panic!("packed arithmetic must pass the checker: {verdict:?}")
        };
        assert!(
            accepted
                .elaboration
                .module
                .operations()
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::PackedIntegerBinary { .. }
                ))
        );
        execute_observed(accepted);
    }
}

#[test]
fn all_six_packed_integer_operations_lower_check_and_execute_on_psi() {
    let typed = super::build_purpose::frontier_with_core(
        r#"
use omega\language\core\packed_integer;

machine shift_wrapper<const Count: u64>(x: @[i16; 8]) -> [i16; 8]
requires Count < 16u64 {
    packed_integer\shift_right_i16x8::<Count>(x)
}

machine main() crashes Trap {
    let a: [i16; 8] = [32767i16, -32768i16, -65i16, 128i16, 10i16, 20i16, 30i16, -40i16];
    let b: [i16; 8] = [1i16, 1i16, 0i16, 0i16, 3i16, 4i16, 5i16, 6i16];
    let added: [i16; 8] = packed_integer\wrapping_add_i16x8(@a, @b);
    let subbed: [i16; 8] = packed_integer\wrapping_sub_i16x8(@added, @b);
    let shifted: [i16; 8] = shift_wrapper::<1>(@subbed);
    let shuffled: [i16; 8] = packed_integer\shuffle_i16x8::<0, 8, 1, 9, 2, 10, 3, 11>(@shifted, @b);
    let bytes_in: [u8; 8] = [0u8, 128u8, 255u8, 1u8, 2u8, 3u8, 4u8, 5u8];
    let widened: [i16; 8] = packed_integer\widen_u8x8_to_i16x8(@bytes_in);
    let to_clip: [i16; 8] = [-1i16, 0i16, 127i16, 255i16, 256i16, 32767i16, -32768i16, 42i16];
    let narrowed: [u8; 8] = packed_integer\narrow_i16x8_to_u8x8_saturating(@to_clip);
    let quad_in: [i16; 4] = [-10i16, 20i16, 300i16, -400i16];
    let quad_loaded: [i16; 8] = packed_integer\load_i16x4_zero_extend(@quad_in);
    let bytes4_in: [u8; 4] = [10u8, 200u8, 255u8, 0u8];
    let widened4: [i16; 8] = packed_integer\widen_u8x4_to_i16x8(@bytes4_in);
    let narrowed4: [u8; 4] = packed_integer\narrow_i16x8_to_u8x4_saturating(@quad_loaded);
    transition subbed[0] == 32767i16 && subbed[1] == -32768i16
        && shifted[2] == -33i16 && shifted[3] == 64i16
        && shuffled[0] == 16383i16 && shuffled[1] == 1i16 && shuffled[2] == -16384i16 && shuffled[3] == 1i16
        && widened[1] == 128i16 && widened[2] == 255i16
        && narrowed[0] == 0u8 && narrowed[2] == 127u8 && narrowed[4] == 255u8 && narrowed[6] == 0u8 && narrowed[7] == 42u8
        && quad_loaded[0] == -10i16 && quad_loaded[3] == -400i16 && quad_loaded[4] == 0i16 && quad_loaded[7] == 0i16
        && widened4[0] == 10i16 && widened4[2] == 255i16 && widened4[4] == 0i16 && widened4[7] == 0i16
        && narrowed4[0] == 0u8 && narrowed4[1] == 20u8 && narrowed4[2] == 255u8 && narrowed4[3] == 0u8
    {
        true -> done(),
        false -> wrong()
    }
    machine done() {}
    machine wrong() { crash Trap; }
}"#,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[(
            "packed_integer/packed_integer.omg",
            "packed_integer",
            PACKED_INTEGER,
        )],
    );
    let verdicts = judge_observed(&typed);
    assert!(!verdicts.is_empty());
    let mut executed = 0usize;
    for verdict in verdicts {
        let Ok(crate::RouteVerdict::Accepted(accepted)) = verdict else {
            panic!("all six packed operations must pass the checker: {verdict:?}")
        };
        if accepted.elaboration.generic_binders.is_empty()
            && accepted.elaboration.requirements.is_empty()
            && accepted
                .elaboration
                .value_parameters
                .iter()
                .all(|(_, count)| *count == 0)
        {
            execute_observed(accepted);
            executed += 1;
        } else {
            assert!(matches!(
                accepted.publish_invocation(),
                Err(crate::PublicationError::GenericCheckForm(_))
            ));
        }
    }
    assert!(executed > 0, "main executable route must publish and run");
}

#[test]
fn selected_byte_observation_executes_the_existing_operation_and_proves_equivalence() {
    let verdicts = observed_program(
        r#"
use omega\language\core\byte_observation;
public machine checked<'input>(bytes: @'input [u8]) -> bool
reaches none crashes none terminates
ensures result == byte_observation\ascii_only(bytes)
{ byte_observation\ascii_only(bytes) }
machine main() crashes Trap {
    let valid: bool = checked("Room A1");
    let invalid: bool = checked("é");
    transition valid && !invalid { true -> {}, false -> bad() }
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
public machine checked<'input>(bytes: @'input [u8]) -> bool
reaches none crashes none terminates
ensures result == byte_observation\ascii_only(bytes)
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
machine main() { let valid: bool = byte_observation\ascii_only("Room A1"); }
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
                .push(crate::source_state::name::Identifier::generated("extra")),
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
                        [
                            crate::source_state::type_identity::LifetimeParameterIdentity {
                                declaration: foreign,
                                ordinal: 0,
                            },
                        ],
                    )
                    .unwrap();
            }
            4 => {
                typed.machine_mut(index).name =
                    crate::source_state::name::Identifier::generated("valid_utf8")
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
    for (argument, accepted) in [("@self[..]", true), ("self", false)] {
        let text = format!(
            r#"
domain [u8; 2]::Ascii requires byte_observation\ascii_only({argument});
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
domain [u8; 2]::Utf8 requires byte_observation\valid_utf8(@self[..]);
machine main() crashes Trap {
    let plain: [u8; 2] = [65u8, 66u8];
    let mutable bytes: [u8; 2] in Utf8 = plain as +Utf8;
    (@mutable bytes as -Utf8)[0] = 65u8;
    let valid: bool = byte_observation\valid_utf8((@bytes as -Utf8)[..]);
    transition valid { true -> {}, false -> bad() }
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
            let write_start = negative_source
                .find("(@mutable bytes as -Utf8)[0] = 65u8;")
                .unwrap();
            let write_end = write_start + "(@mutable bytes as -Utf8)[0] = 65u8;".len();
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
fn a_qualified_window_proves_its_domain_without_qualifying_the_raw_subslice() {
    let program = r#"
domain [u8]::Utf8 requires byte_observation\valid_utf8(@self[..]);
machine whole<'bytes>(bytes: @'bytes [u8] in Utf8) -> @'bytes [u8] in Utf8
requires bytes.length >= 1
{
    (bytes as -Utf8)[..] as +Utf8
}
machine main() crashes Trap {
    let bytes: @[u8] in Utf8 = whole("é" as +Utf8);
    transition bytes.length == 2 { true -> {}, false -> bad() }
    machine bad() crashes Trap { crash Trap; }
}
"#;
    let verdicts = observed_program(program);
    assert!(!verdicts.is_empty());
    for verdict in verdicts {
        let Ok(crate::RouteVerdict::Accepted(accepted)) = verdict else {
            panic!("the complete qualified window must be checked: {verdict:?}");
        };
        for operation in accepted.elaboration.module.operations() {
            if matches!(
                operation.kind,
                terminal_psi::OperationKind::ByteSequenceSubslice { .. }
            ) {
                let result = operation
                    .result
                    .as_structural()
                    .expect("a descriptor result");
                assert!(result.qualifications.is_empty());
                assert!(result.projected_qualifications.is_empty());
                assert!(result.qualification_establishments.is_empty());
            }
        }
        execute_observed(accepted);
    }
    let partial = program.replace(
        "(bytes as -Utf8)[..] as +Utf8",
        "(bytes as -Utf8)[1..] as +Utf8",
    );
    let refused = observed_program(&partial);
    assert!(!refused.is_empty());
    for verdict in refused {
        let Ok(crate::RouteVerdict::Rejected { unproved, .. }) = verdict else {
            panic!("a cut inside the UTF-8 encoding must fail its cast: {verdict:?}");
        };
        assert!(
            unproved
                .iter()
                .any(|failure| failure.statement.contains("domain `[u8]::Utf8`")
                    && failure.statement.contains("valid_utf8")
                    && matches!(
                        failure.site,
                        terminal_check_tables::program::SourceOrigin::Authored(_)
                    )),
            "{unproved:?}"
        );
    }
}

#[test]
fn bounded_concat_keeps_its_original_room_a1_execution_oracle() {
    // Use the actual customer body. Only the harness entry spelling changes;
    // both copies, every bound, and the Room A1/length-seven check stay intact.
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../samples/cli/probes/bounded_concat/main.omg"
    ))
    .replace("machine entry_start()", "machine main()");
    let typed = observed_typed(&source);
    let mut verdicts = judge_observed(&typed);
    let accepted = match verdicts.remove(0) {
        Ok(crate::RouteVerdict::Accepted(accepted)) => accepted,
        other => {
            // Keep the failed original program diagnosable without reducing
            // it to a different buffer example or erasing its requirements.
            let owners = crate::source_state::references::collect_program_operand_owners(&typed);
            let unselected: Vec<_> = typed
                .expression_table
                .expression_entries()
                .filter_map(|(handle, node)| {
                    let crate::source_state::expression::ExpressionNode::Call(call) = node else {
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

#[test]
fn an_indexed_store_proves_both_prior_and_replacement_byte_contents() {
    let positive = r#"
domain [u8; 2]::Ascii requires byte_observation\ascii_only(@self[..]);
machine put(bytes: @mutable [u8; 2] in Ascii, index: u64, value: u8)
requires index < 2, value <= 127u8
{ (bytes as -Ascii)[index] = value; }
machine main() crashes Trap {
    let mutable bytes: [u8; 2] in Ascii = [65u8, 90u8] as +Ascii;
    put(@mutable bytes, 1, 66u8);
    transition (@bytes as -Ascii)[0] == 65u8 && (@bytes as -Ascii)[1] == 66u8 {
        true -> {}, false -> bad()
    }
    machine bad() crashes Trap { crash Trap; }
}
"#;
    let mut checked = observed_program(positive);
    let accepted = match checked.remove(0) {
        Ok(crate::RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("an indexed store must preserve the proved byte bound: {other:?}"),
    };
    execute_observed(accepted);
    for negative in [
        positive.replace("index < 2, value <= 127u8", "index < 2"),
        positive.replace(
            "{ (bytes as -Ascii)[index] = value; }",
            "{ (bytes as -Ascii)[0] = 255u8; (bytes as -Ascii)[index] = value; }",
        ),
    ] {
        let verdicts = observed_program(&negative);
        assert!(verdicts.iter().any(|verdict| matches!(verdict,
            Ok(crate::RouteVerdict::Rejected { formations, violations, unproved, unestablished, conflicts, exceeded })
                if formations.is_empty() && violations.is_empty() && unestablished.is_empty()
                    && conflicts.is_empty() && exceeded.is_empty()
                    && unproved.iter().any(|failure| matches!(failure.kind,
                        terminal_check_tables::obligations::ObligationKind::DataDomain { .. }))
        )), "a missing replacement bound or invalid prior cell must not restore the array: {verdicts:?}");
    }
}
