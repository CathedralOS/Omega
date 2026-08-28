use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValueShape, evaluate_call_plan,
};
use omega_object_file::{
    ObjectLocalSymbolId, RelocationFreeObjectSymbolLinkage, RelocationFreeObjectSymbolRole,
};
use omega_optimization_core::{
    OptimizationSelectionIdentity, OptimizedObjectArtifactIdentity,
    OptimizedObjectArtifactManifestIdentity, OptimizedOrdinaryCallableEntryManifestIdentity,
    OptimizedTerminalOrdinaryCallableEntryIdentity, RelocationFreeObjectContainerIdentity,
    RelocationFreeObjectPlanIdentity,
};
use omega_regalloc::{RegisterHomeIdentity, register_home_identity};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{EdgeId, IntegerCarrier, IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
use psi_terminal::{TerminalMachineResult, TerminalPsiIdentity, Terminator, ValueDeclaration};

use crate::{
    OptimizedObjectArtifactError, StagedValidatedOptimizedObjectArtifact,
    WholeFunctionEntryAssumption, WholeFunctionExitContractIdentity, WholeFunctionExitPolicy,
    WholeFunctionHardeningPolicy, validate_optimized_object_artifact,
};

const RECORD_MAGIC: &[u8; 8] = b"OMGOER\0\0";
const MANIFEST_MAGIC: &[u8; 8] = b"OMGOEM\0\0";
const VERSION: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryStage {
    ValidatedOptimizedTerminalOrdinaryCallableEntryV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryDisposition {
    ExternalProcessEntryBridgeRequiredV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableParameter {
    pub ordinal: u64,
    pub value: ValueId,
    pub scalar_type: ScalarType,
    pub shape: ValueShape,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub abi_register: MachineRegister,
    pub fixed_view: RegisterViewId,
    pub assigned_view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableResult {
    pub declaration: ValueDeclaration,
    pub shape: ValueShape,
    pub abi_register: MachineRegister,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableReturn {
    pub edge: EdgeId,
    pub value: ValueId,
    pub selected_instruction: SelectedInstructionId,
    pub virtual_register: VirtualRegisterId,
    pub view: RegisterViewId,
    pub storage_units: Vec<RegisterUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryRecord {
    pub identity: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_manifest: OptimizedObjectArtifactManifestIdentity,
    pub psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub selected: SelectedInstructionPlanIdentity,
    pub register_homes: RegisterHomeIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub exit_contract: WholeFunctionExitContractIdentity,
    pub object: RelocationFreeObjectPlanIdentity,
    pub object_container: RelocationFreeObjectContainerIdentity,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub semantic_entry_symbol_name: String,
    pub semantic_entry_section_offset: u64,
    pub semantic_entry_byte_count: u64,
    pub calling_policy: CallingPolicy,
    pub parameters: Vec<OptimizedOrdinaryCallableParameter>,
    pub result: OptimizedOrdinaryCallableResult,
    pub returns: Vec<OptimizedOrdinaryCallableReturn>,
    pub exit_policy: WholeFunctionExitPolicy,
    pub hardening: WholeFunctionHardeningPolicy,
    pub entry_assumption: WholeFunctionEntryAssumption,
    pub stack_pointer: RegisterViewId,
    pub stack_alignment: u16,
    pub red_zone_bytes: u16,
    pub disposition: OptimizedOrdinaryCallableEntryDisposition,
}

impl OptimizedOrdinaryCallableEntryRecord {
    pub fn recomputed_identity(
        &self,
    ) -> Result<OptimizedTerminalOrdinaryCallableEntryIdentity, OptimizedOrdinaryCallableEntryError>
    {
        let mut canonical = b"omega.optimized-terminal-ordinary-callable-entry.v3\0".to_vec();
        encode_record_content(&mut canonical, self)?;
        Ok(OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(&canonical))
    }

    pub fn encode(&self) -> Result<Vec<u8>, OptimizedOrdinaryCallableEntryError> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_record_content(&mut bytes, self)?;
        Ok(bytes)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, OptimizedOrdinaryCallableEntryDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != RECORD_MAGIC {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::UnsupportedVersion(version));
        }
        let identity = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let record = decode_record_content(&mut cursor, identity)?;
        if cursor.remaining() != 0 {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::TrailingBytes);
        }
        if record
            .recomputed_identity()
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)?
            != identity
        {
            return Err(OptimizedOrdinaryCallableEntryDecodeError::IdentityMismatch);
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryManifest {
    pub identity: OptimizedOrdinaryCallableEntryManifestIdentity,
    pub stage: OptimizedOrdinaryCallableEntryStage,
    pub entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_manifest: OptimizedObjectArtifactManifestIdentity,
    pub psi: TerminalPsiIdentity,
    pub selections: OptimizationSelectionIdentity,
    pub target: NativeTarget,
    pub semantic_entry: MachineId,
    pub semantic_entry_symbol: ObjectLocalSymbolId,
    pub exit_contract: WholeFunctionExitContractIdentity,
    pub parameter_count: u64,
    pub return_count: u64,
    pub disposition: OptimizedOrdinaryCallableEntryDisposition,
    pub wrapper_bytes: OptimizedOrdinaryCallableEntryUnavailableData,
    pub relocations: OptimizedOrdinaryCallableEntryUnavailableData,
    pub executable_image: OptimizedOrdinaryCallableEntryUnavailableData,
    pub installation: OptimizedOrdinaryCallableEntryUnavailableData,
    pub publication: OptimizedOrdinaryCallableEntryUnavailableData,
}

impl OptimizedOrdinaryCallableEntryManifest {
    pub fn recomputed_identity(&self) -> OptimizedOrdinaryCallableEntryManifestIdentity {
        let mut bytes = b"omega.optimized-terminal-ordinary-callable-entry-manifest.v3\0".to_vec();
        encode_manifest_content(&mut bytes, self);
        OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(&bytes)
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
    ) -> Result<Self, OptimizedOrdinaryCallableEntryManifestDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(
                OptimizedOrdinaryCallableEntryManifestDecodeError::UnsupportedVersion(version),
            );
        }
        let identity = OptimizedOrdinaryCallableEntryManifestIdentity::from_bytes(cursor.array()?);
        let stage = match cursor.byte()? { 1 => OptimizedOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1, tag => return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownStage(tag)) };
        let entry = OptimizedTerminalOrdinaryCallableEntryIdentity::from_bytes(cursor.array()?);
        let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
        let source_manifest = OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let psi = decode_psi(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let selections = OptimizationSelectionIdentity::from_bytes(cursor.array()?);
        let target = decode_target(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry = decode_id(&mut cursor, MachineId::new)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        let semantic_entry_symbol = ObjectLocalSymbolId::new(u64::from_le_bytes(cursor.array()?))
            .ok_or(
            OptimizedOrdinaryCallableEntryManifestDecodeError::Record(
                OptimizedOrdinaryCallableEntryDecodeError::InvalidId,
            ),
        )?;
        let exit_contract = WholeFunctionExitContractIdentity::from_bytes(cursor.array()?);
        let parameter_count = u64::from_le_bytes(cursor.array()?);
        let return_count = u64::from_le_bytes(cursor.array()?);
        decode_disposition(&mut cursor)
            .map_err(OptimizedOrdinaryCallableEntryManifestDecodeError::Record)?;
        for _ in 0..5 {
            if cursor.byte()? != 1 {
                return Err(
                    OptimizedOrdinaryCallableEntryManifestDecodeError::UnknownUnavailableStatus,
                );
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedOrdinaryCallableEntryUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage,
            entry,
            source_artifact,
            source_manifest,
            psi,
            selections,
            target,
            semantic_entry,
            semantic_entry_symbol,
            exit_contract,
            parameter_count,
            return_count,
            disposition:
                OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
            wrapper_bytes: unavailable,
            relocations: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if manifest.recomputed_identity() != identity {
            return Err(OptimizedOrdinaryCallableEntryManifestDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedOrdinaryCallableEntryManifest {
    record: OptimizedOrdinaryCallableEntryManifest,
}
impl ValidatedOptimizedOrdinaryCallableEntryManifest {
    pub const fn record(&self) -> &OptimizedOrdinaryCallableEntryManifest {
        &self.record
    }
    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut OptimizedOrdinaryCallableEntryManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "ordinary-callable entry custody retains its complete optimized Omega object source"]
pub struct StagedValidatedOptimizedOrdinaryCallableEntry {
    source: StagedValidatedOptimizedObjectArtifact,
    entry: OptimizedOrdinaryCallableEntryRecord,
    manifest: ValidatedOptimizedOrdinaryCallableEntryManifest,
    custody: OptimizedOrdinaryCallableEntryCustodyReceipt,
}
impl StagedValidatedOptimizedOrdinaryCallableEntry {
    pub const fn source(&self) -> &StagedValidatedOptimizedObjectArtifact {
        &self.source
    }
    pub const fn entry(&self) -> &OptimizedOrdinaryCallableEntryRecord {
        &self.entry
    }
    pub const fn manifest(&self) -> &ValidatedOptimizedOrdinaryCallableEntryManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> OptimizedOrdinaryCallableEntryCustodyReceipt {
        self.custody
    }
    #[cfg(test)]
    pub(crate) fn entry_mut(&mut self) -> &mut OptimizedOrdinaryCallableEntryRecord {
        &mut self.entry
    }
    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedOptimizedOrdinaryCallableEntryManifest {
        &mut self.manifest
    }
    #[cfg(test)]
    pub(crate) fn corrupt_custody_for_test(&mut self) {
        self.custody.manifest =
            OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"bad");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedOrdinaryCallableEntryCustodyReceipt {
    source_artifact: OptimizedObjectArtifactIdentity,
    entry: OptimizedTerminalOrdinaryCallableEntryIdentity,
    manifest: OptimizedOrdinaryCallableEntryManifestIdentity,
}
impl OptimizedOrdinaryCallableEntryCustodyReceipt {
    pub const fn source_artifact(self) -> OptimizedObjectArtifactIdentity {
        self.source_artifact
    }
    pub const fn entry(self) -> OptimizedTerminalOrdinaryCallableEntryIdentity {
        self.entry
    }
    pub const fn manifest(self) -> OptimizedOrdinaryCallableEntryManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryError {
    Source(OptimizedObjectArtifactError),
    UnsupportedSignature,
    AbiPlan,
    RootMismatch,
    MissingEntry,
    MissingParameter(ValueId),
    MissingHome(VirtualRegisterId),
    MissingView(RegisterViewId),
    MissingReturn(EdgeId),
    EntrySymbolMismatch,
    LengthOverflow,
    RecordMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}
impl std::fmt::Display for OptimizedOrdinaryCallableEntryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "optimized Terminal ordinary-callable entry failed: {self:?}"
        )
    }
}
impl std::error::Error for OptimizedOrdinaryCallableEntryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedOrdinaryCallableEntryDecodeError {
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
pub enum OptimizedOrdinaryCallableEntryManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownUnavailableStatus,
    Record(OptimizedOrdinaryCallableEntryDecodeError),
    IdentityMismatch,
    TrailingBytes,
}

pub fn stage_validated_optimized_ordinary_callable_entry(
    source: StagedValidatedOptimizedObjectArtifact,
) -> Result<StagedValidatedOptimizedOrdinaryCallableEntry, OptimizedOrdinaryCallableEntryError> {
    validate_optimized_object_artifact(&source)
        .map_err(OptimizedOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&source)?;
    let manifest = manifest(&entry)?;
    let custody = receipt(&entry, &manifest);
    Ok(StagedValidatedOptimizedOrdinaryCallableEntry {
        source,
        entry,
        manifest: ValidatedOptimizedOrdinaryCallableEntryManifest { record: manifest },
        custody,
    })
}

pub fn validate_optimized_ordinary_callable_entry(
    staged: &StagedValidatedOptimizedOrdinaryCallableEntry,
) -> Result<OptimizedOrdinaryCallableEntryCustodyReceipt, OptimizedOrdinaryCallableEntryError> {
    validate_optimized_object_artifact(&staged.source)
        .map_err(OptimizedOrdinaryCallableEntryError::Source)?;
    let entry = reconstruct(&staged.source)?;
    if entry != staged.entry {
        return Err(OptimizedOrdinaryCallableEntryError::RecordMismatch);
    }
    let manifest = manifest(&entry)?;
    if manifest != staged.manifest.record {
        return Err(OptimizedOrdinaryCallableEntryError::ManifestMismatch);
    }
    let receipt = receipt(&entry, &manifest);
    if receipt != staged.custody {
        return Err(OptimizedOrdinaryCallableEntryError::ReceiptMismatch);
    }
    Ok(receipt)
}

fn reconstruct(
    source: &StagedValidatedOptimizedObjectArtifact,
) -> Result<OptimizedOrdinaryCallableEntryRecord, OptimizedOrdinaryCallableEntryError> {
    let artifact = source.artifact();
    let object_stage = source.source();
    let object = object_stage.object();
    let fragment = object_stage.source().source();
    let route = fragment.source();
    let module = route.verified_input().context().module();
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    if machine.attachment.is_some()
        || !machine.structural_parameters.is_empty()
        || !matches!(machine.result, TerminalMachineResult::Scalar(_))
    {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_declaration = *machine
        .result
        .scalar_ref()
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
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
        .map_err(|_| OptimizedOrdinaryCallableEntryError::AbiPlan)?;
    let selected = route.selected_plan();
    let selected_function = selected
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    let homes = route.register_homes();
    let home_function = homes
        .plan()
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
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
            return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
        };
        if *byte_size != placement.shape.byte_size {
            return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
        }
        let virtual_register = selected_function.virtual_registers.iter().find(|vreg| matches!(vreg.origin, VirtualRegisterOrigin::EntryParameter { source_value, parameter_index } if source_value == declaration.id && parameter_index == index)).ok_or(OptimizedOrdinaryCallableEntryError::MissingParameter(declaration.id))?;
        let fixed_view = virtual_register.entry_fixed_view.ok_or(
            OptimizedOrdinaryCallableEntryError::MissingParameter(declaration.id),
        )?;
        let expected_view = environment
            .fixed_register_view(*register)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(fixed_view))?;
        let home = home_function
            .assignments
            .iter()
            .find(|home| home.virtual_register == virtual_register.id)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingHome(
                virtual_register.id,
            ))?;
        if fixed_view != expected_view
            || home.view != fixed_view
            || home.class != virtual_register.class
        {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        let view = physical
            .model()
            .views
            .iter()
            .find(|view| view.id == fixed_view)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(fixed_view))?;
        parameters.push(OptimizedOrdinaryCallableParameter {
            ordinal: u64::try_from(index)
                .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
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
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
    let [
        ValueLocation::Register {
            register: result_register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = result_placement.locations.as_slice()
    else {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    };
    if *byte_size != result_placement.shape.byte_size {
        return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature);
    }
    let result_view = environment
        .fixed_register_view(*result_register)
        .ok_or(OptimizedOrdinaryCallableEntryError::UnsupportedSignature)?;
    let result_model_view = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == result_view)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingView(
            result_view,
        ))?;
    let exit = route.exit_contract().contract();
    let exit_function = exit
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizedOrdinaryCallableEntryError::MissingEntry)?;
    if exit.result_view != result_view || exit.physical_register_model != physical.identity() {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
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
                SelectedTerminator::Return {
                    instruction,
                    psi_return_edge,
                } if *psi_return_edge == edge => Some(instruction),
                _ => None,
            })
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let operand = selected_return
            .operands
            .first()
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let vreg = selected_function
            .virtual_registers
            .iter()
            .find(|vreg| vreg.id == operand.virtual_register)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        if origin_value(vreg.origin) != value {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        let evidence = exit_function
            .returns
            .iter()
            .find(|returned| returned.psi_return_edge == edge)
            .ok_or(OptimizedOrdinaryCallableEntryError::MissingReturn(edge))?;
        let crate::whole_function_exit_contract::WholeFunctionReturnValueEvidence::ScalarI64V1 {
            virtual_register,
            view,
            units,
        } = &evidence.value
        else {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        };
        if evidence.instruction != selected_return.id
            || *virtual_register != vreg.id
            || *view != result_view
            || *units != result_model_view.units
        {
            return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
        }
        returns.push(OptimizedOrdinaryCallableReturn {
            edge,
            value,
            selected_instruction: selected_return.id,
            virtual_register: vreg.id,
            view: *view,
            storage_units: units.clone(),
        });
    }
    if returns.is_empty() || returns.len() != exit_function.returns.len() {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
    }
    let symbol = object
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == object.semantic_entry_symbol)
        .ok_or(OptimizedOrdinaryCallableEntryError::EntrySymbolMismatch)?;
    if symbol.machine != machine.id
        || symbol.linkage != RelocationFreeObjectSymbolLinkage::ObjectLocalV1
        || symbol.role != RelocationFreeObjectSymbolRole::SemanticEntryV1
    {
        return Err(OptimizedOrdinaryCallableEntryError::EntrySymbolMismatch);
    }
    if artifact.semantic_entry != module.entry
        || selected.entry != module.entry
        || object.semantic_entry != module.entry
        || exit.selected != object.selected
    {
        return Err(OptimizedOrdinaryCallableEntryError::RootMismatch);
    }
    let mut record = OptimizedOrdinaryCallableEntryRecord {
        identity: OptimizedTerminalOrdinaryCallableEntryIdentity::from_canonical_bytes(b"pending"),
        source_artifact: artifact.identity,
        source_manifest: source.manifest().record().identity,
        psi: artifact.psi,
        selections: artifact.selections,
        target: artifact.target,
        semantic_entry: machine.id,
        selected: exit.selected,
        register_homes: register_home_identity(homes.plan()),
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
        result: OptimizedOrdinaryCallableResult {
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
            OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1,
    };
    record.identity = record.recomputed_identity()?;
    Ok(record)
}

fn manifest(
    entry: &OptimizedOrdinaryCallableEntryRecord,
) -> Result<OptimizedOrdinaryCallableEntryManifest, OptimizedOrdinaryCallableEntryError> {
    let unavailable = OptimizedOrdinaryCallableEntryUnavailableData::Unavailable;
    let mut manifest = OptimizedOrdinaryCallableEntryManifest {
        identity: OptimizedOrdinaryCallableEntryManifestIdentity::from_canonical_bytes(b"pending"),
        stage:
            OptimizedOrdinaryCallableEntryStage::ValidatedOptimizedTerminalOrdinaryCallableEntryV1,
        entry: entry.identity,
        source_artifact: entry.source_artifact,
        source_manifest: entry.source_manifest,
        psi: entry.psi,
        selections: entry.selections,
        target: entry.target,
        semantic_entry: entry.semantic_entry,
        semantic_entry_symbol: entry.semantic_entry_symbol,
        exit_contract: entry.exit_contract,
        parameter_count: u64::try_from(entry.parameters.len())
            .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
        return_count: u64::try_from(entry.returns.len())
            .map_err(|_| OptimizedOrdinaryCallableEntryError::LengthOverflow)?,
        disposition: entry.disposition,
        wrapper_bytes: unavailable,
        relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    manifest.identity = manifest.recomputed_identity();
    Ok(manifest)
}
fn receipt(
    entry: &OptimizedOrdinaryCallableEntryRecord,
    manifest: &OptimizedOrdinaryCallableEntryManifest,
) -> OptimizedOrdinaryCallableEntryCustodyReceipt {
    OptimizedOrdinaryCallableEntryCustodyReceipt {
        source_artifact: entry.source_artifact,
        entry: entry.identity,
        manifest: manifest.identity,
    }
}
fn scalar_shape(scalar: ScalarType) -> Result<ValueShape, OptimizedOrdinaryCallableEntryError> {
    let bytes = match scalar {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer)
            if integer.carrier() == IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            integer.bits().div_ceil(8)
        }
        _ => return Err(OptimizedOrdinaryCallableEntryError::UnsupportedSignature),
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}
fn origin_value(origin: VirtualRegisterOrigin) -> ValueId {
    match origin {
        VirtualRegisterOrigin::EntryParameter { source_value, .. }
        | VirtualRegisterOrigin::InstructionResult { source_value, .. }
        | VirtualRegisterOrigin::LegalizationTemporary { source_value, .. } => source_value,
    }
}

fn encode_record_content(
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

fn decode_record_content(
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

fn encode_manifest_content(bytes: &mut Vec<u8>, m: &OptimizedOrdinaryCallableEntryManifest) {
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
fn encode_psi(bytes: &mut Vec<u8>, id: TerminalPsiIdentity) {
    bytes.extend_from_slice(&id.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(id.program_fingerprint.as_bytes());
}
fn decode_psi(
    cursor: &mut Cursor<'_>,
) -> Result<TerminalPsiIdentity, OptimizedOrdinaryCallableEntryDecodeError> {
    let marker = psi_terminal::VocabularyMarker::new(u16::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedOrdinaryCallableEntryDecodeError::InvalidId)?;
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
fn encode_shape(bytes: &mut Vec<u8>, shape: ValueShape) {
    bytes.push(1);
    bytes.extend_from_slice(&shape.byte_size.to_le_bytes());
    bytes.extend_from_slice(&shape.alignment.to_le_bytes());
}
fn decode_shape(
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
fn exit_policy_tag(policy: WholeFunctionExitPolicy) -> u8 {
    match policy {
        WholeFunctionExitPolicy::SystemVAMD64FramelessLeafV1 => 1,
        WholeFunctionExitPolicy::MicrosoftX64FramelessLeafV1 => 2,
        WholeFunctionExitPolicy::Aapcs64FramelessLeafV1 => 3,
        WholeFunctionExitPolicy::DarwinAapcs64FramelessLeafV1 => 4,
        WholeFunctionExitPolicy::MicrosoftX64BalancedStructuralUnitCallV1 => 5,
        WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1 => 6,
    }
}
fn decode_exit_policy(
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
fn encode_entry_assumption(bytes: &mut Vec<u8>, a: WholeFunctionEntryAssumption) {
    match a {
        WholeFunctionEntryAssumption::CallerReturnAddressAtStackPointerV1 => bytes.push(1),
        WholeFunctionEntryAssumption::CallerLinkRegisterV1 { link_register } => {
            bytes.push(2);
            bytes.extend_from_slice(&link_register.0.to_le_bytes());
        }
    }
}
fn decode_entry_assumption(
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
fn decode_disposition(
    cursor: &mut Cursor<'_>,
) -> Result<OptimizedOrdinaryCallableEntryDisposition, OptimizedOrdinaryCallableEntryDecodeError> {
    match cursor.byte()? {
        1 => Ok(OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1),
        tag => Err(OptimizedOrdinaryCallableEntryDecodeError::UnknownDisposition(tag)),
    }
}
fn encode_units(
    bytes: &mut Vec<u8>,
    units: &[RegisterUnitId],
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    encode_length(bytes, units.len())?;
    for unit in units {
        bytes.extend_from_slice(&unit.0.to_le_bytes());
    }
    Ok(())
}
fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, OptimizedOrdinaryCallableEntryDecodeError> {
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
) -> Result<(), OptimizedOrdinaryCallableEntryError> {
    encode_length(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}
fn decode_string(
    cursor: &mut Cursor<'_>,
) -> Result<String, OptimizedOrdinaryCallableEntryDecodeError> {
    let len = cursor.length()?;
    String::from_utf8(cursor.take(len)?.to_vec())
        .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::InvalidUtf8)
}
fn encode_length(
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
fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: impl FnOnce(u64) -> Option<T>,
) -> Result<T, OptimizedOrdinaryCallableEntryDecodeError> {
    constructor(u64::from_le_bytes(cursor.array()?))
        .ok_or(OptimizedOrdinaryCallableEntryDecodeError::InvalidId)
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], OptimizedOrdinaryCallableEntryDecodeError> {
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
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizedOrdinaryCallableEntryDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, OptimizedOrdinaryCallableEntryDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, OptimizedOrdinaryCallableEntryDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| OptimizedOrdinaryCallableEntryDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

impl From<OptimizedOrdinaryCallableEntryDecodeError>
    for OptimizedOrdinaryCallableEntryManifestDecodeError
{
    fn from(value: OptimizedOrdinaryCallableEntryDecodeError) -> Self {
        match value {
            OptimizedOrdinaryCallableEntryDecodeError::Truncated => Self::Truncated,
            other => Self::Record(other),
        }
    }
}
