//! Complete checked calling policies retain meaning, not replay receipts.

mod support;

use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_calling_policy;
use package_evidence::record::PackagePolicyCallingPlan;
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use support::*;
use symbols::SymbolHandle;
use typed_trees::typed_trees::BoundaryCallingPlanCommitment;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .unwrap()
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        repository_root()
            .join("source/library/std/tests")
            .join(name),
    )
    .unwrap()
}

fn procedure_source() -> String {
    fixture("direct_callback_parameter.omg")
        .split_once("data RegistrarPolicy { }")
        .expect("shared callback canary starts with its complete procedure policy")
        .0
        .to_owned()
}

fn checked(source: &str) -> (TempPackage, CheckedCompilation) {
    let package = TempPackage::new();
    package.write("main.omg", source);
    package.write(
        "calling.omg",
        &fs::read_to_string(repository_root().join("source/library/std/calling.omg")).unwrap(),
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review-fixture\"); }\n",
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x86_64"),
        package_inputs(&package.0),
    )
    .expect("source calling policy should check without native emission");
    (package, checked)
}

fn realization<'a>(
    checked: &'a CheckedCompilation,
    owner: &str,
) -> &'a BoundaryCallingPlanRealization {
    let declaration = checked
        .traits()
        .iter()
        .find(|declaration| declaration.name.as_str() == owner)
        .unwrap();
    let matches = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.boundary_trait == declaration.symbol)
        .collect::<Vec<_>>();
    let [realization] = matches.as_slice() else {
        panic!("one exact checked calling application for {owner}")
    };
    realization
}

fn canonical(policy: &PackagePolicyCallingPlan) -> Vec<u8> {
    let bytes = policy
        .canonical_bytes()
        .expect("encode complete calling policy");
    let recovered =
        PackagePolicyCallingPlan::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .expect("recover complete calling policy without source or replay receipts");
    assert_eq!(&recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    bytes
}

#[test]
fn equal_physical_overloads_keep_distinct_semantic_calling_policy() {
    let source = procedure_source().replace(
        "machine call(message: u64) -> u64;",
        "machine call(message: u64) -> u64;\n    machine call(message: u64) -> u64 in Saturating;",
    );
    let (_package, checked) = checked(&source);
    let declaration = checked
        .traits()
        .iter()
        .find(|declaration| declaration.name.as_str() == "HookProcedure")
        .unwrap();
    let policies = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| realization.boundary_trait == declaration.symbol)
        .map(|realization| project_checked_calling_policy(&checked, realization).unwrap())
        .collect::<Vec<_>>();
    let [first, second] = policies.as_slice() else {
        panic!("both exact result-domain overloads must publish policy")
    };
    assert_eq!(first.boundary_trait(), second.boundary_trait());
    assert_eq!(first.physical(), second.physical());
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_ne!(first.requirement(), second.requirement());
    assert_ne!(first.semantic_result(), second.semantic_result());
    assert_ne!(first, second);
    assert_ne!(canonical(first), canonical(second));
}

#[test]
fn implementation_only_policy_rename_does_not_change_calling_policy() {
    let source = procedure_source();
    let (_first_package, first) = checked(&source);
    let (_second_package, second) =
        checked(&source.replace("HookProcedurePolicy", "RenamedImplementation"));
    let first_realization = realization(&first, "HookProcedure");
    let second_realization = realization(&second, "HookProcedure");
    assert_ne!(
        first_realization.policy_machine,
        second_realization.policy_machine
    );
    let first_policy = project_checked_calling_policy(&first, first_realization).unwrap();
    let second_policy = project_checked_calling_policy(&second, second_realization).unwrap();
    assert_eq!(first_policy, second_policy);
    assert_eq!(canonical(&first_policy), canonical(&second_policy));
    assert!(first_policy.callbacks().binders().is_empty());
    assert!(first_policy.callbacks().demands().is_empty());
    assert!(first_policy.callbacks().materializations().is_empty());
    assert!(first_policy.callbacks().layouts().is_empty());
    assert!(first_policy.opaque_uses().is_empty());
}

#[test]
fn equal_physical_formals_keep_their_declared_native_name() {
    let source = procedure_source();
    let (_first_package, first) = checked(&source);
    let (_second_package, second) =
        checked(&source.replace("machine call(message: u64)", "machine call(renamed: u64)"));
    let first =
        project_checked_calling_policy(&first, realization(&first, "HookProcedure")).unwrap();
    let second =
        project_checked_calling_policy(&second, realization(&second, "HookProcedure")).unwrap();
    assert_eq!(first.physical(), second.physical());
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_ne!(first.native_parameters(), second.native_parameters());
    assert_ne!(first, second);
    assert_ne!(canonical(&first), canonical(&second));
}

#[test]
fn named_private_slot_changes_policy_without_changing_physical_geometry() {
    let source = fixture("callback_materialization_closure.omg");
    let (_first_package, first) = checked(&source);
    let (_second_package, second) = checked(&source.replace(
        "SecondaryWndClassWindowProcedureSlot",
        "AlternateWindowProcedureSlot",
    ));
    let first =
        project_checked_calling_policy(&first, realization(&first, "WindowRegistrar")).unwrap();
    let second =
        project_checked_calling_policy(&second, realization(&second, "WindowRegistrar")).unwrap();
    assert_eq!(first.physical(), second.physical());
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_eq!(first.callbacks().binders().len(), 2);
    assert_eq!(first.callbacks().demands().len(), 2);
    assert_eq!(first.callbacks().materializations().len(), 2);
    assert_eq!(first.callbacks().layouts().len(), 2);
    assert_ne!(first.callbacks(), second.callbacks());
    assert_ne!(first, second);
    assert_ne!(canonical(&first), canonical(&second));
}

#[test]
fn inline_child_callback_paths_retain_full_calling_policy() {
    let source = fixture("callback_materialization_closure.omg")
        .replace(
            "specification: &Spread<ForeignRecord>",
            "specification: &Envelope<Registration>",
        )
        .replace(
            "data Main { }",
            r#"
data Envelope { entries: [FieldEntry; 64]; }
machine Envelope::plan(&mut self, schema: Schema) -> Plan {
    self.entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan {
        entries: self.entries,
        entry_count: 1,
        size_fixed: 32,
        size_is_dynamic: false,
        align: 8,
    }
}
data Registration { callbacks: Spread<ForeignRecord>; }
data Main { }
"#,
        );
    let (_package, checked) = checked(&source);
    let realization = realization(&checked, "WindowRegistrar");
    assert!(
        realization
            .materialized_signature()
            .callback_layout_catalog()
            .iter()
            .all(|entry| entry.inline_field().is_some())
    );
    let policy = project_checked_calling_policy(&checked, realization).unwrap();
    assert_eq!(policy.callbacks().binders().len(), 2);
    assert_eq!(policy.callbacks().demands().len(), 2);
    assert_eq!(policy.callbacks().materializations().len(), 2);
    assert_eq!(policy.callbacks().layouts().len(), 2);
    for layout in policy.callbacks().layouts() {
        assert_eq!(layout.root_layout().policy().path(), "Envelope");
        assert_eq!(layout.root_layout().byte_size(), 32);
        let field = layout.inline_field().expect("retained named inline field");
        assert_eq!(field.field().path(), "Registration::callbacks");
        assert_eq!(field.child_layout().policy().path(), "Spread");
        assert_eq!(field.offset(), 8);
        assert_eq!(field.extent(), 24);
        assert_eq!(layout.composed_offset(), 8 + layout.terminal_offset());
    }
    assert_eq!(policy.native_parameters().len(), 1);
    assert_eq!(policy.semantic_parameters().len(), 1);
    assert!(!canonical(&policy).is_empty());
}

#[test]
fn concrete_boundary_arguments_distinguish_equal_physical_calling_policies() {
    let source = procedure_source().replace(
        "boundary trait HookProcedure:",
        "boundary trait HookProcedure<Tag>:",
    );
    let source = format!(
        r#"{source}
pub data First {{}}
pub data Second {{}}
data Provider {{}}
ProviderHook: Provider satisfies HookProcedure<First>;
machine Provider::call(message: u64) -> u64
    satisfies HookProcedure<First>::call
{{ message }}
"#
    );
    let (_first_package, first) = checked(&source);
    let (_second_package, second) =
        checked(&source.replace("HookProcedure<First>", "HookProcedure<Second>"));
    let first =
        project_checked_calling_policy(&first, realization(&first, "HookProcedure")).unwrap();
    let second =
        project_checked_calling_policy(&second, realization(&second, "HookProcedure")).unwrap();
    assert_eq!(first.physical(), second.physical());
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_eq!(first.boundary_arguments().len(), 1);
    assert_eq!(second.boundary_arguments().len(), 1);
    assert_ne!(first.boundary_arguments(), second.boundary_arguments());
    assert_ne!(first, second);
    assert_ne!(canonical(&first), canonical(&second));
}

#[test]
fn inherited_requirement_retains_declaring_trait_and_concrete_parent_application() {
    let source = procedure_source().replace(
        "boundary trait HookProcedure: Calling<HookProcedurePolicy> {\n    machine call(message: u64) -> u64;\n}",
        "boundary trait ProcedureBase<Value> {\n    machine call(message: Value) -> Value;\n}\n\nboundary trait HookProcedure: ProcedureBase<u64> + Calling<HookProcedurePolicy> {}",
    );
    assert!(source.contains("boundary trait ProcedureBase<Value>"));
    let (_first_package, first) = checked(&source);
    let (_second_package, second) =
        checked(&source.replace("ProcedureBase<u64>", "ProcedureBase<i64>"));
    let first_realization = realization(&first, "HookProcedure");
    let declared_owner = first
        .traits()
        .iter()
        .find(|declaration| declaration.name.as_str() == "ProcedureBase")
        .unwrap();
    assert_ne!(first_realization.boundary_trait, declared_owner.symbol);
    assert!(
        first
            .trait_machine_signatures(declared_owner)
            .iter()
            .any(|signature| signature.symbol == first_realization.requirement_machine)
    );
    let first = project_checked_calling_policy(&first, first_realization).unwrap();
    let second =
        project_checked_calling_policy(&second, realization(&second, "HookProcedure")).unwrap();
    assert_eq!(first.boundary_trait().path(), "HookProcedure");
    assert_eq!(first.requirement_trait().path(), "ProcedureBase");
    assert!(first.boundary_arguments().is_empty());
    let [argument] = first.requirement_arguments() else {
        panic!("one concrete inherited requirement argument")
    };
    assert!(argument.canonical().contains("u64"));
    assert_eq!(first.semantic_parameters()[0].value_type(), argument);
    assert_eq!(first.semantic_result(), Some(argument));
    assert_eq!(first.physical(), second.physical());
    assert_eq!(first.shape_graph(), second.shape_graph());
    assert_eq!(first.boundary_trait(), second.boundary_trait());
    assert_eq!(first.requirement(), second.requirement());
    assert_ne!(
        first.requirement_arguments(),
        second.requirement_arguments()
    );
    assert_ne!(canonical(&first), canonical(&second));
}

#[test]
fn detached_calling_realization_receipts_and_telescope_drift_reject() {
    let (_package, checked) = checked(&fixture("direct_callback_parameter.omg"));
    let original = realization(&checked, "HookRegistrar");
    let policy = project_checked_calling_policy(&checked, original).unwrap();
    assert_eq!(policy.native_parameters().len(), 3);
    assert_eq!(policy.semantic_parameters().len(), 2);
    assert_eq!(policy.callbacks().binders().len(), 1);
    assert_eq!(policy.callbacks().demands().len(), 1);
    assert_eq!(policy.callbacks().materializations().len(), 1);
    assert!(policy.callbacks().layouts().is_empty());
    assert!(!canonical(&policy).is_empty());
    let mutations: &[fn(&mut BoundaryCallingPlanRealization)] = &[
        |realization| realization.boundary_trait = SymbolHandle::invalid(),
        |realization| realization.requirement_machine = SymbolHandle::invalid(),
        |realization| realization.report_fingerprint ^= 1,
        |realization| {
            realization.commitment = BoundaryCallingPlanCommitment::from_digest([0x71; 32])
        },
        |realization| realization.native_parameters.swap(0, 1),
        |realization| realization.callback_binders[0].static_machine_ordinal += 1,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert!(
            project_checked_calling_policy(&checked, &changed).is_err(),
            "detached checked calling application mutation {index}"
        );
    }
    assert_eq!(
        project_checked_calling_policy(&checked, original).unwrap(),
        policy
    );
}
