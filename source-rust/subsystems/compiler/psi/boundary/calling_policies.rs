use crate::TerminalIdentityIndex;
use semantic_vocabulary::{
    BoundaryMachineId, CallbackRequirementId, CallingOperandKind, CallingSignatureId,
    NativeParameterId, StaticMachineBinderId, StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiDeclaration {
    pub id: CallingSignatureId,
    pub requirement_identity: TerminalIdentityIndex,
    pub declaring_application: TerminalIdentityIndex,
    pub operands: Vec<CallingStaticOperand>,
    pub formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    pub default_eligible: bool,
    pub physical_owner: Result<semantic_vocabulary::CallingDeclarationOwner, String>,
    pub storage: Result<CallingAbiStorage, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingStaticOperand {
    pub ordinal: u32,
    pub kind: CallingOperandKind,
    pub identity: TerminalIdentityIndex,
}

impl CallingAbiDeclaration {
    pub fn identity(
        &self,
        identities: &crate::TerminalIdentityTable,
    ) -> Result<semantic_vocabulary::CallingAbiIdentity, String> {
        let text = |identity| {
            identities
                .text(identity)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| "declaration ABI lost a portable identity".to_owned())
        };
        let identity = semantic_vocabulary::CallingAbiIdentity {
            requirement_identity: text(self.requirement_identity)?,
            declaring_application: text(self.declaring_application)?,
            operands: self
                .operands
                .iter()
                .map(|operand| {
                    Ok(semantic_vocabulary::CallingStaticOperand {
                        ordinal: operand.ordinal,
                        kind: operand.kind,
                        identity: text(operand.identity)?,
                    })
                })
                .collect::<Result<_, String>>()?,
        };
        identity.validate()?;
        if identity.id() != self.id {
            return Err(
                "declaration ABI compact coordinate differs from its complete key".into(),
            );
        }
        Ok(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiStorage {
    pub parameters: Vec<CallingAbiParameter>,
    pub result: Option<StructuralTypeId>,
    pub telescope: CallingSignatureTelescope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingAbiParameter {
    pub structural_type: StructuralTypeId,
    pub callback_backing: Option<StructuralTypeId>,
    pub access: crate::StructuralAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOpaqueRepresentation {
    pub opaque: StructuralTypeId,
    pub carrier: StructuralTypeId,
    pub opaque_storage_identity: TerminalIdentityIndex,
    pub carrier_storage_identity: TerminalIdentityIndex,
    pub application: application_identity::opaque_representations::SelectedOpaqueApplication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyRequest {
    pub declaration: Result<TerminalIdentityIndex, String>,
    pub display_name: String,
    pub instances: Result<Vec<CallingPolicyInstance>, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyInstance {
    pub declaring_application: TerminalIdentityIndex,
    pub policy_declaration: Result<crate::ProofDeclarationId, String>,
    pub policy: Result<crate::TargetMachineCallable, String>,
    pub signatures: Result<Vec<CallingSignatureId>, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingSignatureDeclaration {
    pub boundary: BoundaryMachineId,
    pub application: Result<CallingSignatureId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingSignatureTelescope {
    pub parameters: Vec<CallingRuntimeFormal>,
    pub binders: Vec<CallingCallbackBinder>,
    pub direct_callbacks: Vec<CallingDirectCallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallingRuntimeFormal {
    pub identity: NativeParameterId,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingCallbackBinder {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
    pub requirement_identity: TerminalIdentityIndex,
    pub static_machine_ordinal: u32,
    pub invokes_synchronously: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingDirectCallback {
    pub identity: NativeParameterId,
    pub native_ordinal: u32,
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
}
