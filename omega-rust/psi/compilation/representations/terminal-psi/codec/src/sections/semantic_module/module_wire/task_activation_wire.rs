//! Task activation demands on the wire.
//!
//! The bundle is a self-contained evidence section: demands plus the
//! bundle-local structural-type and target-layout tables their value types
//! reference. Identity indices reference the module identity table and are
//! encoded as raw `u32`s, exactly like structural referent identities.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::super::wire_fields::{
    Digest, Field, Flag, Id, Index, List, Referent, Scalar, StructuralType, TaskPackage, Text, U32,
    U64, wire_enum, wire_record,
};
use super::carry_and_suspension_wire::Carry;
use semantic_vocabulary::AuthoredRecordStorage;
use terminal_psi::{
    LayoutApplicationId, TerminalTaskActivationDemand, TerminalTaskActivationDemands,
    TerminalTaskCarryObligations, TerminalTaskCarryStorage, TerminalTaskFrame,
    TerminalTaskFrameCall, TerminalTaskFrameState, TerminalTaskLayoutRealization,
    TerminalTaskLiveCarry, TerminalTaskResidentValue, TerminalTaskSignatureValue,
    TerminalTaskStartOperation, TerminalTaskSuspensionCrossing, TerminalTaskTargetLayout,
    TerminalTaskUnresolvedCall, TerminalTaskUnresolvedCallKind, TerminalTaskValueType,
};

wire_record! {
    Demands: TerminalTaskActivationDemands {
        demands: List<Demand>,
        structural_types: List<StructuralType>,
        structural_referents: List<Referent>,
        target_layout_applications: List<TargetLayout>,
    }
    Demand: TerminalTaskActivationDemand {
        operation: StartOperation,
        report_fingerprint: U64,
        requirement_callable_identity: Index,
        requirement_owner_path: Index,
        requirement_owner_package: TaskPackage,
        requirement_in_scope: Flag,
        authored_requirement_identity: Index,
        machine_callable_identity: Index,
        entry: Index,
        entry_report_fingerprint: U64,
        contract_report_fingerprint: U64,
        specialization_commitment: Digest,
        specialization_report_fingerprint: U64,
        may_suspend: Flag,
        may_block: Flag,
        cancellation_required: Flag,
        signature_arguments: List<SignatureValue>,
        signature_outcome: SignatureValue,
        root_frame: U64,
        frames: List<Frame>,
        crossings: List<Crossing>,
        carry_obligations: CarryObligations,
    }
    SignatureValue: TerminalTaskSignatureValue {
        value_type: ValueType,
        normalized_identity: Index,
        is_mutable: Flag,
    }
    Resident: TerminalTaskResidentValue { value_type: ValueType, normalized_identity: Index }
    LiveCarry: TerminalTaskLiveCarry {
        place: U64,
        value_type: U64,
        storage: CarryStorage,
        claims: List<U64>,
        effective: Carry,
    }
    Crossing: TerminalTaskSuspensionCrossing {
        identity: U64,
        suspension_allowed: Flag,
        preserve_cpu: Flag,
        preserve_host_thread: Flag,
        live_carry: List<LiveCarry>,
    }
    Frame: TerminalTaskFrame {
        identity: U64,
        machine_callable_identity: Index,
        entry: Index,
        states: List<FrameState>,
        calls: List<FrameCall>,
        unresolved_calls: List<UnresolvedCall>,
    }
    FrameState: TerminalTaskFrameState {
        state: Index,
        resident: List<Resident>,
        crossing_arguments: List<List<Resident>>,
    }
    FrameCall: TerminalTaskFrameCall { callee: U64 }
    UnresolvedCall: TerminalTaskUnresolvedCall {
        state: Index,
        statement_index: U32,
        call_ordinal: U32,
        kind: UnresolvedCallKind,
    }
    CarryObligations: TerminalTaskCarryObligations { preserve_cpu: Flag, preserve_host_thread: Flag }
    TargetLayout: TerminalTaskTargetLayout {
        id: ApplicationId,
        record: Id,
        realizations: List<LayoutRealization>,
    }
    LayoutRealization: TerminalTaskLayoutRealization { target: Text, storage: RecordStorage }
    RecordStorage: AuthoredRecordStorage { byte_size: U64, byte_alignment: U64 }
}

wire_enum! {
    ValueType: TerminalTaskValueType, "TerminalTaskValueType" {
        0 => Unit,
        1 => Scalar(scalar: Scalar),
        2 => Structural(identity: Id),
    }
    CarryStorage: TerminalTaskCarryStorage, "TerminalTaskCarryStorage" {
        1 => Persistent,
        2 => Parameter,
        3 => Local,
        4 => CallArgument,
    }
    UnresolvedCallKind: TerminalTaskUnresolvedCallKind, "TerminalTaskUnresolvedCallKind" {
        1 => UnresolvedTarget,
        2 => NonCheckedSupply,
    }
    StartOperation: TerminalTaskStartOperation, "TerminalTaskStartOperation" {
        1 => Start,
        2 => TryStart,
    }
}

struct ApplicationId;
impl Field<LayoutApplicationId> for ApplicationId {
    fn write(writer: &mut Writer, value: &LayoutApplicationId) -> Result<(), CodecError> {
        writer.u32(value.0);
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<LayoutApplicationId, CodecError> {
        Ok(LayoutApplicationId(reader.u32()?))
    }
}
