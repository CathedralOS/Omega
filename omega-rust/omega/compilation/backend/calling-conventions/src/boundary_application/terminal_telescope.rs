/*
We recover native argument order and callback binder correspondence from the
canonical Terminal declaration (`TerminalCallingTelescope`), checking that a
proposed `BoundaryCallingApplication` matches the exact reached boundary
declaration ABI, parameter order, foreign reference descriptor geometry, and
retained callback bindings before native consumers use its placements.
*/

use super::{
    BoundaryCallbackBinding, BoundaryCallingApplication, BoundaryNativeParameterGeometry,
    BoundaryNativeParameterShape,
};
use crate::BoundaryNativeParameterOrigin;
use crate::BoundaryValueClass;
use terminal_psi::{BoundaryParameterKind, StructuralAccess, StructuralTypeShape, TerminalModule};

pub struct TerminalCallingTelescope {
    pub parameters: Vec<BoundaryNativeParameterGeometry>,
    pub binders: Vec<BoundaryCallbackBinding>,
}

impl TerminalCallingTelescope {
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

    pub fn for_application(
        module: &TerminalModule,
        boundary: semantic_vocabulary::BoundaryMachineId,
        application: &BoundaryCallingApplication,
    ) -> Result<Self, String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundarySignatureGeometry, CallSignature, CallingPolicy};
    use semantic_vocabulary::{BoundaryMachineId, CallingOperandKind, CallingSignatureId};

    fn application() -> BoundaryCallingApplication {
        let plan = crate::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![],
                result: None,
            },
        )
        .unwrap();
        BoundaryCallingApplication {
            calling_owner_application: None,
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
            dispatch_only_parameter_count: 0,
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
        }
    }

    #[test]
    fn selected_calling_owner_is_part_of_recovered_application_identity() {
        let default = application();
        let mut first = default.clone();
        first.calling_owner_application = Some("package::First<u8>".into());
        first.commitment = first.recomputed_commitment().unwrap();
        let mut sibling = first.clone();
        sibling.calling_owner_application = Some("package::Second<u8>".into());

        assert_eq!(first.abi, sibling.abi);
        assert_eq!(first.plan, sibling.plan);
        assert_ne!(first.commitment, sibling.recomputed_commitment().unwrap());
        assert_ne!(first.commitment, default.recomputed_commitment().unwrap());

        let recover = |owner: Option<String>| {
            super::super::BoundaryCallingApplicationCandidate {
                calling_owner_application: owner,
                abi: first.abi.clone(),
                requirement_identity: first.requirement_identity.clone(),
                target: first.target,
                report_fingerprint: first.report_fingerprint,
                commitment: first.commitment,
                plan: first.plan.plan().clone(),
                dispatch_only_parameter_count: first.dispatch_only_parameter_count,
                signature: first.signature.clone(),
                opaque_applications: first.opaque_applications.clone(),
                callback_context: first.callback_context.clone(),
                callback_context_closed: first.callback_context_closed,
            }
            .validate()
        };
        assert_eq!(
            recover(first.calling_owner_application.clone()).unwrap(),
            first
        );
        for changed_owner in [sibling.calling_owner_application, None] {
            assert_eq!(
                recover(changed_owner).unwrap_err(),
                "recovered calling application commitment does not match its complete payload"
            );
        }
        assert_eq!(
            recover(Some(String::new())).unwrap_err(),
            "calling application has an empty selected owner"
        );
    }

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
                physical_owner: Ok(semantic_vocabulary::CallingDeclarationOwner::BoundaryTrait),
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
                result_reference_regions: Vec::new(),
                reference_outputs: vec![],
                program_local_root_introductions: vec![],
                content_guarantees: vec![],
                fixed_service_reach: vec![],
                owning_service: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: vec![],
            });
        let application = application();

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
                calling_owner_application: application.calling_owner_application.clone(),
                abi,
                requirement_identity: application.requirement_identity.clone(),
                target: application.target,
                report_fingerprint: application.report_fingerprint,
                commitment,
                plan: application.plan.plan().clone(),
                dispatch_only_parameter_count: application.dispatch_only_parameter_count,
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
