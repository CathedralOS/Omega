/*
We retain completed calling-policy applications (`BoundaryCallingApplication`),
their source-free signature and callback geometry (`BoundarySignatureGeometry`),
and their candidate recovery and commitment verification
(`BoundaryCallingApplicationCandidate`, `recomputed_commitment`). Native
consumers join this output to their requirements without retaining source
handles or re-running the policy evaluator, while receiving admission verifies
the recovered commitment against the independently selected method.
*/

use crate::{
    BoundaryEntryPlan, BoundaryNativeParameterOrigin, BoundaryValueClass, BoundaryValueField,
    BoundaryValueGraph, BoundaryValueShape, CallSignature, ValidatedBoundaryEntryPlan, ValueClass,
    ValueShape,
};
use arena::{Arena, HandleSpan};
use target::NativeTarget;

mod callback_catalog;
mod native_telescope;
mod terminal_telescope;

pub use callback_catalog::{
    BoundaryCallbackLayout, CallbackDataGeometry, CallbackInlineFieldGeometry,
    CallbackSlotGeometry, PrivateCallbackPlacements, callback_destination_slot,
    encode_private_callback_placements,
};
pub use native_telescope::{NativeParameterTelescope, validate_fresh_native_parameter_identity};
pub use terminal_telescope::TerminalCallingTelescope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallingApplication {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub calling_owner_application: Option<String>,
    pub requirement_identity: String,
    pub target: NativeTarget,
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
    pub plan: ValidatedBoundaryEntryPlan,
    pub dispatch_only_parameter_count: u32,
    pub signature: BoundarySignatureGeometry,
    pub opaque_applications: Vec<BoundaryOpaqueApplication>,
    pub callback_context: crate::CallbackMaterializationContext,
    pub callback_context_closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySignatureGeometry {
    pub shapes: Vec<BoundaryValueShape>,
    pub fields: Vec<BoundaryValueField>,
    pub parameters: Vec<u16>,
    pub result: Option<u16>,
    pub native_parameters: Arena<BoundaryNativeParameterGeometry>,
    pub native_parameter_rows: HandleSpan<BoundaryNativeParameterGeometry>,
    pub callback_layouts: Arena<BoundaryCallbackLayout>,
    pub callback_layout_rows: HandleSpan<BoundaryCallbackLayout>,
    pub callback_bindings: Arena<BoundaryCallbackBinding>,
    pub callback_binding_rows: HandleSpan<BoundaryCallbackBinding>,
}

pub struct NativeCallProjection<'geometry> {
    parameters: &'geometry [BoundaryNativeParameterGeometry],
    dispatch_only_parameter_count: u32,
}

impl<'geometry> NativeCallProjection<'geometry> {
    pub fn new(
        parameters: &'geometry [BoundaryNativeParameterGeometry],
        semantic_parameter_count: usize,
        dispatch_only_parameter_count: u32,
    ) -> Result<Self, String> {
        let dispatch_count = usize::try_from(dispatch_only_parameter_count)
            .map_err(|_| "dispatch prefix exceeds usize")?;
        if dispatch_count > semantic_parameter_count {
            return Err("dispatch prefix exceeds the public semantic telescope".into());
        }
        let mut semantic_seen = vec![false; semantic_parameter_count];
        for (ordinal, parameter) in parameters.iter().enumerate() {
            if usize::try_from(parameter.native_ordinal).ok() != Some(ordinal)
                || parameters[..ordinal]
                    .iter()
                    .any(|prior| prior.identity == parameter.identity)
            {
                return Err(
                    "native call projection repeats an identity or changes row order".into(),
                );
            }
            if let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } =
                parameter.origin
            {
                let seen = semantic_seen
                    .get_mut(
                        usize::try_from(formal_ordinal)
                            .map_err(|_| "semantic formal exceeds usize")?,
                    )
                    .ok_or("native call projection has an out-of-range semantic formal")?;
                if std::mem::replace(seen, true) {
                    return Err("native call projection repeats a semantic formal".into());
                }
            }
        }
        if semantic_seen.contains(&false) {
            return Err("native call projection lost a public semantic formal".into());
        }
        Ok(Self {
            parameters,
            dispatch_only_parameter_count,
        })
    }

    fn is_dispatch_only(&self, parameter: &BoundaryNativeParameterGeometry) -> bool {
        matches!(parameter.origin,
            BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal }
            if formal_ordinal < self.dispatch_only_parameter_count)
    }

    pub fn parameters(
        &self,
    ) -> impl Iterator<Item = &'geometry BoundaryNativeParameterGeometry> + '_ {
        self.parameters
            .iter()
            .filter(|parameter| !self.is_dispatch_only(parameter))
    }

    pub fn parameter_ordinal(&self, public_ordinal: u32) -> Result<Option<usize>, String> {
        let ordinal =
            usize::try_from(public_ordinal).map_err(|_| "native ordinal exceeds usize")?;
        let parameter = self
            .parameters
            .get(ordinal)
            .ok_or("native ordinal is outside the public telescope")?;
        if self.is_dispatch_only(parameter) {
            return Ok(None);
        }
        let omitted = self.parameters[..ordinal]
            .iter()
            .filter(|parameter| self.is_dispatch_only(parameter))
            .count();
        Ok(Some(ordinal - omitted))
    }
}

impl BoundaryCallingApplication {
    pub fn call_projection(&self) -> Result<NativeCallProjection<'_>, String> {
        NativeCallProjection::new(
            self.signature
                .native_parameters
                .span(self.signature.native_parameter_rows)
                .ok_or("calling application has stale native telescope storage")?,
            self.signature.parameters.len(),
            self.dispatch_only_parameter_count,
        )
    }

    pub fn recomputed_commitment(&self) -> Result<[u8; 32], String> {
        self.abi.validate()?;
        if self.abi.requirement_identity != self.requirement_identity {
            return Err("calling application requirement differs from its exact ABI key".into());
        }
        let native_parameters = self
            .signature
            .native_parameters
            .span(self.signature.native_parameter_rows)
            .ok_or("native telescope has stale row storage")?;
        let mut strong =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.boundary-plan-application.v10");
        self.call_projection()?;
        strong.raw(&self.dispatch_only_parameter_count.to_le_bytes());
        match &self.calling_owner_application {
            Some(owner) if !owner.is_empty() => {
                strong.byte(1);
                strong.string(owner);
            }
            Some(_) => return Err("calling application has an empty selected owner".into()),
            None => strong.byte(0),
        }
        strong.string(&self.requirement_identity);
        strong.string(&self.abi.declaring_application);
        strong.u64(self.abi.id().get());
        strong.len(self.abi.operands.len());
        for operand in &self.abi.operands {
            strong.raw(&operand.ordinal.to_le_bytes());
            strong.byte(operand.kind as u8);
            strong.string(&operand.identity);
        }
        strong.byte(match self.target.architecture {
            target::Architecture::Aarch64 => 1,
            target::Architecture::X86_64 => 2,
        });
        strong.byte(match self.target.object_format {
            target::ObjectFormat::Elf => 1,
            target::ObjectFormat::MachO => 2,
            target::ObjectFormat::Coff => 3,
        });
        strong.u64(self.target.pointer_size as u64);
        strong.u64(self.target.pointer_alignment as u64);
        strong.u64(self.opaque_applications.len() as u64);
        for representation in &self.opaque_applications {
            strong.raw(&representation.shape_root.to_le_bytes());
            strong.raw(&representation.application_report_fingerprint.to_le_bytes());
            strong.raw(&representation.conformance_application_commitment);
            strong.raw(&representation.representation_schema_version.to_le_bytes());
            strong.byte(1);
            strong.byte(1);
            strong.byte(match representation.checked_semantic_copy {
                false => 1,
                true => 2,
            });
            strong.raw(&representation.selected_application_commitment);
        }
        strong.u64(self.signature.shapes.len() as u64);
        for shape in &self.signature.shapes {
            match shape.class {
                BoundaryValueClass::Integer => strong.byte(1),
                BoundaryValueClass::Float => strong.byte(2),
                BoundaryValueClass::Reference => strong.byte(3),
                BoundaryValueClass::FixedArray { element, length } => {
                    strong.byte(4);
                    strong.raw(&element.to_le_bytes());
                    strong.raw(&length.to_le_bytes());
                }
                BoundaryValueClass::Record {
                    first_field,
                    field_count,
                } => {
                    strong.byte(5);
                    strong.raw(&first_field.to_le_bytes());
                    strong.raw(&field_count.to_le_bytes());
                }
            }
            strong.raw(&shape.byte_size.to_le_bytes());
            strong.raw(&shape.alignment.to_le_bytes());
        }
        strong.u64(self.signature.fields.len() as u64);
        for field in &self.signature.fields {
            strong.raw(&field.shape.to_le_bytes());
            strong.raw(&field.byte_offset.to_le_bytes());
        }
        strong.u64(self.signature.parameters.len() as u64);
        for parameter in &self.signature.parameters {
            strong.raw(&parameter.to_le_bytes());
        }
        match self.signature.result {
            Some(result) => {
                strong.byte(1);
                strong.raw(&result.to_le_bytes());
            }
            None => strong.byte(0),
        }
        strong.u64(native_parameters.len() as u64);
        for parameter in native_parameters {
            strong.raw(&parameter.identity.get().to_le_bytes());
            strong.raw(&parameter.native_ordinal.to_le_bytes());
            match parameter.origin {
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                    strong.byte(1);
                    strong.raw(&formal_ordinal.to_le_bytes());
                }
                BoundaryNativeParameterOrigin::PrivateCallback {
                    binder,
                    requirement,
                } => {
                    strong.byte(2);
                    strong.raw(&binder.get().to_le_bytes());
                    strong.raw(&requirement.get().to_le_bytes());
                }
            }
            match parameter.shape {
                BoundaryNativeParameterShape::Semantic(root) => {
                    strong.byte(1);
                    strong.raw(&root.to_le_bytes());
                }
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => {
                    strong.byte(2);
                    strong.raw(&byte_size.to_le_bytes());
                    strong.raw(&alignment.to_le_bytes());
                }
            }
        }
        let catalog = self
            .signature
            .callback_layouts
            .span(self.signature.callback_layout_rows)
            .ok_or("callback catalog has stale entry storage")?;
        strong.len(catalog.len());
        for entry in catalog {
            entry.field_placements(self.target)?;
            entry.encode(&mut strong)?;
        }
        let bindings = self
            .signature
            .callback_bindings
            .span(self.signature.callback_binding_rows)
            .ok_or("callback application has stale binder correspondence")?;
        strong.len(bindings.len());
        for (position, binding) in bindings.iter().enumerate() {
            if binding.requirement_identity.is_empty()
                || crate::callback_requirement_id(&binding.requirement_identity)
                    != binding.requirement
                || bindings[..position].iter().any(|prior| {
                    prior.binder == binding.binder
                        || prior.static_machine_ordinal >= binding.static_machine_ordinal
                })
            {
                return Err("callback application has invalid exact binder correspondence".into());
            }
            strong.raw(&binding.binder.get().to_le_bytes());
            strong.raw(&binding.requirement.get().to_le_bytes());
            strong.raw(&binding.static_machine_ordinal.to_le_bytes());
            strong.string(&binding.requirement_identity);
            strong.byte(u8::from(binding.invokes_synchronously));
        }
        if bindings.len() != self.callback_context.binders.len()
            || bindings.iter().any(|binding| {
                self.callback_context
                    .binders
                    .iter()
                    .filter(|row| {
                        row.binder == binding.binder && row.requirement == binding.requirement
                    })
                    .count()
                    != 1
            })
        {
            return Err("callback application changed its complete binder correspondence".into());
        }
        strong.raw(&self.plan.contract_commitment_digest());
        let commitment: [u8; 32] = strong.finish();
        Ok(commitment)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackBinding {
    pub binder: crate::StaticMachineBinderId,
    pub requirement: crate::CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
}

impl Default for BoundaryCallbackBinding {
    fn default() -> Self {
        Self {
            binder: crate::StaticMachineBinderId::new(1).expect("nonzero arena sentinel"),
            requirement: crate::CallbackRequirementId::new(1).expect("nonzero arena sentinel"),
            static_machine_ordinal: 0,
            requirement_identity: String::new(),
            invokes_synchronously: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryNativeParameterShape {
    Semantic(u16),
    TargetFunctionPointer { byte_size: u16, alignment: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryNativeParameterGeometry {
    pub identity: crate::NativeParameterId,
    pub native_ordinal: u32,
    pub shape: BoundaryNativeParameterShape,
    pub origin: BoundaryNativeParameterOrigin,
}

impl BoundaryNativeParameterGeometry {
    pub const fn identity(&self) -> crate::NativeParameterId {
        self.identity
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    pub const fn shape(&self) -> BoundaryNativeParameterShape {
        self.shape
    }

    pub const fn origin(&self) -> BoundaryNativeParameterOrigin {
        self.origin
    }
}

impl Default for BoundaryNativeParameterGeometry {
    fn default() -> Self {
        Self {
            identity: crate::NativeParameterId::new(1).expect("nonzero arena sentinel"),
            native_ordinal: 0,
            shape: BoundaryNativeParameterShape::Semantic(0),
            origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOpaqueApplication {
    pub shape_root: u16,
    pub application_report_fingerprint: u64,
    pub selected_application_commitment: [u8; 32],
    pub conformance_application_commitment: [u8; 32],
    pub representation_schema_version: u16,
    pub checked_semantic_copy: bool,
}

pub struct BoundaryCallingApplicationCandidate {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
    pub calling_owner_application: Option<String>,
    pub requirement_identity: String,
    pub target: NativeTarget,
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
    pub plan: BoundaryEntryPlan,
    pub dispatch_only_parameter_count: u32,
    pub signature: BoundarySignatureGeometry,
    pub opaque_applications: Vec<BoundaryOpaqueApplication>,
    pub callback_context: crate::CallbackMaterializationContext,
    pub callback_context_closed: bool,
}

impl BoundaryCallingApplicationCandidate {
    pub fn validate(self) -> Result<BoundaryCallingApplication, String> {
        let Self {
            abi,
            calling_owner_application,
            requirement_identity,
            target,
            report_fingerprint,
            commitment,
            plan,
            dispatch_only_parameter_count,
            signature,
            opaque_applications,
            callback_context,
            callback_context_closed,
        } = self;
        if plan.call.policy.architecture() != plan.state.initial_regime.architecture() {
            return Err(format!(
                "recovered calling application `{requirement_identity}` for {target:?} has policy {:?} incompatible with initial machine regime {:?}",
                plan.call.policy, plan.state.initial_regime,
            ));
        }
        let graph = BoundaryValueGraph {
            shapes: &signature.shapes,
            fields: &signature.fields,
        };
        let native = signature
            .native_parameters
            .span(signature.native_parameter_rows)
            .ok_or("recovered native telescope has stale storage")?;
        let mut parameters = Vec::with_capacity(native.len());
        let projection = NativeCallProjection::new(
            native,
            signature.parameters.len(),
            dispatch_only_parameter_count,
        )?;
        for parameter in native {
            let shape = match parameter.shape {
                BoundaryNativeParameterShape::Semantic(root) => {
                    let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } =
                        parameter.origin
                    else {
                        return Err("recovered semantic parameter has a callback origin".to_owned());
                    };
                    if signature.parameters.get(formal_ordinal as usize) != Some(&root) {
                        return Err(
                            "recovered native parameter changed its semantic formal".to_owned()
                        );
                    }
                    graph.classify(root, plan.call.policy)?
                }
                BoundaryNativeParameterShape::TargetFunctionPointer {
                    byte_size,
                    alignment,
                } => {
                    if usize::from(byte_size) != target.pointer_size
                        || usize::from(alignment) != target.pointer_alignment
                        || !matches!(
                            parameter.origin,
                            BoundaryNativeParameterOrigin::PrivateCallback { .. }
                        )
                    {
                        return Err("recovered callback parameter changed target pointer geometry or origin".to_owned());
                    }
                    ValueShape {
                        class: ValueClass::Integer,
                        byte_size: u32::from(byte_size),
                        alignment,
                    }
                }
            };
            if projection
                .parameter_ordinal(parameter.native_ordinal)?
                .is_some()
            {
                parameters.push(shape);
            }
        }
        let expected = CallSignature {
            parameters,
            result: signature
                .result
                .map(|root| graph.classify(root, plan.call.policy))
                .transpose()?,
        };
        let validated = if callback_context_closed {
            crate::validate_boundary_entry_plan_with_callback_materializations(
                plan.clone(),
                &expected,
                &callback_context,
            )
        } else {
            crate::validate_boundary_entry_plan(plan.clone(), &expected)
        }
        .map_err(|error| format!("recovered boundary calling plan: {error}"))?;
        if validated.plan() != &plan {
            return Err("recovered boundary calling plan is not canonical".to_owned());
        }
        let application = BoundaryCallingApplication {
            abi,
            calling_owner_application,
            requirement_identity,
            target,
            report_fingerprint,
            commitment,
            plan: validated,
            dispatch_only_parameter_count,
            signature,
            opaque_applications,
            callback_context,
            callback_context_closed,
        };
        if application.recomputed_commitment()? != application.commitment {
            return Err(
                "recovered calling application commitment does not match its complete payload"
                    .to_owned(),
            );
        }
        Ok(application)
    }
}
