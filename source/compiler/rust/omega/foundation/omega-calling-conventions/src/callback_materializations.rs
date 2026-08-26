//! Nominal callback-binder to native-place plans for outbound registrars.

use crate::plans::PlanDiagnostic;

macro_rules! nominal_plan_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub const fn new(value: u64) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

nominal_plan_id!(StaticMachineBinderId);
nominal_plan_id!(NativeParameterId);
nominal_plan_id!(LayoutPlanId);
nominal_plan_id!(LayoutSlotId);
nominal_plan_id!(CallbackRequirementId);

fn callback_nominal_identity(domain: &[u8], parts: &[&[u8]]) -> u64 {
    let mut identity = 0xcbf2_9ce4_8422_2325u64;
    for bytes in std::iter::once(domain).chain(parts.iter().copied()) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            identity ^= u64::from(byte);
            identity = identity.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    if identity == 0 { 1 } else { identity }
}

/// Compiler-issued identity of one canonical callback requirement overload.
/// The same constructor is used for boundary binders and target-closed private
/// layout demands, so matching text cannot be rebound through a second hash
/// convention.
pub fn callback_requirement_id(canonical_requirement: &str) -> CallbackRequirementId {
    CallbackRequirementId::new(callback_nominal_identity(
        b"omega.callback-requirement.v1",
        &[canonical_requirement.as_bytes()],
    ))
    .expect("callback requirement identity is nonzero")
}

/// Compiler-issued identity of one complete target-closed native layout.
/// Pointer geometry participates because it supplies the physical callback
/// extent that is absent from the target-neutral layout report.
pub fn callback_layout_plan_id(
    native_layout_fingerprint: u64,
    pointer_size: usize,
    pointer_alignment: usize,
) -> LayoutPlanId {
    LayoutPlanId::new(callback_nominal_identity(
        b"omega.callback-layout-plan.v1",
        &[
            &native_layout_fingerprint.to_le_bytes(),
            &(pointer_size as u64).to_le_bytes(),
            &(pointer_alignment as u64).to_le_bytes(),
        ],
    ))
    .expect("callback layout-plan identity is nonzero")
}

/// Compiler-issued identity of one exact named private slot in one closed
/// layout. The physical offset is already retained by the complete layout
/// identity and is never repeated as slot identity.
pub fn callback_layout_slot_id(layout: LayoutPlanId, canonical_slot: &str) -> LayoutSlotId {
    LayoutSlotId::new(callback_nominal_identity(
        b"omega.callback-layout-slot.v1",
        &[&layout.get().to_le_bytes(), canonical_slot.as_bytes()],
    ))
    .expect("callback layout-slot identity is nonzero")
}

/// One target-owned destination for a compiler-private callback relocation.
///
/// These are nominal plan identities, never source parameter ordinals or byte
/// offsets. A field path is interpreted only through its exact validated
/// layout plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NativePlace {
    Parameter(NativeParameterId),
    Field {
        parameter: NativeParameterId,
        layout: LayoutPlanId,
        field_path: Vec<LayoutSlotId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackMaterialization {
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
}

/// Requirement identity attached to one nominal static-machine binder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackBinderRequirement {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
}

/// One typed private-materialization demand published by validated native
/// parameter/layout custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackDemand {
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
}

/// Closed validation context for the callback rows of one outbound registrar
/// plan. Construction of these nominal identities belongs to the compiler's
/// signature and layout pipelines; the calling-plan validator only joins them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackMaterializationContext {
    pub binders: Vec<CallbackBinderRequirement>,
    pub demands: Vec<NativeCallbackDemand>,
}

pub(super) fn validate_callback_materializations(
    rows: &[CallbackMaterialization],
    context: &CallbackMaterializationContext,
) -> Result<(), PlanDiagnostic> {
    for (index, binder) in context.binders.iter().enumerate() {
        if context.binders[..index]
            .iter()
            .any(|prior| prior.binder == binder.binder)
        {
            return Err(PlanDiagnostic(
                "callback materialization context repeats a binder identity".into(),
            ));
        }
    }
    for (index, demand) in context.demands.iter().enumerate() {
        validate_native_place(&demand.destination)?;
        if context.demands[..index]
            .iter()
            .any(|prior| native_places_overlap(&prior.destination, &demand.destination))
        {
            return Err(PlanDiagnostic(
                "callback materialization context contains overlapping native-place demands".into(),
            ));
        }
    }

    for (index, row) in rows.iter().enumerate() {
        validate_native_place(&row.destination)?;
        let Some(binder) = context
            .binders
            .iter()
            .find(|candidate| candidate.binder == row.binder)
        else {
            return Err(PlanDiagnostic(
                "callback materialization names an unknown binder identity".into(),
            ));
        };
        if rows[..index].iter().any(|prior| prior.binder == row.binder) {
            return Err(PlanDiagnostic(
                "callback materialization repeats a binder identity".into(),
            ));
        }
        if rows[..index]
            .iter()
            .any(|prior| native_places_overlap(&prior.destination, &row.destination))
        {
            return Err(PlanDiagnostic(
                "callback materializations overlap one native destination".into(),
            ));
        }
        let Some(demand) = context
            .demands
            .iter()
            .find(|candidate| candidate.destination == row.destination)
        else {
            return Err(PlanDiagnostic(
                "callback materialization does not name a declared private native-place demand"
                    .into(),
            ));
        };
        if binder.requirement != demand.requirement {
            return Err(PlanDiagnostic(
                "callback materialization binder and native-place demand require different callback contracts"
                    .into(),
            ));
        }
    }

    if context
        .binders
        .iter()
        .any(|binder| !rows.iter().any(|row| row.binder == binder.binder))
    {
        return Err(PlanDiagnostic(
            "callback materialization plan omits a nominal callback binder".into(),
        ));
    }
    if context
        .demands
        .iter()
        .any(|demand| !rows.iter().any(|row| row.destination == demand.destination))
    {
        return Err(PlanDiagnostic(
            "callback materialization plan leaves a private native-place demand unsatisfied".into(),
        ));
    }
    Ok(())
}

fn validate_native_place(place: &NativePlace) -> Result<(), PlanDiagnostic> {
    if let NativePlace::Field { field_path, .. } = place
        && field_path.is_empty()
    {
        return Err(PlanDiagnostic(
            "callback materialization field destination has an empty layout-slot path".into(),
        ));
    }
    Ok(())
}

fn native_places_overlap(left: &NativePlace, right: &NativePlace) -> bool {
    match (left, right) {
        (NativePlace::Parameter(left), NativePlace::Parameter(right)) => left == right,
        (
            NativePlace::Parameter(left),
            NativePlace::Field {
                parameter: right, ..
            },
        )
        | (
            NativePlace::Field {
                parameter: left, ..
            },
            NativePlace::Parameter(right),
        ) => left == right,
        (
            NativePlace::Field {
                parameter: left_parameter,
                layout: left_layout,
                field_path: left_path,
            },
            NativePlace::Field {
                parameter: right_parameter,
                layout: right_layout,
                field_path: right_path,
            },
        ) => {
            left_parameter == right_parameter
                && (left_layout != right_layout
                    || left_path.starts_with(right_path)
                    || right_path.starts_with(left_path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
        validate_boundary_entry_plan, validate_boundary_entry_plan_with_callback_materializations,
    };

    #[test]
    fn target_closed_nominal_identities_are_domain_separated_and_mutation_sensitive() {
        let first_layout = callback_layout_plan_id(0x101, 8, 8);
        assert_eq!(first_layout, callback_layout_plan_id(0x101, 8, 8));
        assert_ne!(first_layout, callback_layout_plan_id(0x102, 8, 8));
        assert_ne!(first_layout, callback_layout_plan_id(0x101, 4, 4));

        let first_slot = callback_layout_slot_id(first_layout, "package::WindowSlot");
        assert_eq!(
            first_slot,
            callback_layout_slot_id(first_layout, "package::WindowSlot")
        );
        assert_ne!(
            first_slot,
            callback_layout_slot_id(first_layout, "package::OtherSlot")
        );
        assert_ne!(
            first_slot,
            callback_layout_slot_id(callback_layout_plan_id(0x102, 8, 8), "package::WindowSlot")
        );

        let requirement = callback_requirement_id("package::WindowProcedure::call#exact");
        assert_eq!(
            requirement,
            callback_requirement_id("package::WindowProcedure::call#exact")
        );
        assert_ne!(
            requirement,
            callback_requirement_id("package::WindowProcedure::call#other")
        );
        assert_ne!(requirement.get(), first_layout.get());
        assert_ne!(requirement.get(), first_slot.get());
    }

    #[test]
    fn binds_nominal_binders_to_exact_native_demands() {
        let signature = CallSignature::default();
        let baseline =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary registrar plan");
        let binder = StaticMachineBinderId::new(11).unwrap();
        let requirement = CallbackRequirementId::new(12).unwrap();
        let destination = NativePlace::Field {
            parameter: NativeParameterId::new(13).unwrap(),
            layout: LayoutPlanId::new(14).unwrap(),
            field_path: vec![
                LayoutSlotId::new(15).unwrap(),
                LayoutSlotId::new(16).unwrap(),
            ],
        };
        let mut registrar = baseline.plan().clone();
        registrar.call.callback_materializations = vec![CallbackMaterialization {
            binder,
            destination: destination.clone(),
        }];
        let context = CallbackMaterializationContext {
            binders: vec![CallbackBinderRequirement {
                binder,
                requirement,
            }],
            demands: vec![NativeCallbackDemand {
                destination,
                requirement,
            }],
        };

        let validated = validate_boundary_entry_plan_with_callback_materializations(
            registrar.clone(),
            &signature,
            &context,
        )
        .expect("exact callback binder/native-place join");
        assert_ne!(
            validated.contract_fingerprint(),
            baseline.contract_fingerprint(),
            "private callback placement is part of the registrar ABI identity"
        );
        assert!(
            validate_boundary_entry_plan(registrar, &signature)
                .expect_err("a callback row without its nominal context must reject")
                .0
                .contains("require their nominal binder")
        );
    }

    #[test]
    fn rejects_missing_and_incompatible_rows() {
        let signature = CallSignature::default();
        let registrar =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary registrar plan")
                .plan()
                .clone();
        let binder = StaticMachineBinderId::new(21).unwrap();
        let destination = NativePlace::Parameter(NativeParameterId::new(22).unwrap());
        let mut context = CallbackMaterializationContext {
            binders: vec![CallbackBinderRequirement {
                binder,
                requirement: CallbackRequirementId::new(23).unwrap(),
            }],
            demands: vec![NativeCallbackDemand {
                destination: destination.clone(),
                requirement: CallbackRequirementId::new(23).unwrap(),
            }],
        };

        let missing = validate_boundary_entry_plan_with_callback_materializations(
            registrar.clone(),
            &signature,
            &context,
        )
        .expect_err("missing binder row");
        assert!(missing.0.contains("omits a nominal callback binder"));

        let mut populated = registrar;
        populated.call.callback_materializations = vec![CallbackMaterialization {
            binder,
            destination,
        }];
        context.demands[0].requirement = CallbackRequirementId::new(24).unwrap();
        let incompatible = validate_boundary_entry_plan_with_callback_materializations(
            populated, &signature, &context,
        )
        .expect_err("requirement mismatch");
        assert!(incompatible.0.contains("different callback contracts"));
    }

    #[test]
    fn rejects_overlapping_field_demands() {
        let parameter = NativeParameterId::new(31).unwrap();
        let layout = LayoutPlanId::new(32).unwrap();
        let outer = LayoutSlotId::new(33).unwrap();
        let context = CallbackMaterializationContext {
            binders: Vec::new(),
            demands: vec![
                NativeCallbackDemand {
                    destination: NativePlace::Field {
                        parameter,
                        layout,
                        field_path: vec![outer],
                    },
                    requirement: CallbackRequirementId::new(34).unwrap(),
                },
                NativeCallbackDemand {
                    destination: NativePlace::Field {
                        parameter,
                        layout,
                        field_path: vec![outer, LayoutSlotId::new(35).unwrap()],
                    },
                    requirement: CallbackRequirementId::new(36).unwrap(),
                },
            ],
        };
        let signature = CallSignature::default();
        let registrar =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                .expect("ordinary registrar plan")
                .plan()
                .clone();

        let error = validate_boundary_entry_plan_with_callback_materializations(
            registrar, &signature, &context,
        )
        .expect_err("overlapping private layout demands");
        assert!(error.0.contains("overlapping native-place demands"));
    }
}
