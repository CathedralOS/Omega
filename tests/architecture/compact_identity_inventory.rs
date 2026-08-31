//! Repository-wide guard for compact fingerprint declarations and accessors.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("architecture crate lives under tests/architecture")
        .to_path_buf()
}

fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("read repository entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
}

fn compact_fingerprint_declaration(line: &str) -> Option<&str> {
    let line = line.trim();
    let declaration = if let Some(declaration) = line.strip_prefix("pub ") {
        declaration
    } else if let Some(rest) = line.strip_prefix("pub(") {
        rest.split_once(')')?.1.trim_start()
    } else {
        line
    };
    let (name, ty) = declaration.split_once(':')?;
    let name = name.trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    let ty = ty.trim_start();
    let ty = ty.split_once("//").map_or(ty, |(ty, _)| ty);
    let contains_compact_u64 = contains_standalone_u64(ty);
    if (name.ends_with("fingerprint") || name.ends_with("fingerprints")) && contains_compact_u64 {
        Some(name)
    } else {
        None
    }
}

fn contains_standalone_u64(text: &str) -> bool {
    text.match_indices("u64").any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + 3..].chars().next();
        let is_identifier = |character: char| character.is_ascii_alphanumeric() || character == '_';
        before.is_none_or(|character| !is_identifier(character))
            && after.is_none_or(|character| !is_identifier(character))
    })
}

fn compact_fingerprint_accessor(declaration: &str) -> Option<&str> {
    let function = declaration.find("fn ")?;
    let after_function = &declaration[function + 3..];
    let name_end = after_function.find('(')?;
    let name = after_function[..name_end].trim();
    if !name.contains("fingerprint")
        || name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    let (_, return_and_tail) = after_function[name_end..].split_once("->")?;
    let return_type = return_and_tail
        .split(['{', ';'])
        .next()
        .unwrap_or(return_and_tail);
    contains_standalone_u64(return_type).then_some(name)
}

fn function_declarations(source: &str) -> Vec<String> {
    let mut declarations = Vec::new();
    let mut pending = None::<String>;
    for line in source.lines() {
        let trimmed = line.trim();
        if pending.is_none() && !trimmed.contains("fn ") {
            continue;
        }
        let declaration = pending.get_or_insert_with(String::new);
        if !declaration.is_empty() {
            declaration.push(' ');
        }
        declaration.push_str(trimmed);
        if trimmed.contains('{') || trimmed.ends_with(';') {
            declarations.push(pending.take().expect("pending function declaration"));
        }
    }
    declarations
}

#[test]
fn compact_fingerprint_scanner_covers_wrapped_and_collection_u64_fields() {
    for declaration in [
        "pub fingerprint: u64,",
        "pub(crate) fingerprint: Option<u64>,",
        "fingerprint: u64,",
        "pub fingerprints: Vec<u64>,",
        "pub fingerprints: [u64; 2],",
        "pub fingerprint: Box<[u64]>,",
    ] {
        assert_eq!(
            compact_fingerprint_declaration(declaration),
            Some(if declaration.contains("fingerprints:") {
                "fingerprints"
            } else {
                "fingerprint"
            }),
            "scanner missed compact field `{declaration}`",
        );
    }
    assert_eq!(
        compact_fingerprint_declaration("pub fingerprint: [u8; 32], // not u64"),
        None,
    );
    assert_eq!(
        compact_fingerprint_declaration("fn helper(fingerprint: u64) {}"),
        None,
    );
}

#[test]
fn compact_fingerprint_scanner_covers_accessor_return_types() {
    for declaration in [
        "pub const fn fingerprint(&self) -> u64 {",
        "fn artifact_fingerprint(&self) -> Option<u64>;",
        "pub fn report_fingerprint(\n    &self,\n) -> u64\n{",
    ] {
        assert!(
            function_declarations(declaration)
                .iter()
                .any(|function| compact_fingerprint_accessor(function).is_some()),
            "scanner missed compact accessor `{declaration}`",
        );
    }
    assert_eq!(
        compact_fingerprint_accessor("pub fn fingerprint(&self) -> [u8; 32] {"),
        None,
    );
}

fn explicitly_non_authoritative(name: &str) -> bool {
    [
        "report",
        "compatibility",
        "cache",
        "discriminator",
        "index",
        "informational",
        "non_authoritative",
    ]
    .iter()
    .any(|classification| name.contains(classification))
}

#[test]
fn every_u64_fingerprint_declaration_requires_explicit_classification() {
    let root = workspace_root();
    let source_root = root.join("source/omega-rust");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);

    // This is a shrinking migration ceiling, not an approval list. Each row is
    // already tracked by CLASSIFY-AND-HARDEN-AUTHORITATIVE-IDENTITIES. A rename
    // to explicit report/cache vocabulary removes it; no path may add another
    // occurrence or introduce a new unclassified private or exported
    // declaration.
    let legacy_maximums = BTreeMap::<&str, usize>::new();
    let mut observed = BTreeMap::<String, usize>::new();
    for path in sources {
        let relative = path.strip_prefix(&root).expect("source is below workspace");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for line in source.lines() {
            let Some(field) = compact_fingerprint_declaration(line) else {
                continue;
            };
            if explicitly_non_authoritative(field) {
                continue;
            }
            let key = format!("{}:{field}", relative.display());
            *observed.entry(key).or_default() += 1;
        }
    }

    let unexpected = observed
        .iter()
        .filter(|(key, count)| {
            legacy_maximums
                .get(key.as_str())
                .is_none_or(|max| *count > max)
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "compact fingerprints must be named as report/cache/compatibility data or gain exact/strong authority replay; unexpected declarations: {unexpected:#?}",
    );
    let stale_or_overstated = legacy_maximums
        .iter()
        .filter(|(key, maximum)| observed.get(**key) != Some(maximum))
        .collect::<Vec<_>>();
    assert!(
        stale_or_overstated.is_empty(),
        "the legacy compact-fingerprint ceiling must shrink in the same change that classifies a field; stale or overstated rows: {stale_or_overstated:#?}",
    );
}

#[test]
fn every_u64_fingerprint_accessor_requires_explicit_classification() {
    let root = workspace_root();
    let source_root = root.join("source/omega-rust");
    let mut sources = Vec::new();
    collect_rust_sources(&source_root, &mut sources);

    // This ceiling records accessor spellings still awaiting classification.
    // It may only shrink: new compact-returning fingerprint accessors must use
    // explicit report/cache/compatibility vocabulary or return strong/exact
    // authority instead.
    let legacy_maximums = BTreeMap::<&str, usize>::new();
    let mut observed = BTreeMap::<String, usize>::new();
    for path in sources {
        let relative = path.strip_prefix(&root).expect("source is below workspace");
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for declaration in function_declarations(&source) {
            let Some(accessor) = compact_fingerprint_accessor(&declaration) else {
                continue;
            };
            if explicitly_non_authoritative(accessor) {
                continue;
            }
            let key = format!("{}:{accessor}", relative.display());
            *observed.entry(key).or_default() += 1;
        }
    }

    let unexpected = observed
        .iter()
        .filter(|(key, count)| {
            legacy_maximums
                .get(key.as_str())
                .is_none_or(|max| *count > max)
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "compact fingerprint accessors must be named as report/cache/compatibility data or return exact/strong authority; unexpected accessors: {unexpected:#?}",
    );
    let stale_or_overstated = legacy_maximums
        .iter()
        .filter(|(key, maximum)| observed.get(**key) != Some(maximum))
        .collect::<Vec<_>>();
    assert!(
        stale_or_overstated.is_empty(),
        "the legacy compact-accessor ceiling must shrink in the same change that classifies an accessor; stale or overstated rows: {stale_or_overstated:#?}",
    );
}

#[test]
fn checked_machine_contract_compact_coordinates_are_reports_beside_strong_authority() {
    let root = workspace_root();
    let plans_path = root.join(
        "source/omega-rust/psi/representations/psi-checked-trees/src/facts/contract_plans.rs",
    );
    let plans = fs::read_to_string(&plans_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", plans_path.display()));
    assert!(
        plans.contains("pub report_fingerprint: u64")
            && plans.contains("pub contract_report_fingerprint: u64")
            && plans.contains("pub commitment: MachineContractCommitment")
            && plans.contains("pub contract_commitment: MachineContractCommitment")
            && plans.contains("contract.commitment.is_zero()")
            && !plans.contains("pub fingerprint: u64")
            && !plans.contains("pub contract_fingerprint: u64"),
        "checked contract plans must label compact coordinates as reports and reject empty strong commitments",
    );

    let terminal_path =
        root.join("source/omega-rust/psi/representations/psi-checked-trees/src/flow/terminal.rs");
    let terminal = fs::read_to_string(&terminal_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", terminal_path.display()));
    for required in [
        "pub target_contract_report_fingerprint: u64",
        "pub cleanup_contract_report_fingerprint: u64",
        "pub contract_report_fingerprint: u64",
        "pub contract_commitment: crate::MachineContractCommitment",
        "pub contract_owner: SymbolHandle",
    ] {
        assert!(
            terminal.contains(required),
            "checked Terminal carrier is missing `{required}`"
        );
    }
    assert!(!terminal.contains("pub target_contract_fingerprint: u64"));
    assert!(!terminal.contains("pub cleanup_contract_fingerprint: u64"));
    assert!(!terminal.contains("pub contract_fingerprint: u64"));

    let attached_path = root
        .join("source/omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/attached_unit.rs");
    let attached = fs::read_to_string(&attached_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", attached_path.display()));
    assert!(
        attached.contains("crash_capsule(target.contract_owner, target.state)")
            && !attached.contains("target.contract_commitment.is_zero()).then_some"),
        "boundary lowering must rejoin exact canonical contract ownership rather than self-authenticate its stored digest",
    );
}

#[test]
fn machine_specialization_compact_coordinate_is_report_only_beside_strong_authority() {
    let root = workspace_root();
    let typed_path =
        root.join("source/omega-rust/psi/representations/psi-typed-trees/src/typed_trees.rs");
    let typed = fs::read_to_string(&typed_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", typed_path.display()));
    assert!(
        typed.contains("pub report_fingerprint: u64")
            && typed.contains("pub commitment: MachineSpecializationCommitment")
            && typed.contains("pub struct MachineSpecializationCommitment([u8; 32])")
            && typed.contains("pub template_contract_report_fingerprint: u64")
            && typed.contains("pub template_contract_commitment: MachineTemplateCommitment")
            && typed.contains("pub struct MachineTemplateCommitment([u8; 32])")
            && !typed.contains("pub fingerprint: u64"),
        "machine specializations must label their compact coordinate as a report and retain a strong commitment",
    );

    let lowering_path = root.join(
        "source/omega-rust/psi/pipeline/psi-checked-trees-to-terminal/src/evidence_lowering.rs",
    );
    let lowering = fs::read_to_string(&lowering_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", lowering_path.display()));
    assert!(
        lowering.contains("specialization.commitment.is_zero()")
            && lowering.contains("specialization.commitment.as_bytes()"),
        "Terminal evidence identity must replay the strong specialization commitment rather than the report coordinate",
    );
}

#[test]
fn provider_grants_and_persisted_trust_admissions_retain_strong_exact_authority() {
    let root = workspace_root();
    let grants_path =
        root.join("source/omega-rust/omega/build/omega-trust-model/src/provider_grants.rs");
    let grants = fs::read_to_string(&grants_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", grants_path.display()));
    for required in [
        "pub selected_plan: ProviderPlan",
        "pub selected_plan_digest: ProviderPlanDigest",
        "pub selected_plan_report_identity: u64",
        "self.selected_plan_digest == plan.identity_digest()",
        "self.selected_plan == *plan",
    ] {
        assert!(
            grants.contains(required),
            "provider-grant custody is missing `{required}`"
        );
    }
    assert!(!grants.contains("pub selected_plan_identity: u64"));

    let admissions_path =
        root.join("source/omega-rust/omega/build/omega-trust-model/src/admissions.rs");
    let admissions = fs::read_to_string(&admissions_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", admissions_path.display()));
    assert!(
        admissions.contains("pub struct TrustAdmissionDigest([u8; 32])")
            && admissions.contains("report_identity: Option<u64>")
            && admissions.contains("omega.trust-admission.v1\\0")
            && admissions.contains("Self::ProviderPlan => b\"provider-plan\"")
            && admissions.contains("Self::MachineTemplate => b\"machine-template\"")
            && admissions.contains("Self::MachineContract => b\"machine-contract\"")
            && admissions.contains("(self.commitment.as_str(), self.digest)")
            && !admissions.contains("\n    identity: u64,"),
        "owner admission must compare human commitment plus strong subject digest and exclude compact reports from authority",
    );

    let ledger_path = root.join("source/omega-rust/omega/build/omega-trust-ledger/src/custody.rs");
    let ledger = fs::read_to_string(&ledger_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", ledger_path.display()));
    assert!(
        ledger.contains("digest_text.len() == 16")
            && ledger.contains("legacy 16-hex compact admission row")
            && ledger.contains("digest_text.len() != 64")
            && ledger.contains("TrustAdmissionDigest::from_digest(digest)"),
        "the persisted trust ledger must reject compact legacy authority and parse full strong digests",
    );
}

#[test]
fn private_authority_carriers_retain_strong_subject_commitments() {
    let root = workspace_root();

    let access_path = root.join("source/omega-rust/psi/foundation/psi-access-plans/src/lib.rs");
    let access = fs::read_to_string(&access_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", access_path.display()));
    assert!(
        access.contains("struct AccessLayoutCommitment([u8; 32])")
            && access.contains("layout_report_fingerprint: u64")
            && access.contains("layout_commitment: AccessLayoutCommitment")
            && access.contains("key.layout_commitment != self.layout_commitment"),
        "access field keys must rejoin their exact issuing layout rather than a compact coordinate",
    );

    let checked_path = root.join(
        "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs",
    );
    let checked = fs::read_to_string(&checked_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", checked_path.display()));
    assert!(
        checked.contains("contract_report_fingerprint: u64")
            && checked.contains("contract_commitment: crate::MachineContractCommitment")
            && checked.contains("self.contract_commitment != envelope.contract_commitment()"),
        "checked callback resource receipts must retain the exact selected machine contract",
    );

    let stack_path = root.join(
        "source/omega-rust/omega/representations/omega-calling-conventions/src/stack_realizations.rs",
    );
    let stack = fs::read_to_string(&stack_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", stack_path.display()));
    let roots_path = root.join(
        "source/omega-rust/omega/backend/runtime/omega-external-roots/src/epoch_stack_demand.rs",
    );
    let roots = fs::read_to_string(&roots_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", roots_path.display()));
    assert!(
        stack.contains("pub boundary_plan_report_fingerprint: u64")
            && stack.contains("pub boundary_plan_commitment: [u8; 32]")
            && roots.contains("boundary_contract_report_fingerprint: u64")
            && roots.contains("boundary_contract_commitment: [u8; 32]")
            && roots.contains("boundary.contract_commitment_digest()"),
        "external-root stack settlement must bind the exact boundary plan beside compact reports",
    );
}

#[test]
fn checked_nominal_machine_use_reports_retain_strong_contract_and_plan_authority() {
    let root = workspace_root();
    let checked_path = root.join(
        "source/omega-rust/psi/representations/psi-checked-trees/src/facts/nominal_machine_uses.rs",
    );
    let checked = fs::read_to_string(&checked_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", checked_path.display()));
    assert!(
        checked.contains("pub contract_report_fingerprint: u64")
            && checked.contains("pub published_requirement_report_fingerprint: u64")
            && checked.contains("pub selected_actual_report_fingerprint: u64")
            && checked.contains("pub boundary_calling_plan_report_fingerprint: u64")
            && checked.contains("pub boundary_calling_plan_commitment:")
            && checked.contains("pub contract_commitment: crate::MachineContractCommitment")
            && !checked.contains("pub boundary_calling_plan_fingerprint: u64"),
        "checked nominal-use compact coordinates must be reports beside strong contract and plan commitments",
    );

    let planning_path = root
        .join("source/omega-rust/omega/build/omega-provider-planning/src/calling_policy_plans.rs");
    let planning = fs::read_to_string(&planning_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", planning_path.display()));
    assert!(
        planning.contains("validated.contract_commitment_digest()")
            && planning
                .contains("placement.boundary_calling_plan_commitment != realized_commitment")
            && planning.contains("realization.exact_boundary_entry_plan() != validated.plan()")
            && planning.contains("published_contract.target_contract_commitment()")
            && planning.contains("placement.boundary_calling_plan_commitment == old_commitment")
            && planning.contains("a compact-equal strong-plan substitution must reject"),
        "nominal callback binding must replay exact plan custody and the strong commitment",
    );
}

#[test]
fn checked_operator_provider_reports_retain_strong_plan_authority() {
    let root = workspace_root();
    let checked_path =
        root.join("source/omega-rust/psi/representations/psi-checked-trees/src/operators.rs");
    let checked = fs::read_to_string(&checked_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", checked_path.display()));
    assert!(
        checked.contains("pub provider_plan_report_fingerprint: u64")
            && checked.contains("pub provider_plan_commitment: CheckedProviderPlanCommitment")
            && checked.contains("pub struct CheckedProviderPlanCommitment([u8; 32])")
            && !checked.contains("pub provider_plan_identity: u64"),
        "checked operator uses must classify compact plan coordinates as reports beside exact commitments",
    );

    let planning_path =
        root.join("source/omega-rust/omega/build/omega-provider-planning/src/plans.rs");
    let planning = fs::read_to_string(&planning_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", planning_path.display()));
    assert!(
        planning.contains("*plan.identity_digest().as_bytes()")
            && planning.contains("operator_use.provider_plan_commitment = commitment"),
        "provider selection must copy the exact selected plan commitment into checked operator evidence",
    );

    for relative in [
        "source/omega-rust/omega/build/omega-selected-dispatch/src/operator_adapter.rs",
        "source/omega-rust/omega/build/omega-selected-dispatch/src/float_intrinsic.rs",
    ] {
        let path = root.join(relative);
        let dispatch = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let direct_join = dispatch.contains(
            "plan.identity_digest().as_bytes() == operator_use.provider_plan_commitment.as_bytes()",
        );
        let centralized_join = dispatch
            .contains("plan.identity_digest().as_bytes() == commitment.as_bytes()")
            && dispatch.contains("operator_use.provider_plan_commitment");
        assert!(
            (direct_join || centralized_join)
                && dispatch.contains("without an exact commitment")
                && dispatch.contains("exact commitment that does not match"),
            "selected operator dispatch must join on the strong plan commitment in {}",
            path.display(),
        );
    }
}

#[test]
fn residual_identity_named_compact_hashes_are_explicit_reports() {
    let root = workspace_root();
    let cases = [
        (
            "source/omega-rust/psi/foundation/psi-layout-plans/src/lib.rs",
            &["pub schema_identity: u64"][..],
        ),
        (
            "source/omega-rust/omega/representations/omega-effects/src/selected_provider_plans.rs",
            &["plan_by_identity"][..],
        ),
        (
            "source/omega-rust/omega/representations/omega-target/src/uefi_system_table.rs",
            &["fn layout_identity(&self) -> u64"][..],
        ),
        (
            "source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs",
            &["normalized_foreign_locator_identity"][..],
        ),
        (
            "source/omega-rust/omega/build/omega-provider-planning/src/task_plans.rs",
            &[
                "fn stack_representation_identity",
                "fn signature_layout_identity",
                "fn entry_identity(",
                "fn calling_plan_identity",
            ][..],
        ),
        (
            "source/omega-rust/omega/representations/omega-calling-conventions/src/callback_materializations.rs",
            &["fn callback_nominal_identity"][..],
        ),
        (
            "source/omega-rust/omega/build/omega-provider-planning/src/calling_policy_plans.rs",
            &["fn callback_plan_identity"][..],
        ),
    ];
    for (relative, forbidden) in cases {
        let path = root.join(relative);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for spelling in forbidden {
            assert!(
                !source.contains(spelling),
                "compact hash `{spelling}` in {} must use explicit report/cache/discriminator vocabulary",
                path.display(),
            );
        }
    }
}

#[test]
fn trust_tooling_compact_coordinates_retain_strong_evidence_and_report_labels() {
    let root = workspace_root();
    let carrier_path = root.join("source/omega-rust/omega/tooling/omega-artifacts/src/lib.rs");
    let report_path =
        root.join("source/omega-rust/omega/tooling/omega-artifacts/src/trust_report.rs");
    let carrier = fs::read_to_string(&carrier_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", carrier_path.display()));
    let report = fs::read_to_string(&report_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", report_path.display()));
    let visualization_path =
        root.join("source/omega-rust/omega/tooling/omega-visualizations/src/checked_trees.rs");
    let visualization = fs::read_to_string(&visualization_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", visualization_path.display()));

    for required in [
        "provider_plan_report_fingerprint: u64",
        "provider_plan_digest: omega_effects::provider_plan::ProviderPlanDigest",
        "template_report_fingerprint: u64",
        "instance_report_fingerprint: u64",
        "instance_contract_report_fingerprint: u64",
        "instance_contract_commitment: psi_checked_trees::MachineContractCommitment",
        "machine_contract_report_fingerprint: Option<u64>",
        "machine_contract_commitment: Option<psi_checked_trees::MachineContractCommitment>",
        "machine_template_report_fingerprint: Option<u64>",
        "machine_argument_contract_report_fingerprints: Vec<u64>",
        "conformance_argument_report_fingerprints: Vec<u64>",
        "selected_provider_closure_report_fingerprint: u64",
        "selected_provider_closure_digest: omega_effects::SelectedProviderClosureDigest",
    ] {
        assert!(
            carrier.contains(required),
            "missing trust evidence field `{required}`"
        );
    }
    assert!(report.contains("selected provider closure report fingerprint:"));
    assert!(report.contains("selected provider closure digest:"));
    assert!(report.contains("plan report fingerprint:"));
    assert!(report.contains("plan digest:"));
    assert!(report.contains("instance contract commitment:"));
    assert!(report.contains("machine contract report fingerprint:"));
    assert!(report.contains("machine contract commitment:"));
    for required in [
        "selected_provider_closure_report_fingerprint",
        "selected_provider_closure_digest",
        "specialization_report_fingerprint",
        "instance_report_fingerprint",
        "instance_contract_report_fingerprint",
        "instance_contract_commitment",
        "template_contract_report_fingerprint",
        "machine_argument_contract_report_fingerprints",
        "machine_argument_contract_commitments",
        "conformance_argument_report_fingerprints",
        "conformance_argument_commitments",
    ] {
        assert!(
            visualization.contains(required),
            "checked-tree visualization is missing `{required}`"
        );
    }
}

#[test]
fn provider_service_calling_plan_reports_retain_strong_commitments() {
    let root = workspace_root();
    let provider_path = root.join(
        "source/omega-rust/omega/representations/omega-effects/src/capabilities/provider_plan.rs",
    );
    let provider = fs::read_to_string(&provider_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", provider_path.display()));
    assert!(provider.contains("pub calling_plan_report_fingerprint: Option<u64>"));
    assert!(
        provider.contains("pub calling_plan_commitment: Option<BoundaryCallingPlanCommitment>")
    );
    assert!(provider.contains("boundary_calling_plan_identity_for_arguments"));
    assert!(provider.contains("self.bytes(&commitment.as_bytes())"));
    assert!(!provider.contains("pub calling_plan_fingerprint: Option<u64>"));
}
