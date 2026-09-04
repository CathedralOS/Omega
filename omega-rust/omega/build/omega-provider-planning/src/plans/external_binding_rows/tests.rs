use super::{
    extract_external_binding_rows, selected_source_boundary_entry_plan,
    settle_external_binding_rows,
};
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
};
use omega_effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::signature::StateSignature;
use psi_typed_trees::trait_definition::TraitDefinition;
use std::sync::Arc;

const PLAN_NAME: &str = "selected::source::plan";
const SCHEMA_NAME: &str = "platform::RootService";
const REQUIREMENT_NAME: &str = "core::RootService";
const METHOD_NAME: &str = "enter";

struct Fixture {
    typed: TypedTrees,
    plans: Vec<ProviderPlan>,
    realizations: Vec<BoundaryCallingPlanRealization>,
    requirement_identity: String,
    expected: BoundaryEntryPlan,
}

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn fixture(inherited_owner: bool) -> Fixture {
    fixture_with_inventory(inherited_owner, 1, 1, true)
}

fn fixture_with_inventory(
    inherited_owner: bool,
    schema_owner_count: u32,
    signature_count: u32,
    requirement_owner_is_boundary: bool,
) -> Fixture {
    let requirement_owner_name = if inherited_owner {
        REQUIREMENT_NAME
    } else {
        SCHEMA_NAME
    };
    let requirement_owner_symbol = symbol(10);
    let requirement_symbol = symbol(20);
    let mut typed = TypedTrees::default();

    let mut requirement_owner = TraitDefinition {
        symbol: requirement_owner_symbol,
        is_boundary: requirement_owner_is_boundary,
        name: Identifier::generated(requirement_owner_name),
        ..Default::default()
    };
    for _ in 0..signature_count {
        typed.push_trait_machine_signature(
            &mut requirement_owner,
            StateSignature {
                symbol: requirement_symbol,
                name: Identifier::generated(METHOD_NAME),
                ..Default::default()
            },
        );
    }
    typed.push_trait_definition(requirement_owner);

    let schema_owner_symbol = if inherited_owner {
        symbol(30)
    } else {
        requirement_owner_symbol
    };
    if inherited_owner {
        for offset in 0..schema_owner_count {
            typed.push_trait_definition(TraitDefinition {
                symbol: if offset == 0 {
                    schema_owner_symbol
                } else {
                    symbol(30 + offset)
                },
                is_boundary: true,
                name: Identifier::generated(SCHEMA_NAME),
                ..Default::default()
            });
        }
    } else {
        for offset in 1..schema_owner_count {
            typed.push_trait_definition(TraitDefinition {
                symbol: symbol(30 + offset),
                is_boundary: true,
                name: Identifier::generated(SCHEMA_NAME),
                ..Default::default()
            });
        }
    }

    let requirement_identity = {
        let owner = typed
            .traits()
            .iter()
            .find(|owner| owner.symbol == requirement_owner_symbol)
            .expect("requirement owner");
        let signature = typed
            .trait_machine_signatures(owner)
            .first()
            .expect("requirement signature");
        typed
            .normalized_trait_requirement_overload_identity(owner, signature)
            .identity()
    };
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature::default(),
    )
    .expect("empty ordinary boundary plan");
    let expected = validated.plan().clone();
    let mut realization = BoundaryCallingPlanRealization {
        boundary_trait: schema_owner_symbol,
        boundary_arguments: Vec::new(),
        requirement_machine: requirement_symbol,
        report_fingerprint: validated.contract_report_fingerprint(),
        commitment: psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
            validated.contract_commitment_digest(),
        ),
        boundary_entry_plan: expected.clone(),
        exact_boundary_entry_plan: expected.clone(),
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_context_closed: false,
        policy_machine: String::new(),
        relationship_span: psi_source::SourceSpan::default(),
        native_parameters: Vec::new(),
        materialized_signature:
            crate::calling_policy_plans::materialized_boundary_signature_from_abi(
                &CallSignature::default(),
            )
            .unwrap(),
    };
    let (_, fingerprint, commitment) = realization
        .replayed_validated_application()
        .expect("fixture application identity");
    realization.report_fingerprint = fingerprint;
    realization.commitment = commitment;
    let method = ServiceMethod {
        name: METHOD_NAME.to_owned(),
        requirement_owner: requirement_owner_name.to_owned(),
        requirement_identity: requirement_identity.clone(),
        calling_plan_report_fingerprint: Some(fingerprint),
        calling_plan_commitment: Some(commitment),
        ..Default::default()
    };
    let plan = ProviderPlan {
        name: PLAN_NAME.to_owned(),
        schema: ServiceSchema {
            trait_name: SCHEMA_NAME.to_owned(),
            trait_package_identity: None,
            methods: vec![method],
        },
        ..Default::default()
    };
    Fixture {
        typed,
        plans: vec![plan],
        realizations: vec![realization],
        requirement_identity,
        expected,
    }
}

fn resolve(fixture: &Fixture, schema_name: &str) -> Result<Option<BoundaryEntryPlan>, String> {
    let plan = fixture
        .plans
        .first()
        .ok_or_else(|| "missing resolved provider plan".to_owned())?;
    selected_source_boundary_entry_plan(
        &fixture.typed,
        &fixture.realizations,
        plan,
        schema_name,
        METHOD_NAME,
        &fixture.requirement_identity,
    )
    .map_err(|diagnostic| diagnostic.message)
}

fn add_bootstrap_row(fixture: &mut Fixture) {
    fixture.plans[0].target = "retained-target".to_owned();
    fixture.plans[0].provider_type = "RetainedProvider".to_owned();
    fixture.plans[0].rows.push(ProviderPlanRow {
        method: METHOD_NAME.to_owned(),
        requirement_identity: fixture.requirement_identity.clone(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::StringBackedImportBootstrap {
            library: "retained-library".to_owned(),
            symbol: "retained-symbol".to_owned(),
        },
    });
}

#[test]
fn checked_surface_settlement_retains_exact_rows_and_preserves_equal_arc_identity() {
    let mut fixture = fixture(false);
    add_bootstrap_row(&mut fixture);
    let expected = extract_external_binding_rows(
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
        &fixture.typed,
    )
    .expect("pre-settlement reference projection");
    let mut retained = Arc::from([]);

    settle_external_binding_rows(
        &mut retained,
        &fixture.typed,
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
    )
    .expect("checked-retained typed projection");
    assert_eq!(retained.as_ref(), expected.as_slice());

    let first = Arc::clone(&retained);
    settle_external_binding_rows(
        &mut retained,
        &fixture.typed,
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
    )
    .expect("identical settlement is a no-op");
    assert!(Arc::ptr_eq(&first, &retained));
}

#[test]
fn empty_settlement_preserves_arc_identity() {
    let fixture = fixture(false);
    let mut retained = Arc::from([]);
    let initial = Arc::clone(&retained);

    settle_external_binding_rows(
        &mut retained,
        &fixture.typed,
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
    )
    .expect("empty selected binding projection");

    assert!(Arc::ptr_eq(&initial, &retained));
    assert!(retained.is_empty());
}

#[test]
fn rejected_settlement_preserves_prior_arc_identity_and_contents() {
    let mut fixture = fixture(false);
    add_bootstrap_row(&mut fixture);
    let mut retained = Arc::from([]);
    settle_external_binding_rows(
        &mut retained,
        &fixture.typed,
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
    )
    .expect("initial exact settlement");
    let accepted = Arc::clone(&retained);
    let retained_contents = retained.to_vec();

    let duplicate = fixture.plans[0].schema.methods[0].clone();
    fixture.plans[0].schema.methods.push(duplicate);
    let diagnostics = settle_external_binding_rows(
        &mut retained,
        &fixture.typed,
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
    )
    .expect_err("duplicate exact schema method must reject before publication");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("binds 2 exact schema methods"))
    );
    assert!(Arc::ptr_eq(&accepted, &retained));
    assert_eq!(retained.as_ref(), retained_contents);
}

#[test]
fn external_abi_rows_derive_from_the_selected_provider_plan() {
    let mut fixture = fixture(false);
    add_bootstrap_row(&mut fixture);

    let rows = extract_external_binding_rows(
        None,
        omega_target::NativeTarget::host(),
        &fixture.plans,
        &fixture.realizations,
        &fixture.typed,
    )
    .expect("selected provider binding should produce one ABI row");
    let [row] = rows.as_slice() else {
        panic!("one selected external ABI row")
    };

    assert_eq!(row.target_name, "retained-target");
    assert_eq!(row.trait_name, SCHEMA_NAME);
    assert_eq!(row.method, METHOD_NAME);
    assert_eq!(row.requirement_identity, fixture.requirement_identity);
    assert_eq!(row.table_type, "RetainedProvider");
    assert_eq!(row.boundary_entry_plan, Some(fixture.expected));
    assert_eq!(
        row.binding,
        omega_calling_conventions::ExternalBindingKind::StringBackedImportBootstrap {
            module: "retained-library".to_owned(),
            symbol: "retained-symbol".to_owned(),
        }
    );
}

#[test]
fn external_top_level_requirement_extracts_its_exact_carrier_abi() {
    let source = r#"
        pub data Counter [copy] { value: u64; }
        pub data LinuxCounter {}

        pub boundary requirement Counter::write(self, value: u64);

        machine LinuxCounter::write(counter: Counter, value: u64)
        satisfies Counter::write
        via Binding::Syscall(1);
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize top-level external ABI fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse top-level external ABI fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve top-level external ABI fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type top-level external ABI fixture");
    psi_typed_trees_to_checked_trees::lower_typed_trees(typed.clone())
        .expect("check exact top-level external satisfier");
    let plans = crate::plans::derive_satisfies_plans(&typed, Some("linux_x86_64"));
    let [plan] = plans.as_slice() else {
        panic!("one top-level external provider plan")
    };

    let rows = extract_external_binding_rows(
        Some("linux_x86_64"),
        omega_target::NativeTarget::linux_x64(),
        std::slice::from_ref(plan),
        &[],
        &typed,
    )
    .expect("selected top-level external satisfier should enter ABI extraction");
    let [row] = rows.as_slice() else {
        panic!("one top-level external ABI row")
    };
    assert_eq!(row.trait_name, "Counter::write");
    assert_eq!(row.method, "write");
    assert!(matches!(
        row.binding,
        omega_calling_conventions::ExternalBindingKind::Syscall { number: 1 }
    ));
    let entry = row
        .boundary_entry_plan
        .as_ref()
        .expect("Linux syscall extraction requires a closed calling plan");
    assert_eq!(
        entry.call.parameters.len(),
        2,
        "the requirement's semantic `self` carrier and ordinary value parameter both cross the external ABI",
    );
    assert!(entry.call.result.is_none());
}

#[test]
fn selected_source_boundary_entry_plan_accepts_exact_direct_and_inherited_owners() {
    for inherited in [false, true] {
        let fixture = fixture(inherited);
        assert_eq!(
            resolve(&fixture, SCHEMA_NAME),
            Ok(Some(fixture.expected.clone()))
        );
    }
}

#[test]
fn selected_source_boundary_entry_plan_accepts_exact_operator_custody_without_trait_abi() {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize operator custody fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse operator custody fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve operator custody fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type operator custody fixture");
    let mut plans = crate::plans::derive_satisfies_plans(&typed, None);
    let plan_index = plans
        .iter()
        .position(|plan| {
            plan.schema
                .trait_name
                .starts_with("operator::CheckedMath::offset_zero")
        })
        .expect("exact operator provider plan");
    let method = plans[plan_index].schema.methods[0].clone();

    assert_eq!(
        selected_source_boundary_entry_plan(
            &typed,
            &[],
            &plans[plan_index],
            &plans[plan_index].schema.trait_name,
            &method.name,
            &method.requirement_identity,
        )
        .expect("exact boundary operator custody"),
        None,
        "operator dispatch owns its selected realization; it is not a trait ABI call",
    );

    plans[plan_index].schema.methods[0].requirement_owner = "other::Owner".to_owned();
    let error = selected_source_boundary_entry_plan(
        &typed,
        &[],
        &plans[plan_index],
        &plans[plan_index].schema.trait_name,
        &method.name,
        &method.requirement_identity,
    )
    .expect_err("operator requirement-owner drift must reject");
    assert!(
        error
            .message
            .contains("does not bind exact boundary operator")
    );
}

#[test]
fn selected_source_boundary_entry_plan_rejects_plan_and_schema_drift_exactly() {
    enum Drift {
        EmptyPlanName,
        SchemaName,
        EmptyRequirementIdentity,
        MissingMethod,
        DuplicateMethod,
        EmptyRequirementOwner,
    }
    let cases = [
        (Drift::EmptyPlanName, "empty ProviderPlan name"),
        (Drift::SchemaName, "not exact requested schema"),
        (
            Drift::EmptyRequirementIdentity,
            "empty exact requirement overload identity",
        ),
        (Drift::MissingMethod, "binds 0 exact schema methods"),
        (Drift::DuplicateMethod, "binds 2 exact schema methods"),
        (
            Drift::EmptyRequirementOwner,
            "empty exact requirement owner",
        ),
    ];

    for (drift, expected) in cases {
        let mut fixture = fixture(false);
        let mut schema_name = SCHEMA_NAME;
        match drift {
            Drift::EmptyPlanName => fixture.plans[0].name.clear(),
            Drift::SchemaName => schema_name = "other::RootService",
            Drift::EmptyRequirementIdentity => fixture.requirement_identity.clear(),
            Drift::MissingMethod => {
                fixture.plans[0].schema.methods[0].requirement_identity = "other".to_owned()
            }
            Drift::DuplicateMethod => {
                let duplicate = fixture.plans[0].schema.methods[0].clone();
                fixture.plans[0].schema.methods.push(duplicate);
            }
            Drift::EmptyRequirementOwner => {
                fixture.plans[0].schema.methods[0].requirement_owner.clear()
            }
        }
        let error = resolve(&fixture, schema_name)
            .expect_err("selected source authority drift must reject");
        assert!(
            error.contains(expected),
            "expected `{expected}` in `{error}`"
        );
    }
}

#[test]
fn selected_source_boundary_entry_plan_rejects_typed_custody_drift_exactly() {
    let duplicate_schema = fixture_with_inventory(true, 2, 1, true);
    assert!(
        resolve(&duplicate_schema, SCHEMA_NAME)
            .expect_err("duplicate schema owner")
            .contains("resolves to 2 exact typed boundary traits")
    );

    let duplicate_signature = fixture_with_inventory(true, 1, 2, true);
    assert!(
        resolve(&duplicate_signature, SCHEMA_NAME)
            .expect_err("duplicate signature")
            .contains("binds 2 exact typed signatures")
    );

    let mut duplicate_requirement_owner = fixture(true);
    duplicate_requirement_owner
        .typed
        .push_trait_definition(TraitDefinition {
            symbol: symbol(50),
            is_boundary: true,
            name: Identifier::generated(REQUIREMENT_NAME),
            ..Default::default()
        });
    assert!(
        resolve(&duplicate_requirement_owner, SCHEMA_NAME)
            .expect_err("duplicate requirement owner")
            .contains("resolves to 2 exact typed traits")
    );

    let mut missing_owner = fixture(true);
    missing_owner.plans[0].schema.methods[0].requirement_owner = "missing::Owner".to_owned();
    assert!(
        resolve(&missing_owner, SCHEMA_NAME)
            .expect_err("missing requirement owner")
            .contains("resolves to 0 exact typed traits")
    );

    let mut missing_signature = fixture(true);
    missing_signature
        .typed
        .push_trait_definition(TraitDefinition {
            symbol: symbol(51),
            is_boundary: true,
            name: Identifier::generated("empty::Owner"),
            ..Default::default()
        });
    missing_signature.plans[0].schema.methods[0].requirement_owner = "empty::Owner".to_owned();
    assert!(
        resolve(&missing_signature, SCHEMA_NAME)
            .expect_err("missing typed signature")
            .contains("binds 0 exact typed signatures")
    );

    let non_boundary = fixture_with_inventory(true, 1, 1, false);
    assert!(
        resolve(&non_boundary, SCHEMA_NAME)
            .expect_err("non-boundary requirement owner")
            .contains("is not an exact boundary trait")
    );

    let mut missing_schema = fixture(true);
    missing_schema.plans[0].schema.trait_name = "missing::Schema".to_owned();
    assert!(
        resolve(&missing_schema, "missing::Schema")
            .expect_err("missing schema owner")
            .contains("resolves to 0 exact typed boundary traits")
    );

    let mut cross_owner = fixture(true);
    let mut other_owner = TraitDefinition {
        symbol: symbol(60),
        is_boundary: true,
        name: Identifier::generated("other::Owner"),
        ..Default::default()
    };
    cross_owner.typed.push_trait_machine_signature(
        &mut other_owner,
        StateSignature {
            symbol: symbol(61),
            name: Identifier::generated(METHOD_NAME),
            ..Default::default()
        },
    );
    cross_owner.typed.push_trait_definition(other_owner);
    let other_identity = {
        let owner = cross_owner
            .typed
            .traits()
            .iter()
            .find(|owner| owner.symbol == symbol(60))
            .expect("other owner");
        cross_owner
            .typed
            .normalized_trait_requirement_overload_identity(
                owner,
                &cross_owner.typed.trait_machine_signatures(owner)[0],
            )
            .identity()
    };
    cross_owner.plans[0].schema.methods[0].requirement_owner = "other::Owner".to_owned();
    cross_owner.plans[0].schema.methods[0].requirement_identity = other_identity.clone();
    cross_owner.requirement_identity = other_identity;
    assert!(
        resolve(&cross_owner, SCHEMA_NAME)
            .expect_err("cross-owner realization")
            .contains("resolves to 0 exact calling-plan realizations")
    );
}

#[test]
fn selected_source_boundary_entry_plan_rejects_realization_drift_exactly() {
    enum Drift {
        Missing,
        Duplicate,
        Fingerprint,
        Commitment,
        CollisionEqualPlanSubstitution,
        SchemaOwner,
        Requirement,
        ZeroFingerprint,
    }
    let cases = [
        (
            Drift::Missing,
            "resolves to 0 exact calling-plan realizations",
        ),
        (
            Drift::Duplicate,
            "resolves to 2 exact calling-plan realizations",
        ),
        (
            Drift::Fingerprint,
            "resolves to 0 exact calling-plan realizations",
        ),
        (
            Drift::Commitment,
            "resolves to 0 exact calling-plan realizations",
        ),
        (
            Drift::CollisionEqualPlanSubstitution,
            "substituted a calling plan behind its compact report fingerprint",
        ),
        (
            Drift::SchemaOwner,
            "resolves to 0 exact calling-plan realizations",
        ),
        (
            Drift::Requirement,
            "resolves to 0 exact calling-plan realizations",
        ),
        (Drift::ZeroFingerprint, "zero calling-plan fingerprint"),
    ];
    for (drift, expected) in cases {
        let mut fixture = fixture(true);
        match drift {
            Drift::Missing => fixture.realizations.clear(),
            Drift::Duplicate => fixture.realizations.push(fixture.realizations[0].clone()),
            Drift::Fingerprint => fixture.realizations[0].report_fingerprint ^= 1,
            Drift::Commitment => {
                fixture.realizations[0].commitment =
                    psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
                        [9; 32],
                    )
            }
            Drift::CollisionEqualPlanSubstitution => {
                fixture.realizations[0].boundary_entry_plan =
                    evaluate_ordinary_boundary_entry_plan(
                        CallingPolicy::SystemVAMD64,
                        &CallSignature::default(),
                    )
                    .expect("alternate valid boundary plan")
                    .plan()
                    .clone();
            }
            Drift::SchemaOwner => fixture.realizations[0].boundary_trait = symbol(90),
            Drift::Requirement => fixture.realizations[0].requirement_machine = symbol(91),
            Drift::ZeroFingerprint => {
                fixture.plans[0].schema.methods[0].calling_plan_report_fingerprint = Some(0)
            }
        }
        let error = resolve(&fixture, SCHEMA_NAME).expect_err("realization drift must reject");
        assert!(
            error.contains(expected),
            "expected `{expected}` in `{error}`"
        );
    }
}

#[test]
fn selected_source_boundary_entry_plan_allows_none_only_for_exact_absent_fingerprint() {
    let mut fixture = fixture(true);
    fixture.plans[0].schema.methods[0].calling_plan_report_fingerprint = None;
    fixture.plans[0].schema.methods[0].calling_plan_commitment = None;
    fixture.realizations.clear();
    assert_eq!(resolve(&fixture, SCHEMA_NAME), Ok(None));

    fixture.plans[0].schema.methods[0].requirement_owner = "missing::Owner".to_owned();
    assert!(
        resolve(&fixture, SCHEMA_NAME)
            .expect_err("missing owner cannot enter compatibility fallback")
            .contains("resolves to 0 exact typed traits")
    );
}
