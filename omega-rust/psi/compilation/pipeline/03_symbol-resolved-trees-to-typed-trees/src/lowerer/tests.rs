//! Fixtures shared by the lowerer tests: the seeded base and extension inputs
//! the continuation tests type. The front-end pipelines they run are named in
//! `crate::front_end`.

mod generated_invocations;
mod machine_contracts;
mod mathematical_declarations;
mod quotients_and_domains;
mod retained_constants;
mod seeded_continuations;
mod seeded_instances;
mod token_bindings;
mod typed_retention;

use super::seeded_continuation::{SeededTypingBase, lower_symbol_resolved_trees_to_seeded_base};
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees_to_symbol_resolved_trees::{
    ExtensionRequest, RebasedSeededSymbolResolvedTrees, resolve_extension,
};

#[test]
fn typed_stage_binds_dynamic_calls_without_compiler_intervention() {
    let typed = crate::front_end::typed_program(
        "trait Shape { machine code(&self) -> i32; }
         trait Decoy { machine code(&self) -> i32; }
         data Holder { receiver: &dyn Shape; }
         machine dispatch(holder: &Holder) -> i32 { holder.receiver.code() }",
    );
    let requirement = typed.trait_machine_signatures(&typed.traits()[0])[0].symbol;
    let calls = typed
        .expression_table
        .expression_nodes()
        .filter_map(|expression| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].target_symbol, requirement);
}

#[test]
fn seeded_typing_binds_new_dynamic_calls_without_changing_the_base() {
    let (base, extension) = seeded_plain_data_inputs(
        "trait Shape { machine code(&self) -> i32; }
         trait Decoy { machine code(&self) -> i32; }
         data Holder { receiver: &dyn Shape; }
         machine authored(holder: &Holder) -> i32 { holder.receiver.code() }",
        "machine generated(holder: &Holder) -> i32 { holder.receiver.code() }",
    );
    let retained = base.typed().clone();
    let typed = super::seeded_continuation::lower_seeded_extension(extension, base)
        .expect("bind the appended cohort");
    assert!(super::seeded_continuation::retained_typed_base_is_exact_prefix(&retained, &typed));
    let requirement = typed.trait_machine_signatures(&typed.traits()[0])[0].symbol;
    let targets = typed
        .expression_table
        .expression_nodes()
        .filter_map(|expression| match expression {
            typed_trees::expression::ExpressionNode::Call(call) => Some(call.target_symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets, [requirement, requirement]);
}

#[test]
fn typed_stage_rejects_ambiguous_inherited_dynamic_calls() {
    let error = crate::front_end::typed_program_result(
        "trait Left { machine code(&self) -> i32; }
         trait Right { machine code(&self) -> i32; }
         trait Both: Left + Right {}
         machine dispatch(receiver: &dyn Both) -> i32 { receiver.code() }",
    )
    .expect_err("a typed dynamic call needs an unambiguous requirement");
    assert!(
        error
            .message
            .contains("ambiguous across inherited requirements"),
        "{error:?}"
    );
}

fn seeded_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let resolved = crate::front_end::resolved_program_from_source_map(
        base_sources.clone(),
        &[(base_id, base_source)],
    );
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    assert_eq!(typing_base.typed().symbols.source_files().count(), 1);
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = crate::front_end::syntax_program_with_id(extension_id, extension_source);
    let seeded = resolve_extension(ExtensionRequest {
        base: typing_base.resolved_base_for_extension(),
        syntax: extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase extension selections");
    assert_eq!(rebased.trees().symbols.source_files().count(), 2);
    (typing_base, rebased)
}

fn seeded_normalized_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let resolved = crate::front_end::resolved_program_from_source_map(
        base_sources.clone(),
        &[(base_id, base_source)],
    );
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = crate::front_end::syntax_program_with_id(extension_id, extension_source);
    let resolved_base = typing_base.resolved_base_for_extension();
    let sources = Arc::new(sources);
    let extension_syntax =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
                syntax: extension_syntax,
                sources: Some(sources.clone()),
                top_level_bindings: Vec::new(),
                retained_base: Some(&resolved_base),
            },
        )
        .expect("normalize extension unit with its exact retained argument declarations");
    let seeded = resolve_extension(ExtensionRequest {
        base: resolved_base,
        syntax: extension_syntax,
        sources,
        top_level_bindings: Vec::new(),
    })
    .expect("resolve normalized seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase normalized extension selections");
    (typing_base, rebased)
}
