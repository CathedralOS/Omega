//! These controls parse real Build binding requests, then supply a completed
//! host event. They check retained admission, without executing a Build.

use super::*;
use language_semantics::declaration_selection::ProductTarget;
use source::{DependencyScope, SourceOrigin, SourceResolutionStratum};
use std::sync::Arc;

const PRELUDE: &str = r#"
public data Build { marker: u64; }
public boundary trait BuildBindingOperations {
    machine prepare(builder: &Build) -> u64 reaches none crashes none;
    machine bind_native<Locator>(activation: u64, target: u64, locator: Locator) reaches none crashes none;
}
public machine build_selection_receiver<'activation>(builder: &'activation mut Build) -> &'activation mut Build { builder }
public data BuildTarget [copy] { case LinuxX86_64; }
public const linux_x86_64: BuildTarget = BuildTarget::LinuxX86_64;
public machine build_selection_target(target: BuildTarget) -> u64 {
    match target { BuildTarget::LinuxX86_64 -> 1u64 }
}
"#;
const REQUIREMENT: &str =
    "public boundary trait Raw { machine read() -> u64 reaches none crashes none; }";

fn package(byte: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([byte; 32]).unwrap()
}

fn typed_requirement(
    requirement: &str,
    origin: SourceOrigin,
    owner: Option<PackageKeyIdentity>,
    stratum: SourceResolutionStratum,
    dependency: bool,
) -> TypedTrees {
    let path = if dependency {
        "dep::Raw::read"
    } else {
        "Raw::read"
    };
    let build = format!(
        "machine build(builder: &mut Build) {{\n\
         let locator: ForeignBinding<0, 0, 0> = ForeignBinding::Syscall {{ number: 9u64 }};\n\
         Build::bind<{path}>(builder, linux_x86_64, locator);\n}}"
    );
    let mut sources = source::SourceMap::default();
    if dependency {
        sources.retain_product_dependency_scope([(package(7), "dep", package(8))]);
    }
    let external = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../source/library/core/external_binding.omg"
    ));
    let inputs = [
        (
            "root/build.omg",
            build.as_str(),
            "root",
            Some(package(7)),
            SourceOrigin::User,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "<build-prelude>",
            PRELUDE,
            "root",
            Some(package(7)),
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "<build-prelude:Build>",
            "module Build;\n",
            "root",
            Some(package(7)),
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "source/library/core/external_binding.omg",
            external,
            "source/library/core",
            None,
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
            DependencyScope::Build,
        ),
        (
            "core/raw.omg",
            requirement,
            "core",
            owner,
            origin,
            stratum,
            DependencyScope::Product,
        ),
    ];
    let mut syntax = None;
    let mut ids = Vec::new();
    for (path, text, root, owner, origin, stratum, scope) in inputs {
        let id = sources
            .add_checked_instance(
                path.into(),
                text,
                root.into(),
                owner,
                origin,
                stratum,
                scope,
            )
            .source_id;
        ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        match &mut syntax {
            None => {
                syntax =
                    Some(tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap())
            }
            Some(syntax) => {
                tokens_to_syntax_trees::parse(syntax, id, &tokens).unwrap();
            }
        }
    }
    let mut request =
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax.unwrap());
    request.sources = Some(Arc::new(sources));
    request.top_level_bindings = vec![
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[1], "Build"),
        symbols::SourceScopedTopLevelBinding::module_import(ids[0], ids[2], "Build", 0),
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[3], "ForeignBinding"),
        symbols::SourceScopedTopLevelBinding::new(ids[0], ids[3], "DllImport"),
    ];
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn capture(typed: &TypedTrees) -> (Vec<CapturedNativeDeclaration>, Vec<CapturedNativeIntent>) {
    let author = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .unwrap()
        .symbol;
    let request = typed_trees::product_operand::collect_product_selection_occurrences(typed)
        .into_iter()
        .find(|occurrence| occurrence.request.operation == BuildOperation::NativeBinding)
        .unwrap()
        .request;
    let formation = native_binding_formation(typed, author, Some(author), request.clone()).unwrap();
    let event = ExecutedNativeBinding {
        request,
        target: ProductTarget::LinuxX86_64,
        locator: semantic_vocabulary::NativeBindingLocator::Syscall { number: 9 },
        selecting_machine: formation.author,
    };
    capture_package_intents(typed, package(7), &[formation], &[event]).unwrap()
}

fn toolchain_product() -> TypedTrees {
    typed_requirement(
        REQUIREMENT,
        SourceOrigin::Toolchain,
        None,
        SourceResolutionStratum::Base,
        false,
    )
}

fn anchor(typed: &TypedTrees) -> symbols::RetainedDeclaration {
    let owner = typed
        .traits()
        .iter()
        .find(|owner| typed.symbols.display_path(owner.symbol, "::") == "Raw")
        .unwrap();
    symbols::RetainedDeclaration::capture(
        &typed.symbols,
        typed.trait_machine_signatures(owner)[0].symbol,
    )
    .unwrap()
}

fn assert_refusal(
    typed: &mut TypedTrees,
    declarations: &[CapturedNativeDeclaration],
    intents: &[CapturedNativeIntent],
    reason: &str,
) {
    let errors = collect_captured(typed, declarations, intents)
        .expect_err("the captured binding must reject");
    assert!(
        errors.iter().any(|error| error.message.contains(reason)),
        "expected {reason}: {errors:?}"
    );
}

#[test]
fn captured_public_toolchain_product_binding_keeps_its_supplier() {
    let (declarations, intents) = capture(&toolchain_product());
    let mut receiving = toolchain_product();
    let selections = collect_captured(&mut receiving, &declarations, &intents).unwrap();
    let [selection] = selections.as_slice() else {
        panic!("one executed binding must survive")
    };
    assert_eq!(
        receiving
            .symbols
            .symbol_product_package_identity(selection.requirement),
        None
    );
    assert_eq!(selection.supplier_package, package(7));
    assert_eq!(selection.producer, declarations[0].source.producer);
    assert_eq!(
        selection.source_occurrence_digest,
        declarations[0].source.source_occurrence_digest
    );
    assert_eq!(selection.target, intents[0].target);
    assert_eq!(selection.locator, intents[0].locator);
}

#[test]
fn captured_native_bindings_preserve_package_and_dependency_ownership() {
    for dependency in [false, true] {
        let owner = if dependency { package(8) } else { package(7) };
        let mut typed = typed_requirement(
            REQUIREMENT,
            SourceOrigin::User,
            Some(owner),
            SourceResolutionStratum::Base,
            dependency,
        );
        let (declarations, intents) = capture(&typed);
        assert_eq!(
            declarations[0].dependency_alias.as_deref(),
            dependency.then_some("dep")
        );
        let selections = collect_captured(&mut typed, &declarations, &intents).unwrap();
        assert_eq!(selections.len(), 1);
        assert_eq!(
            typed
                .symbols
                .symbol_product_package_identity(selections[0].requirement),
            Some(owner)
        );
    }
}

#[test]
fn captured_toolchain_binding_does_not_authorize_other_ownerless_sources() {
    let (declarations, intents) = capture(&toolchain_product());
    // Even a receipt naming the exact receiving declaration cannot make a
    // private, user-authored, contract-only or temporally hidden source visible.
    for (text, origin, stratum) in [
        (
            "boundary trait Raw { machine read() -> u64 reaches none crashes none; }",
            SourceOrigin::Toolchain,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::User,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::ToolchainContract,
            SourceResolutionStratum::Base,
        ),
        (
            REQUIREMENT,
            SourceOrigin::Toolchain,
            SourceResolutionStratum::CurrentActivationExtension,
        ),
    ] {
        let mut receiving = typed_requirement(text, origin, None, stratum, false);
        let mut forged = declarations.clone();
        forged[0].requirement = anchor(&receiving);
        assert_refusal(
            &mut receiving,
            &forged,
            &intents,
            "not visible under its original Product package and temporal scope",
        );
    }
    let mut receiving = typed_requirement(
        REQUIREMENT,
        SourceOrigin::Toolchain,
        None,
        SourceResolutionStratum::Base,
        true,
    );
    let mut forged = declarations.clone();
    forged[0].source.requirement_path = "dep::Raw::read".into();
    forged[0].dependency_alias = Some("dep".into());
    assert_refusal(
        &mut receiving,
        &forged,
        &intents,
        "not visible under its original Product package and temporal scope",
    );
}

#[test]
fn captured_toolchain_binding_keeps_complete_source_and_signature_custody() {
    let (declarations, intents) = capture(&toolchain_product());
    let mut receiving = toolchain_product();
    let mut changed = declarations.clone();
    changed[0].requirement_identity.push_str(" changed");
    assert_refusal(
        &mut receiving,
        &changed,
        &intents,
        "changed its retained complete requirement identity",
    );
    let mut changed = declarations.clone();
    changed[0].dependency_alias = Some("dep".into());
    assert_refusal(
        &mut receiving,
        &changed,
        &intents,
        "changed its original direct Product dependency scope",
    );
    for changed_source in [
        {
            let mut source = declarations[0].requirement.source().clone();
            source.origin = SourceOrigin::ToolchainContract;
            source
        },
        {
            let mut source = declarations[0].requirement.source().clone();
            source.source = format!("{}\n", source.source).into();
            source
        },
        {
            let mut source = declarations[0].requirement.source().clone();
            source.dependency_scope = DependencyScope::Build;
            source
        },
    ] {
        let mut changed = declarations.clone();
        let original = &declarations[0].requirement;
        changed[0].requirement = symbols::RetainedDeclaration::from_retained_parts(
            changed_source,
            original.span(),
            original.kind(),
        )
        .unwrap();
        assert_refusal(
            &mut receiving,
            &changed,
            &intents,
            "retained declaration is absent or changed",
        );
    }
    let original = &declarations[0].requirement;
    let mut shortened = original.span();
    shortened.end -= 1;
    for (span, kind) in [
        (shortened, original.kind()),
        (original.span(), symbols::SymbolKind::Data),
    ] {
        let mut changed = declarations.clone();
        changed[0].requirement = symbols::RetainedDeclaration::from_retained_parts(
            original.source().clone(),
            span,
            kind,
        )
        .unwrap();
        assert_refusal(
            &mut receiving,
            &changed,
            &intents,
            "retained declaration is absent or changed",
        );
    }
}

fn table_selector(typed: &TypedTrees) -> (SymbolHandle, semantic_vocabulary::NativeBindingLocator) {
    table_selector_with_query(typed, None)
}

fn table_selector_with_query(
    typed: &TypedTrees,
    field_query: Option<&typed_trees::expression::StaticMachineArgument>,
) -> (SymbolHandle, semantic_vocabulary::NativeBindingLocator) {
    let owner = typed
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Raw")
        .unwrap();
    let signature = &typed.trait_machine_signatures(owner)[0];
    let formal = &typed.state_signature_parameters(signature)[1];
    let reference = typed
        .type_reference_table
        .unconstrained_type_reference(formal.type_reference);
    let table_type = match typed.type_reference_table.type_reference(reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => *referee,
        _ => reference,
    };
    let data = typed.closed_data_definition(table_type).unwrap();
    let field = typed.data_members(data)[1].as_field().unwrap().symbol;
    let plain_field = typed_trees::expression::StaticMachineArgument {
        symbol: field,
        ..Default::default()
    };
    let from_query = typed_trees::reflection::construction::field_description_identity(
        typed,
        field_query.unwrap_or(&plain_field),
        signature.symbol,
    )
    .unwrap();
    let from_type = typed_trees::reflection::construction::field_description_for_type(
        typed,
        table_type,
        field,
        signature.symbol,
    )
    .unwrap();
    assert_eq!(
        from_query, from_type,
        "the typed borrowed table must describe the existing field selector's exact owner application"
    );
    (
        signature.symbol,
        semantic_vocabulary::NativeBindingLocator::FunctionField {
            table: typed_trees::reflection::construction::parameter_description_identity(
                typed,
                formal.symbol,
                signature.symbol,
            )
            .unwrap(),
            field: typed_trees::reflection::construction::field_description_for_type(
                typed,
                table_type,
                field,
                signature.symbol,
            )
            .unwrap(),
        },
    )
}

const TABLE_REQUIREMENT: &str = "public data Table { marker: u64; function: addr; }\npublic boundary trait Raw { machine read(value: u64, table: &Table) -> u64 reaches none crashes none; }";

#[test]
fn a_table_selector_uses_the_exact_nonprefix_formal_and_field() {
    let make = |source: &str| {
        typed_requirement(
            source,
            SourceOrigin::User,
            Some(package(7)),
            SourceResolutionStratum::Base,
            false,
        )
    };
    let typed = make(TABLE_REQUIREMENT);
    let (requirement, locator) = table_selector(&typed);
    authenticate_locator(
        &typed,
        requirement,
        &locator,
        package(7),
        SourceResolutionStratum::Base,
    )
    .unwrap();
    let semantic_vocabulary::NativeBindingLocator::FunctionField { table, field } = &locator else {
        unreachable!()
    };
    assert_eq!(table.declaration_position, 1);
    assert_eq!(field.declaration_position, 1);
    for (mutated, reason) in [
        (
            {
                let mut x = table.clone();
                x.declaration_position = 0;
                semantic_vocabulary::NativeBindingLocator::FunctionField {
                    table: x,
                    field: field.clone(),
                }
            },
            "exact formal",
        ),
        (
            {
                let mut x = table.clone();
                x.qualified_type.push(0);
                semantic_vocabulary::NativeBindingLocator::FunctionField {
                    table: x,
                    field: field.clone(),
                }
            },
            "exact formal",
        ),
        (
            {
                let mut x = field.clone();
                x.declaration_position = 0;
                semantic_vocabulary::NativeBindingLocator::FunctionField {
                    table: table.clone(),
                    field: x,
                }
            },
            "live non-atomic addr",
        ),
        (
            {
                let mut x = field.clone();
                x.source_revision.push(0);
                semantic_vocabulary::NativeBindingLocator::FunctionField {
                    table: table.clone(),
                    field: x,
                }
            },
            "exact field",
        ),
        (
            {
                let mut x = field.clone();
                x.owner_application.push(0);
                semantic_vocabulary::NativeBindingLocator::FunctionField {
                    table: table.clone(),
                    field: x,
                }
            },
            "exact field",
        ),
    ] {
        assert!(
            authenticate_locator(
                &typed,
                requirement,
                &mutated,
                package(7),
                SourceResolutionStratum::Base
            )
            .unwrap_err()
            .contains(reason)
        );
    }
    for source in [
        TABLE_REQUIREMENT.to_owned(),
        TABLE_REQUIREMENT.replace("table: &Table", "table: &mut Table"),
    ] {
        let readable = make(&source);
        let (requirement, locator) = table_selector(&readable);
        authenticate_locator(
            &readable,
            requirement,
            &locator,
            package(7),
            SourceResolutionStratum::Base,
        )
        .unwrap();
    }
    let write_only = make(&TABLE_REQUIREMENT.replace("table: &Table", "table: &write Table"));
    let (requirement, locator) = table_selector(&write_only);
    assert!(
        authenticate_locator(
            &write_only,
            requirement,
            &locator,
            package(7),
            SourceResolutionStratum::Base
        )
        .unwrap_err()
        .contains("permits reading the dispatch field")
    );
    let owned = make(&TABLE_REQUIREMENT.replace("table: &Table", "table: Table"));
    let (requirement, locator) = table_selector(&owned);
    assert!(
        authenticate_locator(
            &owned,
            requirement,
            &locator,
            package(7),
            SourceResolutionStratum::Base
        )
        .unwrap_err()
        .contains("explicitly borrowed")
    );
    let non_address = make(&TABLE_REQUIREMENT.replace("function: addr", "function: u64"));
    let (requirement, locator) = table_selector(&non_address);
    assert!(
        authenticate_locator(
            &non_address,
            requirement,
            &locator,
            package(7),
            SourceResolutionStratum::Base
        )
        .unwrap_err()
        .contains("live non-atomic addr")
    );
}

#[test]
fn an_applied_table_uses_the_original_field_query_description() {
    use typed_trees::expression::{StaticMachineArgument, StaticSymbolApplication};
    use typed_trees::types::TypeReferenceNode;
    let source = TABLE_REQUIREMENT
        .replace("Table { marker: u64", "Table<T> { marker: T")
        .replace("table: &Table", "table: &Table<u64>");
    let mut typed = typed_requirement(
        &source,
        SourceOrigin::User,
        Some(package(7)),
        SourceResolutionStratum::Base,
        false,
    );
    let owner = typed
        .traits()
        .iter()
        .find(|owner| owner.name.as_str() == "Raw")
        .unwrap();
    let signature = &typed.trait_machine_signatures(owner)[0];
    let author = signature.symbol;
    let formal = &typed.state_signature_parameters(signature)[1];
    let TypeReferenceNode::Reference { referee, .. } = typed
        .type_reference_table
        .type_reference(formal.type_reference)
    else {
        panic!("the authored borrowed table")
    };
    let table = *referee;
    let data = typed.closed_data_definition(table).unwrap();
    let field = typed.data_members(data)[1].as_field().unwrap().symbol;
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        arguments,
        lifetime_arguments,
    } = typed.type_reference_table.type_reference(table).clone()
    else {
        panic!("the authored Table<u64> application")
    };
    let arguments = typed
        .type_reference_table
        .type_reference_handles(arguments)
        .iter()
        .map(|reference| {
            typed
                .static_operand_from_type_reference(*reference)
                .unwrap()
        })
        .collect::<Vec<_>>();
    // The original field query takes the complete selected type and its
    // member path. Form that selection from the actual source application,
    // using its ordinary operand preparation rather than a bare field symbol.
    let mut selected = StaticMachineArgument {
        symbol: base_symbol,
        path: vec![base_name].into_boxed_slice(),
        application: Some(Box::new(StaticSymbolApplication {
            arguments: arguments.into_boxed_slice(),
            lifetime_arguments: lifetime_arguments.into_boxed_slice(),
            member: arena::Handle::invalid(),
        })),
        ..Default::default()
    };
    selected.type_reference = typed
        .static_operand_type_reference_in_scope(&selected, author)
        .unwrap();
    assert!(
        typed
            .prepare_data_application_view(selected.type_reference)
            .unwrap()
    );
    let member = typed
        .expression_table
        .insert_static_member(StaticMachineArgument {
            symbol: field,
            ..Default::default()
        });
    selected.application.as_mut().unwrap().member = member;
    let (requirement, locator) = table_selector_with_query(&typed, Some(&selected));
    authenticate_locator(
        &typed,
        requirement,
        &locator,
        package(7),
        SourceResolutionStratum::Base,
    )
    .unwrap();
    let semantic_vocabulary::NativeBindingLocator::FunctionField { field, .. } = locator else {
        unreachable!()
    };
    assert!(!field.owner_application.is_empty());
}

#[test]
fn constructed_table_descriptions_keep_product_privacy_and_temporal_scope() {
    for (source, stratum, allowed) in [
        (
            TABLE_REQUIREMENT.to_owned(),
            SourceResolutionStratum::Base,
            true,
        ),
        (
            TABLE_REQUIREMENT.replacen("public data Table", "data Table", 1),
            SourceResolutionStratum::Base,
            false,
        ),
        (
            TABLE_REQUIREMENT.to_owned(),
            SourceResolutionStratum::CurrentActivationExtension,
            false,
        ),
    ] {
        let typed = typed_requirement(&source, SourceOrigin::User, Some(package(8)), stratum, true);
        let (requirement, locator) = table_selector(&typed);
        let result = authenticate_locator(
            &typed,
            requirement,
            &locator,
            package(7),
            SourceResolutionStratum::Base,
        );
        assert_eq!(result.is_ok(), allowed, "{result:?}");
        if !allowed {
            assert!(
                result
                    .unwrap_err()
                    .contains("original Product visibility or temporal scope")
            );
        }
        assert!(
            authenticate_locator(
                &typed,
                requirement,
                &locator,
                package(9),
                SourceResolutionStratum::Base
            )
            .unwrap_err()
            .contains("original Product visibility or temporal scope")
        );
    }
}

#[test]
fn captured_table_selectors_keep_the_original_supplier_and_exact_fields() {
    let mut typed = typed_requirement(
        TABLE_REQUIREMENT,
        SourceOrigin::User,
        Some(package(8)),
        SourceResolutionStratum::Base,
        true,
    );
    let (_, locator) = table_selector(&typed);
    // These admission controls supply a completed host event, as the other
    // capture tests do; they do not claim to execute Build or native code.
    let (declarations, mut intents) = capture(&typed);
    intents[0].locator = locator.clone();
    let selections = collect_captured(&mut typed, &declarations, &intents).unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].locator, locator);
    assert_eq!(selections[0].supplier_package, package(7));
    let semantic_vocabulary::NativeBindingLocator::FunctionField { table, field } = locator else {
        unreachable!()
    };
    for changed in [
        {
            let mut changed = field.clone();
            changed.owner_application.push(0);
            changed
        },
        {
            let mut changed = field;
            changed.member_identity.push(0);
            changed
        },
    ] {
        intents[0].locator = semantic_vocabulary::NativeBindingLocator::FunctionField {
            table: table.clone(),
            field: changed,
        };
        assert_refusal(
            &mut typed,
            &declarations,
            &intents,
            "changes the exact field",
        );
    }
    let source = TABLE_REQUIREMENT.replacen("public data Table", "data Table", 1);
    let mut private = typed_requirement(
        &source,
        SourceOrigin::User,
        Some(package(8)),
        SourceResolutionStratum::Base,
        true,
    );
    let (_, locator) = table_selector(&private);
    let (declarations, mut intents) = capture(&private);
    intents[0].locator = locator;
    assert_refusal(
        &mut private,
        &declarations,
        &intents,
        "original Product visibility or temporal scope",
    );
}
