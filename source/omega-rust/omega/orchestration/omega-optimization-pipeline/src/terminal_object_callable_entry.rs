use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_object_file::{
    TerminalObjectLocalSymbolId, TerminalRelocationFreeObjectSymbolLinkage,
    TerminalRelocationFreeObjectSymbolRole,
};
use omega_optimization_core::{
    OptimizationSelectionIdentity, OptimizedTerminalObjectArtifactIdentity,
    OptimizedTerminalObjectArtifactManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity,
    OptimizedTerminalOrdinaryCallableEntryManifestIdentity,
    TerminalRelocationFreeObjectContainerIdentity, TerminalRelocationFreeObjectPlanIdentity,
};
use omega_regalloc::{TerminalRegisterHomeIdentity, terminal_register_home_identity};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_terminal_selected_instructions::{
    TerminalSelectedInstructionId, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedTerminator, TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use psi_core::{EdgeId, IntegerCarrier, IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
use psi_terminal::{TerminalMachineResult, TerminalPsiIdentity, Terminator, ValueDeclaration};

use crate::{
    OptimizedTerminalObjectArtifactError, StagedValidatedOptimizedTerminalObjectArtifact,
    TerminalWholeFunctionEntryAssumption, TerminalWholeFunctionExitContractIdentity,
    TerminalWholeFunctionExitPolicy, TerminalWholeFunctionHardeningPolicy,
    validate_optimized_terminal_object_artifact,
};

const RECORD_MAGIC: &[u8; 8] = b"OMGOER\0\0";
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOEM\0\0";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryStage {
    ValidatedOptimizedTerminalOrdinaryCallableEntryV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryDisposition {
    ExternalProcessEntryBridgeRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableParameter {
    pub ordinal: u64,
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub virtual_register: TerminalVirtualRegisterId,
    pub class: RegisterClassId,
    pub abi_register: MachineRegister,
    pub fixed_view: RegisterViewId,
    pub assigned_view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableResult {
    pub declaration: ValueDeclaration,
    pub shape: ValueShape,
    pub abi_register: MachineRegister,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableReturn {
    pub edge: EdgeId,
    pub value: ValueId,
    pub selected_instruction: TerminalSelectedInstructionId,
    pub virtual_register: TerminalVirtualRegisterId,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableEntryRecord {
    pub identity: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedTerminalObjectArtifactIdentity,
    pub source_manifest: OptimizedTerminalObjectArtifactManifestIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub register_homes: TerminalRegisterHomeIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub exit_contract: TerminalWholeFunctionExitContractIdentity,
    pub object: TerminalRelocationFreeObjectPlanIdentity,
    pub object_container: TerminalRelocationFreeObjectContainerIdentity,
    pub semantic_entry_symbol: TerminalObjectLocalSymbolId,
    pub semantic_entry_symbol_name: String,
    pub semantic_entry_section_offset: u64,
    pub semantic_entry_byte_count: u64,
    pub calling_policy: CallingPolicy,
    pub parameters: Vec<OptimizedTerminalOrdinaryCallableParameter>,
    pub result: OptimizedTerminalOrdinaryCallableResult,
    pub returns: Vec<OptimizedTerminalOrdinaryCallableReturn>,
    pub exit_policy: TerminalWholeFunctionExitPolicy,
    pub hardening: TerminalWholeFunctionHardeningPolicy,
    pub entry_assumption: TerminalWholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub disposition: OptimizedTerminalOrdinaryCallableEntryDisposition,
}

impl OptimizedTerminalOrdinaryCallableEntryRecord {
    pub fn recomputed_identity(
        &self,
    ) -> Result<
        OptimizedTerminalOrdinaryCallableEntryIdentity,
        OptimizedTerminalOrdinaryCallableEntryError,
    > {
        let mut canonical = b"omega.optimized-terminal-ordinary-callable-entry.v1\0".to_vec();
        encode_record_content(&mut canonical, self)?;
        Ok(OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(&canonical))
    }

    pub fn encode(&self) -> Result<Vec<u8>, OptimizedTerminalOrdinaryCallableEntryError> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_record_content(&mut bytes, self)?;
        Ok(bytes)
    }

    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != RECORD_MAGIC {
            return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(
                OptimizedTerminalOrdinaryCallableEntryDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let record = decode_record_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::TrailingBytes);
        }
        if record
            .recomputed_identity()
            .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)?
            != identity
        {
            return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableEntryManifest {
    pub identity: OptimizedTerminalOrdinaryCallableEntryManifestIdentity,
    pub stage: OptimizedTerminalOrdinaryCallableEntryStage,
    pub entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedTerminalObjectArtifactIdentity,
    pub source_manifest: OptimizedTerminalObjectArtifactManifestIdentity,
    pub terminal_psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: TerminalObjectLocalSymbolId,
    pub exit_contract: TerminalWholeFunctionExitContractIdentity,
    pub parameter_count: u64,
    pub return_count: u64,
    pub disposition: OptimizedTerminalOrdinaryCallableEntryDisposition,
    pub wrapper_bytes: OptimizedTerminalOrdinaryCallableEntryUnavailableData,
    pub relocations: OptimizedTerminalOrdinaryCallableEntryUnavailableData,
    pub executable_image: OptimizedTerminalOrdinaryCallableEntryUnavailableData,
    pub installation: OptimizedTerminalOrdinaryCallableEntryUnavailableData,
    pub publication: OptimizedTerminalOrdinaryCallableEntryUnavailableData,
}

impl OptimizedTerminalOrdinaryCallableEntryManifest {
    pub fn recomputed_identity(&self) -> OptimizedTerminalOrdinaryCallableEntryManifestIdentity {
        let mut bytes = b"omega.optimized-terminal-ordinary-callable-entry-manifest.v1\0".to_vec();
        encode_manifest_content(&mut bytes, self);
        OptimizedTerminalOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(&bytes)
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MANIFEST_MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_manifest_content(&mut bytes, self);
        bytes
    }
    pub fn decode(
        encoded: &[u8],
    ) -> Result<Self, OptimizedTerminalOrdinaryCallableEntryManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(
                OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity =
            OptimizedTerminalOrdinaryCallableEntryManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? { 1 => OptimizedTerminalOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1, tag => return Err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::UnknownStage(tag)) };
        let entry = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let source_artifact = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
        let source_manifest =
            OptimizedTerminalObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let terminal_psi = decode_terminal_psi(&mut cursor)
            .map_err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::Record)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)
            .map_err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry = decode_id(&mut cursor, MachineId::new)
            .map_err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry_symbol =
            TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?)).ok_or(
                OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::Record(
                    OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidId,
                ),
            )?;
        let exit_contract = TerminalWholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let parameter_count = u64::from_le_bytes(cursor.array()?);
        let return_count = u64::from_le_bytes(cursor.array()?);
        decode_disposition(&mut cursor)
            .map_err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::Record)?;
        for _ in 0..5 {
            if cursor.byte()? != 1 {
                return Err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus);
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedTerminalOrdinaryCallableEntryUnavailableData::Unavailable;
        let manifest = Self { identity, stage, entry, source_artifact, source_manifest, terminal_psi, selections, target, semantic_entry, semantic_entry_symbol, exit_contract, parameter_count, return_count, disposition: OptimizedTerminalOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1, wrapper_bytes: unavailable, relocations: unavailable, executable_image: unavailable, installation: unavailable, publication: unavailable };
        if manifest.recomputed_identity() != identity {
            return Err(
                OptimizedTerminalOrdinaryCallableEntryManifestDecodeError::IdentityMismatch,
            );
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedTerminalOrdinaryCallableEntryManifest {
    record: OptimizedTerminalOrdinaryCallableEntryManifest,
}
impl ValidatedOptimizedTerminalOrdinaryCallableEntryManifest {
    pub const fn record(&self) -> &OptimizedTerminalOrdinaryCallableEntryManifest {
        &self.record
    }
    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut OptimizedTerminalOrdinaryCallableEntryManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "ordinary-callable entry custody retains its complete optimized Terminal object source"]
pub struct StagedValidatedOptimizedTerminalOrdinaryCallableEntry {
    source: StagedValidatedOptimizedTerminalObjectArtifact,
    entry: OptimizedTerminalOrdinaryCallableEntryRecord,
    manifest: ValidatedOptimizedTerminalOrdinaryCallableEntryManifest,
    custody: OptimizedTerminalOrdinaryCallableEntryCustodyReceipt,
}
impl StagedValidatedOptimizedTerminalOrdinaryCallableEntry {
    pub const fn source(&self) -> &StagedValidatedOptimizedTerminalObjectArtifact {
        &self.source
    }
    pub const fn entry(&self) -> &OptimizedTerminalOrdinaryCallableEntryRecord {
        &self.entry
    }
    pub const fn manifest(&self) -> &ValidatedOptimizedTerminalOrdinaryCallableEntryManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> OptimizedTerminalOrdinaryCallableEntryCustodyReceipt {
        self.custody
    }
    #[cfg(test)]
    pub(crate) fn entry_mut(&mut self) -> &mut OptimizedTerminalOrdinaryCallableEntryRecord {
        &mut self.entry
    }
    #[cfg(test)]
    pub(crate) fn manifest_mut(
        &mut self,
    ) -> &mut ValidatedOptimizedTerminalOrdinaryCallableEntryManifest {
        &mut self.manifest
    }
    #[cfg(test)]
    pub(crate) fn corrupt_custody_for_test(&mut self) {
        self.custody.manifest =
            OptimizedTerminalOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"bad");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedTerminalOrdinaryCallableEntryCustodyReceipt {
    source_artifact: OptimizedTerminalObjectArtifactIdentity,
    entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    manifest: OptimizedTerminalOrdinaryCallableEntryManifestIdentity,
}
impl OptimizedTerminalOrdinaryCallableEntryCustodyReceipt {
    pub const fn source_artifact(self) -> OptimizedTerminalObjectArtifactIdentity {
        self.source_artifact
    }
    pub const fn entry(self) -> OptimizedTerminalOrdinaryCallableEntryIdentity {
        self.entry
    }
    pub const fn manifest(self) -> OptimizedTerminalOrdinaryCallableEntryManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryError {
    Source(OptimizedTerminalObjectArtifactError),
    UnsupportedSignature,
    AbiPlan,
    RootMismatch,
    MissingEntry,
    MissingParameter(ValueId),
    MissingHome(TerminalVirtualRegisterId),
    MissingView(RegisterViewId),
    MissingReturn(EdgeId),
    EntrySymbolMismatch,
    LengthOverflow,
    RecordMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}
impl std::fmt::Display for OptimizedTerminalOrdinaryCallableEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "optimized Terminal ordinary-callable entry failed: {self:?}"
        )
    }
}
impl std::error::Error for OptimizedTerminalOrdinaryCallableEntryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    InvalidTarget,
    InvalidId,
    InvalidScalarType,
    InvalidIntegerType,
    UnknownCallingPolicy(u8),
    UnknownRegister(u8),
    UnknownExitPolicy(u8),
    UnknownHardening(u8),
    UnknownEntryAssumption(u8),
    UnknownDisposition(u8),
    InvalidUtf8,
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedTerminalOrdinaryCallableEntryManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownUnavailableStatus,
    Record(OptimizedTerminalOrdinaryCallableEntryDecodeError),
    IdentityMismatch,
    TrailingBytes,
}

pub fn stage_validated_optimized_terminal_ordinary_callable_entry(
    source: StagedValidatedOptimizedTerminalObjectArtifact,
) -> Result<
    StagedValidatedOptimizedTerminalOrdinaryCallableEntry,
    OptimizedTerminalOrdinaryCallableEntryError,
> {
    validate_optimized_terminal_object_artifact(&source)
        .map_err(OptimizedTerminalOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&source)?;
    let manifest = manifest(&entry)?;
    let custody = receipt(&entry, &manifest);
    Ok(StagedValidatedOptimizedTerminalOrdinaryCallableEntry {
        source,
        entry,
        manifest: ValidatedOptimizedTerminalOrdinaryCallableEntryManifest { record: manifest },
        custody,
    })
}

pub fn validate_optimized_terminal_ordinary_callable_entry(
    staged: &StagedValidatedOptimizedTerminalOrdinaryCallableEntry,
) -> Result<
    OptimizedTerminalOrdinaryCallableEntryCustodyReceipt,
    OptimizedTerminalOrdinaryCallableEntryError,
> {
    validate_optimized_terminal_object_artifact(&staged.source)
        .map_err(OptimizedTerminalOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&staged.source)?;
    if entry != staged.entry {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::RecordMismatch);
    }
    let manifest = manifest(&entry)?;
    if manifest != staged.manifest.record {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::ManifestMismatch);
    }
    let receipt = receipt(&entry, &manifest);
    if receipt != staged.custody {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::ReceiptMismatch);
    }
    Ok(receipt)
}

fn reconstruct(
    source: &StagedValidatedOptimizedTerminalObjectArtifact,
) -> Result<OptimizedTerminalOrdinaryCallableEntryRecord, OptimizedTerminalOrdinaryCallableEntryError>
{
    let artifact = source.artifact();
    let object_stage = source.source();
    let object = object_stage.object();
    let fragment = object_stage.source().source();
    let route = fragment.source();
    let module = route.verified_input().context().terminal_module();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingEntry)?;
    if machine.attachment.is_some()
        || !machine.structural_parameters.is_empty()
        || !matches!(machine.result, TerminalMachineResult::Scalar(_))
    {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_declaration = *machine
        .result
        .scalar_ref()
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature)?;
    let signature = CallSignature {
        parameters: machine
            .parameters
            .iter()
            .map(|p| scalar_shape(p.scalar_type))
            .collect::<Result<_, _>>()?,
        result: Some(scalar_shape(result_declaration.scalar_type)?),
    };
    let calling_policy = CallingPolicy::native_for_target(artifact.target);
    let call = evaluate_call_plan(calling_policy, &signature)
        .map_err(|_| OptimizedTerminalOrdinaryCallableEntryError::AbiPlan)?;
    let selected = route.selected_plan();
    let selected_function = selected
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingEntry)?;
    let homes = route.register_homes();
    let home_function = homes
        .plan()
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingEntry)?;
    let environment = route.register_environment();
    let physical = environment.physical();
    let mut parameters = Vec::with_capacity(machine.parameters.len());
    for (index, (declaration, placement)) in
        machine.parameters.iter().zip(&call.parameters).enumerate()
    {
        let [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] = placement.locations.as_slice()
        else {
            return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature);
        };
        if *byte_size != placement.shape.byte_size {
            return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature);
        }
        let virtual_register = selected_function.virtual_registers.iter().find(|vreg| matches!(vreg.origin, TerminalVirtualRegisterOrigin::EntryParameter { source_value, parameter_index } if source_value == declaration.id && parameter_index == index)).ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingParameter(declaration.id))?;
        let fixed_view = virtual_register
            .entry_fixed_view
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingParameter(declaration.id))?;
        let expected_view = environment.fixed_register_view(*register).ok_or(
            OptimizedTerminalOrdinaryCallableEntryError::MissingView(fixed_view),
        )?;
        let home = home_function
            .assignments
            .iter()
            .find(|home| home.virtual_register == virtual_register.id)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingHome(
                virtual_register.id,
            ))?;
        if fixed_view != expected_view
            || home.view != fixed_view
            || home.class != virtual_register.class
        {
            return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
        }
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed_view)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingView(
                fixed_view,
            ))?;
        parameters.push(OptimizedTerminalOrdinaryCallableParameter {
            ordinal: u64::try_from(index)
                .map_err(|_| OptimizedTerminalOrdinaryCallableEntryError::LengthOverflow)?,
            value: declaration.id,
            scalar_type: declaration.scalar_type,
            shape: placement.shape,
            virtual_register: virtual_register.id,
            class: virtual_register.class,
            abi_register: *register,
            fixed_view,
            assigned_view: home.view,
            storage_units: view.units.clone(),
        });
    }
    let result_placement = call
        .result
        .as_ref()
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature)?;
    let [
        ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result_placement.locations.as_slice()
    else {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature);
    };
    if *byte_size != result_placement.shape.byte_size {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_view = environment
        .fixed_register_view(*result_register)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature)?;
    let result_model_view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == result_view)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingView(
            result_view,
        ))?;
    let exit = route.exit_contract().contract();
    let exit_function = exit
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingEntry)?;
    if exit.result_view != result_view || exit.physical_register_model != physical.identity() {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
    }
    let mut returns = Vec::new();
    for block in &machine.blocks {
        let Terminator::Return { edge, value, .. } = block.terminator else {
            continue;
        };
        let selected_return = selected_function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                TerminalSelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } if *psi_return_edge == edge => Some(instruction),
                _ => None,
            })
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingReturn(
                edge,
            ))?;
        let operand = selected_return.operands.first().ok_or(
            OptimizedTerminalOrdinaryCallableEntryError::MissingReturn(edge),
        )?;
        let vreg = selected_function
            .virtual_registers
            .iter()
            .find(|vreg| vreg.id == operand.virtual_register)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingReturn(
                edge,
            ))?;
        if origin_value(vreg.origin) != value {
            return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
        }
        let evidence = exit_function
            .returns
            .iter()
            .find(|returned| returned.psi_return_edge == edge)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryError::MissingReturn(
                edge,
            ))?;
        if evidence.instruction != selected_return.id
            || evidence.result_virtual_register != vreg.id
            || evidence.result_view != result_view
            || evidence.result_units != result_model_view.units
        {
            return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
        }
        returns.push(OptimizedTerminalOrdinaryCallableReturn {
            edge,
            value,
            selected_instruction: selected_return.id,
            virtual_register: vreg.id,
            view: evidence.result_view,
            storage_units: evidence.result_units.clone(),
        });
    }
    if returns.is_empty() || returns.len() != exit_function.returns.len() {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
    }
    let symbol = object
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == object.semantic_entry_symbol)
        .ok_or(OptimizedTerminalOrdinaryCallableEntryError::EntrySymbolMismatch)?;
    if symbol.machine != machine.id
        || symbol.linkage != TerminalRelocationFreeObjectSymbolLinkage::ObjectLocalV1
        || symbol.role != TerminalRelocationFreeObjectSymbolRole::SemanticEntryV1
    {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::EntrySymbolMismatch);
    }
    if artifact.semantic_entry != module.entry
        || selected.entry != module.entry
        || object.semantic_entry != module.entry
        || exit.selected != object.selected
    {
        return Err(OptimizedTerminalOrdinaryCallableEntryError::RootMismatch);
    }
    let mut record = OptimizedTerminalOrdinaryCallableEntryRecord {
        identity: OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"pending"),
        source_artifact: artifact.identity,
        source_manifest: source.manifest().record().identity,
        terminal_psi: artifact.terminal_psi,
        selections: artifact.selections,
        target: artifact.target,
        semantic_entry: machine.id,
        selected: exit.selected,
        register_homes: terminal_register_home_identity(homes.plan()),
        physical_register_model: physical.identity(),
        exit_contract: route.exit_contract().identity(),
        object: object.identity,
        object_container: object_stage.container().identity,
        semantic_entry_symbol: symbol.symbol,
        semantic_entry_symbol_name: symbol.name.clone(),
        semantic_entry_section_offset: symbol.section_offset,
        semantic_entry_byte_count: symbol.byte_count,
        calling_policy,
        parameters,
        result: OptimizedTerminalOrdinaryCallableResult {
            declaration: result_declaration,
            shape: result_placement.shape,
            abi_register: *result_register,
            view: result_view,
            storage_units: result_model_view.units.clone(),
        },
        returns,
        exit_policy: exit.policy,
        hardening: exit.hardening,
        entry_assumption: exit.entry_assumption,
        stack_pointer: exit.stack_pointer,
        stack_alignment: exit.stack_alignment,
        red_zone_bytes: exit.red_zone_bytes,
        disposition:
            OptimizedTerminalOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
    };
    record.identity = record.recomputed_identity()?;
    Ok(record)
}

fn manifest(
    entry: &OptimizedTerminalOrdinaryCallableEntryRecord,
) -> Result<
    OptimizedTerminalOrdinaryCallableEntryManifest,
    OptimizedTerminalOrdinaryCallableEntryError,
> {
    let unavailable = OptimizedTerminalOrdinaryCallableEntryUnavailableData::Unavailable;
    let mut manifest = OptimizedTerminalOrdinaryCallableEntryManifest { identity: OptimizedTerminalOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"pending"), stage: OptimizedTerminalOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1, entry: entry.identity, source_artifact: entry.source_artifact, source_manifest: entry.source_manifest, terminal_psi: entry.terminal_psi, selections: entry.selections, target: entry.target, semantic_entry: entry.semantic_entry, semantic_entry_symbol: entry.semantic_entry_symbol, exit_contract: entry.exit_contract, parameter_count: u64::try_from(entry.parameters.len()).map_err(|_| OptimizedTerminalOrdinaryCallableEntryError::LengthOverflow)?, return_count: u64::try_from(entry.returns.len()).map_err(|_| OptimizedTerminalOrdinaryCallableEntryError::LengthOverflow)?, disposition: entry.disposition, wrapper_bytes: unavailable, relocations: unavailable, executable_image: unavailable, installation: unavailable, publication: unavailable };
    manifest.identity = manifest.recomputed_identity();
    Ok(manifest)
}
fn receipt(
    entry: &OptimizedTerminalOrdinaryCallableEntryRecord,
    manifest: &OptimizedTerminalOrdinaryCallableEntryManifest,
) -> OptimizedTerminalOrdinaryCallableEntryCustodyReceipt {
    OptimizedTerminalOrdinaryCallableEntryCustodyReceipt {
        source_artifact: entry.source_artifact,
        entry: entry.identity,
        manifest: manifest.identity,
    }
}
fn scalar_shape(
    scalar: ScalarType,
) -> Result<ValueShape, OptimizedTerminalOrdinaryCallableEntryError> {
    let bytes = match scalar {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            integer.bits().div_ceil(8)
        }
        _ => return Err(OptimizedTerminalOrdinaryCallableEntryError::UnsupportedSignature),
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}
fn origin_value(origin: TerminalVirtualRegisterOrigin) -> ValueId {
    match origin {
        TerminalVirtualRegisterOrigin::EntryParameter { source_value, .. }
        | TerminalVirtualRegisterOrigin::InstructionResult { source_value, .. }
        | TerminalVirtualRegisterOrigin::LegalizationTemporary { source_value, .. } => source_value,
    }
}

fn encode_record_content(
    bytes: &mut Vec<u8>,
    record: &OptimizedTerminalOrdinaryCallableEntryRecord,
) -> Result<(), OptimizedTerminalOrdinaryCallableEntryError> {
    bytes.extend_from_slice(&record.source_artifact.bytes());
    bytes.extend_from_slice(&record.source_manifest.bytes());
    encode_terminal_psi(bytes, record.terminal_psi);
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

fn decode_record_content(
    cursor: &mut Cursor<'_>,
    identity: OptimizedTerminalOrdinaryCallableEntryIdentity,
) -> Result<
    OptimizedTerminalOrdinaryCallableEntryRecord,
    OptimizedTerminalOrdinaryCallableEntryDecodeError,
> {
    let source_artifact = OptimizedTerminalObjectArtifactIdentity::from_bytes(cursor.array()?);
    let source_manifest =
        OptimizedTerminalObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
    let terminal_psi = decode_terminal_psi(cursor)?;
    let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let semantic_entry = decode_id(cursor, MachineId::new)?;
    let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let register_homes = TerminalRegisterHomeIdentity::from_bytes(cursor.array()?);
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let exit_contract = TerminalWholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
    let object = TerminalRelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
    let object_container =
        TerminalRelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
    let semantic_entry_symbol =
        TerminalObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidId)?;
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
                .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)?
        {
            return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidId);
        }
        parameters.push(OptimizedTerminalOrdinaryCallableParameter {
            ordinal: encoded_ordinal,
            value: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
            shape: decode_shape(cursor)?,
            virtual_register: TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
            class: RegisterClassId(u16::from_le_bytes(cursor.array()?)),
            abi_register: decode_register(cursor)?,
            fixed_view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            assigned_view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            storage_units: decode_units(cursor)?,
        });
    }
    let result = OptimizedTerminalOrdinaryCallableResult {
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
        returns.push(OptimizedTerminalOrdinaryCallableReturn {
            edge: decode_id(cursor, EdgeId::new)?,
            value: decode_id(cursor, ValueId::new)?,
            selected_instruction: TerminalSelectedInstructionId(u32::from_le_bytes(
                cursor.array()?,
            )),
            virtual_register: TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
            view: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
            storage_units: decode_units(cursor)?,
        });
    }
    let exit_policy = decode_exit_policy(cursor)?;
    let hardening_tag = cursor.byte()?;
    if hardening_tag != 1 {
        return Err(
            OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownHardening(hardening_tag),
        );
    }
    let entry_assumption = decode_entry_assumption(cursor)?;
    let stack_pointer = RegisterViewId(u16::from_le_bytes(cursor.array()?));
    let stack_alignment = u16::from_le_bytes(cursor.array()?);
    let red_zone_bytes = u16::from_le_bytes(cursor.array()?);
    let disposition = decode_disposition(cursor)?;
    Ok(OptimizedTerminalOrdinaryCallableEntryRecord {
        identity,
        source_artifact,
        source_manifest,
        terminal_psi,
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
        hardening: TerminalWholeFunctionHardeningPolicy::NoAdditionalEntryExitHardeningV1,
        entry_assumption,
        stack_pointer,
        stack_alignment,
        red_zone_bytes,
        disposition,
    })
}

fn encode_manifest_content(
    bytes: &mut Vec<u8>,
    m: &OptimizedTerminalOrdinaryCallableEntryManifest,
) {
    bytes.push(1);
    bytes.extend_from_slice(&m.entry.bytes());
    bytes.extend_from_slice(&m.source_artifact.bytes());
    bytes.extend_from_slice(&m.source_manifest.bytes());
    encode_terminal_psi(bytes, m.terminal_psi);
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
fn encode_terminal_psi(bytes: &mut Vec<u8>, id: TerminalPsiIdentity) {
    bytes.extend_from_slice(&id.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(id.program_fingerprint.as_bytes());
}
fn decode_terminal_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    let marker = psi_terminal::VocabularyMarker::new(u16::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidId)?;
    Ok(TerminalPsiIdentity {
        vocabulary_marker: marker,
        program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes(cursor.array()?),
    })
}
fn encode_target(bytes: &mut Vec<u8>, target: NativeTarget) {
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
fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    let architecture = match cursor.byte()? {
        1 => Architecture::X86_64,
        2 => Architecture::Aarch64,
        tag => {
            return Err(
                OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownArchitecture(tag),
            );
        }
    };
    let object_format = match cursor.byte()? {
        1 => ObjectFormat::Elf,
        2 => ObjectFormat::MachO,
        3 => ObjectFormat::Coff,
        tag => {
            return Err(
                OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownObjectFormat(tag),
            );
        }
    };
    let pointer_size = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)?;
    let pointer_alignment = usize::try_from(u64::from_le_bytes(cursor.array()?))
        .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)?;
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
        return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidTarget);
    }
    Ok(target)
}
fn encode_scalar(bytes: &mut Vec<u8>, scalar: ScalarType) {
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
fn decode_scalar(
    cursor: &mut Cursor<'_>,
) -> Result<ScalarType, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(ScalarType::Boolean),
        2 => {
            let carrier = cursor.byte()?;
            let sign = match cursor.byte()? {
                1 => IntegerSign::Signed,
                2 => IntegerSign::Unsigned,
                _ => {
                    return Err(
                        OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidIntegerType,
                    );
                }
            };
            let bits = u16::from_le_bytes(cursor.array()?);
            let integer = match carrier {
                1 => IntegerType::new(sign, bits),
                2 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                _ => {
                    return Err(
                        OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidIntegerType,
                    );
                }
            }
            .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        _ => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidScalarType),
    }
}
fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    bytes.push(1);
    bytes.extend_from_slice(&shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&shape.alignment.to_le_bytes());
}
fn decode_shape(
    cursor: &mut Cursor<'_>,
) -> Result<ValueShape, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    if cursor.byte()? != 1 {
        return Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidScalarType);
    }
    Ok(ValueShape::integer(
        u16::from_le_bytes(cursor.array()?),
        u16::from_le_bytes(cursor.array()?),
    ))
}
fn policy_tag(policy: CallingPolicy) -> u8 {
    match policy {
        CallingPolicy::MicrosoftX64 => 1,
        CallingPolicy::SystemVAMD64 => 2,
        CallingPolicy::Aapcs64 => 3,
        CallingPolicy::LinuxSyscallX86_64 => 4,
        CallingPolicy::LinuxSyscallAarch64 => 5,
    }
}
fn decode_policy(
    cursor: &mut Cursor<'_>,
) -> Result<CallingPolicy, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(CallingPolicy::MicrosoftX64),
        2 => Ok(CallingPolicy::SystemVAMD64),
        3 => Ok(CallingPolicy::Aapcs64),
        4 => Ok(CallingPolicy::LinuxSyscallX86_64),
        5 => Ok(CallingPolicy::LinuxSyscallAarch64),
        tag => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownCallingPolicy(tag)),
    }
}
fn encode_register(bytes: &mut Vec<u8>, r: MachineRegister) {
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
fn decode_register(
    cursor: &mut Cursor<'_>,
) -> Result<MachineRegister, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
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
        (tag, _) => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownRegister(tag)),
    }
}
fn exit_policy_tag(policy: TerminalWholeFunctionExitPolicy) -> u8 {
    match policy {
        TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
    }
}
fn decode_exit_policy(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalWholeFunctionExitPolicy, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(TerminalWholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1),
        2 => Ok(TerminalWholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1),
        3 => Ok(TerminalWholeFunctionExitPolicy::Aapcs64FramelessLeafV1),
        4 => Ok(TerminalWholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1),
        tag => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownExitPolicy(tag)),
    }
}
fn encode_entry_assumption(bytes: &mut Vec<u8>, a: TerminalWholeFunctionEntryAssumption) {
    match a {
        TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => bytes.push(1),
        TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            bytes.push(2);
            bytes.extend_from_slice(&link_register.0.to_le_bytes());
        }
    }
}
fn decode_entry_assumption(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalWholeFunctionEntryAssumption, OptimizedTerminalOrdinaryCallableEntryDecodeError>
{
    match cursor.byte()? {
        1 => Ok(TerminalWholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1),
        2 => Ok(TerminalWholeFunctionEntryAssumption::CallerLinkRegisterV1 {
            link_register: RegisterViewId(u16::from_le_bytes(cursor.array()?)),
        }),
        tag => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownEntryAssumption(tag)),
    }
}
fn decode_disposition(
    cursor: &mut Cursor<'_>,
) -> Result<
    OptimizedTerminalOrdinaryCallableEntryDisposition,
    OptimizedTerminalOrdinaryCallableEntryDecodeError,
> {
    match cursor.byte()? {
        1 => Ok(
            OptimizedTerminalOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
        ),
        tag => Err(OptimizedTerminalOrdinaryCallableEntryDecodeError::UnknownDisposition(tag)),
    }
}
fn encode_units(
    bytes: &mut Vec<u8>,
    units: &[RegisterUnitId],
) -> Result<(), OptimizedTerminalOrdinaryCallableEntryError> {
    encode_length(bytes, units.len())?;
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
    Ok(())
}
fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count);
    for _ in 0..count {
        units.push(RegisterUnitId(u16::from_le_bytes(cursor.array()?)));
    }
    Ok(units)
}
fn encode_string(
    bytes: &mut Vec<u8>,
    value: &str,
) -> Result<(), OptimizedTerminalOrdinaryCallableEntryError> {
    encode_length(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}
fn decode_string(
    cursor: &mut Cursor<'_>,
) -> Result<String, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    let len = cursor.length()?;
    String::from_utf8(cursor.take(len)?.to_vec())
        .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidUtf8)
}
fn encode_length(
    bytes: &mut Vec<u8>,
    len: usize,
) -> Result<(), OptimizedTerminalOrdinaryCallableEntryError> {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .map_err(|_| OptimizedTerminalOrdinaryCallableEntryError::LengthOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}
fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl FnOnce(u64) -> Option<T>,
) -> Result<T, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
    constructor(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedTerminalOrdinaryCallableEntryDecodeError::InvalidId)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(
        &mut self,
        n: usize,
    ) -> Result<&'a [u8], OptimizedTerminalOrdinaryCallableEntryDecodeError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(OptimizedTerminalOrdinaryCallableEntryDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedTerminalOrdinaryCallableEntryDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, OptimizedTerminalOrdinaryCallableEntryDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| OptimizedTerminalOrdinaryCallableEntryDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

impl From<OptimizedTerminalOrdinaryCallableEntryDecodeError>
    for OptimizedTerminalOrdinaryCallableEntryManifestDecodeError
{
    fn from(value: OptimizedTerminalOrdinaryCallableEntryDecodeError) -> Self {
        match value {
            OptimizedTerminalOrdinaryCallableEntryDecodeError::Truncated => Self::Truncated,
            other => Self::Record(other),
        }
    }
}
