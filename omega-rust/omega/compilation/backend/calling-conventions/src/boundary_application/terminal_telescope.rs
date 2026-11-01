//! Recover native argument order and callback binders from the canonical declaration.

use super::{
    BoundaryCallbackBinding, BoundaryCallingApplication, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterShape,
};
use crate::BoundaryNativeParameterOrigin;
use crate::BoundaryValueClass;
use terminal_psi::{BoundaryParameterKind, StructuralAccess, StructuralTypeShape, TerminalModule};

/// Target-closed argument telescope. Value layouts still come from the calling
/// application; declaration identity, native-only arguments and callback
/// correspondence come exclusively from Terminal Psi.
pub struct TerminalCallingTelescope {
    pub parameters: Vec<BoundaryNativeParameterGeometry>,
    pub binders: Vec<BoundaryCallbackBinding>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundarySignatureGeometry, CallSignature, CallingPolicy};
    use semantic_vocabulary::{BoundaryMachineId, CallingOperandKind, CallingSignatureId};

    #[test]
    fn phantom_local_application_cannot_borrow_equal_geometry() {
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let requirement = identities.intern("package::Service::call");
        let declaring = identities.intern("package::Service<u8>");
        let first_actual = identities.intern("u32");
        let second_actual = identities.intern("u64");
        let first = CallingSignatureId::for_application(
            "package::Service::call",
            "package::Service<u8>",
            [(0, CallingOperandKind::Type, "u32")],
        );
        let second = CallingSignatureId::for_application(
            "package::Service::call",
            "package::Service<u8>",
            [(0, CallingOperandKind::Type, "u64")],
        );
        let boundary = BoundaryMachineId::new(1).unwrap();
        let mut module = TerminalModule::declarations();
        module.identities = identities.build();
        module.calling_abi_signatures = [(first, first_actual), (second, second_actual)]
            .into_iter()
            .map(|(id, actual)| terminal_psi::CallingAbiDeclaration {
                id,
                requirement_identity: requirement,
                declaring_application: declaring,
                operands: vec![terminal_psi::CallingStaticOperand {
                    ordinal: 0,
                    kind: CallingOperandKind::Type,
                    identity: actual,
                }],
                formals: vec![semantic_vocabulary::CallingGenericFormal {
                    ordinal: 0,
                    kind: CallingOperandKind::Type,
                    callback: None,
                }],
                default_eligible: true,
                storage: Ok(terminal_psi::CallingAbiStorage {
                    parameters: vec![],
                    result: None,
                    telescope: terminal_psi::CallingSignatureTelescope {
                        parameters: vec![],
                        binders: vec![],
                        direct_callbacks: vec![],
                    },
                }),
            })
            .collect();
        module.calling_abi_signatures.sort_by_key(|row| row.id);
        module
            .calling_signatures
            .push(terminal_psi::CallingSignatureDeclaration {
                boundary,
                application: Ok(first),
            });
        module
            .boundary_machines
            .push(terminal_psi::BoundaryMachineDeclaration {
                id: boundary,
                declaration: None,
                numeric_intrinsic: None,
                role: terminal_psi::BoundaryDeclarationRole::Callable,
                kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                unconditional_termination: true,
                identity: requirement,
                owner: terminal_psi::NominalDeclarationIdentity::new(declaring),
                attachment: None,
                parameter_order: vec![],
                scalar_parameters: vec![],
                crash_routes: vec![],
                structural_parameters: vec![],
                result: terminal_psi::BoundaryMachineResult::Unit,
                requires: vec![],
                normal_qualifications: vec![],
                scalar_domain_requirements: vec![],
                scalar_requires: vec![],
                scalar_ensures: vec![],
                reference_outputs: vec![],
                program_local_root_introductions: vec![],
                content_guarantees: vec![],
                fixed_service_reach: vec![],
                owning_service: None,
                published_service_ceiling: vec![],
            });
        let plan = crate::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![],
                result: None,
            },
        )
        .unwrap();
        let application = BoundaryCallingApplication {
            abi: semantic_vocabulary::CallingAbiIdentity {
                requirement_identity: "package::Service::call".into(),
                declaring_application: "package::Service<u8>".into(),
                operands: vec![semantic_vocabulary::CallingStaticOperand {
                    ordinal: 0,
                    kind: CallingOperandKind::Type,
                    identity: "u32".into(),
                }],
            },
            requirement_identity: "package::Service::call".into(),
            target: target::NativeTarget::from_omega_target_name(Some("windows_x86_64")).unwrap(),
            report_fingerprint: 1,
            commitment: [0; 32],
            plan,
            signature: BoundarySignatureGeometry {
                shapes: vec![],
                fields: vec![],
                parameters: vec![],
                result: None,
                native_parameters: Default::default(),
                native_parameter_rows: Default::default(),
                callback_layouts: Default::default(),
                callback_layout_rows: Default::default(),
                callback_bindings: Default::default(),
                callback_binding_rows: Default::default(),
            },
            opaque_applications: vec![],
            callback_context: Default::default(),
            callback_context_closed: false,
        };
        assert!(TerminalCallingTelescope::for_application(&module, boundary, &application).is_ok());
        module.calling_signatures[0].application = Ok(second);
        assert!(
            TerminalCallingTelescope::for_application(&module, boundary, &application).is_err(),
            "equal geometry must not authorize another phantom actual"
        );
        let mut second_application = application.clone();
        second_application.abi.operands[0].identity = "u64".into();
        assert!(
            TerminalCallingTelescope::for_application(&module, boundary, &second_application)
                .is_ok()
        );
        assert_ne!(
            application.recomputed_commitment().unwrap(),
            second_application.recomputed_commitment().unwrap()
        );
        let commitment = application.recomputed_commitment().unwrap();
        for mutation in 0..3 {
            let mut abi = application.abi.clone();
            match mutation {
                0 => abi.operands[0].identity = "u64".into(),
                1 => abi.operands[0].ordinal = 1,
                _ => abi.declaring_application = "package::Service<u64>".into(),
            }
            let candidate = super::super::BoundaryCallingApplicationCandidate {
                abi,
                requirement_identity: application.requirement_identity.clone(),
                target: application.target,
                report_fingerprint: application.report_fingerprint,
                commitment,
                plan: application.plan.plan().clone(),
                signature: application.signature.clone(),
                opaque_applications: vec![],
                callback_context: Default::default(),
                callback_context_closed: false,
            };
            assert!(
                candidate.validate().is_err(),
                "mutated ABI key retained an old commitment"
            );
        }
    }
}

impl TerminalCallingTelescope {
    /// Rejoin every retained binder to its complete requirement identity,
    /// including unused binders. Both policy input and proposal comparison use
    /// this declaration projection rather than rebuilding source correspondence.
    pub fn callback_bindings(
        module: &TerminalModule,
        telescope: &terminal_psi::CallingSignatureTelescope,
    ) -> Result<Vec<BoundaryCallbackBinding>, String> {
        telescope
            .binders
            .iter()
            .map(|binder| {
                Ok(BoundaryCallbackBinding {
                    binder: binder.binder,
                    requirement: binder.requirement,
                    static_machine_ordinal: binder.static_machine_ordinal,
                    requirement_identity: module
                        .identities
                        .text(binder.requirement_identity)
                        .filter(|identity| !identity.is_empty())
                        .ok_or("Terminal callback binder lost its exact requirement identity")?
                        .to_owned(),
                    invokes_synchronously: binder.invokes_synchronously,
                })
            })
            .collect()
    }

    /// Recover the declaration and check that the proposed ABI application is
    /// for that exact telescope before native consumers use its placements.
    pub fn for_application(
        module: &TerminalModule,
        boundary: semantic_vocabulary::BoundaryMachineId,
        application: &BoundaryCallingApplication,
    ) -> Result<Self, String> {
        // Several closed applications can share the same template requirement.
        // The actual boundary occurrence, not an inverse template-name search,
        // selects the retained declaration.
        let declaration = module
            .calling_signatures
            .iter()
            .find(|declaration| declaration.boundary == boundary)
            .ok_or("callback boundary has no Terminal calling declaration")?;
        let signature = module
            .calling_abi_signature(*declaration.application.as_ref().map_err(Clone::clone)?)
            .ok_or("calling boundary lost its exact declaration ABI")?;
        if signature.identity(&module.identities)? != application.abi
            || application.requirement_identity != application.abi.requirement_identity
        {
            return Err(
                "calling application names a different exact Terminal declaration ABI".into(),
            );
        }
        let telescope = &signature.storage.as_ref().map_err(Clone::clone)?.telescope;
        let boundary = module
            .boundary_machine(declaration.boundary)
            .ok_or("Terminal calling telescope lost its boundary declaration")?;
        if telescope.parameters.len() != boundary.parameter_order.len()
            || telescope.parameters.len() != application.signature.parameters.len()
        {
            return Err(
                "Terminal calling telescope differs from the semantic parameter roster".into(),
            );
        }
        let mut parameters =
            Vec::with_capacity(telescope.parameters.len() + telescope.direct_callbacks.len());
        for (ordinal, (formal, &shape)) in telescope
            .parameters
            .iter()
            .zip(&application.signature.parameters)
            .enumerate()
        {
            parameters.push(BoundaryNativeParameterGeometry {
                identity: formal.identity,
                native_ordinal: formal.native_ordinal,
                shape: BoundaryNativeParameterShape::Semantic(shape),
                origin: BoundaryNativeParameterOrigin::SemanticFormal {
                    formal_ordinal: u32::try_from(ordinal)
                        .map_err(|_| "Terminal semantic parameter ordinal exceeds u32")?,
                },
            });
        }
        let parameters = super::NativeParameterTelescope::close(
            parameters,
            telescope.direct_callbacks.iter().cloned(),
            application.target,
        )?
        .parameters;
        let proposed = application
            .signature
            .native_parameters
            .span(application.signature.native_parameter_rows)
            .ok_or("calling application has stale native parameter storage")?;
        if parameters != proposed {
            return Err(
                "calling application native parameters differ from the Terminal telescope".into(),
            );
        }
        let mut structural = boundary.structural_parameters.iter();
        for (ordinal, kind) in boundary.parameter_order.iter().enumerate() {
            if *kind != BoundaryParameterKind::Structural {
                continue;
            }
            let parameter = structural
                .next()
                .ok_or("Terminal calling telescope lost a structural parameter")?;
            let referent = if parameter.access != StructuralAccess::Owned {
                Some(parameter.structural_type)
            } else {
                match module
                    .structural_type(parameter.structural_type)
                    .map(|row| &row.shape)
                {
                    Some(StructuralTypeShape::Reference { referent, .. }) => Some(*referent),
                    Some(
                        StructuralTypeShape::ByteSequence(_)
                        | StructuralTypeShape::ElementView { .. },
                    ) => Some(parameter.structural_type),
                    _ => None,
                }
            };
            let Some(referent) = referent else { continue };
            let descriptor = module
                .foreign_reference_descriptor(referent)
                .ok_or("callback reference has no retained foreign transport role")?;
            let (byte_size, alignment) = if descriptor {
                let geometry = runtime_abi::fat_descriptor_layout(application.target);
                (geometry.byte_size, geometry.alignment)
            } else {
                (
                    application.target.pointer_size,
                    application.target.pointer_alignment,
                )
            };
            let shape = application
                .signature
                .parameters
                .get(ordinal)
                .and_then(|root| application.signature.shapes.get(usize::from(*root)))
                .ok_or("callback reference has no value shape")?;
            if shape.class != BoundaryValueClass::Reference
                || usize::from(shape.byte_size) != byte_size
                || usize::from(shape.alignment) != alignment
            {
                return Err("calling application reference geometry differs from the Terminal transport role".into());
            }
        }
        let binders = Self::callback_bindings(module, telescope)?;
        let proposed = application
            .signature
            .callback_bindings
            .span(application.signature.callback_binding_rows)
            .ok_or("calling application has stale callback binder storage")?;
        if binders != proposed {
            return Err(
                "calling application callback binders differ from the Terminal telescope".into(),
            );
        }
        Ok(Self {
            parameters,
            binders,
        })
    }
}
