use super::super::prelude::*;
use super::cursor::Cursor;
use super::error::FunctionRelativeOptimizationRealizationManifestDecodeError as Error;

pub(super) fn encode_optional_custody(
    bytes: &mut Vec<u8>,
    custody: Option<PostAllocationMachineOptimizationCustody>,
) {
    match custody {
        Some(custody) => {
            bytes.push(1);
            bytes.push(custody.optimization() as u8);
            bytes.extend_from_slice(&custody.artifact_identity());
            bytes.extend_from_slice(&custody.selections().bytes());
            bytes.extend_from_slice(&custody.post_allocation_machine_selections().bytes());
            bytes.extend_from_slice(&custody.source().bytes());
            bytes.extend_from_slice(&(custody.action_count() as u64).to_le_bytes());
            bytes.extend_from_slice(&custody.baseline_bytes().to_le_bytes());
            bytes.extend_from_slice(&custody.selected_bytes().to_le_bytes());
        }
        None => bytes.push(0),
    }
}

pub(super) fn decode_optional_custody(
    cursor: &mut Cursor<'_>,
) -> Result<Option<PostAllocationMachineOptimizationCustody>, Error> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => {
            let optimization = decode_optimization(cursor.byte()?)?;
            let artifact_identity = cursor.array()?;
            let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
            let post_allocation_machine_selections =
                OptimizationSelectionIdentity::from_bytes(cursor.array()?);
            let source =
                omega_machine_optimizer::PostAllocationMachineIdentity::from_bytes(cursor.array()?);
            let action_count = usize::try_from(u64::from_le_bytes(cursor.array()?))
                .map_err(|_| Error::ActionCountOverflow)?;
            let baseline_bytes = u64::from_le_bytes(cursor.array()?);
            let selected_bytes = u64::from_le_bytes(cursor.array()?);
            Ok(Some(PostAllocationMachineOptimizationCustody::from_parts(
                optimization,
                artifact_identity,
                selections,
                post_allocation_machine_selections,
                source,
                action_count,
                baseline_bytes,
                selected_bytes,
            )))
        }
        tag => Err(Error::UnknownPostAllocationMachineOptimizationStatus(tag)),
    }
}

fn decode_optimization(tag: u8) -> Result<Optimization, Error> {
    match tag {
        10 => Ok(Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1),
        13 => Ok(Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1),
        14 => Ok(Optimization::X86SelectXorZeroI64MaterializationV1),
        15 => Ok(Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1),
        16 => Ok(Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1),
        17 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1),
        18 => Ok(Optimization::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1),
        value => Err(Error::UnknownPostAllocationMachineOptimization(value)),
    }
}
