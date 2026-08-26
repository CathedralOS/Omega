//! Reconstructs imported, runtime-I/O, indirect-call, and syscall specifications.

use super::*;
use omega_target_operations::InstructionOperandLike;

pub(super) fn expected_outbound_call_spec(
    architecture: Architecture,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Option<CompilerInstructionSpec>, Diagnostic> {
    let spec: CompilerInstructionSpec = match kind {
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if !storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final immediate-import replay unexpectedly retained storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            43u8,
                            CompilerInstructionRelocationRecipe::ImmediateImport {
                                call_site,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final immediate-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            45u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) =
                            encode_float_parameter_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &plan,
                            )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final float-parameter import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            47u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "final dereferenced-result import replay unexpectedly retained argument storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            48u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final data-parameter import replay lost its address relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            49u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator: omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                                    library,
                                    symbol,
                                },
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if address_sites.len() < 2
                            || !address_sites.iter().any(|(_, target)| {
                                matches!(target, OutboundCallRelocationTarget::Storage(_))
                            })
                        {
                            return Err(Diagnostic::error(
                                "final result-bearing data-parameter import replay lost its relocation roots",
                            ));
                        }
                        (
                            None,
                            bytes,
                            50u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator: omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                                    library,
                                    symbol,
                                },
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            51u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            52u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            53u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        if !address_sites.iter().any(|(_, target)| {
                            matches!(target, OutboundCallRelocationTarget::Storage(_))
                        }) {
                            return Err(Diagnostic::error(
                                "final result-bearing authored float import replay lost its result root",
                            ));
                        }
                        (
                            None,
                            bytes,
                            54u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            55u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_scalar_parameter_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            56u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
                        operation_key,
                        operands,
                        data_symbols,
                        locator,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) =
                            encode_authored_aggregate_result_import(
                                architecture,
                                operation_key,
                                &operands,
                                &data_symbols,
                                &plan,
                            )?;
                        (
                            None,
                            bytes,
                            57u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
                        operands,
                        data_symbols,
                        mechanism,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_indirect_call_replay(
                            architecture,
                            &operands,
                            &data_symbols,
                            &mechanism,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            76u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
                        operation_key,
                        operands,
                        data_symbols,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, address_sites) = encode_open_create_import(
                            architecture,
                            operation_key,
                            &operands,
                            &data_symbols,
                            &plan,
                        )?;
                        (
                            None,
                            bytes,
                            58u8,
                            CompilerInstructionRelocationRecipe::PlannedImport {
                                call_site,
                                address_sites,
                                locator: omega_calling_conventions::HostImportLocator::StringBackedBootstrap {
                                    library,
                                    symbol,
                                },
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
                        target_region,
                        target_offset,
                        payload_offset,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            true,
                            target_offset,
                            payload_offset,
                            OutboundCallRelocationTarget::Storage(target_region),
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            59u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
                        source_region,
                        source_offset,
                        literal_symbol,
                        source_is_place,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let address_target = if source_is_place {
                            OutboundCallRelocationTarget::Storage(source_region)
                        } else {
                            OutboundCallRelocationTarget::Data(literal_symbol)
                        };
                        let replay = encode_runtime_byte_replay(
                            architecture,
                            false,
                            source_offset,
                            0,
                            address_target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            60u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
                        buffer_symbol,
                        target_region,
                        target_offset,
                        byte_capacity,
                        target,
                        mechanism,
                        plan,
                        get_std_handle,
                        ..
                    } => {
                        let replay = encode_runtime_line_read_replay(
                            architecture,
                            buffer_symbol,
                            target_region,
                            target_offset,
                            byte_capacity,
                            target,
                            &mechanism,
                            &plan,
                            get_std_handle.as_ref(),
                        )?;
                        (
                            None,
                            replay.bytes,
                            61u8,
                            CompilerInstructionRelocationRecipe::RuntimeTextBoundary {
                                call_sites: replay.call_sites,
                                address_sites: replay.address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_no_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "final storage-import replay lost its storage sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            44u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
                        operation_key,
                        operands,
                        library,
                        symbol,
                        plan,
                    } => {
                        let (bytes, call_site, storage_sites) = encode_integer_result_import(
                            architecture,
                            operation_key,
                            &operands,
                            &plan,
                        )?;
                        if storage_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "final result-bearing storage import replay lost its argument sites",
                            ));
                        }
                        (
                            None,
                            bytes,
                            46u8,
                            CompilerInstructionRelocationRecipe::StorageImport {
                                call_site,
                                storage_sites,
                                library,
                                symbol,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscall {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if !address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "no-result outbound syscall replay unexpectedly produced a result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            35u8,
                            CompilerInstructionRelocationRecipe::NoRelocations,
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "storage-argument outbound syscall replay lost its operand relocations",
                            ));
                        }
                        (
                            None,
                            bytes,
                            37u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            &operands,
                            &data_symbols,
                        )?;
                        if data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "data-argument outbound syscall replay lost its data-object relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            39u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() != 1 {
                            return Err(Diagnostic::error(
                                "result-bearing outbound syscall replay lost its result relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            36u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        if address_sites.len() < 2 {
                            return Err(Diagnostic::error(
                                "result-bearing storage-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            38u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallStorage {
                                address_sites,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
                        operands,
                        data_symbols,
                        number,
                        plan,
                    } => {
                        let (bytes, storage_sites) = encode_simple_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let Some((_, arguments)) = operands.split_first() else {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost its result operand",
                            ));
                        };
                        let data_sites = outbound_syscall_argument_data_sites(
                            architecture,
                            arguments,
                            &data_symbols,
                        )?;
                        if storage_sites.is_empty() || data_sites.is_empty() {
                            return Err(Diagnostic::error(
                                "result-bearing data-argument syscall replay lost a relocation",
                            ));
                        }
                        (
                            None,
                            bytes,
                            40u8,
                            CompilerInstructionRelocationRecipe::OutboundSyscallData {
                                address_sites: outbound_syscall_data_relocation_targets(
                                    storage_sites,
                                    data_sites,
                                ),
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
                        operands,
                        number,
                        plan,
                    } => {
                        let Some((result_region, _, _)) = operands
                            .first()
                            .and_then(InstructionOperandLike::runtime_scalar_integer)
                        else {
                            return Err(Diagnostic::error(
                                "timespec-result syscall replay lost its semantic result storage",
                            ));
                        };
                        let (bytes, address_site) = encode_linux_timespec_result_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let address_site = match architecture {
                            Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                Diagnostic::error(
                                    "x86 timespec-result relocation precedes its address opcode",
                                )
                            })?,
                            Architecture::Aarch64 => address_site,
                        };
                        (
                            None,
                            bytes,
                            41u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: result_region,
                                address_site,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
                        operands,
                        number,
                        plan,
                    } => {
                        let (bytes, address_site) = encode_linux_timespec_argument_outbound_syscall(
                            architecture,
                            &operands,
                            number,
                            &plan,
                        )?;
                        let relocation_recipe = match (
                            operands.first().and_then(InstructionOperandLike::runtime_scalar_integer),
                            address_site,
                        ) {
                            (Some((storage_region, _, _)), Some(address_site)) => {
                                CompilerInstructionRelocationRecipe::StaticStorage {
                                    storage_region,
                                    address_site: match architecture {
                                        Architecture::X86_64 => address_site.checked_sub(2).ok_or_else(|| {
                                            Diagnostic::error(
                                                "x86 timespec-argument relocation precedes its address opcode",
                                            )
                                        })?,
                                        Architecture::Aarch64 => address_site,
                                    },
                                }
                            }
                            (None, None) => CompilerInstructionRelocationRecipe::NoRelocations,
                            _ => {
                                return Err(Diagnostic::error(
                                    "timespec-argument syscall replay retained inconsistent operand relocation evidence",
                                ));
                            }
                        };
                        (None, bytes, 42u8, relocation_recipe)
                    }
        _ => return Ok(None),
    };

    Ok(Some(spec))
}
