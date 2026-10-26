//! Execute the authored initialization/transport/extraction journey against
//! one real supplied buffer. Its source checks the extracted value; handback
//! checks the original allocation and untouched tail, not numeric geometry alone.
#[path = "terminal_provision.rs"]
mod terminal_provision;

use access_plans::{
    AtomicCapability, BoundaryReach, ExternalCapability, PeerWritability, ResourceProfile,
    ResourceProfileGrant, ResourceProfileReceiptId, ResourceRegion, StableCapability,
};
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::IntegerValue;
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    ProviderInstallationSelection, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInstanceCloseError, TerminalInstantiation, TerminalProgram,
    TerminalProviderStorageInput, TerminalRuntimeSupplyRequest, TerminalScalarValue,
    TerminalStructuralInputs, TerminalStructuralScalarFieldValue, TerminalStructuralValue,
    admit_provider_installation,
};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

pub(super) fn observe(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    peak: &str,
    payload_peak: &str,
    values: &[&str],
) -> Result<(), String> {
    let peak: u64 = peak
        .parse()
        .map_err(|error| format!("invalid cell ceiling: {error}"))?;
    if peak < 2 {
        return Err("resident storage ceiling must exceed one".into());
    }
    let payload_peak: u64 = payload_peak
        .parse()
        .map_err(|error| format!("invalid payload ceiling: {error}"))?;
    if payload_peak < 2 || values.is_empty() {
        return Err("resident image needs a payload ceiling and expected words".into());
    }
    let expected = values
        .iter()
        .map(|value| value.parse::<u64>().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    terminal_provision::result(
        || {
            let loaded = TerminalProgram::load_envelope(&artifact.to_bytes()).unwrap();
            for program in [TerminalProgram::borrow(artifact.view()), loaded] {
                for (ceiling, payload_ceiling, supply, constrain_after_image) in [
                    (peak, payload_peak, StableCapability::ReadWrite, false),
                    (peak - 1, payload_peak, StableCapability::ReadWrite, false),
                    (peak, payload_peak, StableCapability::Write, false),
                    (peak, 1, StableCapability::ReadWrite, false),
                    (peak, payload_peak - 1, StableCapability::ReadWrite, false),
                    (peak, payload_peak, StableCapability::ReadWrite, true),
                ] {
                    exercise(
                        artifact,
                        &program,
                        peak,
                        ceiling,
                        payload_peak,
                        payload_ceiling,
                        supply,
                        &expected,
                        constrain_after_image,
                    );
                }
            }
        },
        "resident storage provision failed",
    )
}

fn exercise(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    program: &TerminalProgram<'_>,
    peak: u64,
    ceiling: u64,
    payload_peak: u64,
    payload_ceiling: u64,
    supply: StableCapability,
    expected: &[u64],
    constrain_after_image: bool,
) {
    let expected_bytes = expected
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let allocation_length = expected_bytes.len() + 8;
    let module = program.module();
    let plan = module
        .placement_lifecycle_plans
        .iter()
        .find(|plan| {
            plan.operation == terminal_psi::TerminalPlacementLifecycleOperation::Initialize
        })
        .expect("source publishes initialization geometry");
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
            installation: Some(&installed),
            host_imports: &[],
            x86_flags: None,
            memory_ordering: None,
            entry: None,
            target_machines: None,
            target_layouts: None,
        })
        .unwrap();
    let entry = program.entry().unwrap();
    let [parameter] = entry.structural_parameters else {
        panic!("one owned extent input");
    };
    let [reason] = entry.scalar_parameters else {
        panic!("one scalar reason input");
    };
    let StructuralTypeShape::Record { fields, .. } = &program
        .structural_type(parameter.structural_type)
        .unwrap()
        .shape
    else {
        panic!("extent record");
    };
    let scalars = fields
        .iter()
        .map(|field| {
            let coordinate = if field.id == plan.storage.base {
                4096_u64
            } else {
                assert_eq!(field.id, plan.storage.length);
                allocation_length as u64
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
    let values = [TerminalStructuralValue {
        opaque_identity: 100,
        structural_type: parameter.structural_type,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        path: Vec::new(),
    }];
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(1_000_000, 8, 2, 2, ceiling, payload_ceiling, 1000, 1000)
            .unwrap(),
        measurement,
    )
    .unwrap();
    let account = sponsor.begin_invocation(measurement, 1_000_000).unwrap();
    let arguments = [TerminalScalarValue::Integer {
        scalar_type: reason.scalar_type.as_integer().unwrap(),
        value: IntegerValue::Unsigned(0),
    }];
    let mut instance = program
        .instantiate(TerminalInstantiation {
            scalar_arguments: &arguments,
            structural_inputs: TerminalStructuralInputs {
                arguments: &values,
                scalar_fields: &scalars,
                ..Default::default()
            },
            account: Some(&account),
            installation: Some(&supplies),
            ..Default::default()
        })
        .unwrap();
    let allocation = super::extent_view::allocation(24, allocation_length as u64);
    let original_authority = (
        allocation.address_space(),
        allocation.provenance(),
        allocation.era(),
        allocation.origin(),
        allocation.lineage_root(),
        allocation.sharing(),
        allocation.rights().clone(),
    );
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
            length: allocation_length as u64,
            peer: PeerWritability::Exclusive,
            stable: supply,
            external: ExternalCapability::None,
            atomic: AtomicCapability::None,
            reach: BoundaryReach::default(),
        }],
    })
    .unwrap();
    let bytes = vec![0xa5; allocation_length];
    let pointer = bytes.as_ptr();
    instance
        .bind_provider_storage(TerminalProviderStorageInput {
            argument_index: 0,
            path: Vec::new(),
            extent: allocation,
            profile,
            bytes,
        })
        .unwrap();
    let mut meter = TerminalFuelMeter::with_invocation(account.clone(), Some(0)).unwrap();
    let mut competing_payload = None;
    let result = loop {
        match instance.invoke(&mut meter) {
            Ok(TerminalExecutionStatus::SponsorExhausted(_)) => {
                // The source's complete initialization image has existed and
                // retired. A root-owned competing reservation now leaves one
                // byte for extraction, without changing the source or relying
                // on interpreter program counters or private storage access.
                if constrain_after_image
                    && competing_payload.is_none()
                    && account.snapshot().unwrap().record().peak_live_payload_bytes == payload_peak
                    && sponsor.live_payload_bytes() == 0
                {
                    competing_payload = Some(account.reserve_storage(0, payload_peak - 1).unwrap());
                }
                meter.replenish(1).unwrap();
            }
            result => break result,
        }
    };
    if constrain_after_image {
        assert!(
            competing_payload.is_some(),
            "extraction control must run after the complete image retires"
        );
    }
    assert_eq!(
        sponsor.live_payload_bytes(),
        if competing_payload.is_some() {
            payload_peak - 1
        } else {
            0
        },
        "completed or refused codec scratch must already be retired"
    );
    if ceiling < peak
        || payload_ceiling < payload_peak
        || supply == StableCapability::Write
        || constrain_after_image
    {
        let refusal = result.expect_err("insufficient budget or read authority must refuse");
        let diagnostic = format!("{refusal:?}");
        let expected = if ceiling < peak || payload_ceiling < payload_peak || constrain_after_image
        {
            "AllowanceExhausted(LiveStorage)"
        } else {
            "extraction profile differs from the original admitted allocation supply"
        };
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert!(instance.result().is_none());
        assert_eq!(instance.close(), Err(TerminalInstanceCloseError::Busy));
        let usage = *account.snapshot().unwrap().record();
        assert_eq!(instance.invoke(&mut meter), Err(refusal));
        assert_eq!(*account.snapshot().unwrap().record(), usage);
        assert!(instance.take_returned_provider_storage(&[]).is_err());
    } else {
        let Ok(TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(returned))) =
            result
        else {
            panic!("resident lifecycle failed: {result:?}");
        };
        let StructuralTypeShape::Record { fields, .. } = &program
            .structural_type(returned.value.structural_type)
            .unwrap()
            .shape
        else {
            panic!("placement return record");
        };
        let source = fields.iter().find(|field| matches!(field.field_type,
        terminal_psi::StructuralFieldType::Structural(structural_type) if structural_type == parameter.structural_type
    )).expect("returned extent field");
        let path = [StructuralPathSegment::Field(source.identity)];
        let before = sponsor.live_storage_cells();
        let returned = instance.take_returned_provider_storage(&path).unwrap();
        assert_eq!(returned.bytes.as_ptr(), pointer);
        assert_eq!(&returned.bytes[..expected_bytes.len()], expected_bytes);
        assert_eq!(&returned.bytes[expected_bytes.len()..], &[0xa5; 8]);
        assert_eq!(
            (returned.extent.base(), returned.extent.length()),
            (4096, allocation_length as u64)
        );
        assert_eq!(
            (
                returned.extent.address_space(),
                returned.extent.provenance(),
                returned.extent.era(),
                returned.extent.origin(),
                returned.extent.lineage_root(),
                returned.extent.sharing(),
                returned.extent.rights().clone()
            ),
            original_authority
        );
        assert!(sponsor.live_storage_cells() <= before - 2);
        assert!(instance.take_returned_provider_storage(&path).is_err());
        assert_eq!(sponsor.peak_live_storage_cells(), peak);
        assert_eq!(
            account.snapshot().unwrap().record().peak_live_payload_bytes,
            payload_peak
        );
    }
    eprintln!(
        "resident storage ceiling={ceiling}, payload_ceiling={payload_ceiling}, supply={supply:?}, peak={}",
        sponsor.peak_live_storage_cells()
    );
    drop(instance);
    drop(competing_payload);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}
