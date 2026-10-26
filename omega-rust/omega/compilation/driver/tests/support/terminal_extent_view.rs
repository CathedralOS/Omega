//! Borrowed nonresident access must use the exact admitted provider allocation.
//! These are embedding inputs, not authority produced by numeric addresses.
#[path = "terminal_provision.rs"]
mod terminal_provision;

use access_plans::{
    AtomicCapability, BoundaryReach, ExternalCapability, ExternalReadBehavior, PeerWritability,
    ResourceProfile, ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion,
    StableCapability, TransferRule,
};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use extents::{
    AddressSpaceId, Extent, ExtentLineageId, ExtentProvenanceId, ExtentProviderIssuance,
    ExtentRightId, ExtentRights, ExtentRootGrant, MappingEraId,
};
use semantic_vocabulary::IntegerValue;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstanceCloseError, TerminalInstanceError, TerminalProgram,
    TerminalProviderStorageInput, TerminalRuntimeSupplyRequest, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralScalarFieldValue, TerminalStructuralValue,
    admit_provider_installation,
};
use terminal_psi::StructuralTypeShape;

pub(super) fn allocation(provenance: u64, length: u64) -> Extent {
    ExtentRootGrant::from_admitted_provider(
        ExtentProviderIssuance::from_normalized_identities([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
        ])
        .unwrap(),
        ExtentLineageId::from_normalized_identity(21).unwrap(),
        AddressSpaceId::from_normalized_identity(22).unwrap(),
        ExtentRights::from_normalized_identities([
            ExtentRightId::from_normalized_identity(23).unwrap()
        ]),
        ExtentProvenanceId::from_normalized_identity(provenance).unwrap(),
        MappingEraId::from_normalized_identity(25).unwrap(),
    )
    .mint(4096, length)
    .unwrap()
}

/// Host-side observations fail this fixture without aborting the corpus.
pub(super) fn observe(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    writes: bool,
) -> Result<(), String> {
    terminal_provision::result(
        || {
            let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap();
            for program in [TerminalProgram::borrow(artifact.view()), loaded] {
                exercise(artifact, &program, writes);
            }
        },
        "external extent view provision failed",
    )
}

fn exercise(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    program: &TerminalProgram<'_>,
    writes: bool,
) {
    let module = program.module();
    let plan = module
        .placement_lifecycle_plans
        .iter()
        .find(|plan| plan.operation == terminal_psi::TerminalPlacementLifecycleOperation::View)
        .expect("exact View operation retained");
    assert_eq!(plan.storage.vacant, None);
    assert_eq!(plan.storage.resident, None);
    let selections = module
        .provider_candidates
        .iter()
        .map(|candidate| ProviderInstallationSelection {
            boundary: candidate.boundary,
            provider_identity: module
                .identities
                .text(candidate.provider_identity)
                .unwrap()
                .to_owned(),
            provider_declaration: module
                .identities
                .text(candidate.provider_declaration.identity())
                .unwrap()
                .to_owned(),
            candidate: candidate.candidate,
        })
        .collect::<Vec<_>>();
    let installed = admit_provider_installation(artifact.view(), &selections, &[]).unwrap();
    let supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            memory_ordering: None,
            entry: None,
            installation: Some(&installed),
            host_imports: &[],
            target_machines: None,
            target_layouts: None,
        })
        .unwrap();
    let entry = program.entry().unwrap();
    let [parameter] = entry.structural_parameters else {
        panic!("one owned extent input");
    };
    let terminal_psi::TerminalMachineResult::Structural(expected_result) = entry.result else {
        panic!("one owned extent result");
    };
    let StructuralTypeShape::Record { fields, .. } = &program
        .structural_type(parameter.structural_type)
        .unwrap()
        .shape
    else {
        panic!("extent record");
    };
    for (second_word, compatible_transfer, bind_backing, crosswire, writable, payload_ceiling) in [
        (41_u64, true, true, false, writes, 8),
        (99, true, true, false, writes, 8),
        (41, false, true, false, writes, 8),
        (41, true, false, false, writes, 8),
        (41, true, true, true, writes, 8),
        (41, true, true, false, writes, 7),
        (41, true, true, false, writes, 1),
    ]
    .into_iter()
    .chain(writes.then_some((41, true, true, false, false, 8)))
    {
        let measurement = EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        };
        let sponsor = BuildEvaluationSponsor::new_with_measurement(
            BuildEvaluationSponsorLimits::new(
                1_000_000,
                8,
                2,
                2,
                100_000,
                payload_ceiling,
                1000,
                1000,
            )
            .unwrap(),
            measurement,
        )
        .unwrap();
        let account = sponsor.begin_invocation(measurement, 1_000_000).unwrap();
        let allocation = allocation(24, 16);
        let profile = ResourceProfileGrant::from_admitted_provider(
            ResourceProfileReceiptId::from_normalized_identity(31).unwrap(),
            &allocation,
            allocation.rights().clone(),
            BoundaryReach::default(),
        )
        .unwrap()
        .admit(ResourceProfile {
            regions: vec![ResourceRegion {
                offset: 0,
                length: 16,
                peer: PeerWritability::HostileShared,
                stable: StableCapability::None,
                external: ExternalCapability::Access {
                    read: ExternalReadBehavior::Repeatable,
                    write: writable,
                    transfers: vec![TransferRule {
                        width_bits: if compatible_transfer { 64 } else { 8 },
                        alignment_bytes: 8,
                    }],
                },
                atomic: AtomicCapability::None,
                reach: BoundaryReach::default(),
            }],
        })
        .unwrap();
        let values = [TerminalStructuralValue {
            opaque_identity: 100,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            projected_qualifications: parameter.projected_qualifications.clone(),
            path: Vec::new(),
        }];
        let scalars = fields
            .iter()
            .map(|field| {
                let coordinate = if field.id == plan.storage.base {
                    4096_u64
                } else {
                    assert_eq!(field.id, plan.storage.length);
                    16
                };
                TerminalStructuralScalarFieldValue {
                    argument_index: 0,
                    path: Vec::new(),
                    field: field.id,
                    value: TerminalScalarValue::Integer {
                        scalar_type: field
                            .field_type
                            .scalar_type()
                            .unwrap()
                            .as_integer()
                            .unwrap(),
                        value: IntegerValue::Unsigned(coordinate.into()),
                    },
                }
            })
            .collect::<Vec<_>>();
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                account: Some(&account),
                scalar_arguments: &[],
                structural_inputs: TerminalStructuralInputs {
                    arguments: &values,
                    scalar_fields: &scalars,
                    ..Default::default()
                },
                installation: Some(&supplies),
                ..Default::default()
            })
            .unwrap();
        if bind_backing {
            let before = sponsor.live_storage_cells();
            let bytes = [17_u64.to_le_bytes(), second_word.to_le_bytes()].concat();
            // Identical geometry and profile contents cannot authorize another
            // issuance. Only the SDK input's provenance changes in this control.
            let (supplied_allocation, matching_allocation) = if crosswire {
                (self::allocation(124, 16), Some(allocation))
            } else {
                (allocation, None)
            };
            let mut input = TerminalProviderStorageInput {
                argument_index: 0,
                path: Vec::new(),
                extent: supplied_allocation,
                profile,
                bytes,
            };
            if second_word == 41 && compatible_transfer && !crosswire && writable == writes {
                // Leave room for only one of the two records. Failure must
                // return the same backing and allow retry on this instance.
                let held = account.reserve_storage(100_000 - before - 1, 0).unwrap();
                let pointer = input.bytes.as_ptr();
                let failure = instance
                    .bind_provider_storage(input)
                    .expect_err("provider binding must reserve both records before publication");
                assert!(
                    failure.reason.contains("AllowanceExhausted(LiveStorage)"),
                    "{}",
                    failure.reason
                );
                assert_eq!(failure.input.bytes.as_ptr(), pointer);
                assert_eq!(
                    failure.input.bytes,
                    [17_u64.to_le_bytes(), second_word.to_le_bytes()].concat()
                );
                assert_eq!(sponsor.live_storage_cells(), 99_999);
                input = failure.input;
                drop(held);
                assert_eq!(sponsor.live_storage_cells(), before);
            }
            let result = instance.bind_provider_storage(input);
            if crosswire {
                let failure =
                    result.expect_err("equal geometry cannot crosswire profile authority");
                assert_eq!(
                    failure.reason,
                    "provider storage profile: extent provenance does not match admitted resource profile"
                );
                assert_eq!(
                    failure.input.bytes,
                    [17_u64.to_le_bytes(), second_word.to_le_bytes()].concat()
                );
                assert_eq!(
                    (failure.input.extent.base(), failure.input.extent.length()),
                    (4096, 16)
                );
                eprintln!("external extent crosswire refused: {}", failure.reason);
                // Retry the returned unchanged profile and bytes with their
                // original allocation. Refusal must not poison the instance.
                let mut input = failure.input;
                input.extent = matching_allocation.unwrap();
                instance.bind_provider_storage(input).unwrap();
            } else {
                result.unwrap();
            }
            assert_eq!(
                sponsor.live_storage_cells(),
                before + 2,
                "provider allocation and descriptor must each own one live cell"
            );
        }
        let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0)).unwrap();
        let result = loop {
            match instance.invoke(&mut meter) {
                Ok(TerminalExecutionStatus::SponsorExhausted(_)) => meter.replenish(1).unwrap(),
                result => break result,
            }
        };
        assert_eq!(
            sponsor.live_payload_bytes(),
            0,
            "transfer scratch retires before return or refusal"
        );
        if payload_ceiling < 8 {
            let refusal = result
                .as_ref()
                .expect_err("field transfer must reserve its complete footprint");
            assert!(
                format!("{refusal:?}").contains("AllowanceExhausted(LiveStorage)"),
                "{refusal:?}"
            );
            assert!(instance.result().is_none());
            assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));
            let usage = *account.snapshot().unwrap().record();
            assert_eq!(instance.invoke(&mut meter), Err(refusal.clone()));
            assert_eq!(*account.snapshot().unwrap().record(), usage);
        } else if !bind_backing {
            assert!(
                matches!(result, Err(TerminalInstanceError::Execution(_))),
                "{result:?}"
            );
            assert!(
                format!("{result:?}").contains("placement extent has no actual provider backing"),
                "{result:?}"
            );
        } else if second_word != 41 || !compatible_transfer || (writes && !writable) {
            assert!(
                matches!(result, Ok(TerminalExecutionStatus::Crashed(_))),
                "{result:?}"
            );
        } else {
            let Ok(TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(
                ref returned,
            ))) = result
            else {
                panic!("qualified external access failed: {result:?}");
            };
            // The program may establish an additional descriptor qualification
            // by a checked issuer. Compare the complete published result
            // contract, not the input's weaker precondition. This still rejects
            // missing qualifications or invented Resident/Vacant custody.
            assert_eq!(
                returned.value.qualifications,
                expected_result.qualifications
            );
            assert_eq!(
                returned.claims.len(),
                1,
                "original extent claim remains owned"
            );
            assert_eq!(
                account.snapshot().unwrap().record().peak_live_payload_bytes,
                8
            );
        }
        eprintln!(
            "external extent second_word={second_word}, transfer={compatible_transfer}, bound={bind_backing}, writable={writable}: {result:?}"
        );
        // Neither success nor rejection manufactures vacancy for host handback.
        assert!(instance.take_returned_provider_storage(&[]).is_err());
        drop(instance);
        assert_eq!(sponsor.live_storage_cells(), 0);
        assert_eq!(sponsor.live_payload_bytes(), 0);
    }
}
