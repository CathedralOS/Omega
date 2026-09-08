use compiler::{CheckedCompilation, compile_to_checked_with_packages};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

static NEXT_TREE: AtomicU64 = AtomicU64::new(0);
const BUFFER: &str = "pub data Buffer<const N: u64> { value: [u8; N]; }";

#[test]
fn ambiguous_qualified_constant_index_cannot_fall_back_to_root_spelling() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use combat; const combat::SIZE: u64 = 1; {BUFFER}
         data Main {{ value: Buffer<combat::SIZE>; }}"
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("ambiguous qualified index cannot select the lexical constant map");
}

#[test]
fn nested_constant_indices_preserve_each_live_use_exposure() {
    let tree = Sources::new();
    let root = tree.package("root");
    let declarations = format!("use combat; {BUFFER} pub data Wrap<T> {{ value: T; }}");
    let private_use = "data Private { value: Wrap<Buffer<combat::SIZE>>; }";
    let public_use = "pub data Public { value: Wrap<Buffer<combat::SIZE>>; }";
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!("{declarations} {private_use}"),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("private nested use does not inherit public template exposure");
    assert_buffer_instances(&checked, &[2]);
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
    assert!(!has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PublicInterface
    ));

    Sources::write(
        root.join("main.omg"),
        &format!("{declarations} {private_use} {public_use}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("a deduplicated nested instance cannot hide its public live use");
    Sources::write(
        root.join("combat.omg"),
        "module combat; pub const SIZE: u64 = 2;",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("public index permits both independently exposed nested uses");
    assert_buffer_instances(&checked, &[2]);
    for exposure in [
        AuthoredDeclarationSelectionExposure::PrivateImplementation,
        AuthoredDeclarationSelectionExposure::PublicInterface,
    ] {
        assert!(has_selection(
            &checked,
            "combat::SIZE",
            identity(1),
            exposure
        ));
    }
}

struct Sources(PathBuf);

impl Sources {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-module-indices-{}-{}",
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

fn assert_buffer_instances(checked: &CheckedCompilation, expected_lengths: &[usize]) {
    let template = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Buffer")
        .unwrap();
    let instances = checked
        .typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            definition.generic_instance.is_some_and(|origin| matches!(
            checked.typed.type_reference_table.type_reference(origin),
            TypeReferenceNode::Generic { base_symbol, .. } if *base_symbol == template.symbol
        ))
        })
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), expected_lengths.len());
    let mut lengths = Vec::new();
    for (position, instance) in instances.iter().enumerate() {
        assert!(
            instances[..position]
                .iter()
                .all(|prior| prior.symbol != instance.symbol)
        );
        let field = checked
            .typed
            .data_members(instance)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => Some(field),
                _ => None,
            })
            .unwrap();
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } = checked
            .typed
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            panic!("closed Buffer field must have an exact literal extent")
        };
        let TypeReferenceNode::Generic { arguments, .. } = checked
            .typed
            .type_reference_table
            .type_reference(instance.generic_instance.unwrap())
        else {
            unreachable!()
        };
        let [argument] = checked
            .typed
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            panic!("Buffer has one retained canonical index")
        };
        assert!(
            matches!(checked.typed.type_reference_table.type_reference(*argument),
            TypeReferenceNode::Named { symbol, name } if !symbol.is_valid() && name.as_str() == length.to_string())
        );
        lengths.push(*length);
    }
    lengths.sort_unstable();
    assert_eq!(lengths, expected_lengths);
}

fn has_selection(
    checked: &CheckedCompilation,
    path: &str,
    owner: PackageKeyIdentity,
    exposure: AuthoredDeclarationSelectionExposure,
) -> bool {
    checked
        .authored_declaration_selections()
        .iter()
        .any(|selection| {
            matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
            if checked.symbols.display_path(target.selected_symbol(), "::") == path
                && checked.symbols.symbol_package_identity(target.selected_symbol()) == Some(owner))
                && selection.exposure() == exposure
        })
}

#[test]
fn qualified_and_local_indices_keep_distinct_canonical_instances_and_array_lengths() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2; data Local { value: Buffer<SIZE>; }",
    );
    Sources::write(
        root.join("rooms.omg"),
        "module rooms; const SIZE: u64 = 3; data Local { value: Buffer<SIZE>; }",
    );
    for imports in ["use combat; use rooms;", "use rooms; use combat;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} {BUFFER} const SIZE: u64 = 1;
             data Root {{ value: Buffer<SIZE>; }}
             data Combat {{ value: Buffer<combat::SIZE>; }}
             data Rooms {{ value: Buffer<rooms::SIZE>; }}"
            ),
        );
        let checked = compile_to_checked_with_packages(
            &root.join("main.omg"),
            None,
            root_inputs(&root),
        )
        .expect("qualified module indices select distinct constants rather than the root shadow");
        assert_buffer_instances(&checked, &[1, 2, 3]);
        for path in ["SIZE", "combat::SIZE", "rooms::SIZE"] {
            assert!(
                has_selection(
                    &checked,
                    path,
                    identity(1),
                    AuthoredDeclarationSelectionExposure::PrivateImplementation
                ),
                "missing {path}"
            );
        }
    }
}

#[test]
fn leaf_imports_and_equal_named_values_share_one_canonical_application() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(root.join("rooms.omg"), "module rooms; const SIZE: u64 = 2;");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use combat::SIZE; use rooms; {BUFFER}
         data First {{ value: Buffer<SIZE>; }}
         data Second {{ value: Buffer<rooms::SIZE>; }}
         data Literal {{ value: Buffer<2>; }}"
        ),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("same canonical values may deduplicate while their selections remain distinct");
    assert_buffer_instances(&checked, &[2]);
    for path in ["combat::SIZE", "rooms::SIZE"] {
        assert!(has_selection(
            &checked,
            path,
            identity(1),
            AuthoredDeclarationSelectionExposure::PrivateImplementation
        ));
    }
}

#[test]
fn named_index_carrier_is_checked_even_for_unused_const_binders() {
    let tree = Sources::new();
    let root = tree.package("root");
    for generic in [BUFFER, "pub data Buffer<const N: u64> { value: u8; }"] {
        Sources::write(
            root.join("main.omg"),
            &format!("use combat::SIZE; {generic} data Main {{ value: Buffer<SIZE>; }}"),
        );
        Sources::write(
            root.join("combat.omg"),
            "module combat; const SIZE: u64 = 2;",
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("matching declared carrier is valid");
        Sources::write(
            root.join("combat.omg"),
            "module combat; const SIZE: u32 = 2;",
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("fitting named u32 is not an anonymous u64 const argument");
    }
}

#[test]
fn public_index_exposure_cannot_erase_private_constant_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!("use combat; {BUFFER} data Private {{ value: Buffer<combat::SIZE>; }}"),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root)).unwrap();
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
    Sources::write(
        root.join("main.omg"),
        &format!("use combat; {BUFFER} pub data Public {{ value: Buffer<combat::SIZE>; }}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("normalization cannot publish a private named index");
    Sources::write(
        root.join("combat.omg"),
        "module combat; pub const SIZE: u64 = 2;",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("public constant closes the public application");
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PublicInterface
    ));
}

#[test]
fn ambiguous_leaf_indices_reject_independent_of_import_order() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, value) in [("combat", 2), ("rooms", 3)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; const SIZE: u64 = {value};"),
        );
    }
    for imports in [
        "use combat::SIZE; use rooms::SIZE;",
        "use rooms::SIZE; use combat::SIZE;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} {BUFFER} data Main {{ value: Buffer<SIZE>; }}"),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("an ambiguous static index cannot choose a same-leaf declaration");
    }
}

#[test]
fn dependency_named_indices_require_direct_reach_and_public_visibility() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::combat; pub machine bridge() -> u64 { leaf::combat::SIZE }",
    );
    Sources::write(
        leaf.join("combat.omg"),
        "module combat; pub const SIZE: u64 = 2;",
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
    Sources::write(
        root.join("main.omg"),
        &format!("use middle::bridge; {BUFFER} data Main {{ value: Buffer<combat::SIZE>; }}"),
    );
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("loaded transitive constants do not grant authored index authority");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let inputs = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!("use leaf::combat::SIZE; {BUFFER} data Main {{ value: Buffer<SIZE>; }}"),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone()).unwrap();
    assert_buffer_instances(&checked, &[2]);
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(3),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
    Sources::write(
        leaf.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("a direct edge cannot publish the dependency's private index");
}

#[test]
fn compound_module_indices_keep_values_and_repeated_authored_selections() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(root.join("rooms.omg"), "module rooms; const SIZE: u64 = 3;");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use combat; use rooms; {BUFFER} const SIZE: u64 = 1;
         data Root {{ value: Buffer<SIZE + 0>; }}
         data Combat {{ value: Buffer<combat::SIZE + 0>; }}
         data Rooms {{ value: Buffer<rooms::SIZE + 0>; }}
         data Next {{ value: Buffer<rooms::SIZE + 1>; }}
         data Repeated {{ value: Buffer<combat::SIZE + combat::SIZE>; }}"
        ),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("compound indices preserve exact selected module values");
    assert_buffer_instances(&checked, &[1, 2, 3, 4]);
    for path in ["SIZE", "combat::SIZE", "rooms::SIZE"] {
        assert!(has_selection(
            &checked,
            path,
            identity(1),
            AuthoredDeclarationSelectionExposure::PrivateImplementation
        ));
    }
    let combat_uses = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
            if checked.symbols.display_path(target.selected_symbol(), "::") == "combat::SIZE")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        combat_uses.len(),
        3,
        "each operand occurrence survives canonical deduplication"
    );
    for (position, selection) in combat_uses.iter().enumerate() {
        assert!(
            combat_uses[..position]
                .iter()
                .all(|prior| prior.source_span() != selection.source_span())
        );
    }
}

#[test]
fn compound_integer_indices_obey_each_typed_operation_boundary() {
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
            format!("use constants::SIZE; pub data Buffer<const N: {carrier}> {{ value: u8; }}");
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} data Main {{ value: Buffer<SIZE + 0>; }}"),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("same-carrier compound control is well formed");
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} data Main {{ value: Buffer<{expression}>; }}"),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err(&format!("unsafe or incompatible {carrier} operation {expression} cannot be folded before checking"));
    }
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u32 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!("use constants::SIZE; {BUFFER} data Main {{ value: Buffer<SIZE + 0>; }}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("compound u32 result is not an anonymous u64 index");
}

#[test]
fn typed_compound_division_differs_from_anonymous_fractional_intermediates() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 3;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use constants::SIZE; {BUFFER}
         data Typed {{ value: Buffer<(SIZE / 2) * 2>; }}
         data Anonymous {{ value: Buffer<(3 / 2) * 2>; }}
         data Mixed {{ value: Buffer<SIZE + ((3 / 2) * 2)>; }}"
        ),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("typed truncation and anonymous rational evaluation retain different meanings");
    assert_buffer_instances(&checked, &[2, 3, 6]);
    Sources::write(
        root.join("main.omg"),
        &format!("use constants::SIZE; {BUFFER} data Main {{ value: Buffer<SIZE + (3 / 2)>; }}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("an anonymous fractional peer cannot land at the typed operand");
}

#[test]
fn compound_index_exposure_keeps_private_constant_authority() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!("use combat; {BUFFER} data Private {{ value: Buffer<combat::SIZE + 1>; }}"),
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("private compound index use");
    assert_buffer_instances(&checked, &[3]);
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
    Sources::write(
        root.join("main.omg"),
        &format!("use combat; {BUFFER} pub data Public {{ value: Buffer<combat::SIZE + 1>; }}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("public compound application cannot erase a private operand");
    Sources::write(
        root.join("combat.omg"),
        "module combat; pub const SIZE: u64 = 2;",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("public compound operand closes interface");
    assert_buffer_instances(&checked, &[3]);
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(1),
        AuthoredDeclarationSelectionExposure::PublicInterface
    ));
}

#[test]
fn compound_dependency_indices_require_direct_public_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::combat; pub machine bridge() -> u64 { leaf::combat::SIZE }",
    );
    Sources::write(
        leaf.join("combat.omg"),
        "module combat; pub const SIZE: u64 = 2;",
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
    Sources::write(
        root.join("main.omg"),
        &format!("use middle::bridge; {BUFFER} data Main {{ value: Buffer<combat::SIZE + 1>; }}"),
    );
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("loaded transitive operand is not admitted for early compound evaluation");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let inputs = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!("use leaf::combat::SIZE; {BUFFER} data Main {{ value: Buffer<SIZE + 1>; }}"),
    );
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone())
        .expect("direct public compound operand");
    assert_buffer_instances(&checked, &[3]);
    assert!(has_selection(
        &checked,
        "combat::SIZE",
        identity(3),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    ));
    Sources::write(
        leaf.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("direct dependency does not authorize its private compound operand");
}

#[test]
fn authored_compound_operator_cannot_be_executed_as_builtin_addition() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let declarations = format!("use constants::SIZE; {BUFFER}");
    Sources::write(
        root.join("main.omg"),
        &format!("{declarations} data Main {{ value: Buffer<SIZE + 1>; }}"),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect("builtin addition control");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} operator + u64::add(left: u64, right: u64) -> u64;
         data Main {{ value: Buffer<SIZE + 1>; }}"
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("unresolved authored addition cannot use token builtin meaning");
}

#[test]
fn private_compound_operands_reject_before_division_is_evaluated() {
    let tree = Sources::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    Sources::write(
        dependency.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use dependency::combat; {BUFFER} data Main {{ value: Buffer<dependency::combat::SIZE / 0>; }}"
        ),
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dependency",
            identity(2),
        )],
    )
    .unwrap();
    let failure = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("private dependency operand cannot reach arithmetic");
    let diagnostic = format!("{failure:?}");
    assert!(diagnostic.contains("private"), "{diagnostic}");
    assert!(
        !diagnostic.contains("division by zero"),
        "authority must precede arithmetic: {diagnostic}"
    );

    Sources::write(
        root.join("combat.omg"),
        "module combat; const SIZE: u64 = 2;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!("use combat; {BUFFER} pub data Public {{ value: Buffer<combat::SIZE / 0>; }}"),
    );
    let failure =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("public exposure of private operand rejects before arithmetic");
    let diagnostic = format!("{failure:?}");
    assert!(diagnostic.contains("private"), "{diagnostic}");
    assert!(
        !diagnostic.contains("division by zero"),
        "authority must precede arithmetic: {diagnostic}"
    );
}

#[test]
fn machine_local_index_cannot_select_a_shadowed_module_constant() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("constants.omg"),
        "module constants; const SIZE: u64 = 2;",
    );
    let declarations = "use constants::SIZE; data Buffer<const N: u64> { value: u8; }";
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine inspect(input: u64) -> u8 {{ let SIZE: u64 = input; let buffer: Buffer<2> = Buffer {{ value: 7 }}; buffer.value }}"
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect("runtime local and literal static application coexist");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine inspect(input: u64) -> u8 {{ let SIZE: u64 = input; let buffer: Buffer<SIZE + 0> = Buffer {{ value: 7 }}; buffer.value }}"
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("data-only constant evaluation cannot replace a runtime lexical local");
}
