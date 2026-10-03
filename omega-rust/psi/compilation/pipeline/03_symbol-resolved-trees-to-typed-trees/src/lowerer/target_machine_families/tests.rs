use crate::{
    lower_seeded_extension, lower_symbol_resolved_trees,
    lower_symbol_resolved_trees_to_seeded_base, retained_typed_base_is_exact_prefix,
};
use source::{DependencyScope, SourceId, SourceMap, SourceOrigin, SourceResolutionStratum};
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::item::Item;
use syntax_trees_to_symbol_resolved_trees::{
    ExtensionRequest, ResolutionRequest, resolve, resolve_extension,
};

fn admitted_syntax(source_id: SourceId, text: &str) -> SyntaxTrees {
    let mut syntax = crate::front_end::syntax_program_with_id(source_id, text);
    // Model admission's marker clearing without depending on Omega target
    // policy. Copying the resulting forest must retain the authored head.
    for handle in syntax.root_item_handles().to_vec() {
        let Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        if machine
            .target
            .as_ref()
            .is_some_and(|target| target.as_str() == "linux_x86_64")
        {
            let mut machine = machine.clone();
            machine.target = None;
            syntax.items.replace_item(handle, Item::Machine(machine));
        }
    }
    let mut copied = SyntaxTrees::default();
    copied.extend_from(&syntax);
    copied
}

#[test]
fn copied_target_heads_produce_exact_product_family_handles() {
    let text = "
        windows_x86_64 machine family_value() -> u64 { 2u64 }
        linux_x86_64 machine family_value() -> u64 { 1u64 }
        demo_target machine inert() -> u64 { 3u64 }
        machine ordinary() -> u64 { 4u64 }
        linux_x86_64 boundary machine native_leaf() -> u64;
    ";
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for scope in [DependencyScope::Build, DependencyScope::Product] {
        let source_id = sources
            .add_checked_instance(
                PathBuf::from("family.omg"),
                text.to_owned(),
                PathBuf::from("."),
                None,
                SourceOrigin::User,
                SourceResolutionStratum::Base,
                scope,
            )
            .source_id;
        syntax.extend_from(&admitted_syntax(source_id, text));
    }
    let resolved = resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources)),
        top_level_bindings: Vec::new(),
    })
    .expect("both checked contexts resolve independently");
    let typed = lower_symbol_resolved_trees(&resolved).expect("both contexts type");
    assert_eq!(typed.target_machine_families.len(), 2);
    let family = &typed.target_machine_families[0];
    assert_eq!(family.name.as_str(), "family_value");
    assert_eq!(
        family
            .members
            .iter()
            .map(|member| member.target.as_str())
            .collect::<Vec<_>>(),
        ["linux_x86_64", "windows_x86_64"],
    );
    assert_eq!(family.members[0].machine, family.canonical);
    for machine in &resolved.machines {
        let scope = resolved
            .symbols
            .source_file(machine.name.source_span())
            .expect("retained source scope")
            .dependency_scope;
        let is_family_member = scope == DependencyScope::Product
            && matches!(machine.name.as_str(), "family_value" | "native_leaf");
        assert_eq!(machine.target_family.is_valid(), is_family_member);
        if machine.target_family == family.canonical {
            assert!(
                family
                    .members
                    .iter()
                    .any(|member| member.machine == machine.symbol)
            );
        }
    }
    let leaf = &typed.target_machine_families[1];
    assert_eq!(leaf.name.as_str(), "native_leaf");
    assert_eq!(leaf.members.len(), 1);
    assert_eq!(leaf.members[0].machine, leaf.canonical);
    assert_eq!(leaf.members[0].target.as_str(), "linux_x86_64");
}

#[test]
fn generated_families_append_without_rebuilding_the_retained_prefix() {
    let base_text = "
        linux_x86_64 machine retained_value() -> u64 { 1u64 }
        windows_x86_64 machine retained_value() -> u64 { 2u64 }
    ";
    let extension_text = "
        windows_x86_64 machine generated_value() -> u64 { 4u64 }
        linux_x86_64 machine generated_value() -> u64 { 3u64 }
    ";
    let mut sources = SourceMap::default();
    let base_id = sources
        .add(PathBuf::from("base.omg"), base_text.to_owned())
        .source_id;
    let syntax = admitted_syntax(base_id, base_text);
    let resolved = resolve(ResolutionRequest {
        syntax,
        sources: Some(Arc::new(sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve retained family");
    let base = lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained family");
    let retained = base.typed().clone();
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_text.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = admitted_syntax(extension_id, extension_text);
    let extension = resolve_extension(ExtensionRequest {
        base: base.resolved_base_for_extension(),
        syntax: extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve generated family")
    .rebase_authored_selections_for_typed_continuation(
        base.typed().authored_declaration_selections(),
    )
    .expect("rebase extension selections");
    let mut extended = lower_seeded_extension(extension, base).expect("type generated family");
    assert!(retained_typed_base_is_exact_prefix(&retained, &extended));
    assert_eq!(
        extended
            .target_machine_families
            .iter()
            .map(|family| family.name.as_str())
            .collect::<Vec<_>>(),
        ["retained_value", "generated_value"],
    );
    assert_eq!(
        extended.target_machine_families[0],
        retained.target_machine_families[0]
    );
    let generated = &extended.target_machine_families[1];
    assert_eq!(generated.members.len(), 2);
    for member in &generated.members {
        let machine = extended
            .machine_by_symbol(member.machine)
            .expect("exact generated body");
        assert_eq!(machine.name.as_str(), "generated_value");
    }
    extended.target_machine_families[0].members.swap(0, 1);
    assert!(
        !retained_typed_base_is_exact_prefix(&retained, &extended),
        "retained family records are part of exact-prefix custody"
    );
}
