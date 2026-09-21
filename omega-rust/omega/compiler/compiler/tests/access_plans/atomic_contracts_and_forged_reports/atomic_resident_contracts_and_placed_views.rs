use crate::{POLICY_SOURCE, extent_identity, provider_issuance, write_program};
use access_plans::{
    AtomicCapability, AtomicTransferRule, BoundaryReach, ExternalCapability, FieldAccess,
    PeerWritability, PlacedOccurrenceId, PlacementAdmissionId, ResourceProfile,
    ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion, StableCapability, TransferRule,
    admit_owned_placement, admit_placement, adopt_owned_atomic, place,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use extents::{
    AddressSpaceId, ExtentContentCustodyReceiptId, ExtentContentValidityReceiptId, ExtentLineageId,
    ExtentProvenanceId, ExtentRightId, ExtentRights, ExtentRootGrant, MappingEraId,
    ResidentClaimId,
};
use language_core::atomic::{
    AtomicObservingCompareExchangeOperation, AtomicObservingCompareExchangeResultShape,
};
use language_semantics::Multiplicity;

#[test]
fn checked_atomic_resident_contract_replays_observing_axes_and_result_shapes() {
    let source = POLICY_SOURCE
        .replace("compare_exchange: false", "compare_exchange: true")
        .replace(
            "compare_exchange_once: false",
            "compare_exchange_once: true",
        )
        .replace(
            "data Main {}",
            "machine retain(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
        );
    let main = write_program("placed-atomic-resident-contract", &source);
    let mut checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("copyable resident should retain both observing result contracts")
        .into_program();
    let view_index = checked
        .typed
        .placed_view_plans
        .iter()
        .position(|view| {
            view.fields
                .iter()
                .any(|field| field.field_name == "counter")
        })
        .expect("Registers placed-view plan with its Atomic counter");
    let field_index = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .position(|field| field.field_name == "counter")
        .expect("retained Atomic counter field");
    let field = &checked.typed.placed_view_plans[view_index].fields[field_index];
    let retained = field
        .atomic_resident
        .as_ref()
        .expect("Atomic field retains a checked resident contract");
    assert_eq!(retained.field_symbol, field.field_symbol);
    assert_eq!(retained.resident_type, field.value_type);
    assert_eq!(retained.multiplicity, Multiplicity::Unrestricted);
    assert_eq!(retained.transfer_width_bits, 64);
    assert!(retained.compare_exchange);
    assert!(retained.compare_exchange_once);
    assert_eq!(
        retained
            .observing_results
            .iter()
            .map(|row| (row.operation, row.result_shape))
            .collect::<Vec<_>>(),
        vec![
            (
                AtomicObservingCompareExchangeOperation::Decisive,
                AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved,
            ),
            (
                AtomicObservingCompareExchangeOperation::SingleAttempt,
                AtomicObservingCompareExchangeResultShape::
                    ExchangedOrMismatchedOrUncommittedObserved,
            ),
        ]
    );
    validation::validate_program(&checked.typed)
        .expect("independent replay accepts the exact resident contract");

    for (permission, expected_operation, expected_shape) in [
        (
            "compare_exchange",
            AtomicObservingCompareExchangeOperation::Decisive,
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved,
        ),
        (
            "compare_exchange_once",
            AtomicObservingCompareExchangeOperation::SingleAttempt,
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved,
        ),
    ] {
        let source = POLICY_SOURCE
            .replace(
                &format!("{permission}: false"),
                &format!("{permission}: true"),
            )
            .replace(
                "data Main {}",
                "machine retain(view: &Placed<UartPlacement, Registers>) {}\n\ndata Main {}",
            );
        let main = write_program(&format!("placed-atomic-resident-{permission}"), &source);
        let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
            .expect("each observing permission forms one exact resident/result row");
        let resident = checked
            .typed
            .placed_view_plans
            .iter()
            .flat_map(|view| view.fields.iter())
            .find(|field| field.field_name == "counter")
            .and_then(|field| field.atomic_resident.as_ref())
            .expect("independent observing resident contract");
        assert_eq!(resident.compare_exchange, permission == "compare_exchange");
        assert_eq!(
            resident.compare_exchange_once,
            permission == "compare_exchange_once"
        );
        assert_eq!(resident.observing_results.len(), 1);
        assert_eq!(resident.observing_results[0].operation, expected_operation);
        assert_eq!(resident.observing_results[0].result_shape, expected_shape);
    }

    let original = retained.clone();
    let sibling_symbol = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .find(|field| field.field_name == "status")
        .expect("sibling status field")
        .field_symbol;
    let sibling_type = checked.typed.placed_view_plans[view_index]
        .fields
        .iter()
        .find(|field| field.field_name == "status")
        .expect("sibling status field")
        .value_type;

    let mut reject_drift = |mutated: typed_trees::typed_trees::PlacedAtomicResidentContract,
                            description: &str| {
        checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident =
            Some(mutated);
        let diagnostics = validation::validate_program(&checked.typed)
            .expect_err("resident-contract drift must fail closed");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("changed its checked Atomic resident/result contract")),
            "{description}: {diagnostics:?}"
        );
        checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident =
            Some(original.clone());
    };

    let mut drifted = original.clone();
    drifted.field_symbol = sibling_symbol;
    reject_drift(drifted, "sibling field substitution");
    let mut drifted = original.clone();
    drifted.resident_type = sibling_type;
    reject_drift(drifted, "resident type substitution");
    let mut drifted = original.clone();
    drifted.multiplicity = Multiplicity::Affine;
    reject_drift(drifted, "multiplicity substitution");
    let mut drifted = original.clone();
    drifted.transfer_width_bits = 32;
    reject_drift(drifted, "transfer-width substitution");
    let mut drifted = original.clone();
    drifted.compare_exchange = false;
    reject_drift(drifted, "decisive observing permission-axis substitution");
    let mut drifted = original.clone();
    drifted.compare_exchange_once = false;
    reject_drift(
        drifted,
        "single-attempt observing permission-axis substitution",
    );
    let mut drifted = original.clone();
    drifted.observing_results.push(drifted.observing_results[0]);
    reject_drift(drifted, "duplicate observing result row");
    let mut drifted = original.clone();
    drifted.observing_results.swap(0, 1);
    reject_drift(drifted, "canonical result-row order substitution");
    let mut drifted = original.clone();
    drifted.observing_results[0].operation = AtomicObservingCompareExchangeOperation::SingleAttempt;
    reject_drift(drifted, "observing operation-identity substitution");
    let mut drifted = original.clone();
    drifted.observing_results[1].result_shape =
        AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved;
    reject_drift(drifted, "single-attempt result-shape substitution");

    checked.typed.placed_view_plans[view_index].fields[field_index].atomic_resident = None;
    let diagnostics = validation::validate_program(&checked.typed)
        .expect_err("missing resident contract must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its checked Atomic resident/result contract")
    }));
}

#[test]
fn checked_atomic_resident_contract_joins_provider_backed_runtime_custody() {
    let source = r#"
use omega::language::core::layout;

pub data Counter {
    value: u64;
}

pub data AtomicPlacement {
    entries: [FieldEntry; 64];
    services: [u64; 32];
}

machine AtomicPlacement::plan(&mut self, schema: Schema) -> PlacementPlan {
    let mut owned_entries: [FieldEntry; 64];
    let access: AccessPlan = AccessPlan::inaccessible(&schema);
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 0 }
    };
    PlacementPlan {
        layout: Plan {
            entries: owned_entries,
            entry_count: 1,
            size_fixed: 8,
            size_is_dynamic: false,
            align: 8
        },
        access: access.with(
            schema.fields[0].key,
            FieldAccess::Atomic {
                operations: AtomicOperations {
                    load: true,
                    store: false,
                    fetch_add: false,
                    fetch_sub: false,
                    fetch_xor: false,
                    fetch_or: false,
                    fetch_and: false,
                    swap: false,
                    compare_exchange: true,
                    compare_exchange_once: true,
                    try_exchange: false,
                    try_exchange_once: false
                },
                exposure: Exposure::Exported
            }
        ),
        reach: BoundaryReach {
            services: self.services,
            service_count: 0
        }
    }
}

machine retain_source_plan(counter: &Placed<AtomicPlacement, Counter>) {}

data Main {}
machine Main::main(&mut self) {}
"#;
    let main = write_program("checked-atomic-runtime-resident-join", source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("observing Atomic contract should reach checked custody");
    let view = checked
        .typed
        .placed_view_plans
        .iter()
        .find(|view| view.policy_name == "AtomicPlacement")
        .expect("checked Atomic placement");
    let field = view
        .fields
        .iter()
        .find(|field| field.field_name == "value")
        .expect("checked Atomic field");
    let entry = view
        .placement
        .access()
        .plan()
        .entries()
        .iter()
        .find(|entry| !matches!(entry.access(), FieldAccess::Inaccessible))
        .expect("canonical Atomic access entry");
    let operations = match entry.access() {
        FieldAccess::Atomic { operations, .. } => *operations,
        other => panic!("expected Atomic access, got {other:?}"),
    };

    let rights = ExtentRights::from_normalized_identities([extent_identity(
        451,
        ExtentRightId::from_normalized_identity,
    )]);
    let (extent, content) = ExtentRootGrant::from_admitted_provider(
        provider_issuance(29),
        extent_identity(452, ExtentLineageId::from_normalized_identity),
        extent_identity(453, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(454, ExtentProvenanceId::from_normalized_identity),
        extent_identity(455, MappingEraId::from_normalized_identity),
    )
    .mint_provider_existing_content(
        0xb000,
        8,
        view.placement.content_interpretation(),
        extent_identity(456, ResidentClaimId::from_normalized_identity),
        extent_identity(
            457,
            ExtentContentValidityReceiptId::from_normalized_identity,
        ),
        extent_identity(458, ExtentContentCustodyReceiptId::from_normalized_identity),
    )
    .expect("provider-backed Atomic content");
    let profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(459).expect("profile receipt"),
        &extent,
        rights.clone(),
        BoundaryReach::default(),
    )
    .expect("Atomic profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    },
                    operations,
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("admitted Atomic profile");
    let admission_id =
        PlacementAdmissionId::from_normalized_identity(460).expect("placement admission");
    let admission = admit_owned_placement(admission_id, extent, &view.placement, &profile)
        .expect("owned Atomic placement admission");
    let dormant = adopt_owned_atomic(admission, content).expect("Atomic resident adoption");
    let resident_claim = dormant.resident_claim();
    let occurrence = PlacedOccurrenceId::from_normalized_identity(461).expect("placed occurrence");
    let established = dormant.view(occurrence).expect("Atomic resident view");

    let request_snapshot = |access: &access_plans::AtomicPrimitiveAccessRequest<'_, '_>| {
        let request = access.primitive_request();
        (
            access.operation(),
            request.plan(),
            request.admission(),
            request.effective_supply().key(),
            request.transfer_width_bits(),
            request.resident_claim(),
            request.placed_occurrence(),
        )
    };

    let projection = established
        .project(entry.key())
        .expect("provider-backed Atomic projection");
    let access = projection
        .atomic_compare_exchange_once(
            language_core::atomic::MemoryOrdering::ReceivePublish,
            language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("single-attempt observing access")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("Atomic specialization");
    let snapshot = request_snapshot(&access);

    let rejection = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.policy_symbol,
        field.field_symbol,
        access,
    )
    .expect_err("a policy symbol cannot substitute the exact placed-view identity");
    assert!(
        rejection
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no exact placed-view identity"))
    );
    let (access, diagnostics) = rejection.into_parts();
    assert!(!diagnostics.is_empty());
    assert_eq!(request_snapshot(&access), snapshot);

    let rejection = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        view.policy_symbol,
        access,
    )
    .expect_err("a policy symbol cannot substitute the exact checked field identity");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no exact checked Atomic field identity")
    }));
    let (access, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&access), snapshot);

    let joined = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        access,
    )
    .expect("exact checked/runtime resident join");
    assert_eq!(
        joined.resident_contract(),
        field.atomic_resident.as_ref().expect("resident contract")
    );
    assert_eq!(
        joined.observing_result().operation,
        AtomicObservingCompareExchangeOperation::SingleAttempt
    );
    assert_eq!(
        joined.observing_result().result_shape,
        AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedOrUncommittedObserved
    );
    assert_eq!(
        joined.atomic_access().primitive_request().resident_claim(),
        Some(resident_claim)
    );
    assert_eq!(
        joined
            .atomic_access()
            .primitive_request()
            .placed_occurrence(),
        Some(occurrence)
    );
    joined
        .validate_for_result_custody()
        .expect("post-construction replay preserves both authorities");
    let access = joined.into_atomic_access();
    assert_eq!(request_snapshot(&access), snapshot);

    let mut drifted = checked.clone().into_program();
    let drifted_view_symbol = view.data_symbol;
    let drifted_field_symbol = field.field_symbol;
    {
        let drifted_view = drifted
            .typed
            .placed_view_plans
            .iter_mut()
            .find(|candidate| candidate.data_symbol == drifted_view_symbol)
            .expect("drifted checked view");
        let drifted_field = drifted_view
            .fields
            .iter_mut()
            .find(|candidate| candidate.field_symbol == drifted_field_symbol)
            .expect("drifted checked field");
        drifted_field
            .atomic_resident
            .as_mut()
            .expect("drifted resident contract")
            .observing_results[1]
            .result_shape =
            AtomicObservingCompareExchangeResultShape::ExchangedOrMismatchedObserved;
    }
    let rejection = validation::bind_checked_atomic_resident_access(
        &drifted.typed,
        drifted_view_symbol,
        drifted_field_symbol,
        access,
    )
    .expect_err("result-shape drift must reject before runtime custody handoff");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("changed its checked Atomic resident/result contract")
    }));
    let (access, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&access), snapshot);
    let joined = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        access,
    )
    .expect("unchanged request supports corrected checked-contract retry");
    let _access = joined.into_atomic_access();

    let projection = established
        .project(entry.key())
        .expect("provider-backed Atomic load projection");
    let load = projection
        .atomic_load(language_core::atomic::MemoryOrdering::Receive)
        .expect("resident Atomic load")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("Atomic load specialization");
    let rejection = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        load,
    )
    .expect_err("non-observing Atomic operations cannot consume the result-shape contract");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("accepts only observing decisive or single-attempt")
    }));
    let (load, _) = rejection.into_parts();
    load.validate_for_lowering()
        .expect("rejection returns the unchanged non-observing request");

    let shifted_source = source
        .replace(
            "placement: FieldPlan::At { offset: 0 }",
            "placement: FieldPlan::At { offset: 8 }",
        )
        .replace("size_fixed: 8", "size_fixed: 16");
    let shifted_main = write_program("checked-atomic-runtime-resident-shifted", &shifted_source);
    let shifted = compile_to_checked(CheckedCompileRequest::new(&shifted_main, None))
        .expect("shifted checked Atomic plan");
    let shifted_view = shifted
        .typed
        .placed_view_plans
        .iter()
        .find(|candidate| candidate.policy_name == "AtomicPlacement")
        .expect("shifted checked view");
    let shifted_field = shifted_view
        .fields
        .iter()
        .find(|candidate| candidate.field_name == "value")
        .expect("shifted checked field");
    let projection = established
        .project(entry.key())
        .expect("provider-backed decisive projection");
    let decisive = projection
        .atomic_compare_exchange(
            language_core::atomic::MemoryOrdering::ReceivePublish,
            language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("decisive observing access")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("decisive Atomic specialization");
    let decisive_snapshot = request_snapshot(&decisive);
    let rejection = validation::bind_checked_atomic_resident_access(
        &shifted.typed,
        shifted_view.data_symbol,
        shifted_field.field_symbol,
        decisive,
    )
    .expect_err("a distinct checked placement cannot substitute for runtime custody");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("placement structure differs from the independently checked placement")
    }));
    let (decisive, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&decisive), decisive_snapshot);
    let joined = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        decisive,
    )
    .expect("decisive observing contract joins exact resident custody");
    assert_eq!(
        joined.observing_result().operation,
        AtomicObservingCompareExchangeOperation::Decisive
    );
    let _decisive = joined.into_atomic_access();

    let ordinary_extent = ExtentRootGrant::from_admitted_provider(
        provider_issuance(30),
        extent_identity(462, ExtentLineageId::from_normalized_identity),
        extent_identity(463, AddressSpaceId::from_normalized_identity),
        rights.clone(),
        extent_identity(464, ExtentProvenanceId::from_normalized_identity),
        extent_identity(465, MappingEraId::from_normalized_identity),
    )
    .mint(0xb100, 8)
    .expect("ordinary Atomic extent");
    let ordinary_profile = ResourceProfileGrant::from_admitted_provider(
        ResourceProfileReceiptId::from_normalized_identity(466).expect("ordinary receipt"),
        &ordinary_extent,
        rights,
        BoundaryReach::default(),
    )
    .expect("ordinary Atomic profile grant")
    .admit(ResourceProfile {
        regions: vec![ResourceRegion {
            offset: 0,
            length: 8,
            peer: PeerWritability::Exclusive,
            stable: StableCapability::None,
            external: ExternalCapability::None,
            atomic: AtomicCapability::Access {
                transfers: vec![AtomicTransferRule {
                    transfer: TransferRule {
                        width_bits: 64,
                        alignment_bytes: 8,
                    },
                    operations,
                }],
            },
            reach: BoundaryReach::default(),
        }],
    })
    .expect("ordinary Atomic profile");
    let ordinary_loan = ordinary_extent.loan(0, 8).expect("ordinary Atomic loan");
    let ordinary_view = place(
        admit_placement(
            PlacementAdmissionId::from_normalized_identity(467).expect("ordinary admission"),
            ordinary_loan,
            &view.placement,
            &ordinary_profile,
        )
        .expect("ordinary Atomic placement admission"),
    )
    .expect("ordinary Atomic placed view");
    let ordinary_projection = ordinary_view
        .project(entry.key())
        .expect("ordinary Atomic projection");
    let ordinary = ordinary_projection
        .atomic_compare_exchange_once(
            language_core::atomic::MemoryOrdering::ReceivePublish,
            language_core::atomic::MemoryOrdering::Receive,
        )
        .expect("ordinary observing request")
        .into_primitive_request()
        .into_atomic_primitive_access()
        .expect("ordinary Atomic specialization");
    let ordinary_snapshot = request_snapshot(&ordinary);
    let rejection = validation::bind_checked_atomic_resident_access(
        &checked.typed,
        view.data_symbol,
        field.field_symbol,
        ordinary,
    )
    .expect_err("correspondence-free ordinary Atomic storage has no resident custody");
    assert!(rejection.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lacks runtime resident custody")
    }));
    let (ordinary, _) = rejection.into_parts();
    assert_eq!(request_snapshot(&ordinary), ordinary_snapshot);
    ordinary
        .validate_for_lowering()
        .expect("missing-custody rejection returns the exact Atomic request");
}

#[test]
fn placed_view_exposes_each_individually_admitted_atomic_family() {
    let source = POLICY_SOURCE
        .replace("store: false", "store: true")
        .replace("fetch_sub: false", "fetch_sub: true")
        .replace("fetch_xor: false", "fetch_xor: true")
        .replace("fetch_or: false", "fetch_or: true")
        .replace("fetch_and: false", "fetch_and: true")
        .replace("swap: false", "swap: true")
        .replace("compare_exchange: false", "compare_exchange: true")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let observed: u64 = view.counter.load(NoOrdering);
    view.counter.store(observed, NoOrdering);
    let added: u64 = view.counter.fetch_add(1, NoOrdering);
    let subtracted: u64 = view.counter.fetch_sub(1, NoOrdering);
    let xored: u64 = view.counter.fetch_xor(1, NoOrdering);
    let ored: u64 = view.counter.fetch_or(1, NoOrdering);
    let anded: u64 = view.counter.fetch_and(1, NoOrdering);
    let swapped: u64 = view.counter.swap(1, NoOrdering);
    let exchanged: u64 = view.counter.compare_exchange(1, 2, NoOrdering, NoOrdering);
}

data Main {}
"#,
        );
    let main = write_program("placed-view-atomic-families", &source);
    compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect("each individually admitted atomic operation family should compile");
}

#[test]
fn placed_view_atomic_accessor_cannot_materialize_as_an_ordinary_value() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {
    let leaked: u64 = view.counter;
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic-leak", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("an atomic accessor must not coerce into its carried primitive");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("counter") && rendered.contains("accessor, not an ordinary value"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_cannot_recast_around_its_admitted_policy() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine reinterpret(view: Placed<UartPlacement, Registers>) {
    let alias: &Placed<UartPlacement, Registers> =
        &view as &Placed<UartPlacement, Registers>;
}

data Main {}
"#,
    );
    let main = write_program("placed-view-recast", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("a placed view must not be reconstructed through recast");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("placed-view recast")
            && rendered.contains("explicitly admit the intended placement"),
        "unexpected diagnostic: {rendered}"
    );
}

#[test]
fn placed_view_rejects_atomic_operations_outside_the_plan() {
    let source = POLICY_SOURCE.replace(
        "data Main {}",
        r#"
machine inspect(view: &mut Placed<UartPlacement, Registers>) {
    view.counter.store(1, NoOrdering);
    let subtracted: u64 = view.counter.fetch_sub(1, NoOrdering);
    let xored: u64 = view.counter.fetch_xor(1, NoOrdering);
    let ored: u64 = view.counter.fetch_or(1, NoOrdering);
    let anded: u64 = view.counter.fetch_and(1, NoOrdering);
    let swapped: u64 = view.counter.swap(1, NoOrdering);
    let exchanged: u64 = view.counter.compare_exchange(1, 2, NoOrdering, NoOrdering);
}

data Main {}
"#,
    );
    let main = write_program("placed-view-atomic-denied", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("the placement does not admit atomic store");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for operation in [
        "store",
        "fetch_sub",
        "fetch_xor",
        "fetch_or",
        "fetch_and",
        "swap",
        "compare_exchange",
    ] {
        assert!(
            rendered.contains(&format!("does not admit `{operation}`")),
            "missing `{operation}` diagnostic: {rendered}"
        );
    }
}

#[test]
fn placed_view_rejects_atomic_access_for_an_unsupported_schema_primitive() {
    let source = POLICY_SOURCE
        .replace("counter: u64;", "counter: u16;")
        .replace(
            "data Main {}",
            r#"
machine inspect(view: &Placed<UartPlacement, Registers>) {}

data Main {}
"#,
        );
    let main = write_program("placed-view-atomic-width", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&main, None))
        .expect_err("placed atomics are currently limited to supported atomic primitives");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("counter")
            && rendered.contains("requires schema type `bool`, `u32`, or `u64`"),
        "unexpected diagnostic: {rendered}"
    );
}
