//! Canonical Terminal-to-object rejoin for mixed structural/scalar ABIs.
//!
//! Object, image, and installation owners can validate physical consistency,
//! but they do not independently own the semantic declarations named by an
//! ABI row. The native artifact does: it owns both canonical Terminal-Psi and
//! the emitted object, so this is the first honest place to rejoin those IDs.

use omega_image_emission::ObjectArtifact;
use psi_core::ScalarType;
use psi_terminal::{TerminalMachine, TerminalModule};

pub(super) fn validate(
    module: &TerminalModule,
    object: &ObjectArtifact,
) -> Result<(), &'static str> {
    for function in object
        .functions()
        .iter()
        .filter(|function| function.mixed_structural_scalar_abi.is_some())
    {
        let matching = module
            .machines
            .iter()
            .filter(|machine| machine.id == function.machine)
            .collect::<Vec<_>>();
        let [machine] = matching.as_slice() else {
            return Err("mixed structural/scalar object ABI does not rejoin one Terminal machine");
        };
        if function.attachment != machine.attachment
            || !matches_terminal_machine(
                machine,
                function
                    .mixed_structural_scalar_abi
                    .as_ref()
                    .expect("filtered mixed structural/scalar ABI exists"),
            )
        {
            return Err(
                "mixed structural/scalar object ABI disagrees with canonical Terminal declarations",
            );
        }
    }
    Ok(())
}

fn matches_terminal_machine(
    machine: &TerminalMachine,
    abi: &omega_target_operations::MixedStructuralScalarFunctionAbi,
) -> bool {
    machine.parameters.len() == abi.scalar_parameters.len()
        && machine
            .parameters
            .iter()
            .zip(&abi.scalar_parameters)
            .all(|(declared, retained)| {
                declared.id == retained.value
                    && declared.scalar_type == ScalarType::Integer(retained.scalar_type)
            })
        && machine.result.scalar_ref().is_some_and(|declared| {
            declared.id == abi.result.value
                && declared.scalar_type == ScalarType::Integer(abi.result.scalar_type)
        })
        && machine.structural_parameters.len() == abi.structural_parameters.len()
        && machine
            .structural_parameters
            .iter()
            .zip(&abi.structural_parameters)
            .enumerate()
            .all(|(position, (declared, retained))| {
                usize::try_from(declared.position) == Ok(position)
                    && declared.place == retained.place
                    && declared.structural_type == retained.structural_type
                    && declared.multiplicity == retained.multiplicity
                    && declared.access == retained.access
                    && declared.projected_qualifications == retained.projected_qualifications
            })
}

#[cfg(test)]
mod tests {
    use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use omega_target_operations::{
        FixedIntegerScalarAbiValue, MixedStructuralScalarFunctionAbi, TargetStructuralParameter,
    };
    use psi_core::{
        BlockId, ContractId, IntegerSign, IntegerType, MachineId, PlaceId, ScalarType,
        StructuralTypeId, ValueId,
    };
    use psi_terminal::{
        MachineContract, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
        TerminalMachine, TerminalMachineResult, ValueDeclaration,
    };

    use super::matches_terminal_machine;

    fn fixture() -> (TerminalMachine, MixedStructuralScalarFunctionAbi) {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let shape = ValueShape::integer(4, 4);
        let call_plan = evaluate_call_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![shape, shape],
                result: Some(shape),
            },
        )
        .expect("mixed call plan");
        let scalar_parameter = ValueId::new(1).expect("scalar parameter");
        let structural_parameter = PlaceId::new(2).expect("structural parameter");
        let result = ValueId::new(3).expect("result");
        let structural_type = StructuralTypeId::new(4).expect("structural type");
        let machine = TerminalMachine {
            id: MachineId::new(1).expect("machine"),
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: scalar_parameter,
                scalar_type: ScalarType::Integer(integer),
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                place: structural_parameter,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Integer(integer),
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("entry"),
            blocks: Vec::new(),
            contract: MachineContract {
                id: ContractId::new(1).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        };
        let abi = MixedStructuralScalarFunctionAbi {
            scalar_parameters: vec![FixedIntegerScalarAbiValue {
                value: scalar_parameter,
                scalar_type: integer,
                placement: call_plan.parameters[0].clone(),
            }],
            structural_parameters: vec![TargetStructuralParameter {
                place: structural_parameter,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape,
                placement: call_plan.parameters[1].clone(),
            }],
            result: FixedIntegerScalarAbiValue {
                value: result,
                scalar_type: integer,
                placement: call_plan.result.clone().expect("result placement"),
            },
            call_plan,
        };
        (machine, abi)
    }

    #[test]
    fn exact_semantic_roster_rejoins_terminal() {
        let (machine, abi) = fixture();
        assert!(matches_terminal_machine(&machine, &abi));
    }

    #[test]
    fn fresh_semantic_ids_do_not_self_authenticate() {
        let (machine, abi) = fixture();

        let mut drifted = abi.clone();
        drifted.scalar_parameters[0].value = ValueId::new(11).expect("drifted parameter");
        assert!(!matches_terminal_machine(&machine, &drifted));

        let mut drifted = abi.clone();
        drifted.result.value = ValueId::new(12).expect("drifted result");
        assert!(!matches_terminal_machine(&machine, &drifted));

        let mut drifted = abi;
        drifted.structural_parameters[0].place = PlaceId::new(13).expect("drifted place");
        assert!(!matches_terminal_machine(&machine, &drifted));
    }
}
