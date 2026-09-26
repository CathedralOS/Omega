//! Evaluated via-binding tests.

use super::{
    EvaluatedForeignImport, EvaluatedViaBinding, EvaluatedViaBindingRow, EvaluatedViaBindingTable,
    ExpressionNode, SymbolHandle, TargetProfile, TypedTrees,
};
use crate::evaluated_via_bindings::binding_values::{DecodedBindingValue, decode_binding_value};
use crate::provider_planning::{
    ProviderPlanDerivation, derive_satisfies_plans, select_derived_provider_plans,
    selected_provider_plan_facts, validate_derived_provider_plan_candidates,
};
use build_time_evaluation::BuildTimeValue;
use effects::provider_plan::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    ProviderBinding,
};
use target::{ForeignLocatorCandidate, normalize_foreign_locator};

struct ReplayFixture {
    typed: TypedTrees,
    table: EvaluatedViaBindingTable,
    producer: SymbolHandle,
}

const TRAIT_SOURCE: &str = r#"
    boundary trait Surface {
        machine invoke();
    }

    data Provider {}
    machine binding() -> u8 { 0 }
    machine Provider::leaf()
        satisfies Surface::invoke
        via binding();
"#;

const REQUIREMENT_SOURCE: &str = r#"
    data Subject {}
    pub boundary requirement Subject::invoke();

    data Provider {}
    machine binding() -> u8 { 0 }
    machine Provider::leaf()
        satisfies Subject::invoke
        via binding();
"#;

const OPERATOR_SOURCE: &str = r#"
    data Operand {}
    boundary operator Operand::invoke(value: u8) -> u8;

    data Provider {}
    machine binding() -> u8 { 0 }
    machine Provider::leaf(value: u8) -> u8
        satisfies Operand::invoke
        via binding();
"#;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize evaluated-via replay fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse evaluated-via replay fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve evaluated-via replay fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type evaluated-via replay fixture")
}

#[test]
fn product_binding_evaluation_excludes_build_checked_leaves() {
    for source in replay_sources() {
        for scope in [
            source::DependencyScope::Build,
            source::DependencyScope::Product,
        ] {
            let mut sources = source::SourceMap::default();
            let source_id = sources
                .add_checked_instance(
                    "helper/shared.omg".into(),
                    source.to_owned(),
                    "helper".into(),
                    Some(semantic_vocabulary::PackageKeyIdentity::from_digest([4; 32]).unwrap()),
                    source::SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            let tokens = source_files_to_tokens::Lexer::new(source)
                .tokenize()
                .unwrap();
            let syntax =
                tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
            let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
                syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
                    syntax: &syntax,
                    sources: Some(std::sync::Arc::new(sources)),
                    top_level_bindings: Vec::new(),
                },
            )
            .unwrap();
            let typed =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .unwrap();
            let result =
                super::evaluate_via_bindings(&typed, Some(TargetProfile::WindowsX64), None);
            if scope == source::DependencyScope::Build {
                let table = result.expect("host leaves must not run in product binding evaluation");
                assert!(table.rows().is_empty());
                table
                    .validate_against_typed(&typed)
                    .expect("replay uses the same product frontier");
            } else {
                // This fixture deliberately returns u8 rather than an admitted
                // Binding. Product leaves still evaluate and reject; scope
                // filtering must not silently erase the real product demand.
                assert!(result.is_err(), "invalid product binding cannot disappear");
            }
        }
    }
}

fn retained_import(
    typed: &TypedTrees,
    producer: &typed_trees::machine::Machine,
    export: &[u8],
    digest_seed: u8,
) -> EvaluatedForeignImport {
    let locator = normalize_foreign_locator(
        ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: export.to_vec(),
        },
        TargetProfile::WindowsX64,
    )
    .expect("valid test locator");
    let usage = EvaluatedBindingUsage::from_evaluator(1, 1, 10, 1_000, 0, 0, 4, 12, 3, 0)
        .expect("valid retained usage");
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        typed.symbols.symbol_package_identity(producer.symbol),
        typed
            .normalized_machine_overload_identity(producer)
            .expect("producer callable identity")
            .identity(),
        EvaluatedBindingProducerClosureDigest::from_bytes([digest_seed; 32])
            .expect("nonzero producer closure"),
        1,
        usage,
        EvaluatedBindingEvaluationDigest::from_bytes([digest_seed + 1; 32])
            .expect("nonzero evaluation digest"),
        1,
        EvaluatedBindingMaterializationDigest::from_bytes([digest_seed + 2; 32])
            .expect("nonzero materialization digest"),
        locator.identity_digest(),
    )
    .expect("valid retained receipt");
    EvaluatedForeignImport::from_retained_evidence(locator, receipt)
        .expect("receipt commits to locator")
}

fn replay_fixture(source: &str) -> ReplayFixture {
    let typed = typed(source);
    let realization = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::leaf")
        .expect("external realization");
    let [conformance] = typed.machine_trait_conformances(realization) else {
        panic!("one exact external conformance")
    };
    let ExpressionNode::Call(call) = typed
        .expression_table
        .expression(conformance.via_expression)
    else {
        panic!("one exact via call")
    };
    let producer = typed
        .machines()
        .iter()
        .find(|machine| {
            typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
        .expect("exact binding producer");
    let producer_entry = typed
        .machine_states(producer)
        .first()
        .expect("producer entry");
    let producer_symbol = producer.symbol;
    let table = EvaluatedViaBindingTable {
        target: Some(TargetProfile::WindowsX64),
        rows: vec![EvaluatedViaBindingRow {
            realization_machine: realization.symbol,
            satisfied_owner: conformance.symbol,
            requirement: conformance.requirement_symbol,
            via_expression: conformance.via_expression,
            producer_machine: producer.symbol,
            producer_entry_state: producer_entry.symbol,
            via_source_span: typed
                .expression_table
                .source_span(conformance.via_expression),
            evaluated: EvaluatedViaBinding::Import(retained_import(
                &typed,
                producer,
                b"ExitProcess",
                11,
            )),
        }],
    };
    table
        .validate_against_typed(&typed)
        .expect("fixture table rejoins exact typed custody");
    ReplayFixture {
        typed,
        table,
        producer: producer_symbol,
    }
}

fn replay_sources() -> [&'static str; 3] {
    [TRAIT_SOURCE, REQUIREMENT_SOURCE, OPERATOR_SOURCE]
}

fn bytes(value: &[u8]) -> BuildTimeValue {
    BuildTimeValue::Array(
        value
            .iter()
            .map(|byte| BuildTimeValue::Int(i64::from(*byte)))
            .collect(),
    )
}

fn binding(inner: BuildTimeValue) -> BuildTimeValue {
    BuildTimeValue::Case {
        variant: "DllImport".to_owned(),
        payload: vec![("import".to_owned(), inner)],
    }
}

#[test]
fn decodes_exact_atomic_pe_name() {
    let value = binding(BuildTimeValue::Case {
        variant: "PeByName".to_owned(),
        payload: vec![
            ("library".to_owned(), bytes(b"kernel32.dll")),
            ("export".to_owned(), bytes(b"ExitProcess")),
        ],
    });
    assert_eq!(
        decode_binding_value(&value, [12, 11, 0]).unwrap(),
        DecodedBindingValue::Import(ForeignLocatorCandidate::PeByName {
            library: b"kernel32.dll".to_vec(),
            export: b"ExitProcess".to_vec()
        })
    );
}

#[test]
fn decodes_exact_pe_ordinal_and_rejects_reserved_ordinal_or_used_widths() {
    let value = binding(BuildTimeValue::Case {
        variant: "PeByOrdinal".to_owned(),
        payload: vec![
            ("library".to_owned(), bytes(b"kernel32.dll")),
            ("ordinal".to_owned(), BuildTimeValue::Int(37)),
        ],
    });
    assert_eq!(
        decode_binding_value(&value, [12, 0, 0]).unwrap(),
        DecodedBindingValue::Import(ForeignLocatorCandidate::PeByOrdinal {
            library: b"kernel32.dll".to_vec(),
            ordinal: 37,
        })
    );
    let zero = binding(BuildTimeValue::Case {
        variant: "PeByOrdinal".to_owned(),
        payload: vec![
            ("library".to_owned(), bytes(b"kernel32.dll")),
            ("ordinal".to_owned(), BuildTimeValue::Int(0)),
        ],
    });
    assert!(decode_binding_value(&zero, [12, 0, 0]).is_err());
    assert!(decode_binding_value(&value, [12, 1, 0]).is_err());
    assert!(decode_binding_value(&value, [12, 0, 1]).is_err());
}

#[test]
fn decodes_exact_syscall_and_rejects_width_or_number_substitution() {
    let value = BuildTimeValue::Case {
        variant: "Syscall".to_owned(),
        payload: vec![("number".to_owned(), BuildTimeValue::Int(60))],
    };
    assert_eq!(
        decode_binding_value(&value, [0, 0, 0]).unwrap(),
        DecodedBindingValue::Syscall { number: 60 }
    );
    assert!(decode_binding_value(&value, [1, 0, 0]).is_err());
    let negative = BuildTimeValue::Case {
        variant: "Syscall".to_owned(),
        payload: vec![("number".to_owned(), BuildTimeValue::Int(-1))],
    };
    assert!(decode_binding_value(&negative, [0, 0, 0]).is_err());
    let substituted = BuildTimeValue::Case {
        variant: "Syscall".to_owned(),
        payload: vec![("ordinal".to_owned(), BuildTimeValue::Int(60))],
    };
    assert!(decode_binding_value(&substituted, [0, 0, 0]).is_err());
}

#[test]
fn rejects_width_and_field_drift() {
    let wrong_width = binding(BuildTimeValue::Case {
        variant: "PeByName".to_owned(),
        payload: vec![
            ("library".to_owned(), bytes(b"a")),
            ("export".to_owned(), bytes(b"b")),
        ],
    });
    assert!(decode_binding_value(&wrong_width, [2, 1, 0]).is_err());
    let wrong_field = binding(BuildTimeValue::Case {
        variant: "PeByOrdinal".to_owned(),
        payload: vec![
            ("ordinal".to_owned(), BuildTimeValue::Int(1)),
            ("library".to_owned(), bytes(b"a")),
        ],
    });
    assert!(decode_binding_value(&wrong_field, [1, 0, 0]).is_err());
}

#[test]
fn exact_candidate_and_selected_replay_cover_every_provider_schema_category() {
    for source in replay_sources() {
        let fixture = replay_fixture(source);
        let derived = ProviderPlanDerivation::evaluated(
            &fixture.typed,
            Some("windows_x86_64"),
            &fixture.table,
            &[],
        )
        .map(|derivation| derive_satisfies_plans(&fixture.typed, derivation))
        .expect("strict evaluated-via derivation");
        assert_eq!(derived.len(), 1);
        let diagnostics =
            validate_derived_provider_plan_candidates(&fixture.typed, &fixture.table, &derived);
        assert!(
            diagnostics.is_empty(),
            "exact candidate provenance must replay: {diagnostics:?}"
        );
        let selected =
            select_derived_provider_plans(&derived, target::NativeTarget::windows_x64(), &[], &[])
                .expect("unique exact provider candidate");
        selected_provider_plan_facts(&fixture.typed, &fixture.table, selected)
            .expect("selected provider provenance must replay");
    }
}

#[test]
fn provider_replay_rejects_every_fallback_and_provenance_substitution() {
    for source in replay_sources() {
        let fixture = replay_fixture(source);
        let derived = ProviderPlanDerivation::evaluated(
            &fixture.typed,
            Some("windows_x86_64"),
            &fixture.table,
            &[],
        )
        .map(|derivation| derive_satisfies_plans(&fixture.typed, derivation))
        .expect("strict evaluated-via derivation");
        let producer = fixture
            .typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == fixture.producer)
            .expect("producer machine");

        let mut slot_fallback = derived.clone();
        slot_fallback[0].plan.rows[0].binding = ProviderBinding::VtableSlot { index: 0 };
        assert!(
            !validate_derived_provider_plan_candidates(
                &fixture.typed,
                &fixture.table,
                &slot_fallback,
            )
            .is_empty()
        );

        let mut adapter_fallback = derived.clone();
        adapter_fallback[0].plan.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: fixture
                .typed
                .normalized_machine_overload_identity(producer)
                .expect("producer identity")
                .identity(),
            machine_package_identity: fixture
                .typed
                .symbols
                .symbol_package_identity(producer.symbol),
        };
        assert!(
            !validate_derived_provider_plan_candidates(
                &fixture.typed,
                &fixture.table,
                &adapter_fallback,
            )
            .is_empty()
        );

        let mut import_substitution = derived.clone();
        import_substitution[0].plan.rows[0].binding = ProviderBinding::Import {
            evaluated: retained_import(&fixture.typed, producer, b"TerminateProcess", 21),
        };
        assert!(
            !validate_derived_provider_plan_candidates(
                &fixture.typed,
                &fixture.table,
                &import_substitution,
            )
            .is_empty()
        );

        let selected =
            select_derived_provider_plans(&derived, target::NativeTarget::windows_x64(), &[], &[])
                .expect("unique exact provider candidate");

        let mut realization_substitution = selected.clone();
        realization_substitution[0]
            .derived
            .provenance
            .row_realizations[0] = fixture.producer;
        assert!(
            selected_provider_plan_facts(&fixture.typed, &fixture.table, realization_substitution)
                .is_err()
        );

        let mut requirement_substitution = selected.clone();
        requirement_substitution[0]
            .derived
            .provenance
            .row_requirements[0] = fixture.producer;
        assert!(
            selected_provider_plan_facts(&fixture.typed, &fixture.table, requirement_substitution)
                .is_err()
        );

        let mut selected_binding_substitution = selected;
        selected_binding_substitution[0].derived.plan.rows[0].binding = ProviderBinding::Import {
            evaluated: retained_import(&fixture.typed, producer, b"TerminateProcess", 31),
        };
        assert!(
            selected_provider_plan_facts(
                &fixture.typed,
                &fixture.table,
                selected_binding_substitution
            )
            .is_err()
        );
    }
}
