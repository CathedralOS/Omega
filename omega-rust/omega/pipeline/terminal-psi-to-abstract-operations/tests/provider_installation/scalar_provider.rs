use super::builders::{artifact, provider_module, selected};
use super::ids::{machine_id, operation_id};
use abstract_operations::AbstractOperation;
use installation_evidence::ProviderInstallationEvidence;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, OperationResult, TerminalModule, ValueDeclaration};
use terminal_psi_to_abstract_operations::{
    ProviderInstallationError, admit_provider_installation, lower_artifact_sections,
};

#[test]
fn omega_retains_the_exact_installed_provider_scalar_argument() {
    let module = scalar_provider_module();
    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    let installation = admit_provider_installation(&plan, &semantic, &proof, &profile, &selected)
        .expect("fixed i32 provider installation");
    let [call] = installation.installed_unit_calls() else {
        panic!("one installed scalar provider call")
    };
    assert_eq!(call.scalar_arguments(), &[value_id(1)]);

    let projected = ProviderInstallationEvidence::installed_provider_unit_calls(&installation);
    let [evidence] = projected.as_slice() else {
        panic!("one projected scalar provider call")
    };
    assert_eq!(evidence.scalar_arguments, [value_id(1)]);
}

#[test]
fn omega_rejects_removing_the_installed_provider_scalar_argument() {
    let (semantic, proof, profile, selected) = scalar_fixture();
    let mut plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    scalar_call_arguments_mut(&mut plan).clear();

    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));
}

#[test]
fn omega_rejects_substituting_a_computed_i32_for_the_exact_caller_parameter() {
    let mut module = scalar_provider_module();
    let scalar_type = signed_i32();
    let replacement = value_id(9);
    let caller_block = &mut module.machines[0].blocks[0];
    caller_block.operations.insert(
        0,
        Operation {
            id: operation_id(9),
            result: OperationResult::Scalar(ValueDeclaration {
                id: replacement,
                scalar_type,
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Signed(7),
            },
        },
    );
    let OperationKind::BoundaryCall { arguments, .. } = &mut caller_block.operations[1].kind else {
        panic!("fixture retains its provider boundary call")
    };
    arguments[0] = replacement;

    let (semantic, proof) = artifact(&module);
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile)
        .expect("computed i32 argument is valid Terminal Psi");
    let selected = selected("second-plan", "SecondProvider", "SecondProvider::emit");
    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::InstalledUnitCallReplayMismatch { .. })
    ));
}

#[test]
fn omega_rejects_installed_provider_boundary_scalar_type_drift() {
    let (semantic, proof, profile, selected) = scalar_fixture();
    let mut plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    plan.boundary_machines[0].scalar_parameters[0] = ScalarType::Boolean;

    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));
}

#[test]
fn omega_rejects_installed_provider_candidate_scalar_type_drift() {
    let (semantic, proof, profile, selected) = scalar_fixture();
    let mut plan = lower_artifact_sections(&semantic, &proof, &profile).expect("verified lowering");
    plan.functions
        .iter_mut()
        .find(|function| function.machine == machine_id(3))
        .expect("selected candidate function")
        .parameters[0]
        .scalar_type = ScalarType::Boolean;

    assert!(matches!(
        admit_provider_installation(&plan, &semantic, &proof, &profile, &selected),
        Err(ProviderInstallationError::PlanReplayMismatch)
    ));
}

fn scalar_fixture() -> (
    Vec<u8>,
    Vec<u8>,
    AdmissionProfile,
    Vec<terminal_psi_to_abstract_operations::SelectedProviderAdapter>,
) {
    let module = scalar_provider_module();
    let (semantic, proof) = artifact(&module);
    (
        semantic,
        proof,
        AdmissionProfile::default(),
        selected("second-plan", "SecondProvider", "SecondProvider::emit"),
    )
}

fn scalar_provider_module() -> TerminalModule {
    let mut module = provider_module();
    let scalar_type = signed_i32();
    module.boundary_machines[0].scalar_parameters = vec![scalar_type];
    for (index, machine) in module.machines.iter_mut().enumerate() {
        let parameter = ValueDeclaration {
            id: value_id(index as u64 + 1),
            scalar_type,
        };
        machine.parameters = vec![parameter];
        if index == 0 {
            let OperationKind::BoundaryCall { arguments, .. } =
                &mut machine.blocks[0].operations[0].kind
            else {
                panic!("caller fixture starts with its provider boundary call")
            };
            *arguments = vec![parameter.id];
        }
    }
    module
}

fn scalar_call_arguments_mut(
    plan: &mut abstract_operations::AbstractOperationPlan,
) -> &mut Vec<ValueId> {
    plan.functions[0]
        .operations
        .iter_mut()
        .find_map(|operation| match operation {
            AbstractOperation::BoundaryCall { arguments, .. } => Some(arguments),
            _ => None,
        })
        .expect("fixture retains its provider boundary call")
}

fn signed_i32() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("valid signed i32"))
}

fn value_id(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}
