//! Canonical installation transport for compiler-private callback functions.

use function_identity::{MachineFunctionIdentity, StateKey};
use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::{
    InstallationError, InstalledCompilerPrivateFunction, Reader,
    fixed_integer_scalar_abi_codec::{
        decode_fixed_integer_scalar_abi, encode_fixed_integer_scalar_abi,
    },
    push_u16, push_u32, push_u64,
};

const CALLBACK_THUNK_ROLE: u8 = 1;

pub(super) fn encode_private_functions(
    bytes: &mut Vec<u8>,
    count: u32,
    functions: &[InstalledCompilerPrivateFunction],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for function in functions {
        let placement_index = function
            .identity
            .callback_thunk_placement_index()
            .ok_or(InstallationError::InvalidCompilerPrivateFunctionIdentity)?;
        let continuation = function.identity.associated_source_continuation();
        bytes.push(CALLBACK_THUNK_ROLE);
        bytes.extend_from_slice(&[0; 7]);
        push_u32(bytes, continuation.machine.arena_index());
        push_u32(bytes, continuation.machine.generation());
        push_u32(bytes, continuation.state.arena_index());
        push_u32(bytes, continuation.state.generation());
        push_u64(
            bytes,
            u64::try_from(continuation.segment_index)
                .map_err(|_| InstallationError::InvalidCompilerPrivateFunctionIdentity)?,
        );
        push_u64(
            bytes,
            u64::try_from(placement_index)
                .map_err(|_| InstallationError::InvalidCompilerPrivateFunctionIdentity)?,
        );
        push_u16(bytes, function.source_psi.vocabulary_marker.get());
        push_u16(bytes, 0);
        bytes.extend_from_slice(function.source_psi.program_fingerprint.as_bytes());
        push_u64(bytes, function.machine.get());
        push_u64(
            bytes,
            u64::try_from(function.text_offset)
                .map_err(|_| InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(function.byte_count)
                .map_err(|_| InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?,
        );
        encode_fixed_integer_scalar_abi(bytes, Some(&function.fixed_integer_scalar_abi))?;
    }
    Ok(())
}

pub(super) fn decode_private_functions(
    reader: &mut Reader<'_>,
) -> Result<Vec<InstalledCompilerPrivateFunction>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyCompilerPrivateFunctions)?;
    if count > 1 {
        return Err(InstallationError::TooManyCompilerPrivateFunctions);
    }
    let mut functions = Vec::with_capacity(count);
    for _ in 0..count {
        let role = reader.u8()?;
        if role != CALLBACK_THUNK_ROLE {
            return Err(InstallationError::InvalidCompilerPrivateFunctionRoleTag(
                role,
            ));
        }
        if reader.take(7)? != [0; 7] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let continuation = StateKey {
            machine: SymbolHandle::from_parts(reader.u32()?, reader.u32()?),
            state: SymbolHandle::from_parts(reader.u32()?, reader.u32()?),
            segment_index: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InvalidCompilerPrivateFunctionIdentity)?,
        };
        let placement_index = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InvalidCompilerPrivateFunctionIdentity)?;
        let identity = MachineFunctionIdentity::callback_thunk(continuation, placement_index)
            .ok_or(InstallationError::InvalidCompilerPrivateFunctionIdentity)?;
        let vocabulary_raw = reader.u16()?;
        let vocabulary_marker = VocabularyMarker::new(vocabulary_raw).ok_or(
            InstallationError::UnsupportedVocabularyMarker(vocabulary_raw),
        )?;
        if reader.u16()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let source_psi = TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(reader.array()?),
        };
        let machine = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::InvalidCompilerPrivateFunctionIdentity)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?;
        let fixed_integer_scalar_abi = decode_fixed_integer_scalar_abi(reader)?
            .ok_or(InstallationError::MissingCompilerPrivateFunctionAbi)?;
        functions.push(InstalledCompilerPrivateFunction {
            identity,
            source_psi,
            machine,
            fixed_integer_scalar_abi,
            text_offset,
            byte_count,
        });
    }
    Ok(functions)
}

#[cfg(test)]
mod tests {
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};
    use target::NativeTarget;
    use target_operations::{FixedIntegerScalarAbiValue, FixedIntegerScalarFunctionAbi};

    use super::*;

    fn private_function() -> InstalledCompilerPrivateFunction {
        let target = NativeTarget::windows_x64();
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
        let shape = ValueShape::integer(8, 8);
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![shape],
                result: Some(shape),
            },
        )
        .expect("one-u64 callback ABI");
        let parameter_placement = call_plan.parameters[0].clone();
        let result_placement = call_plan.result.clone().expect("result placement");
        InstalledCompilerPrivateFunction {
            identity: MachineFunctionIdentity::callback_thunk(
                StateKey {
                    machine: SymbolHandle::from_parts(11, 2),
                    state: SymbolHandle::from_parts(13, 3),
                    segment_index: 5,
                },
                7,
            )
            .expect("callback identity"),
            source_psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
            },
            machine: MachineId::new(17).expect("private machine"),
            fixed_integer_scalar_abi: FixedIntegerScalarFunctionAbi {
                call_plan,
                parameters: vec![FixedIntegerScalarAbiValue {
                    value: ValueId::new(19).expect("parameter value"),
                    scalar_type,
                    placement: parameter_placement,
                }],
                result: FixedIntegerScalarAbiValue {
                    value: ValueId::new(23).expect("result value"),
                    scalar_type,
                    placement: result_placement,
                },
            },
            text_offset: 29,
            byte_count: 31,
        }
    }

    #[test]
    fn private_callback_row_round_trips_every_identity_axis() {
        let function = private_function();
        let mut bytes = Vec::new();
        encode_private_functions(&mut bytes, 1, std::slice::from_ref(&function))
            .expect("encode private callback");
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_private_functions(&mut reader),
            Ok(vec![function.clone()])
        );
        assert_eq!(reader.remaining(), 0);

        let mut drifted_source = bytes.clone();
        drifted_source[48] ^= 0x80;
        let mut reader = Reader::new(&drifted_source);
        let decoded = decode_private_functions(&mut reader).expect("source-Psi drift decodes");
        assert_ne!(decoded[0].source_psi, function.source_psi);

        let mut drifted_interval = bytes.clone();
        drifted_interval[88] ^= 1;
        let mut reader = Reader::new(&drifted_interval);
        let decoded = decode_private_functions(&mut reader).expect("interval drift decodes");
        assert_ne!(decoded[0].text_offset, function.text_offset);
    }

    #[test]
    fn private_callback_codec_rejects_fabricated_roles_and_cardinality() {
        let function = private_function();
        let mut bytes = Vec::new();
        encode_private_functions(&mut bytes, 1, &[function]).expect("encode private callback");

        let mut duplicate_count = bytes.clone();
        duplicate_count[..4].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            decode_private_functions(&mut Reader::new(&duplicate_count)),
            Err(InstallationError::TooManyCompilerPrivateFunctions)
        );

        let mut wrong_role = bytes.clone();
        wrong_role[4] = 2;
        assert_eq!(
            decode_private_functions(&mut Reader::new(&wrong_role)),
            Err(InstallationError::InvalidCompilerPrivateFunctionRoleTag(2))
        );

        let mut invalid_continuation = bytes.clone();
        invalid_continuation[12..16].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            decode_private_functions(&mut Reader::new(&invalid_continuation)),
            Err(InstallationError::InvalidCompilerPrivateFunctionIdentity)
        );

        let mut missing_abi = bytes;
        missing_abi[104] = 0;
        assert_eq!(
            decode_private_functions(&mut Reader::new(&missing_abi)),
            Err(InstallationError::MissingCompilerPrivateFunctionAbi)
        );
    }
}
