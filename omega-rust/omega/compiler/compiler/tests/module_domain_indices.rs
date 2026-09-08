use compiler::{CheckedCompilation, compile_to_checked_with_packages};
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use typed_trees::types::{
    DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

static NEXT_TREE: AtomicU64 = AtomicU64::new(0);
const INDEXED: &str = "pub domain<T, const N: u64> T::Indexed<N>;";

#[test]
fn named_and_computed_module_domain_indices_have_exact_canonical_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, value) in [("combat", 2), ("rooms", 3)] {
        Sources::write(root.join(format!("{module}.omg")), &format!(
            "module {module}; const SIZE: u64 = {value};
             data Local {{ named: u64 in Indexed<SIZE>; same: u64 in Indexed<SIZE + 0>; next: u64 in Indexed<SIZE + 1>; }}"
        ));
    }
    for imports in ["use combat; use rooms;", "use rooms; use combat;"] {
        Sources::write(root.join("main.omg"), &format!(
            "{imports} {INDEXED} const SIZE: u64 = 1;
             data Root {{ named: u64 in Indexed<SIZE>; same: u64 in Indexed<SIZE + 0>; next: u64 in Indexed<SIZE + 1>; }}
             data Combat {{ value: u64 in Indexed<combat::SIZE>; }}
             data Rooms {{ value: u64 in Indexed<rooms::SIZE>; }}
             data Repeated {{ value: u64 in Indexed<combat::SIZE + combat::SIZE>; }}"
        ));
        let checked = compile(&root, root_inputs(&root));
        for (path, value) in [("Root", 1), ("combat::Local", 2), ("rooms::Local", 3)] {
            let named = field(&checked, path, "named");
            let same = field(&checked, path, "same");
            let next = field(&checked, path, "next");
            assert_index(&checked, named, "u64", value);
            assert_index(&checked, same, "u64", value);
            assert_index(&checked, next, "u64", value + 1);
            assert_eq!(
                checked.typed.normalized_type_identity(named),
                checked.typed.normalized_type_identity(same)
            );
            assert_ne!(
                checked.typed.normalized_type_identity(named),
                checked.typed.normalized_type_identity(next)
            );
            assert_eq!(
                domain(&checked, named).semantic_id,
                domain(&checked, same).semantic_id
            );
            assert_ne!(
                domain(&checked, named).semantic_id,
                domain(&checked, next).semantic_id
            );
        }
        assert_index(&checked, field(&checked, "Combat", "value"), "u64", 2);
        assert_index(&checked, field(&checked, "Rooms", "value"), "u64", 3);
        assert_index(&checked, field(&checked, "Repeated", "value"), "u64", 4);
        let uses = selections(&checked, "combat::SIZE", identity(1));
        assert_eq!(
            uses.len(),
            6,
            "three local, one qualified, and two repeated operand occurrences survive"
        );
        for (position, selection) in uses.iter().enumerate() {
            assert!(
                uses[..position]
                    .iter()
                    .all(|prior| prior.source_span() != selection.source_span())
            );
            assert_eq!(
                selection.exposure(),
                AuthoredDeclarationSelectionExposure::PrivateImplementation
            );
        }
    }
}

#[test]
fn domain_index_carriers_and_each_exact_operation_are_checked() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, value, expression) in [
        ("u8", "255", "(SIZE + 1) - 1"),
        ("i8", "-128", "SIZE % -1"),
        ("u8", "1", "SIZE << 8"),
        ("u8", "1", "SIZE + 1u16"),
    ] {
        Sources::write(
            root.join("constants.omg"),
            &format!("module constants; const SIZE: {carrier} = {value};"),
        );
        let declarations =
            format!("use constants::SIZE; domain<T, const N: {carrier}> T::Indexed<N>;");
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} data Main {{ value: u64 in Indexed<SIZE + 0>; }}"),
        );
        compile(&root, root_inputs(&root));
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} data Main {{ value: u64 in Indexed<{expression}>; }}"),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("domain indices cannot erase a failing typed arithmetic node");
        let expected = if expression == "SIZE + 1u16" {
            "incompatible landed integer carriers"
        } else {
            "Exact integer constant operation"
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expression}: {diagnostics:?}"
        );
    }
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u32 = 2;",
    );
    for expression in ["SIZE", "SIZE + 0"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use constants::SIZE; {INDEXED} data Main {{ value: u64 in Indexed<{expression}>; }}"
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("a named u32 does not reland as the domain's u64 index");
        let expected = if expression == "SIZE" {
            "declares type `u32`, but the parameter requires `u64`"
        } else {
            "landed `u32` result cannot initialize `u64`"
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn canonical_domain_deduplication_retains_private_and_public_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let private = "data Private { value: u64 in Indexed<constants::SIZE + 0>; }";
    let public = "pub data Public { value: u64 in Indexed<constants::SIZE>; }";
    Sources::write(
        root.join("main.omg"),
        &format!("use constants; {INDEXED} {private}"),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!("use constants; {INDEXED} {private} {public}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("an equal private application cannot hide a public constant selection");
    Sources::write(
        root.join("constants.omg"),
        "module constants; pub const SIZE: u64 = 2;",
    );
    let checked = compile(&root, root_inputs(&root));
    let private = field(&checked, "Private", "value");
    let public = field(&checked, "Public", "value");
    assert_eq!(
        checked.typed.normalized_type_identity(private),
        checked.typed.normalized_type_identity(public)
    );
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 2);
    assert_ne!(uses[0].source_span(), uses[1].source_span());
    for exposure in [
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    ] {
        assert!(
            uses.iter()
                .any(|selection| selection.exposure() == exposure)
        );
    }
}

#[test]
fn domain_indices_cannot_select_private_or_transitive_package_constants() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::constants; pub machine bridge() -> u64 { leaf::constants::SIZE }",
    );
    Sources::write(
        leaf.join("constants.omg"),
        "module constants; pub const SIZE: u64 = 2;",
    );
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    for expression in ["constants::SIZE", "constants::SIZE + 0"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use middle::bridge; {INDEXED} data Main {{ value: u64 in Indexed<{expression}>; }}"
            ),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone()).expect_err(
            "loading a transitive constant does not authorize a domain index selection",
        );
    }
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let inputs = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    for expression in ["SIZE", "SIZE + 0"] {
        Sources::write(
            leaf.join("constants.omg"),
            "module constants; pub const SIZE: u64 = 2;",
        );
        let source = format!(
            "use leaf::constants::SIZE; {INDEXED} data Main {{ value: u64 in Indexed<{expression}>; }}"
        );
        Sources::write(root.join("main.omg"), &source);
        let checked = compile(&root, inputs.clone());
        assert_index(&checked, field(&checked, "Main", "value"), "u64", 2);
        let uses = selections(&checked, "constants::SIZE", identity(3));
        assert_eq!(
            uses.len(),
            2,
            "the import and the argument are independent selections"
        );
        assert!(
            uses.iter().any(
                |selection| selection.source_span().span.start == source.rfind("SIZE").unwrap()
            )
        );
        Sources::write(
            leaf.join("constants.omg"),
            "module constants; const SIZE: u64 = 2;",
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone())
            .expect_err("a direct dependency does not expose a private domain index constant");
    }
}

#[test]
fn nested_domain_arguments_keep_the_live_uses_exposure() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let declarations = format!("use constants; {INDEXED} pub data Wrap<T> {{ value: T; }}");
    let private = "data Private { value: Wrap<u64 in Indexed<constants::SIZE + 0> >; }";
    let public = "pub data Public { value: Wrap<u64 in Indexed<constants::SIZE> >; }";
    Sources::write(root.join("main.omg"), &format!("{declarations} {private}"));
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 1);
    assert_eq!(
        uses[0].exposure(),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    );
    Sources::write(
        root.join("main.omg"),
        &format!("{declarations} {private} {public}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("a nested equal application must retain its independent public use");
    Sources::write(
        root.join("constants.omg"),
        "module constants; pub const SIZE: u64 = 2;",
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    for exposure in [
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    ] {
        assert!(
            uses.iter()
                .any(|selection| selection.exposure() == exposure)
        );
    }
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants; {INDEXED} pub data Wrap<T> {{ value: T; owned: u64 in Indexed<constants::SIZE>; }} {private}"
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("derived argument suppression cannot hide the template's own public index");
}

#[test]
fn open_domain_binders_do_not_capture_a_same_named_module_constant() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 9;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants::SIZE; {INDEXED}
         data Generic<const SIZE: u64> {{ value: u64 in Indexed<SIZE>; }}
         machine inspect<const SIZE: u64>(value: u64 in Indexed<SIZE>) {{}}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(
        uses.len(),
        1,
        "only the authored import selects the constant"
    );
    assert_eq!(
        uses[0].source_span().span,
        source::Span::new(4, "use constants::SIZE".len())
    );
    let generic = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generic")
        .unwrap();
    let [parameter] = checked.typed.data_type_parameters(generic) else {
        panic!("one data const binder")
    };
    let argument = domain(&checked, field(&checked, "Generic", "value")).arguments[0];
    assert!(
        matches!(checked.typed.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
    );
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let [parameter] = checked.typed.machine_type_parameters(machine) else {
        panic!("one machine const binder")
    };
    let state = &checked.typed.machine_states(machine)[0];
    let argument = domain(
        &checked,
        checked.typed.state_parameters(state)[0].type_reference,
    )
    .arguments[0];
    assert!(
        matches!(checked.typed.type_reference_table.type_reference(argument), TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
    );
}

#[test]
fn payload_domain_indices_preserve_canonical_values_and_operand_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    Sources::write(root.join("main.omg"), &format!(
        "use constants; {INDEXED}
         data Outcome {{ case Ready(named: u64 in Indexed<constants::SIZE>, same: u64 in Indexed<constants::SIZE + 0>, next: u64 in Indexed<constants::SIZE + 1>); }}"
    ));
    let checked = compile(&root, root_inputs(&root));
    let outcome = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outcome")
        .unwrap();
    let [typed_trees::data::DataMember::Variant(ready)] = checked.typed.data_members(outcome)
    else {
        panic!("one payload case")
    };
    let [named, same, next] = checked.typed.data_payload_fields(ready) else {
        panic!("three qualified payload fields")
    };
    assert_index(&checked, named.type_reference, "u64", 2);
    assert_index(&checked, same.type_reference, "u64", 2);
    assert_index(&checked, next.type_reference, "u64", 3);
    assert_eq!(
        checked.typed.normalized_type_identity(named.type_reference),
        checked.typed.normalized_type_identity(same.type_reference)
    );
    assert_ne!(
        checked.typed.normalized_type_identity(named.type_reference),
        checked.typed.normalized_type_identity(next.type_reference)
    );
    let uses = selections(&checked, "constants::SIZE", identity(1));
    assert_eq!(uses.len(), 3);
    for (position, selection) in uses.iter().enumerate() {
        assert_eq!(
            selection.exposure(),
            AuthoredDeclarationSelectionExposure::PrivateImplementation
        );
        assert!(
            uses[..position]
                .iter()
                .all(|prior| prior.source_span() != selection.source_span())
        );
    }
}

#[test]
fn represented_nested_machine_domain_arguments_never_capture_global_constant_headers() {
    use syntax_trees::types::TypeReferenceNode as SyntaxType;
    for index in ["2", "SIZE"] {
        for machine in [
            format!("machine inspect(SIZE: u64, value: u64 in D<{index}>) {{}}"),
            format!(
                "machine inspect(input: u64) {{ let SIZE: u64 = input; let value: u64 in D<{index}>; }}"
            ),
        ] {
            let mut syntax = syntax_trees::SyntaxTrees::default();
            let root = format!(
                "use constants::SIZE; domain<T, U> T::D<U>; data Buffer<const N: u64> {{ value: u8; }} {machine}"
            );
            for (source_id, text) in [
                (source::SourceId(1), root.as_str()),
                (
                    source::SourceId(2),
                    "module constants; pub const SIZE: u64 = 9;",
                ),
            ] {
                let tokens = source_files_to_tokens::Lexer::new(text)
                    .tokenize()
                    .expect("domain placeholder source tokens");
                tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
                    &mut syntax,
                    source_id,
                    &tokens,
                )
                .expect("domain placeholder source parses");
            }
            // The parser's domain index surface does not yet accept nested
            // generic type arguments. Pin the traversal contract directly on
            // its existing representation, retaining the leaf's authored span.
            let domains = syntax.type_references.domain_constraints();
            assert!(!domains.is_empty(), "represented domain placeholders");
            let mut replaced_arguments = Vec::new();
            for domain in domains {
                let [argument] = syntax
                    .type_references
                    .type_reference_handles(domain.arguments)
                else {
                    panic!("one placeholder argument")
                };
                let argument = *argument;
                if replaced_arguments.contains(&argument) {
                    continue;
                }
                replaced_arguments.push(argument);
                let leaf = syntax.type_references.type_reference(argument).clone();
                let leaf = syntax.type_references.insert(leaf);
                let arguments = syntax.type_references.insert_type_reference_handles([leaf]);
                syntax.type_references.replace_type_reference(
                    argument,
                    SyntaxType::Generic {
                        base_name: syntax_trees::identifier::Identifier::generated("Buffer"),
                        lifetime_arguments: Vec::new(),
                        arguments,
                    },
                );
            }
            let syntax = syntax_trees_to_symbol_resolved_trees::normalize_generic_data(syntax)
                .expect("normalization defers machine-owned domain arguments without selecting global headers");
            let domains = syntax.type_references.domain_constraints();
            assert!(!domains.is_empty(), "retained machine domain arguments");
            for domain in domains {
                let [argument] = syntax
                    .type_references
                    .type_reference_handles(domain.arguments)
                else {
                    panic!("one nested type index")
                };
                let SyntaxType::Generic { arguments, .. } =
                    syntax.type_references.type_reference(*argument)
                else {
                    panic!("machine domain nested application must remain structural")
                };
                let [index_argument] = syntax.type_references.type_reference_handles(*arguments)
                else {
                    panic!("one Buffer const index")
                };
                assert!(
                    matches!(syntax.type_references.type_reference(*index_argument), SyntaxType::Named(name) if name.as_str() == index)
                );
                assert!(
                    syntax
                        .type_references
                        .const_argument_normalization(*index_argument)
                        .is_none(),
                    "machine lexical bindings never create global constant selection custody"
                );
            }
        }
    }
}

fn field(checked: &CheckedCompilation, path: &str, name: &str) -> TypeReferenceHandle {
    let definition = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| checked.symbols.display_path(definition.symbol, "::") == path)
        .expect("data declaration");
    checked
        .typed
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                Some(field.type_reference)
            }
            _ => None,
        })
        .expect("domain-qualified field")
}

fn domain(checked: &CheckedCompilation, reference: TypeReferenceHandle) -> &DomainConstraint {
    let TypeReferenceNode::Constrained { constraints, .. } =
        checked.typed.type_reference_table.type_reference(reference)
    else {
        panic!("constrained field")
    };
    let [TypeConstraintNode::Domain(domain)] =
        checked.typed.type_reference_table.constraints(*constraints)
    else {
        panic!("one normalized domain atom")
    };
    assert!(domain.symbol.is_valid());
    assert!(domain.semantic_id.is_valid());
    domain
}

fn assert_index(
    checked: &CheckedCompilation,
    reference: TypeReferenceHandle,
    carrier: &str,
    value: i128,
) {
    let domain = domain(checked, reference);
    let [argument] = domain.arguments.as_slice() else {
        panic!("one domain index")
    };
    let TypeReferenceNode::Named { symbol, name } =
        checked.typed.type_reference_table.type_reference(*argument)
    else {
        panic!("closed canonical domain index")
    };
    assert!(
        !symbol.is_valid(),
        "canonical values are not declaration handles"
    );
    if let Some(atom) = CanonicalConstValue::from_atom(name.as_str()) {
        assert_eq!(
            atom.decode_encoding(),
            Some(DecodedCanonicalConstValue::Integer {
                type_name: carrier.to_owned(),
                value
            })
        );
    } else {
        assert_eq!(
            name.as_str(),
            value.to_string(),
            "closed integer argument uses the canonical decimal leaf encoding"
        );
    }
}

fn selections(
    checked: &CheckedCompilation,
    path: &str,
    owner: PackageKeyIdentity,
) -> Vec<language_semantics::declaration_selection::AuthoredDeclarationSelection> {
    checked.authored_declaration_selections().iter().filter(|selection| matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
        if checked.symbols.display_path(target.selected_symbol(), "::") == path && checked.symbols.symbol_package_identity(target.selected_symbol()) == Some(owner))).copied().collect()
}

fn compile(root: &Path, inputs: PackageCompilationInputs) -> CheckedCompilation {
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("domain index fixture checks")
}

fn identity(marker: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([marker; 32]).unwrap()
}

fn root_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "root",
            root.to_path_buf(),
        )],
        Vec::new(),
    )
    .unwrap()
}

struct Sources(PathBuf);

impl Sources {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-module-domain-indices-{}-{}",
            std::process::id(),
            NEXT_TREE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn package(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
    fn write(path: impl AsRef<Path>, source: &str) {
        fs::write(path, source).unwrap();
    }
}

impl Drop for Sources {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
