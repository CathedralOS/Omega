/*
We define nominal callback-binder to native-place plans for outbound
registrars and validate that every declared static-machine binder and private
native-place demand joins one-to-one without overlapping destinations or
mismatching callback requirement contracts. Nominal layout and slot IDs act as
compact discriminators inside our retained callback catalog; authority comes
from replaying the complete owning requirement and layout catalog.
*/

use crate::plans::{PlanDiagnostic, ValuePlacement, ValueShape};

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

nominal_plan_id!(LayoutPlanId);
nominal_plan_id!(LayoutSlotId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeParameterApplication {
    pub parameter: NativeParameterId,
    pub native_ordinal: u32,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
}

pub use semantic_vocabulary::{
    BoundaryNativeParameterOrigin, CallbackRequirementId, NativeParameterId, StaticMachineBinderId,
    callback_nominal_report_fingerprint, callback_requirement_id,
    nominal_callback_native_parameter_id,
};

pub fn callback_layout_plan_id(
    native_layout_report_fingerprint: u64,
    pointer_size: usize,
    pointer_alignment: usize,
) -> LayoutPlanId {
    LayoutPlanId::new(callback_nominal_report_fingerprint(
        b"omega.callback-layout-plan.v1",
        &[
            &native_layout_report_fingerprint.to_le_bytes(),
            &(pointer_size as u64).to_le_bytes(),
            &(pointer_alignment as u64).to_le_bytes(),
        ],
    ))
    .expect("callback layout-plan identity is nonzero")
}

pub fn callback_plan_laid_layout_id(
    native_layout_report_fingerprint: u64,
    canonical_data_identity: &str,
    canonical_layout_subject: &str,
    pointer_size: usize,
    pointer_alignment: usize,
) -> LayoutPlanId {
    LayoutPlanId::new(callback_nominal_report_fingerprint(
        b"omega.callback-plan-laid-layout.v1",
        &[
            &native_layout_report_fingerprint.to_le_bytes(),
            canonical_data_identity.as_bytes(),
            canonical_layout_subject.as_bytes(),
            &(pointer_size as u64).to_le_bytes(),
            &(pointer_alignment as u64).to_le_bytes(),
        ],
    ))
    .expect("callback plan-laid layout identity is nonzero")
}

pub fn callback_layout_slot_id(layout: LayoutPlanId, canonical_slot: &str) -> LayoutSlotId {
    LayoutSlotId::new(callback_nominal_report_fingerprint(
        b"omega.callback-layout-slot.v1",
        &[&layout.get().to_le_bytes(), canonical_slot.as_bytes()],
    ))
    .expect("callback layout-slot identity is nonzero")
}

pub fn callback_layout_field_slot_id(layout: LayoutPlanId, canonical_field: &str) -> LayoutSlotId {
    LayoutSlotId::new(callback_nominal_report_fingerprint(
        b"omega.callback-layout-field-slot.v1",
        &[&layout.get().to_le_bytes(), canonical_field.as_bytes()],
    ))
    .expect("callback layout field-slot identity is nonzero")
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackBinderRequirement {
    pub binder: StaticMachineBinderId,
    pub requirement: CallbackRequirementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCallbackDemand {
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackMaterializationContext {
    pub binders: Vec<CallbackBinderRequirement>,
    pub demands: Vec<NativeCallbackDemand>,
}

pub(crate) fn validate_callback_materializations(
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
    use super::{
        CallbackBinderRequirement, CallbackMaterialization, CallbackMaterializationContext,
        CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativeCallbackDemand, NativeParameterId,
        NativePlace, StaticMachineBinderId, nominal_callback_native_parameter_id,
    };
    use crate::{
        CallSignature, CallingPolicy, evaluate_ordinary_boundary_entry_plan,
        validate_boundary_entry_plan, validate_boundary_entry_plan_with_callback_materializations,
    };

    #[test]
    fn nominal_native_parameter_identity_is_name_owned_and_order_independent() {
        let requirement = "package::Registrar::install#exact";
        let callback = nominal_callback_native_parameter_id(requirement, "procedure");
        assert_eq!(
            callback,
            nominal_callback_native_parameter_id(requirement, "procedure")
        );
        assert_ne!(
            callback,
            nominal_callback_native_parameter_id(requirement, "module")
        );
        assert_ne!(
            callback,
            nominal_callback_native_parameter_id("package::Other::install#exact", "procedure")
        );
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
            validated.contract_report_fingerprint(),
            baseline.contract_report_fingerprint(),
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
