//! Applied source data must arrive through ordinary parsing and typing. A
//! manually supplied closed-row marker does not establish this producer path.

use super::*;
use std::sync::Arc;
use typed_trees::data::DataMember;

fn source_program(payload: &str) -> typed_trees::TypedTrees {
    source_program_in_scope(payload, true)
}

fn source_program_in_scope(payload: &str, bundled_optional: bool) -> typed_trees::TypedTrees {
    // Schema uses the actual bundled Optional, not a new local generic
    // declaration. Keep the full file, including its open generic proof.
    let optional = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/core/option.omg"
    ));
    let declaration = if bundled_optional {
        "use omega::language::core::Optional;"
    } else {
        optional
    };
    let text = format!(
        r#"
{declaration}
data Applied {{
    small: Optional<u32>;
    identity: Optional<u64>;
    nested: Optional<Optional<u64>>;
}}
machine entry() -> Applied {{
    Applied {{
        small: Optional::Some {{ value: 1u32 }},
        identity: Optional::Some {{ value: {payload} }},
        nested: Optional::Some {{ value: Optional::Some {{ value: 9u64 }} }}
    }}
}}
"#
    );
    let mut sources = source::SourceMap::default();
    let source = sources
        .add("applications/source.omg".into(), text.clone())
        .source_id;
    let core = bundled_optional.then(|| {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../source/library/core");
        sources
            .add_with_metadata(
                root.join("option.omg"),
                optional.into(),
                root,
                None,
                source::SourceOrigin::Toolchain,
            )
            .source_id
    });
    let tokens = source_files_to_tokens::Lexer::new(&text)
        .tokenize()
        .unwrap();
    let mut syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    if let Some(core) = core {
        let tokens = source_files_to_tokens::Lexer::new(optional)
            .tokenize()
            .unwrap();
        tokens_to_syntax_trees::parse(&mut syntax, core, &tokens).unwrap();
    }
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: core
                .map(|core| {
                    symbols::SourceScopedTopLevelBinding::logical_module_import(
                        source,
                        core,
                        "omega::language::core::Optional",
                        3,
                    )
                })
                .into_iter()
                .collect(),
            retained_base: None,
        },
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

#[test]
fn source_applications_prepare_substituted_cases_before_checked_construction() {
    let typed = source_program("7u64");
    let applied = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Applied")
        .unwrap();
    let optional = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Optional")
        .unwrap();
    assert_eq!(
        typed.symbols.symbol_source_origin(optional.symbol),
        Some(source::SourceOrigin::Toolchain)
    );
    assert_eq!(
        typed.symbols.symbol_source_origin(applied.symbol),
        Some(source::SourceOrigin::User)
    );
    let mut open_applications = 0;
    let mut open_reference = None;
    for (reference, node) in typed.type_reference_table.type_reference_rows_from(0) {
        let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } = node else {
            continue;
        };
        if *base_symbol != optional.symbol
            || typed_trees::domain::index_argument_is_closed(&typed, reference)
        {
            continue;
        }
        let view = typed
            .data_application_definition(reference)
            .expect("an open receiving application still retains its substituted field metadata");
        assert_eq!(view.generic_instance, Some(reference));
        assert!(typed.closed_data_definition(reference).is_none());
        open_applications += 1;
        open_reference = Some(reference);
    }
    assert!(
        open_applications > 0,
        "the actual core zero_is_none proof retains an open tuple"
    );
    let fields: Vec<_> = typed
        .data_members(applied)
        .iter()
        .map(|member| {
            let DataMember::Field(field) = member else {
                panic!("source record field")
            };
            field.type_reference
        })
        .collect();
    assert_eq!(fields.len(), 3);
    let mut prepared_again = typed.clone();
    let retained_members = prepared_again.data_members.len();
    let retained_payloads = prepared_again.data_payload_fields.len();
    for reference in &fields {
        assert!(
            prepared_again
                .prepare_data_application_view(*reference)
                .unwrap()
        );
    }
    assert!(
        !prepared_again
            .prepare_data_application_view(
                open_reference.expect("the parsed generic core application")
            )
            .unwrap()
    );
    assert_eq!(prepared_again.data_members.len(), retained_members);
    assert_eq!(prepared_again.data_payload_fields.len(), retained_payloads);
    for reference in &fields {
        let retained = typed
            .closed_data_definition(*reference)
            .expect("the source producer retains each actual application");
        assert_eq!(retained.symbol, optional.symbol);
        assert_eq!(retained.generic_instance, Some(*reference));
        assert!(retained.type_parameters.is_empty());
        assert_eq!(retained.properties, optional.properties);
        assert_eq!(retained.requires_facts, optional.requires_facts);
        let DataMember::Variant(some) = &typed.data_members(retained)[1] else {
            panic!("Some case")
        };
        assert_eq!(typed.data_payload_fields(some).len(), 1);
    }
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "entry")
        .unwrap()
        .symbol;
    let mut verdicts = crate::judge_compilation_for(
        &typed,
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            calling: None,
            providers: None,
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    assert_eq!(verdicts.len(), 1);
    let verdict = verdicts.remove(0).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    let records: Vec<_> = accepted
        .elaboration
        .source_records
        .iter()
        .filter(|record| record.declaration == optional.symbol)
        .collect();
    assert_eq!(
        records.len(),
        3,
        "u32, u64 and finite nested applications stay distinct"
    );
    assert!(records.iter().all(|record| {
        record
            .application
            .as_ref()
            .is_some_and(|application| application.substitutions.len() == 1)
    }));
    assert_ne!(records[0].record, records[1].record);
}

#[test]
fn source_application_cannot_change_its_payload_type() {
    let typed = source_program("true");
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "entry")
        .unwrap()
        .symbol;
    let verdicts = crate::judge_compilation_for(
        &typed,
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            calling: None,
            providers: None,
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refused))]
        if refused.detail.contains("record literal field type")),
        "{verdicts:?}"
    );
}

#[test]
fn a_local_generic_data_template_remains_unserved() {
    let typed = source_program_in_scope("7u64", false);
    let optional = typed
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Optional")
        .unwrap();
    assert_eq!(
        typed.symbols.symbol_source_origin(optional.symbol),
        Some(source::SourceOrigin::User)
    );
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "entry")
        .unwrap()
        .symbol;
    let verdicts = crate::judge_compilation_for(
        &typed,
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            calling: None,
            providers: None,
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    assert!(
        matches!(verdicts.as_slice(), [Ok(RouteVerdict::FallBack(refused))]
        if refused.detail == "type generic data"),
        "{verdicts:?}"
    );
}
