//! Decode the current publication contracts from the recovery artifact.
//!
//! The ABI owner supplies the canonical encoder. This reader uses the existing
//! calling-plan and scalar decoders, bounds each new roster by its minimum
//! wire row, and reconstructs the immutable contract owner rather than trusting
//! a caller-supplied commitment.

use std::sync::Arc;

use selected_instructions::SelectedProgramContracts;
use semantic_vocabulary::{MachineId, ValueId};
use target_vocabulary::function_abi::LegalizedFunctionAbi;
use target_vocabulary::{MixedStructuralScalarFunctionAbi, ScalarAbiValue, ScalarFunctionAbi};

use super::fixed_view_copy_codec::{Cursor, decode_id, decode_scalar, length};
use super::structural_calls::{decode_call_plan, decode_placement};
use super::structural_declarations::decode_abi_parameter;
use crate::FixedViewCopyDecodeError as Error;

pub(super) fn encode(bytes: &mut Vec<u8>, contracts: &SelectedProgramContracts) {
    match contracts.profile() {
        None => bytes.push(0),
        Some(profile) => {
            bytes.push(1);
            let identity = profile.identity().as_str().as_bytes();
            length(bytes, identity.len());
            bytes.extend_from_slice(identity);
        }
    }
    length(bytes, contracts.function_abis().len());
    for abi in contracts.function_abis().iter() {
        target_vocabulary::identity::function_abi::encode_function_abi(bytes, abi);
    }
    contracts.publication().encode(bytes);
    match contracts.execution() {
        None => bytes.push(0),
        Some(facts) => {
            bytes.push(1);
            facts.encode(bytes);
        }
    }
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
    borrowed_storage_layouts: runtime_abi::NativeBorrowedStorageLayouts,
) -> Result<SelectedProgramContracts, Error> {
    let profile = match cursor.byte()? {
        0 => None,
        1 => {
            let length = cursor.length()?;
            let identity = cursor.take(length)?;
            Some(
                target::TargetProfile::ALL
                    .into_iter()
                    .find(|profile| profile.identity().as_str().as_bytes() == identity)
                    .ok_or(Error::InvalidForeignTargetProfile)?,
            )
        }
        tag => return Err(Error::UnknownOption(tag)),
    };
    // Machine ID, empty calling plan, three row counts and two option tags.
    let function_abis = decode_rows(cursor, 65, decode_abi)?;
    let publication = super::publication::decode(cursor)?;
    let execution = match cursor.byte()? {
        0 => None,
        1 => Some(super::execution_facts::decode(cursor)?),
        tag => return Err(Error::UnknownOption(tag)),
    };
    SelectedProgramContracts::new(
        borrowed_storage_layouts,
        profile,
        Arc::from(function_abis),
        publication,
        execution,
    )
    .map_err(Error::InvalidProgramContracts)
}

fn decode_abi(cursor: &mut Cursor<'_>) -> Result<LegalizedFunctionAbi, Error> {
    let machine = decode_id(cursor, MachineId::new)?;
    let call_plan = decode_call_plan(cursor)?;
    let count = cursor.length()?;
    if count != call_plan.parameters.len() {
        return Err(Error::InvalidProgramContracts(
            "function ABI entry conversions have another parameter count",
        ));
    }
    if count > cursor.remaining() {
        return Err(Error::Truncated);
    }
    let mut entry_scalar_codecs = Vec::new();
    entry_scalar_codecs
        .try_reserve_exact(count)
        .map_err(|_| Error::LengthOverflow)?;
    for _ in 0..count {
        entry_scalar_codecs.push(crate::cursor::optional(
            cursor,
            |cursor| Ok(cursor.u64()?),
            Error::UnknownOption,
        )?);
    }
    let scalar_parameters = decode_scalars(cursor)?;
    let structural_parameters = decode_parameters(cursor)?;
    let scalar = crate::cursor::optional(
        cursor,
        |cursor| {
            Ok(ScalarFunctionAbi {
                call_plan: decode_call_plan(cursor)?,
                parameters: decode_scalars(cursor)?,
                result: crate::cursor::optional(cursor, decode_scalar_value, Error::UnknownOption)?,
            })
        },
        Error::UnknownOption,
    )?;
    let mixed_structural_scalar = crate::cursor::optional(
        cursor,
        |cursor| {
            Ok(MixedStructuralScalarFunctionAbi {
                call_plan: decode_call_plan(cursor)?,
                scalar_parameters: decode_scalars(cursor)?,
                structural_parameters: decode_parameters(cursor)?,
                result: decode_scalar_value(cursor)?,
            })
        },
        Error::UnknownOption,
    )?;
    Ok(LegalizedFunctionAbi {
        machine,
        call_plan,
        entry_scalar_codecs,
        scalar_parameters,
        structural_parameters,
        scalar,
        mixed_structural_scalar,
    })
}

fn decode_scalars(cursor: &mut Cursor<'_>) -> Result<Vec<ScalarAbiValue>, Error> {
    // Value ID, Boolean type, and a placement with no locations.
    decode_rows(cursor, 24, decode_scalar_value)
}

fn decode_scalar_value(cursor: &mut Cursor<'_>) -> Result<ScalarAbiValue, Error> {
    Ok(ScalarAbiValue {
        value: decode_id(cursor, ValueId::new)?,
        scalar_type: decode_scalar(cursor)?,
        placement: decode_placement(cursor)?,
    })
}

fn decode_parameters(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<target_vocabulary::TargetStructuralParameter>, Error> {
    // Two IDs, multiplicity/access, shape, placement and qualification count.
    decode_rows(cursor, 48, decode_abi_parameter)
}

pub(super) fn decode_rows<T>(
    cursor: &mut Cursor<'_>,
    minimum: usize,
    mut decode: impl FnMut(&mut Cursor<'_>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let count = cursor.length()?;
    if count > cursor.remaining() / minimum {
        return Err(Error::Truncated);
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Error::LengthOverflow)?;
    for _ in 0..count {
        rows.push(decode(cursor)?);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, PlaceId, ScalarType, StructuralDomainId, StructuralTypeId,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathQualification};

    fn abi(machine: MachineId, structural: bool) -> LegalizedFunctionAbi {
        let mut shapes = vec![ValueShape::integer(8, 8)];
        if structural {
            shapes.push(ValueShape::borrowed_scalar_reference());
        }
        let call_plan = evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: shapes,
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        let scalar_parameters = vec![ScalarAbiValue {
            value: ValueId::new(1).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            placement: call_plan.parameters[0].clone(),
        }];
        let result = ScalarAbiValue {
            value: ValueId::new(2).unwrap(),
            placement: call_plan.result.clone().unwrap(),
            ..scalar_parameters[0].clone()
        };
        let structural_parameters = if structural {
            vec![target_vocabulary::TargetStructuralParameter {
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                projected_qualifications: vec![StructuralPathQualification {
                    path: Vec::new(),
                    domain: StructuralDomainId::new(3).unwrap(),
                }],
                shape: call_plan.parameters[1].shape,
                placement: call_plan.parameters[1].clone(),
            }]
        } else {
            Vec::new()
        };
        LegalizedFunctionAbi {
            machine,
            call_plan: call_plan.clone(),
            entry_scalar_codecs: if structural {
                vec![Some(0), runtime_abi::ScalarStorageCodec::Boolean.encode()]
            } else {
                vec![Some(0)]
            },
            scalar_parameters: scalar_parameters.clone(),
            structural_parameters: structural_parameters.clone(),
            scalar: (!structural).then(|| ScalarFunctionAbi {
                call_plan: call_plan.clone(),
                parameters: scalar_parameters.clone(),
                result: Some(result.clone()),
            }),
            mixed_structural_scalar: structural.then_some(MixedStructuralScalarFunctionAbi {
                call_plan,
                scalar_parameters,
                structural_parameters,
                result,
            }),
        }
    }

    fn publication(machine: MachineId) -> target_vocabulary::NativePublicationFacts {
        target_vocabulary::NativePublicationFacts::seal(
            vec![target_vocabulary::NativeFunctionPublication {
                call_targets: Vec::new(),
                machine,
                operation_count: 0,
                sites: Vec::new(),
                byte_literals: Vec::new(),
            }],
            Default::default(),
        )
        .unwrap()
    }

    fn execution(
        psi: terminal_psi::TerminalPsiIdentity,
        work_schedule: semantic_vocabulary::WorkScheduleIdentity,
        entry: MachineId,
    ) -> Option<target_vocabulary::NativeExecutionFacts> {
        Some(target_vocabulary::NativeExecutionFacts::seal(target_vocabulary::NativeExecutionRoots {
            psi, work_schedule, entry,
            optimization_unit: optimization_core::OptimizationUnitIdentity::from_canonical_bytes(b"metadata fixture unit"),
            pre_physical: optimization_core::PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"metadata fixture prephysical"),
        }, Arc::new(target_vocabulary::execution_facts::ReachableCallableInvocations {
            roots: vec![entry], reached_machines: vec![entry], invocations: Vec::new(), inputs: Vec::new(),
        }), Vec::new()).unwrap())
    }

    #[test]
    fn recovery_retains_complete_nonempty_abi_rows_and_profile() {
        use crate::{FixedViewCopyPlan, FixedViewCopyPolicy};
        for structural in [false, true] {
            let mut plan = super::super::codec_tests::plan(
                FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            );
            let abi = abi(plan.transformed.entry, structural);
            let execution = execution(
                plan.transformed.psi,
                plan.transformed.work_schedule,
                plan.transformed.entry,
            );
            Arc::make_mut(&mut plan.transformed).contracts = Arc::new(
                SelectedProgramContracts::new(
                    Default::default(),
                    Some(target::TargetProfile::WindowsX64),
                    Arc::from([abi]),
                    publication(plan.transformed.entry),
                    execution,
                )
                .unwrap(),
            );
            let encoded = plan.encode().unwrap();
            let received = FixedViewCopyPlan::decode(&encoded).unwrap();
            assert_eq!(received, plan);
            assert_eq!(
                received.transformed.contracts.profile(),
                Some(target::TargetProfile::WindowsX64)
            );
            assert_eq!(received.transformed.contracts.function_abis().len(), 1);
            let current = &plan.transformed.contracts;
            let old = current.execution().unwrap();
            let mut roots = old.roots();
            roots.psi.program_fingerprint =
                terminal_psi::SemanticFingerprint::from_bytes([255; 32]);
            assert_ne!(roots.psi, plan.transformed.psi);
            let foreign = target_vocabulary::NativeExecutionFacts::seal(
                roots,
                Arc::new(old.callable_invocations().clone()),
                old.entry_callable_invocations().to_vec(),
            )
            .unwrap();
            let contracts = SelectedProgramContracts::new(
                current.borrowed_storage_layouts.clone(),
                current.profile(),
                Arc::clone(current.function_abis()),
                current.publication().clone(),
                Some(foreign),
            )
            .unwrap();
            Arc::make_mut(&mut plan.transformed).contracts = Arc::new(contracts);
            // The envelope and selected identity are freshly computed for the
            // foreign roots; the reader must reject their header correspondence.
            assert!(matches!(
                FixedViewCopyPlan::decode(&plan.encode().unwrap()),
                Err(Error::InvalidExecutionFacts(
                    target_vocabulary::NativeExecutionFactsError::Invalid(
                        "execution facts differ from the current selected header"
                    )
                ))
            ));
        }
    }

    #[test]
    fn foreign_abi_counts_and_duplicate_rows_are_refused() {
        let abi = abi(MachineId::new(1).unwrap(), true);
        let mut bytes = vec![0]; // Absent profile, then two canonical ABI rows.
        length(&mut bytes, 2);
        for _ in 0..2 {
            target_vocabulary::identity::function_abi::encode_function_abi(&mut bytes, &abi);
        }
        target_vocabulary::NativePublicationFacts::default().encode(&mut bytes);
        bytes.push(0); // Required execution-context option after complete publication.
        assert_eq!(
            decode(&mut Cursor::new(&bytes), Default::default()).unwrap_err(),
            Error::InvalidProgramContracts("function ABI rows are unordered or repeated")
        );
        bytes[1..9].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            decode(&mut Cursor::new(&bytes), Default::default()).unwrap_err(),
            Error::Truncated
        );
    }

    #[test]
    fn entry_conversion_unavailability_is_preserved_and_never_becomes_direct_transport() {
        let mut abi = abi(MachineId::new(1).unwrap(), true);
        abi.entry_scalar_codecs[1] = None;
        let contracts = SelectedProgramContracts::new(
            Default::default(),
            None,
            Arc::from([abi.clone()]),
            publication(abi.machine),
            execution(
                terminal_psi::TerminalPsiIdentity {
                    program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
                    vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                },
                semantic_vocabulary::WorkScheduleIdentity::new(1).unwrap(),
                abi.machine,
            ),
        )
        .unwrap();
        let mut bytes = Vec::new();
        encode(&mut bytes, &contracts);
        assert_eq!(
            decode(&mut Cursor::new(&bytes), Default::default()).unwrap(),
            contracts
        );
        for codec in [Some(0), Some(u64::MAX)] {
            abi.entry_scalar_codecs[1] = codec;
            assert_eq!(
                SelectedProgramContracts::new(
                    Default::default(),
                    None,
                    Arc::from([abi.clone()]),
                    publication(abi.machine),
                    None,
                )
                .unwrap_err(),
                "function ABI has an incompatible entry conversion"
            );
        }
    }

    #[test]
    fn foreign_profile_and_unclosed_abi_scalar_reach_their_own_checks() {
        let mut profile = vec![1];
        length(&mut profile, 1);
        profile.push(255);
        assert_eq!(
            decode(&mut Cursor::new(&profile), Default::default()).unwrap_err(),
            Error::InvalidForeignTargetProfile
        );
        let mut raw = abi(MachineId::new(1).unwrap(), false);
        raw.scalar_parameters[0].scalar_type = ScalarType::Integer(IntegerType::parameterized(
            IntegerSign::Unsigned,
            semantic_vocabulary::WidthParameter {
                owner: raw.machine,
                value: ValueId::new(3).unwrap(),
            },
        ));
        let mut bytes = vec![0];
        length(&mut bytes, 1);
        target_vocabulary::identity::function_abi::encode_function_abi(&mut bytes, &raw);
        assert_eq!(
            decode(&mut Cursor::new(&bytes), Default::default()).unwrap_err(),
            Error::InvalidIntegerType
        );
    }
}
