//! Canonical callable-entry and manifest wire encoding.

use super::*;

pub(super) fn encode_record_content(
    bytes: &mut Vec<u8>,
    record: &OptimizedOrdinaryCallableEntryRecord,
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    bytes.extend_from_slice(&record.source_artifact.bytes());
    bytes.extend_from_slice(&record.source_manifest.bytes());
    encode_psi(bytes, record.psi);
    bytes.extend_from_slice(&record.selections.bytes());
    encode_target(bytes, record.target);
    bytes.extend_from_slice(&record.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&record.selected.bytes());
    bytes.extend_from_slice(&record.register_homes.bytes());
    bytes.extend_from_slice(&record.physical_register_model.bytes());
    bytes.extend_from_slice(&record.exit_contract.bytes());
    bytes.extend_from_slice(&record.object.bytes());
    bytes.extend_from_slice(&record.object_container.bytes());
    bytes.extend_from_slice(&record.semantic_entry_symbol.get().to_le_bytes());
    encode_string(bytes, &record.semantic_entry_symbol_name)?;
    bytes.extend_from_slice(&record.semantic_entry_section_offset.to_le_bytes());
    bytes.extend_from_slice(&record.semantic_entry_byte_count.to_le_bytes());
    bytes.push(policy_tag(record.calling_policy));
    encode_length(bytes, record.parameters.len())?;
    for parameter in &record.parameters {
        bytes.extend_from_slice(&parameter.ordinal.to_le_bytes());
        bytes.extend_from_slice(&parameter.value.get().to_le_bytes());
        encode_scalar(bytes, parameter.scalar_type);
        encode_shape(bytes, parameter.shape);
        bytes.extend_from_slice(&parameter.virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&parameter.class.0.to_le_bytes());
        encode_register(bytes, parameter.abi_register);
        bytes.extend_from_slice(&parameter.fixed_view.0.to_le_bytes());
        bytes.extend_from_slice(&parameter.assigned_view.0.to_le_bytes());
        encode_units(bytes, &parameter.storage_units)?;
    }
    bytes.extend_from_slice(&record.result.declaration.id.get().to_le_bytes());
    encode_scalar(bytes, record.result.declaration.scalar_type);
    encode_shape(bytes, record.result.shape);
    encode_register(bytes, record.result.abi_register);
    bytes.extend_from_slice(&record.result.view.0.to_le_bytes());
    encode_units(bytes, &record.result.storage_units)?;
    encode_length(bytes, record.returns.len())?;
    for returned in &record.returns {
        bytes.extend_from_slice(&returned.edge.get().to_le_bytes());
        bytes.extend_from_slice(&returned.value.get().to_le_bytes());
        bytes.extend_from_slice(&returned.selected_instruction.0.to_le_bytes());
        bytes.extend_from_slice(&returned.virtual_register.0.to_le_bytes());
        bytes.extend_from_slice(&returned.view.0.to_le_bytes());
        encode_units(bytes, &returned.storage_units)?;
    }
    bytes.push(exit_policy_tag(record.exit_policy));
    bytes.push(1);
    encode_entry_assumption(bytes, record.entry_assumption);
    bytes.extend_from_slice(&record.stack_pointer.0.to_le_bytes());
    bytes.extend_from_slice(&record.stack_alignment.to_le_bytes());
    bytes.extend_from_slice(&record.red_zone_bytes.to_le_bytes());
    bytes.push(1);
    Ok(())
}

pub(super) fn decode_record_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedTerminalOrdinaryCallableEntryIdentity,
) -> Result<OptimizedOrdinaryCallableEntryRecord, OptimizedOrdinaryCallableEntryDecodeError> {
    let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
    let source_manifest = OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
    let psi = decode_psi(cursor)?;
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let semantic_entry = decode_id(cursor, MachineId::new)?;
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let register_homes = RegisterHomeIdentity::from_bytes(cursor.array()?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let exit_contract = WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
    let object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let object_container = RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
    let semantic_entry_symbol = ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedOrdinaryCallableEntryDecodeError::InvalidId)?;
    let semantic_entry_symbol_name = decode_string(cursor)?;
    let semantic_entry_section_offset = u64::from_le_bytes(cursor.array()?);
    let semantic_entry_byte_count = u64::from_le_bytes(cursor.array()?);
    let calling_policy = decode_policy(cursor)?;
    let parameter_count = cursor.length()?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for ordinal in 0..parameter_count {
        let encoded_ordinal = u64::from_le_bytes(cursor.array()?);
        if encoded_ordinal
            != u64::try_from(ordinal)
                .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?
        {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidId);
        }
        parameters.push(OptimizedOrdinaryCallableParameter {
            ordinal: encoded_ordinal,
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
            shape: decode_shape(cursor)?,
            virtual_register: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
            class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
            abi_register: decode_register(cursor)?,
            fixed_view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            assigned_view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            storage_units: decode_units(cursor)?,
        });
    }
    let result = OptimizedOrdinaryCallableResult {
        declaration: ValueDeclaration {
            id: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        },
        shape: decode_shape(cursor)?,
        abi_register: decode_register(cursor)?,
        view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
        storage_units: decode_units(cursor)?,
    };
    let return_count = cursor.length()?;
    let mut returns = Vec::with_capacity(return_count);
    for _ in 0..return_count {
        returns.push(OptimizedOrdinaryCallableReturn {
            edge: decode_id(cursor, EdgeId::new)?,
            value: decode_id(cursor, ValueId::new)?,
            selected_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
            virtual_register: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
            view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            storage_units: decode_units(cursor)?,
        });
    }
    let exit_policy = decode_exit_policy(cursor)?;
    let hardening_tag = cursor.byte()?;
    if hardening_tag != 1 {
        return Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownHardening(
            hardening_tag,
        ));
    }
    let entry_assumption = decode_entry_assumption(cursor)?;
    let stack_pointer = RegisterViewId(u16::from_le_bytes(cursor.array()?));
    let stack_alignment = u16::from_le_bytes(cursor.array()?);
    let red_zone_bytes = u16::from_le_bytes(cursor.array()?);
    let disposition = decode_disposition(cursor)?;
    Ok(OptimizedOrdinaryCallableEntryRecord {
        identity,
        source_artifact,
        source_manifest,
        psi,
        selections,
        target,
        semantic_entry,
        selected,
        register_homes,
        physical_register_model,
        exit_contract,
        object,
        object_container,
        semantic_entry_symbol,
        semantic_entry_symbol_name,
        semantic_entry_section_offset,
        semantic_entry_byte_count,
        calling_policy,
        parameters,
        result,
        returns,
        exit_policy,
        hardening: WholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
        entry_assumption,
        stack_pointer,
        stack_alignment,
        red_zone_bytes,
        disposition,
    })
}

pub(super) fn encode_manifest_content(
    bytes: &mut Vec<u8>,
    m: &OptimizedOrdinaryCallableEntryManifest,
) {
    bytes.push(1);
    bytes.extend_from_slice(&m.entry.bytes());
    bytes.extend_from_slice(&m.source_artifact.bytes());
    bytes.extend_from_slice(&m.source_manifest.bytes());
    encode_psi(bytes, m.psi);
    bytes.extend_from_slice(&m.selections.bytes());
    encode_target(bytes, m.target);
    bytes.extend_from_slice(&m.semantic_entry.get().to_le_bytes());
    bytes.extend_from_slice(&m.semantic_entry_symbol.get().to_le_bytes());
    bytes.extend_from_slice(&m.exit_contract.bytes());
    bytes.extend_from_slice(&m.parameter_count.to_le_bytes());
    bytes.extend_from_slice(&m.return_count.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&[1; 5]);
}
pub(super) fn encode_psi(bytes: &mut Vec<u8>, id: TerminalPsiIdentity) {
    bytes.extend_from_slice(&id.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(id.program_fingerprint.as_bytes());
}
pub(super) fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedOrdinaryCallableEntryDecodeError> {
    let marker = psi_terminal::VocabularyMarker::new(u16::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedOrdinaryCallableEntryDecodeError::InvalidId)?;
    Ok(TerminalPsiIdentity {
        vocabulary_marker: marker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes(cursor.array()?),
    })
}
pub(super) fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
    bytes.push(match target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 2,
    });
    bytes.push(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("target pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("target pointer alignment fits u64")
            .to_le_bytes(),
    );
}
pub(super) fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedOrdinaryCallableEntryDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::X86_64,
        2 => Architecture::Aarch64,
        tag => {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownArchitecture(tag));
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownObjectFormat(tag));
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?;
    let target = NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    };
    if target.pointer_size != 8
        || target.pointer_alignment != 8
        || !matches!(
            (target.architecture, target.object_format),
            (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::Coff)
                | (
                    Architecture::Aarch64,
                    ObjectFormat::Elf | ObjectFormat::MachO
                )
        )
    {
        return Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidTarget);
    }
    Ok(target)
}
pub(super) fn encode_scalar(bytes: &mut Vec<u8>, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => bytes.push(1),
        ScalarType::Integer(integer) => {
            bytes.push(2);
            bytes.push(match integer.carrier() {
                IntegerCarrier::Fixed => 1,
                IntegerCarrier::Address => 2,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
    }
}
pub(super) fn decode_scalar(
    cursor: &mut Cursor<'_>,
) -> Result<ScalarType, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(ScalarType::Boolean),
        2 => {
            let carrier = cursor.byte()?;
            let sign = match cursor.byte()? {
                1 => IntegerSign::Signed,
                2 => IntegerSign::Unsigned,
                _ => {
                    return Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType);
                }
            };
            let bits = u16::from_le_bytes(cursor.array()?);
            let integer = match carrier {
                1 => IntegerType::new(sign, bits),
                2 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                _ => {
                    return Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType);
                }
            }
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        _ => Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType),
    }
}
pub(super) fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    bytes.push(1);
    bytes.extend_from_slice(&shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&shape.alignment.to_le_bytes());
}
pub(super) fn decode_shape(
    cursor: &mut Cursor<'_>,
) -> Result<ValueShape, OptimizedOrdinaryCallableEntryDecodeError> {
    if cursor.byte()? != 1 {
        return Err(OptimizedOrdinaryCallableEntryDecodeError::InvalidScalarType);
    }
    Ok(ValueShape::integer(
        u16::from_le_bytes(cursor.array()?),
        u16::from_le_bytes(cursor.array()?),
    ))
}
pub(super) fn policy_tag(policy: CallingPolicy) -> u8 {
    match policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    }
}
pub(super) fn decode_policy(
    cursor: &mut Cursor<'_>,
) -> Result<CallingPolicy, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(CallingPolicy::MicrosoftX64),
        2 => Ok(CallingPolicy::SystemVAMD64),
        3 => Ok(CallingPolicy::Aapcs64),
        4 => Ok(CallingPolicy::LinuxSyscallX86_64),
        5 => Ok(CallingPolicy::LinuxSyscallAarch64),
        tag => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownCallingPolicy(tag)),
    }
}
pub(super) fn encode_register(bytes: &mut Vec<u8>, r: MachineRegister) {
    let (tag, index) = match r {
        MachineRegister::X86Rax => (1, 0),
        MachineRegister::X86Rcx => (2, 0),
        MachineRegister::X86Rdx => (3, 0),
        MachineRegister::X86Rbx => (4, 0),
        MachineRegister::X86Rsp => (5, 0),
        MachineRegister::X86Rbp => (6, 0),
        MachineRegister::X86Rsi => (7, 0),
        MachineRegister::X86Rdi => (8, 0),
        MachineRegister::X86R8 => (9, 0),
        MachineRegister::X86R9 => (10, 0),
        MachineRegister::X86R10 => (11, 0),
        MachineRegister::X86R11 => (12, 0),
        MachineRegister::X86R12 => (13, 0),
        MachineRegister::X86R13 => (14, 0),
        MachineRegister::X86R14 => (15, 0),
        MachineRegister::X86R15 => (16, 0),
        MachineRegister::X86Xmm(i) => (17, i),
        MachineRegister::Aarch64X(i) => (18, i),
        MachineRegister::Aarch64V(i) => (19, i),
    };
    bytes.push(tag);
    bytes.push(index);
}
pub(super) fn decode_register(
    cursor: &mut Cursor<'_>,
) -> Result<MachineRegister, OptimizedOrdinaryCallableEntryDecodeError> {
    let tag = cursor.byte()?;
    let i = cursor.byte()?;
    match (tag, i) {
        (1, 0) => Ok(MachineRegister::X86Rax),
        (2, 0) => Ok(MachineRegister::X86Rcx),
        (3, 0) => Ok(MachineRegister::X86Rdx),
        (4, 0) => Ok(MachineRegister::X86Rbx),
        (5, 0) => Ok(MachineRegister::X86Rsp),
        (6, 0) => Ok(MachineRegister::X86Rbp),
        (7, 0) => Ok(MachineRegister::X86Rsi),
        (8, 0) => Ok(MachineRegister::X86Rdi),
        (9, 0) => Ok(MachineRegister::X86R8),
        (10, 0) => Ok(MachineRegister::X86R9),
        (11, 0) => Ok(MachineRegister::X86R10),
        (12, 0) => Ok(MachineRegister::X86R11),
        (13, 0) => Ok(MachineRegister::X86R12),
        (14, 0) => Ok(MachineRegister::X86R13),
        (15, 0) => Ok(MachineRegister::X86R14),
        (16, 0) => Ok(MachineRegister::X86R15),
        (17, i) => Ok(MachineRegister::X86Xmm(i)),
        (18, i) => Ok(MachineRegister::Aarch64X(i)),
        (19, i) => Ok(MachineRegister::Aarch64V(i)),
        (tag, _) => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownRegister(
            tag,
        )),
    }
}
pub(super) fn exit_policy_tag(policy: WholeFunctionExitPolicy) -> u8 {
    match policy {
        WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
        WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1 => 5,
        WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1 => 6,
    }
}
pub(super) fn decode_exit_policy(
    cursor: &mut Cursor<'_>,
) -> Result<WholeFunctionExitPolicy, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1),
        2 => Ok(WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1),
        3 => Ok(WholeFunctionExitPolicy::Aapcs64FramelessLeafV1),
        4 => Ok(WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1),
        5 => Ok(WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1),
        6 => Ok(WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1),
        tag => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownExitPolicy(tag)),
    }
}
pub(super) fn encode_entry_assumption(bytes: &mut Vec<u8>, a: WholeFunctionEntryAssumption) {
    match a {
        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => bytes.push(1),
        WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            bytes.push(2);
            bytes.extend_from_slice(&link_register.0.to_le_bytes());
        }
    }
}
pub(super) fn decode_entry_assumption(
    cursor: &mut Cursor<'_>,
) -> Result<WholeFunctionEntryAssumption, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1),
        2 => Ok(WholeFunctionEntryAssumption::CallerLinkRegisterV1 {
            link_register: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
        }),
        tag => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownEntryAssumption(tag)),
    }
}
pub(super) fn decode_disposition(
    cursor: &mut Cursor<'_>,
) -> Result<OptimizedOrdinaryCallableEntryDisposition, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1),
        tag => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(tag)),
    }
}
pub(super) fn encode_units(
    bytes: &mut Vec<u8>,
    units: &[RegisterUnitId],
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    encode_length(bytes, units.len())?;
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
    Ok(())
}
pub(super) fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, OptimizedOrdinaryCallableEntryDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count);
    for _ in 0..count {
        units.push(RegisterUnitId(u16::from_le_bytes(cursor.array()?)));
    }
    Ok(units)
}
pub(super) fn encode_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    encode_length(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}
pub(super) fn decode_string(
    cursor: &mut Cursor<'_>,
) -> Result<String, OptimizedOrdinaryCallableEntryDecodeError> {
    let len = cursor.length()?;
    String::from_utf8(cursor.take(len)?.to_vec())
        .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::InvalidUtf8)
}
pub(super) fn encode_length(
    bytes: &mut Vec<u8>,
    len: usize,
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}
pub(super) fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl FnOnce(u64) -> Option<T>,
) -> Result<T, OptimizedOrdinaryCallableEntryDecodeError> {
    constructor(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedOrdinaryCallableEntryDecodeError::InvalidId)
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(super) fn take(
        &mut self,
        n: usize,
    ) -> Result<&'a [u8], OptimizedOrdinaryCallableEntryDecodeError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(OptimizedOrdinaryCallableEntryDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedOrdinaryCallableEntryDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::Truncated)
    }
    pub(super) fn byte(&mut self) -> Result<u8, OptimizedOrdinaryCallableEntryDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    pub(super) fn length(&mut self) -> Result<usize, OptimizedOrdinaryCallableEntryDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)
    }
    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}
