use super::{
    BuildTimeEvaluationRequest, BuildTimeSelectionAuthority, BuildTimeSourceContext,
    evaluate_pre_resolution,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokens_to_syntax_trees::parse_syntax_trees_with_id;
use typed_trees::types::FixedArrayLength;

const CONST_ARRAY_SOURCE: &str = r#"
    data Main { slots: [i64; table_size()]; }

    machine table_size() -> u64 {
        transition { _ -> (12 + 4) }
    }
"#;

#[test]
fn standalone_request_preserves_the_two_stage_evaluation() {
    let tokens = Lexer::new(CONST_ARRAY_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees_with_id(source::SourceId(0), &tokens).expect("parse");
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: None,
    })
    .expect("standalone evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    assert!(pre_check.selection_authority.is_none());
    let resolved = crate::syntax_probes::resolve(&syntax, None, &[]).expect("resolve");
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    pre_check.evaluate(&mut typed).expect("finish evaluation");
    assert!(
        typed
            .type_reference_table
            .fixed_array_lengths()
            .any(|(_, length)| *length == FixedArrayLength::Literal(16))
    );
}

#[test]
fn request_retains_exact_loader_module_alias_binding() {
    let mut sources = SourceMap::default();
    let requester_text = "use selected::combat::Damage; data Main { damage: Damage; }";
    let declaration_text = "module combat; pub data Damage { amount: u64; }";
    let requester = sources
        .add(PathBuf::from("root/main.omg"), requester_text.to_owned())
        .source_id;
    let declaration = sources
        .add(
            PathBuf::from("dependency/combat.omg"),
            declaration_text.to_owned(),
        )
        .source_id;
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (source_id, text) in [(requester, requester_text), (declaration, declaration_text)] {
        let tokens = Lexer::new(text)
            .tokenize()
            .expect("tokenize module alias probe");
        tokens_to_syntax_trees::parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("parse module alias probe");
    }
    let bindings = [symbols::SourceScopedTopLevelBinding::module_import(
        requester,
        declaration,
        "selected::combat::Damage",
        1,
    )];
    let sources = Arc::new(sources);
    let unbound = crate::syntax_probes::resolve(&syntax, Some(sources.clone()), &[])
        .expect("unresolved nominal names remain for typing");
    let root = unbound
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("unbound requester");
    let symbol_resolved_trees::data::DataMember::Field(field) =
        &unbound.data_members(root.members)[0]
    else {
        panic!("unbound requester field");
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } = &field.type_reference
    else {
        panic!("unbound nominal field");
    };
    assert!(
        !symbol.is_valid(),
        "alias cannot be reconstructed without loader bindings"
    );
    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &bindings,
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("evaluate with borrowed loader bindings");
    let (syntax, _) = evaluated.into_syntax_and_pre_check();
    let resolved = crate::syntax_probes::resolve(&syntax, Some(sources), &bindings)
        .expect("resolve probe with exact alias binding");
    let declaration = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Damage")
        .expect("module data");
    let root = resolved
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("requester data");
    let symbol_resolved_trees::data::DataMember::Field(field) =
        &resolved.data_members(root.members)[0]
    else {
        panic!("requester field");
    };
    let symbol_resolved_trees::types::TypeReference::Named { symbol, .. } = &field.type_reference
    else {
        panic!("nominal module field");
    };
    assert_eq!(*symbol, declaration.symbol);
}

fn parsed_source(
    source: &str,
    package: PackageKeyIdentity,
) -> (syntax_trees::SyntaxTrees, Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add_with_metadata(
            PathBuf::from("cache/selected/main.omg"),
            source.to_owned(),
            PathBuf::from("cache/selected"),
            Some(package),
            SourceOrigin::User,
        )
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse");
    (syntax, Arc::new(sources))
}

fn typed_after_pre_resolution(
    syntax: &syntax_trees::SyntaxTrees,
    sources: Arc<SourceMap>,
) -> typed_trees::TypedTrees {
    let resolved = crate::syntax_probes::resolve(syntax, Some(sources), &[])
        .expect("resolve pre-evaluated syntax");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type pre-evaluated syntax")
}

#[test]
fn package_aware_probe_retains_authored_symbol_ownership() {
    let source = "machine selected() {}";
    let package = PackageKeyIdentity::from_digest([0x6a; 32]).expect("nonzero package identity");
    let (syntax, sources) = parsed_source(source, package);

    let resolved = crate::syntax_probes::resolve(&syntax, Some(sources), &[])
        .expect("package-aware probe resolution");
    let machine = resolved.machines.first().expect("selected machine");

    assert_eq!(
        resolved.symbols.symbol_package_identity(machine.symbol),
        Some(package)
    );
}

#[test]
fn plain_pre_resolution_owns_one_coherent_pre_check_continuation() {
    let package = PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    let (syntax, sources) = parsed_source(CONST_ARRAY_SOURCE, package);

    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &[],
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("plain pre-resolution evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    assert!(pre_check.selection_authority.is_none());

    let mut typed = typed_after_pre_resolution(&syntax, sources);
    assert!(
        typed
            .type_reference_table
            .fixed_array_lengths()
            .any(|(_, length)| matches!(length, FixedArrayLength::ConstCall { .. }))
    );
    pre_check
        .evaluate(&mut typed)
        .expect("the matching plain continuation should evaluate exactly once");
    assert!(
        typed
            .type_reference_table
            .fixed_array_lengths()
            .any(|(_, length)| *length == FixedArrayLength::Literal(16))
    );
    assert!(
        !typed
            .type_reference_table
            .fixed_array_lengths()
            .any(|(_, length)| matches!(length, FixedArrayLength::ConstCall { .. }))
    );
}

#[derive(Default)]
struct AllowAllSelections {
    consultations: AtomicUsize,
}

impl BuildTimeSelectionAuthority for AllowAllSelections {
    fn allows_declaration_selection(
        &self,
        _requester: PackageKeyIdentity,
        _owner: PackageKeyIdentity,
    ) -> bool {
        self.consultations.fetch_add(1, Ordering::SeqCst);
        true
    }

    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        format!("package-{identity:?}")
    }
}

#[test]
fn package_permission_does_not_authorize_builtin_operator_substitution() {
    let package = PackageKeyIdentity::from_digest([0x73; 32]).expect("nonzero package");
    let source = r#"
        data Math {}
        boundary operator % Math::remainder(left: u64, right: u64) -> u64;
        data Provider {}
        machine Provider::remainder(left: u64, right: u64) -> u64 satisfies Math::remainder { 0 }
        machine count() -> u64 { 7u64 % 2 }
        data Buffer<const N: u64> { values: [u8; N]; }
        data Main { value: Buffer<count()>; }
    "#;
    let (syntax, sources) = parsed_source(source, package);
    let result = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources,
            source_scoped_top_level_bindings: &[],
            selection_authority: Some(Arc::new(AllowAllSelections::default())),
            retained_base: None,
        }),
    });
    let errors = match result {
        Ok(_) => panic!("package permission cannot supply selected operator semantics"),
        Err(errors) => errors,
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("requires exact authored selection")),
        "{errors:?}"
    );
}

#[test]
fn authority_pre_resolution_retains_the_exact_authority_for_pre_check() {
    let package = PackageKeyIdentity::from_digest([0x72; 32]).expect("nonzero package identity");
    let source = r#"
        machine table_size() -> u64 {
            transition { _ -> 4 }
        }

        data FixedBuffer<const N: u64> {
            items: [i32 in Wrapping; N];
        }

        data Main {
            generic: FixedBuffer<table_size()>;
            slots: [i64; table_size()];
        }
    "#;
    let (syntax, sources) = parsed_source(source, package);
    let counter = Arc::new(AllowAllSelections::default());
    let authority: Arc<dyn BuildTimeSelectionAuthority> = counter.clone();
    let expected_authority = authority.clone();

    let evaluated = evaluate_pre_resolution(BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: &[],
            selection_authority: Some(authority),
            retained_base: None,
        }),
    })
    .expect("authority-bearing pre-resolution evaluation");
    let pre_resolution_consultations = counter.consultations.load(Ordering::SeqCst);
    assert!(
        pre_resolution_consultations > 0,
        "const-generic evaluation must consult the selected authority before resolution"
    );
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    assert!(Arc::ptr_eq(
        pre_check
            .selection_authority
            .as_ref()
            .expect("retained selection authority"),
        &expected_authority,
    ));

    let mut typed = typed_after_pre_resolution(&syntax, sources);
    pre_check
        .evaluate(&mut typed)
        .expect("the authority-bearing continuation should evaluate exactly once");
    assert!(
        counter.consultations.load(Ordering::SeqCst) > pre_resolution_consultations,
        "the retained authority must be consulted again by fixed-array pre-check evaluation"
    );
    assert!(
        typed
            .type_reference_table
            .fixed_array_lengths()
            .all(|(_, length)| !matches!(length, FixedArrayLength::ConstCall { .. }))
    );
}
