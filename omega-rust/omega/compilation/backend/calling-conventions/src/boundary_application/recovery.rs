//! Recover candidates through calling-plan validation. Their commitments still
//! need an independently selected method at receiving admission.
use super::*;
use crate::{BoundaryEntryPlan, BoundaryValueGraph, CallSignature, ValueClass, ValueShape};

pub struct BoundaryCallingApplicationCandidate {
    pub abi: semantic_vocabulary::CallingAbiIdentity,
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
        // The target owns the signature's geometry, not necessarily its ABI
        // policy: a portable semantic arrival and its target-physical arrival
        // retain distinct applications. Match ordinary boundary-plan validation
        // here; the selected receiving slot independently admits both policies.
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
                    let crate::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } =
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
                            crate::BoundaryNativeParameterOrigin::PrivateCallback { .. }
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
