//! Two-pass native-fuel instrumentation over immutable terminal machine code.
//!
//! Semantic bytes and their original evidence remain in the source plan. This
//! transform creates a parallel metered carrier with exact hot/cold records;
//! object construction can therefore replay source semantics first and fuel
//! bytes independently instead of rebasing dozens of unrelated evidence rows.

use std::collections::BTreeSet;

use omega_installation_evidence::{FuelAttributionSite, NativeFuelTargetPlanProjection};
use omega_machine_code::{
    MachineCodePlan, NativeFuelAttribution, NativeFuelChargeRecord, NativeFuelInstrumentedFunction,
    NativeFuelInstrumentedPlan, NativeFuelSite,
};
use omega_target::Architecture;
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeFuelInstrumentationError {
    TargetMismatch,
    NoAttributions,
    NonCanonicalAttributions(MachineId),
    InvalidAttribution(MachineId),
    RankedCountdownRequiresBranchRebasing(MachineId),
    SizeOverflow,
    Encoding(String),
}

struct PreparedFunction {
    machine: MachineId,
    bytes: Vec<u8>,
    semantic_end_offset: usize,
    rows: Vec<PreparedRow>,
    final_byte_count: usize,
}

struct PreparedRow {
    attribution: NativeFuelAttribution,
    charge_code_offset: usize,
    semantic_code_offset: usize,
}

/// Insert one hot precharge before every attributed semantic site (including
/// zero-byte sites), preserve source bytes in one forward pass, and append one
/// cold dispatcher per site after the semantic function end.
pub fn instrument_native_fuel(
    source: MachineCodePlan,
    target_policy: NativeFuelTargetPlanProjection,
) -> Result<NativeFuelInstrumentedPlan, NativeFuelInstrumentationError> {
    if source.target != target_policy.target
        || target_policy.profile.native_target() != target_policy.target
    {
        return Err(NativeFuelInstrumentationError::TargetMismatch);
    }
    let mut total_sites = 0usize;
    let mut prepared = Vec::with_capacity(source.functions.len());
    for function in &source.functions {
        if function.requires_ranked_countdown_replay() {
            return Err(
                NativeFuelInstrumentationError::RankedCountdownRequiresBranchRebasing(
                    function.machine,
                ),
            );
        }
        total_sites = total_sites
            .checked_add(function.fuel_attribution.len())
            .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
        prepared.push(prepare_function(
            source.target.architecture,
            &target_policy,
            function,
        )?);
    }
    if total_sites == 0 {
        return Err(NativeFuelInstrumentationError::NoAttributions);
    }

    let mut function_bases = Vec::with_capacity(prepared.len());
    let mut next_base = 0usize;
    for function in &prepared {
        function_bases.push(next_base);
        next_base = next_base
            .checked_add(function.final_byte_count)
            .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
    }

    let mut functions = Vec::with_capacity(prepared.len());
    for (mut function, function_base) in prepared.into_iter().zip(function_bases) {
        let mut charges = Vec::with_capacity(function.rows.len());
        let cold_byte_count = cold_dispatch_byte_count(source.target.architecture);
        for row in function.rows {
            let cold_dispatch_code_offset = function.bytes.len();
            let retry_text_offset = function_base
                .checked_add(row.charge_code_offset)
                .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
            let cold = encode_cold_dispatch(
                source.target.architecture,
                &target_policy,
                row.attribution,
                u64::try_from(retry_text_offset)
                    .map_err(|_| NativeFuelInstrumentationError::SizeOverflow)?,
            )?;
            debug_assert_eq!(cold.len(), cold_byte_count);
            function.bytes.extend_from_slice(&cold);

            let branch_origin = row
                .charge_code_offset
                .checked_add(failure_branch_origin(source.target.architecture))
                .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
            let branch_distance = signed_distance(cold_dispatch_code_offset, branch_origin)?;
            let hot = encode_hot_charge(
                source.target.architecture,
                &target_policy,
                row.attribution.units,
                branch_distance,
            )?;
            let hot_end = row
                .charge_code_offset
                .checked_add(hot.len())
                .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
            function.bytes[row.charge_code_offset..hot_end].copy_from_slice(&hot);
            charges.push(NativeFuelChargeRecord {
                attribution: row.attribution,
                charge_code_offset: row.charge_code_offset,
                charge_byte_count: hot.len(),
                semantic_code_offset: row.semantic_code_offset,
                cold_dispatch_code_offset,
                cold_dispatch_byte_count: cold.len(),
            });
        }
        debug_assert_eq!(function.bytes.len(), function.final_byte_count);
        functions.push(NativeFuelInstrumentedFunction {
            machine: function.machine,
            bytes: function.bytes,
            semantic_end_offset: function.semantic_end_offset,
            charges,
        });
    }

    Ok(NativeFuelInstrumentedPlan {
        source,
        target_policy,
        functions,
    })
}

fn prepare_function(
    architecture: Architecture,
    target_policy: &NativeFuelTargetPlanProjection,
    function: &omega_machine_code::MachineCodeFunction,
) -> Result<PreparedFunction, NativeFuelInstrumentationError> {
    if function.fuel_attribution.windows(2).any(|pair| {
        pair[0].operation_ordinal >= pair[1].operation_ordinal
            || pair[0].code_offset > pair[1].code_offset
    }) {
        return Err(NativeFuelInstrumentationError::NonCanonicalAttributions(
            function.machine,
        ));
    }
    let mut sites = BTreeSet::new();
    let mut bytes = Vec::new();
    let mut rows = Vec::with_capacity(function.fuel_attribution.len());
    let mut cursor = 0usize;
    let mut schedule = None;
    for attribution in &function.fuel_attribution {
        let end = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .ok_or(NativeFuelInstrumentationError::InvalidAttribution(
                function.machine,
            ))?;
        if attribution.units == 0
            || attribution.code_offset < cursor
            || end > function.bytes.len()
            || !sites.insert(attribution.site)
            || schedule.is_some_and(|schedule| schedule != attribution.schedule)
        {
            return Err(NativeFuelInstrumentationError::InvalidAttribution(
                function.machine,
            ));
        }
        schedule = Some(attribution.schedule);
        bytes.extend_from_slice(&function.bytes[cursor..attribution.code_offset]);
        cursor = attribution.code_offset;
        let charge_code_offset = bytes.len();
        let placeholder = encode_hot_charge(architecture, target_policy, attribution.units, 0)?;
        bytes.extend_from_slice(&placeholder);
        rows.push(PreparedRow {
            attribution: *attribution,
            charge_code_offset,
            semantic_code_offset: bytes.len(),
        });
    }
    bytes.extend_from_slice(&function.bytes[cursor..]);
    let semantic_end_offset = bytes.len();
    let cold_bytes = cold_dispatch_byte_count(architecture)
        .checked_mul(rows.len())
        .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
    let final_byte_count = semantic_end_offset
        .checked_add(cold_bytes)
        .ok_or(NativeFuelInstrumentationError::SizeOverflow)?;
    bytes.reserve(cold_bytes);
    Ok(PreparedFunction {
        machine: function.machine,
        bytes,
        semantic_end_offset,
        rows,
        final_byte_count,
    })
}

fn encode_hot_charge(
    architecture: Architecture,
    plan: &NativeFuelTargetPlanProjection,
    units: u64,
    distance: isize,
) -> Result<Vec<u8>, NativeFuelInstrumentationError> {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_native_fuel_charge(plan, units, distance),
        Architecture::Aarch64 => {
            omega_isa_aarch64::encode_native_fuel_charge(plan, units, distance)
        }
    }
    .map_err(|diagnostic| NativeFuelInstrumentationError::Encoding(diagnostic.to_string()))
}

fn encode_cold_dispatch(
    architecture: Architecture,
    plan: &NativeFuelTargetPlanProjection,
    attribution: NativeFuelAttribution,
    retry_text_offset: u64,
) -> Result<Vec<u8>, NativeFuelInstrumentationError> {
    let site = match attribution.site {
        NativeFuelSite::Operation(operation) => FuelAttributionSite::Operation(operation),
        NativeFuelSite::Edge(edge) => FuelAttributionSite::Edge(edge),
    };
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::encode_native_fuel_cold_dispatch(
            plan,
            site,
            attribution.units,
            retry_text_offset,
        ),
        Architecture::Aarch64 => omega_isa_aarch64::encode_native_fuel_cold_dispatch(
            plan,
            site,
            attribution.units,
            retry_text_offset,
        ),
    }
    .map_err(|diagnostic| NativeFuelInstrumentationError::Encoding(diagnostic.to_string()))
}

const fn cold_dispatch_byte_count(architecture: Architecture) -> usize {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT,
        Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_COLD_DISPATCH_BYTE_COUNT,
    }
}

const fn failure_branch_origin(architecture: Architecture) -> usize {
    match architecture {
        Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_FAILURE_BRANCH_END_OFFSET,
        Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_FAILURE_BRANCH_OFFSET,
    }
}

fn signed_distance(target: usize, origin: usize) -> Result<isize, NativeFuelInstrumentationError> {
    let target =
        isize::try_from(target).map_err(|_| NativeFuelInstrumentationError::SizeOverflow)?;
    let origin =
        isize::try_from(origin).map_err(|_| NativeFuelInstrumentationError::SizeOverflow)?;
    target
        .checked_sub(origin)
        .ok_or(NativeFuelInstrumentationError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::MachineRegister;
    use omega_installation_evidence::{NativeFuelContextLayout, SponsorContextTransport};
    use omega_target::TargetProfile;
    use omega_target_operations::TerminalPsiProvenance;
    use psi_core::{EdgeId, FuelScheduleIdentity, OperationId};
    use psi_terminal::{SemanticFingerprint, VocabularyMarker};

    fn target_policy(profile: TargetProfile) -> NativeFuelTargetPlanProjection {
        let register = match profile.native_target().architecture {
            Architecture::X86_64 => MachineRegister::X86Rbx,
            Architecture::Aarch64 => MachineRegister::Aarch64X(28),
        };
        NativeFuelTargetPlanProjection {
            profile,
            target: profile.native_target(),
            transport: SponsorContextTransport::ReservedNonvolatileRegister { register },
            context: NativeFuelContextLayout {
                byte_size: 256,
                alignment: 16,
                remaining_units_offset: 24,
                unpaid_site_kind_offset: 32,
                unpaid_site_identity_offset: 40,
                required_units_offset: 48,
                transfer_entry_offset: 56,
                retry_code_offset_offset: 64,
                sponsor_stack_top_offset: 72,
                activation_state_offset: 80,
                activation_state_byte_count: 176,
            },
            transfer_plan_identity: 7,
        }
    }

    fn source_function(machine: u64) -> omega_machine_code::MachineCodeFunction {
        let schedule = FuelScheduleIdentity::new(1).unwrap();
        omega_machine_code::MachineCodeFunction {
            machine: MachineId::new(machine).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: vec![
                    OperationId::new(machine * 10 + 1).unwrap(),
                    OperationId::new(machine * 10 + 2).unwrap(),
                ],
                edges: vec![EdgeId::new(machine * 10 + 3).unwrap()],
            },
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8],
            unit_stack: None,
            unit_parameter_homes: Vec::new(),
            unit_parameters: Vec::new(),
            scalar_stack: None,
            internal_calls: Vec::new(),
            internal_unit_calls: Vec::new(),
            unit_affine_cleanup: None,
            scalar_affine_cleanup: None,
            scalar_control_affine_cleanups: Vec::new(),
            scalar_structural_parameters: Vec::new(),
            scalar_structural_parameter_homes: Vec::new(),
            ranked_u32_countdown: None,
            fuel_attribution: vec![
                NativeFuelAttribution {
                    schedule,
                    site: NativeFuelSite::Operation(OperationId::new(machine * 10 + 1).unwrap()),
                    units: 2,
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 0,
                },
                NativeFuelAttribution {
                    schedule,
                    site: NativeFuelSite::Operation(OperationId::new(machine * 10 + 2).unwrap()),
                    units: 3,
                    operation_ordinal: 1,
                    code_offset: 0,
                    byte_count: 4,
                },
                NativeFuelAttribution {
                    schedule,
                    site: NativeFuelSite::Edge(EdgeId::new(machine * 10 + 3).unwrap()),
                    units: 5,
                    operation_ordinal: 2,
                    code_offset: 4,
                    byte_count: 4,
                },
            ],
            port_effects: Vec::new(),
            boundary_settlements: Vec::new(),
            structural_return: None,
        }
    }

    fn source(profile: TargetProfile, function_count: u64) -> MachineCodePlan {
        MachineCodePlan {
            psi: psi_terminal::TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
            },
            target: profile.native_target(),
            entry: MachineId::new(1).unwrap(),
            functions: (1..=function_count).map(source_function).collect(),
        }
    }

    fn assert_exact_instrumentation(profile: TargetProfile) {
        let policy = target_policy(profile);
        let source = source(profile, 2);
        let instrumented = instrument_native_fuel(source.clone(), policy).expect("instrumentation");
        assert_eq!(instrumented.source, source);

        let architecture = profile.native_target().architecture;
        let hot_size = match architecture {
            Architecture::X86_64 => omega_isa_x86_64::X86_NATIVE_FUEL_CHARGE_BYTE_COUNT,
            Architecture::Aarch64 => omega_isa_aarch64::AARCH64_NATIVE_FUEL_CHARGE_BYTE_COUNT,
        };
        let cold_size = cold_dispatch_byte_count(architecture);
        let expected_hot_offsets = [0, hot_size, 4 + 2 * hot_size];
        let expected_semantic_offsets = [hot_size, 2 * hot_size, 4 + 3 * hot_size];
        let semantic_end = 8 + 3 * hot_size;
        let function_size = semantic_end + 3 * cold_size;

        for (function_ordinal, function) in instrumented.functions.iter().enumerate() {
            assert_eq!(function.semantic_end_offset, semantic_end);
            assert_eq!(function.bytes.len(), function_size);
            assert_eq!(function.charges.len(), 3);
            assert_eq!(
                &function.bytes[2 * hot_size..2 * hot_size + 4],
                &[1, 2, 3, 4]
            );
            assert_eq!(
                &function.bytes[semantic_end - 4..semantic_end],
                &[5, 6, 7, 8]
            );

            for (row_ordinal, row) in function.charges.iter().enumerate() {
                let hot_offset = expected_hot_offsets[row_ordinal];
                let cold_offset = semantic_end + row_ordinal * cold_size;
                assert_eq!(row.charge_code_offset, hot_offset);
                assert_eq!(row.charge_byte_count, hot_size);
                assert_eq!(
                    row.semantic_code_offset,
                    expected_semantic_offsets[row_ordinal]
                );
                assert_eq!(row.cold_dispatch_code_offset, cold_offset);
                assert_eq!(row.cold_dispatch_byte_count, cold_size);

                let distance = signed_distance(
                    cold_offset,
                    hot_offset + failure_branch_origin(architecture),
                )
                .unwrap();
                let expected_hot =
                    encode_hot_charge(architecture, &policy, row.attribution.units, distance)
                        .unwrap();
                assert_eq!(
                    &function.bytes[hot_offset..hot_offset + hot_size],
                    expected_hot
                );

                let retry = function_ordinal * function_size + hot_offset;
                let expected_cold = encode_cold_dispatch(
                    architecture,
                    &policy,
                    row.attribution,
                    u64::try_from(retry).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    &function.bytes[cold_offset..cold_offset + cold_size],
                    expected_cold
                );
            }
        }
    }

    #[test]
    fn x86_instrumentation_preserves_semantics_and_replays_every_charge() {
        assert_exact_instrumentation(TargetProfile::LinuxX64);
    }

    #[test]
    fn aarch64_instrumentation_preserves_semantics_and_replays_every_charge() {
        assert_exact_instrumentation(TargetProfile::LinuxArm64);
    }

    #[test]
    fn malformed_attribution_and_target_policy_fail_closed() {
        let profile = TargetProfile::LinuxX64;
        let mut plan = source(profile, 1);
        plan.functions[0].fuel_attribution[1].operation_ordinal = 0;
        assert_eq!(
            instrument_native_fuel(plan, target_policy(profile)),
            Err(NativeFuelInstrumentationError::NonCanonicalAttributions(
                MachineId::new(1).unwrap()
            ))
        );

        let mut plan = source(profile, 1);
        plan.functions[0].fuel_attribution[0].units = 0;
        assert_eq!(
            instrument_native_fuel(plan, target_policy(profile)),
            Err(NativeFuelInstrumentationError::InvalidAttribution(
                MachineId::new(1).unwrap()
            ))
        );

        let plan = source(profile, 1);
        assert_eq!(
            instrument_native_fuel(plan, target_policy(TargetProfile::WindowsX64)),
            Err(NativeFuelInstrumentationError::TargetMismatch)
        );
    }

    #[test]
    fn ranked_shape_rejects_instrumentation_even_after_optional_custody_is_removed() {
        let profile = TargetProfile::LinuxX64;
        let mut plan = source(profile, 1);
        let function = &mut plan.functions[0];
        let operations = [
            OperationId::new(11).unwrap(),
            OperationId::new(12).unwrap(),
            OperationId::new(13).unwrap(),
            OperationId::new(14).unwrap(),
        ];
        let edges = [
            EdgeId::new(11).unwrap(),
            EdgeId::new(12).unwrap(),
            EdgeId::new(13).unwrap(),
            EdgeId::new(14).unwrap(),
            EdgeId::new(15).unwrap(),
        ];
        function.provenance = TerminalPsiProvenance {
            operations: operations.to_vec(),
            edges: edges.to_vec(),
        };
        function.ranked_u32_countdown = None;
        let schedule = FuelScheduleIdentity::new(1).unwrap();
        function.fuel_attribution = [
            NativeFuelSite::Edge(edges[0]),
            NativeFuelSite::Operation(operations[0]),
            NativeFuelSite::Operation(operations[1]),
            NativeFuelSite::Edge(edges[1]),
            NativeFuelSite::Operation(operations[2]),
            NativeFuelSite::Operation(operations[3]),
            NativeFuelSite::Edge(edges[3]),
            NativeFuelSite::Edge(edges[2]),
            NativeFuelSite::Edge(edges[4]),
        ]
        .into_iter()
        .enumerate()
        .map(|(operation_ordinal, site)| NativeFuelAttribution {
            schedule,
            site,
            units: 1,
            operation_ordinal,
            code_offset: 0,
            byte_count: 0,
        })
        .collect();
        assert_eq!(
            instrument_native_fuel(plan, target_policy(profile)),
            Err(
                NativeFuelInstrumentationError::RankedCountdownRequiresBranchRebasing(
                    MachineId::new(1).unwrap()
                )
            )
        );
    }
}
