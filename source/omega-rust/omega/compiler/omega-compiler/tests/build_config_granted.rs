use omega_compiler::{compile_to_checked, compile_to_checked_with_packages_in_sponsored_build_dir};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_checked_interpreter::FilesystemSponsor;
use psi_core::PackageKeyIdentity;
use std::path::{Path, PathBuf};

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(label: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("omega-build-facet-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).expect("create build-facet project");
        Self { root }
    }

    fn write(&self, relative: &str, source: &str) {
        std::fs::write(self.root.join(relative), source).expect("write build-facet fixture");
    }

    fn main(&self) -> PathBuf {
        self.root.join("main.omg")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        set_canonical_source_tree_permissions(&self.root, false);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn set_canonical_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(root).expect("inspect package source fixture");
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal package source directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate package source fixture") {
            set_canonical_source_tree_permissions(
                &entry.expect("read package source entry").path(),
                sealed,
            );
        }
        if sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                .expect("seal package source directory");
        }
    } else {
        let mode = if sealed { 0o444 } else { 0o644 };
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode))
            .expect("set package source file permissions");
    }
}

#[cfg(not(unix))]
fn set_canonical_source_tree_permissions(_root: &Path, _sealed: bool) {}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
    PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "build-facet", root.to_path_buf())
                .with_canonical_source_metadata()
                .expect("capture canonical package source"),
        ],
        Vec::new(),
    )
    .expect("single-package build-facet input")
}

#[test]
fn compiler_owned_source_and_output_facets_publish_generated_source() {
    let profile = omega_target::TargetProfile::host();
    let project = Project::new("generated-source");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write("input.txt", "input\n");
    project.write(
        "build.omg",
        &format!(
            r#"target {target} {{}}

machine build(builder: &mut Build) {{
    builder.application("build-facet-generated-source");
    let input: BuildPath = builder.source.resolve("input.txt");
    let input_descriptor: i32 = builder.source.open(input, 0);
    let mut input_bytes: [u8; 6];
    let input_count: i64 = builder.source.read(input_descriptor, &mut input_bytes, 6);
    let input_close: i32 = builder.source.close(input_descriptor);

    let generated: BuildPath = builder.output.resolve("generated.omg");
    let output_descriptor: i32 = builder.output.create(generated, 438);
    let output_count: i64 = builder.output.write(
        output_descriptor,
        "data Generated {{ base: Main; }}\n"
    );
    let output_close: i32 = builder.output.close(output_descriptor);
    builder.output.include_source(generated);
}}
"#,
            target = profile.target_name(),
        ),
    );

    let session =
        std::env::temp_dir().join(format!("omega-build-facet-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create build session");
    let session = std::fs::canonicalize(session).expect("canonicalize build session");
    let sponsor = FilesystemSponsor::new(&session).expect("create build sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind build output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare build output root");
    std::fs::create_dir(&build_dir).expect("create build output root");
    prepared_build_dir
        .commit()
        .expect("commit build output root");
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.main(),
        &build_dir,
        Some(profile.target_name()),
        package_inputs(&project.root),
        sponsor,
    )
    .expect("compiler-owned Build facets should execute and publish generated source");
    set_canonical_source_tree_permissions(&project.root, false);

    let generated = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated source reaches final checked program");
    let main = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("authored source remains in final checked program");
    let [psi_typed_trees::data::DataMember::Field(base)] = checked.typed.data_members(generated)
    else {
        panic!("Generated has one nominal field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .typed
        .type_reference_table
        .type_reference(base.type_reference)
    else {
        panic!("Generated.base remains nominal")
    };
    assert_eq!(*symbol, main.symbol);
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("facet execution retains observations")
            .filesystem_operation_attempts()
            .len(),
        6,
        "open/read/close and create/write/close execute once; handoff is separate custody"
    );
    checked
        .verify_current_source_consumption()
        .expect("generated bytes remain tied to staged-output custody");
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn generated_local_instance_collection_preserves_build_symbol_and_source_custody() {
    let profile = omega_target::TargetProfile::host();
    let project = Project::new("generated-local-instance");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        &format!(
            r#"target {target} {{}}

machine build(builder: &mut Build) {{
    builder.application("build-facet-generated-local-instance");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(
        descriptor,
        "data Cell<T [copy]> [copy] {{ values: [T; 2]; }}\ndata Pair<A, B> {{ first: A; second: B; }}\ndata Outer<T [copy]> [copy] {{ inner: Cell<T>; direct: T; }}\ndata Maybe<T> {{ case #1 None; case #2 Some(#1 value: T, retired #3); retired #4; }}\ndata Borrowed<'scope, T> {{ value: &'scope T; }}\ndata NestedBorrow<'scope, T> {{ value: Borrowed<'scope, T>; }}\ndata WithBorrow<'scope> {{ value: Borrowed<'scope, u32>; }}\ndata WithNestedBorrow<'scope> {{ value: NestedBorrow<'scope, u32>; }}\ndata LifetimeBox<'boxed, T> {{ value: T; }}\ndata LifetimeOuter<'outer, T> {{ value: T; }}\ndata WithLifetimeTypeArgument<'call> {{ value: LifetimeOuter<'call, LifetimeBox<'call, Borrowed<'call, u32>>>; }}\ndata ConstBlock<T, const N: u64> {{ values: [T; N]; }}\ndata NestedConst<T, const N: u64> {{ value: ConstBlock<T, N>; }}\ndata WithConst {{ value: NestedConst<u16, 2>; }}\ndata BoolFlag<const ENABLED: bool> {{ marker: u8; }}\ndata NestedBool<const ENABLED: bool> {{ value: BoolFlag<ENABLED>; }}\ndata WithBool {{ value: NestedBool<true>; }}\ndata StructuredConfig {{ count: u8; enabled: bool; }}\ndata StructuredConfigs {{}}\nconst StructuredConfigs::PRIMARY: StructuredConfig = StructuredConfig {{ count: 7, enabled: true }};\ndata StructuredIndexed<const C: StructuredConfig> {{ marker: u8; }}\ndata StructuredNested<const C: StructuredConfig> {{ value: StructuredIndexed<C>; }}\ndata WithStructured {{ value: StructuredNested<StructuredConfigs::PRIMARY>; }}\ndata StructuredMode {{ case Left(value: u8); case Right; }}\ndata StructuredModes {{}}\nconst StructuredModes::LEFT: StructuredMode = StructuredMode::Left {{ value: 9 }};\ndata StructuredByMode<const M: StructuredMode> {{ marker: u8; }}\ndata WithStructuredMode {{ value: StructuredByMode<StructuredModes::LEFT>; }}\ndata Item [copy] {{ value: u8; }}\ndata Generated {{ first: Cell<u32>; second: Cell<u32>; pair: Pair<u16, u64>; outer: Outer<u32>; maybe: Maybe<u32>; nominal: Cell<Item>; base: Main; }}\ndata More {{ indirect: [Cell<Item>; 2]; repeated: Pair<u16, u64>; nested: Outer<u32>; }}\n"
    );
    let close: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}}
"#,
            target = profile.target_name(),
        ),
    );

    let session = std::env::temp_dir().join(format!(
        "omega-build-facet-local-instance-session-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create build session");
    let session = std::fs::canonicalize(session).expect("canonicalize build session");
    let sponsor = FilesystemSponsor::new(&session).expect("create build sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind build output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare build output root");
    std::fs::create_dir(&build_dir).expect("create build output root");
    prepared_build_dir
        .commit()
        .expect("commit build output root");
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.main(),
        &build_dir,
        Some(profile.target_name()),
        package_inputs(&project.root),
        sponsor,
    )
    .expect("the exact generated local instance should continue from the retained frontend");
    set_canonical_source_tree_permissions(&project.root, false);

    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("selected build machine remains in the checked program");
    assert_eq!(checked.selected_build_machine_symbol(), Some(build.symbol));
    let template = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("generated template");
    let [template_parameter] = checked.typed.data_type_parameters(template) else {
        panic!("Cell retains its one Type parameter")
    };
    assert_eq!(
        template_parameter.bounds.multiplicity,
        psi_language_semantics::Multiplicity::Unrestricted,
        "the seeded continuation must retain the declared [copy] bound"
    );
    assert_eq!(
        template.properties.multiplicity,
        psi_language_semantics::Multiplicity::Unrestricted,
        "the generated template retains its [copy] data property"
    );
    let instances = checked
        .typed
        .data_definitions()
        .iter()
        .filter(|definition| definition.generic_instance.is_some())
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 16, "sixteen deduplicated closed instances");
    let instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "Cell<u32>")
        .expect("selected Cell<u32> instance");
    assert_eq!(
        instance.properties, template.properties,
        "the closed instance retains the exact bounded template properties"
    );
    let maybe_instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "Maybe<u32>")
        .expect("selected Maybe<u32> instance");
    assert_eq!(maybe_instance.retired_identities, [4]);
    let [
        psi_typed_trees::data::DataMember::Variant(none),
        psi_typed_trees::data::DataMember::Variant(some),
    ] = checked.typed.data_members(maybe_instance)
    else {
        panic!("Maybe<u32> retains its two exact cases")
    };
    assert_eq!(none.identity, Some(1));
    assert_eq!(some.identity, Some(2));
    assert_eq!(some.retired_payload_identities, [3]);
    let [payload] = checked.typed.data_payload_fields(some) else {
        panic!("Maybe<u32>::Some retains its payload")
    };
    assert_eq!(payload.identity, Some(1));
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(payload.type_reference),
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "u32"
    ));
    let borrowed_instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "Borrowed<u32>")
        .expect("selected Borrowed<u32> instance");
    assert_eq!(
        borrowed_instance
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["scope"]
    );
    let [psi_typed_trees::data::DataMember::Field(borrowed_value)] =
        checked.typed.data_members(borrowed_instance)
    else {
        panic!("Borrowed<u32> retains its reference field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(borrowed_value.type_reference)
    else {
        panic!("Borrowed<u32>.value remains a reference")
    };
    assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("scope"));
    assert!(matches!(
        checked.typed.type_reference_table.type_reference(*referee),
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "u32"
    ));
    let nested_borrow_instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "NestedBorrow<u32>")
        .expect("selected NestedBorrow<u32> instance");
    let assert_erased_application = |owner: &psi_typed_trees::data::DataDefinition,
                                     expected_base,
                                     expected_lifetime| {
        let [psi_typed_trees::data::DataMember::Field(field)] = checked.typed.data_members(owner)
        else {
            panic!("{} retains its one field", owner.name.as_str())
        };
        let psi_typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = checked
            .typed
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            panic!(
                "{} retains its erased-lifetime application",
                owner.name.as_str()
            )
        };
        assert_eq!(*base_symbol, expected_base, "{}", owner.name.as_str());
        assert_eq!(
            lifetime_arguments
                .iter()
                .map(|argument| argument.as_str())
                .collect::<Vec<_>>(),
            [expected_lifetime],
            "{}",
            owner.name.as_str()
        );
        assert!(arguments.is_empty(), "{}", owner.name.as_str());
    };
    let find_data = |name: &str| {
        checked
            .typed
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    assert_erased_application(find_data("WithBorrow"), borrowed_instance.symbol, "scope");
    assert_erased_application(nested_borrow_instance, borrowed_instance.symbol, "scope");
    assert_erased_application(
        find_data("WithNestedBorrow"),
        nested_borrow_instance.symbol,
        "scope",
    );
    let lifetime_box_instance = find_data("LifetimeBox<Borrowed<u32>>");
    let lifetime_outer_instance = find_data("LifetimeOuter<LifetimeBox<Borrowed<u32>>>");
    assert_eq!(
        lifetime_box_instance
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["boxed"],
        "the inner instance retains its template binder rather than a use-site spelling"
    );
    assert_eq!(
        lifetime_outer_instance
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["outer"],
        "the outer instance retains its template binder rather than a use-site spelling"
    );
    assert_erased_application(lifetime_box_instance, borrowed_instance.symbol, "boxed");
    assert_erased_application(
        lifetime_outer_instance,
        lifetime_box_instance.symbol,
        "outer",
    );
    assert_erased_application(
        find_data("WithLifetimeTypeArgument"),
        lifetime_outer_instance.symbol,
        "call",
    );
    let const_block_template = find_data("ConstBlock");
    let [_, const_parameter] = checked.typed.data_type_parameters(const_block_template) else {
        panic!("ConstBlock retains its Type and const binders")
    };
    let psi_typed_trees::data::TypeParameterKind::Const { type_reference } = const_parameter.kind
    else {
        panic!("ConstBlock.N remains a const binder")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(type_reference),
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "u64"
    ));
    let const_block_instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "ConstBlock<u16, 2>")
        .expect("selected ConstBlock<u16, 2> instance");
    let [psi_typed_trees::data::DataMember::Field(values)] =
        checked.typed.data_members(const_block_instance)
    else {
        panic!("ConstBlock<u16, 2> retains its values field")
    };
    let psi_typed_trees::types::TypeReferenceNode::FixedArray {
        element_type,
        length,
    } = checked
        .typed
        .type_reference_table
        .type_reference(values.type_reference)
    else {
        panic!("ConstBlock<u16, 2>.values remains an array")
    };
    assert_eq!(
        *length,
        psi_typed_trees::types::FixedArrayLength::Literal(2)
    );
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(*element_type),
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "u16"
    ));
    let const_origin = const_block_instance
        .generic_instance
        .expect("const instance retains its exact origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = checked
        .typed
        .type_reference_table
        .type_reference(const_origin)
    else {
        panic!("const instance origin remains structural")
    };
    let [_, const_argument] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("const instance origin retains two arguments")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(*const_argument),
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if !symbol.is_valid() && name.as_str() == "2"
    ));
    let nested_const_instance = instances
        .iter()
        .copied()
        .find(|definition| definition.name.as_str() == "NestedConst<u16, 2>")
        .expect("selected NestedConst<u16, 2> instance");
    let assert_named_field = |owner: &psi_typed_trees::data::DataDefinition, expected_symbol| {
        let [psi_typed_trees::data::DataMember::Field(field)] = checked.typed.data_members(owner)
        else {
            panic!("{} retains its one field", owner.name.as_str())
        };
        assert!(matches!(
            checked
                .typed
                .type_reference_table
                .type_reference(field.type_reference),
            psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                if *symbol == expected_symbol
        ));
    };
    assert_named_field(nested_const_instance, const_block_instance.symbol);
    assert_named_field(find_data("WithConst"), nested_const_instance.symbol);
    let bool_flag_template = find_data("BoolFlag");
    let [bool_parameter] = checked.typed.data_type_parameters(bool_flag_template) else {
        panic!("BoolFlag retains its Boolean const binder")
    };
    let psi_typed_trees::data::TypeParameterKind::Const { type_reference } = bool_parameter.kind
    else {
        panic!("BoolFlag.ENABLED remains a const binder")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(type_reference),
        psi_typed_trees::types::TypeReferenceNode::Named { name, .. }
            if name.as_str() == "bool"
    ));
    let instance_for_template = |template_symbol| {
        instances
            .iter()
            .copied()
            .find(|definition| {
                definition.generic_instance.is_some_and(|origin| {
                    matches!(
                        checked.typed.type_reference_table.type_reference(origin),
                        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
                            if *base_symbol == template_symbol
                    )
                })
            })
            .unwrap_or_else(|| panic!("missing instance for {template_symbol:?}"))
    };
    let bool_flag_instance = instance_for_template(bool_flag_template.symbol);
    let bool_origin = bool_flag_instance
        .generic_instance
        .expect("Boolean instance retains its origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = checked
        .typed
        .type_reference_table
        .type_reference(bool_origin)
    else {
        panic!("Boolean instance origin remains structural")
    };
    let [bool_argument] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("Boolean instance origin retains one argument")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(*bool_argument),
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if !symbol.is_valid()
                && psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
                    == Some(psi_language_semantics::const_value::CanonicalConstValue::boolean(true))
    ));
    let nested_bool_instance = instance_for_template(find_data("NestedBool").symbol);
    assert_named_field(nested_bool_instance, bool_flag_instance.symbol);
    assert_named_field(find_data("WithBool"), nested_bool_instance.symbol);
    assert_eq!(
        checked.typed.const_declarations().len(),
        2,
        "the two private structured support consts retain exact provenance"
    );
    let structured_indexed_instance = instance_for_template(find_data("StructuredIndexed").symbol);
    let structured_origin = structured_indexed_instance
        .generic_instance
        .expect("structured record instance retains its origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = checked
        .typed
        .type_reference_table
        .type_reference(structured_origin)
    else {
        panic!("structured record instance origin remains structural")
    };
    let [structured_argument] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("structured record instance origin retains one argument")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } = checked
        .typed
        .type_reference_table
        .type_reference(*structured_argument)
    else {
        panic!("structured record argument remains one compiler-owned atom")
    };
    assert!(!symbol.is_valid());
    let structured_value =
        psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
            .expect("structured record atom decodes");
    assert!(matches!(
        structured_value.decode_encoding(),
        Some(psi_language_semantics::const_value::DecodedCanonicalConstValue::Record {
            type_name,
            fields,
        }) if type_name == "StructuredConfig"
            && fields.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>()
                == ["count", "enabled"]
    ));
    let structured_nested_instance = instance_for_template(find_data("StructuredNested").symbol);
    assert_named_field(
        structured_nested_instance,
        structured_indexed_instance.symbol,
    );
    assert_named_field(
        find_data("WithStructured"),
        structured_nested_instance.symbol,
    );

    let structured_mode_instance = instance_for_template(find_data("StructuredByMode").symbol);
    let structured_mode_origin = structured_mode_instance
        .generic_instance
        .expect("structured sum instance retains its origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } = checked
        .typed
        .type_reference_table
        .type_reference(structured_mode_origin)
    else {
        panic!("structured sum instance origin remains structural")
    };
    let [structured_mode_argument] = checked
        .typed
        .type_reference_table
        .type_reference_handles(*arguments)
    else {
        panic!("structured sum instance origin retains one argument")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } = checked
        .typed
        .type_reference_table
        .type_reference(*structured_mode_argument)
    else {
        panic!("structured sum argument remains one compiler-owned atom")
    };
    assert!(!symbol.is_valid());
    let structured_mode_value =
        psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
            .expect("structured sum atom decodes");
    assert!(matches!(
        structured_mode_value.decode_encoding(),
        Some(psi_language_semantics::const_value::DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        }) if type_name == "StructuredMode"
            && case_name == "Left"
            && fields.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>() == ["value"]
    ));
    assert_named_field(
        find_data("WithStructuredMode"),
        structured_mode_instance.symbol,
    );
    let wrapper = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    let origin = instance.generic_instance.expect("retained instance origin");
    let psi_typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        arguments,
        ..
    } = checked.typed.type_reference_table.type_reference(origin)
    else {
        panic!("closed instance retains its generic origin")
    };
    assert_eq!(*base_symbol, template.symbol);
    assert_eq!(
        checked
            .typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .len(),
        1
    );
    assert_eq!(
        checked
            .typed
            .data_members(wrapper)
            .iter()
            .filter(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return false;
                };
                matches!(
                    checked
                        .typed
                        .type_reference_table
                        .type_reference(field.type_reference),
                    psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                        if *symbol == instance.symbol
                )
            })
            .count(),
        2,
        "repeated application spellings deduplicate to one selected instance"
    );
    checked
        .verify_current_source_consumption()
        .expect("normalized continuation retains the exact generated bytes and package custody");
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn generated_base_owned_type_application_graph_preserves_build_symbol_and_source_custody() {
    let profile = omega_target::TargetProfile::host();
    let project = Project::new("generated-base-instance");
    project.write(
        "main.omg",
        "data Cell<T> { value: T; }\ndata Pair<A, B> { first: A; second: B; }\ndata Main { value: u8; }\n",
    );
    project.write(
        "build.omg",
        &format!(
            r#"target {target} {{}}

machine build(builder: &mut Build) {{
    builder.application("build-facet-generated-base-instance");
    let generated: BuildPath = builder.output.resolve("generated.omg");
    let descriptor: i32 = builder.output.create(generated, 438);
    let count: i64 = builder.output.write(
        descriptor,
        "data Generated {{ first: Cell<u32>; second: Cell<u64>; nested: Pair<Cell<u32>, u64>; indirect: [Cell<u16>; 2]; base: Main; }}\ndata AlsoGenerated {{ only: Cell<u8>; }}\n"
    );
    let close: i32 = builder.output.close(descriptor);
    builder.output.include_source(generated);
}}
"#,
            target = profile.target_name(),
        ),
    );

    let session = std::env::temp_dir().join(format!(
        "omega-build-facet-base-instance-session-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create build session");
    let session = std::fs::canonicalize(session).expect("canonicalize build session");
    let sponsor = FilesystemSponsor::new(&session).expect("create build sponsor");
    let build_dir = session.join("output");
    let bound_build_dir = sponsor
        .bind_path(&build_dir)
        .expect("bind build output root");
    let prepared_build_dir = sponsor
        .prepare_create_directory(&bound_build_dir)
        .expect("prepare build output root");
    std::fs::create_dir(&build_dir).expect("create build output root");
    prepared_build_dir
        .commit()
        .expect("commit build output root");
    set_canonical_source_tree_permissions(&project.root, true);
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.main(),
        &build_dir,
        Some(profile.target_name()),
        package_inputs(&project.root),
        sponsor,
    )
    .expect("the generated base-owned type graph should continue from the retained frontend");
    set_canonical_source_tree_permissions(&project.root, false);

    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("selected build machine remains in the checked program");
    assert_eq!(checked.selected_build_machine_symbol(), Some(build.symbol));
    let template = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Cell")
        .expect("retained base template");
    let wrapper = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("generated wrapper");
    let pair = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Pair")
        .expect("retained two-parameter base template");
    assert_eq!(
        checked
            .typed
            .data_members(wrapper)
            .iter()
            .filter(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return false;
                };
                matches!(
                    checked
                        .typed
                        .type_reference_table
                        .type_reference(field.type_reference),
                    psi_typed_trees::types::TypeReferenceNode::Generic { .. }
                )
            })
            .count(),
        3,
    );
    assert!(checked.typed.data_members(wrapper).iter().any(|member| {
        let psi_typed_trees::data::DataMember::Field(field) = member else {
            return false;
        };
        matches!(
            checked
                .typed
                .type_reference_table
                .type_reference(field.type_reference),
            psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, arguments, .. }
                if *base_symbol == pair.symbol
                    && checked.typed.type_reference_table.type_reference_handles(*arguments).len() == 2
        )
    }));
    let also_generated = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "AlsoGenerated")
        .expect("second generated wrapper");
    let [psi_typed_trees::data::DataMember::Field(only)] =
        checked.typed.data_members(also_generated)
    else {
        panic!("second generated wrapper retains one field")
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(only.type_reference),
        psi_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
            if *base_symbol == template.symbol
    ));
    checked
        .verify_current_source_consumption()
        .expect("seeded continuation retains exact generated bytes and package custody");
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn authored_build_path_shape_has_no_compiler_root_authority() {
    let profile = omega_target::TargetProfile::host();
    let project = Project::new("forged-path");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        &format!(
            r#"target {target} {{}}

machine build(builder: &mut Build) {{
    builder.application("forged-build-path");
    let forged: BuildPath = BuildPath {{}};
    let descriptor: i32 = builder.source.open(forged, 0);
}}
"#,
            target = profile.target_name(),
        ),
    );

    let diagnostics = compile_to_checked(&project.main(), Some(profile.target_name()))
        .expect_err("an authored BuildPath shape must not carry compiler root authority");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("must come from BuildSource::resolve or BuildOutput::resolve"),
        "{rendered}"
    );
}

#[test]
fn runtime_boundary_service_is_not_build_authority() {
    let profile = omega_target::TargetProfile::host();
    let project = Project::new("runtime-boundary");
    project.write("main.omg", "data Main { value: u8; }\n");
    project.write(
        "build.omg",
        &format!(
            r#"use omega::language::std::console;

target {target} {{}}

machine build(builder: &mut Build)
reaches Console
{{
    builder.application("runtime-boundary-build");
    let mut console: Console;
    console.write_line("ordinary runtime boundary");
}}
"#,
            target = profile.target_name(),
        ),
    );

    let diagnostics = compile_to_checked(&project.main(), Some(profile.target_name()))
        .expect_err("runtime boundary services must not be admitted for build execution");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("build.omg may not reach runtime boundary services")
            && rendered.contains("compiler-owned Build facets"),
        "{rendered}"
    );
}
