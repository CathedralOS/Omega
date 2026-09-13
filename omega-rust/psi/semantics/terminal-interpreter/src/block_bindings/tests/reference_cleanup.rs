use super::*;
use crate::StructuralRuntimePlace;
use terminal_psi::{StructuralAffineDiscard, TerminalAffineCleanupAction};

#[test]
fn edge_and_scalar_cleanup_release_reference_descriptor_after_fuel_commit() {
    let carrier_place = PlaceId::new(1).unwrap();
    for scalar_return in [false, true] {
        let terminator = if scalar_return {
            Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(carrier_place)],
            }
        } else {
            Terminator::Jump {
                edge: EdgeId::new(1).unwrap(),
                target: BlockId::new(2).unwrap(),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                trivial_affine_discards: vec![carrier_place],
                residual_affine_discards: Vec::new(),
            }
        };
        let mut execution = execution(terminator);
        let reference_type = StructuralTypeId::new(1).unwrap();
        let referent_type = StructuralTypeId::new(2).unwrap();
        execution
            .structural_types
            .get_mut(&reference_type)
            .unwrap()
            .shape = StructuralTypeShape::Reference {
            referent: referent_type,
            access: StructuralAccess::MutableBorrow,
        };
        execution.structural_types.insert(
            referent_type,
            StructuralTypeDeclaration {
                id: referent_type,
                identity: "boolean".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
        );
        execution
            .structural_values
            .retain(|place, _| *place == carrier_place);
        execution.byte_sequence_values.clear();
        let carrier = StructuralRuntimePlace::from(&execution.structural_values[&carrier_place]);
        let referent = TerminalStructuralValue {
            opaque_identity: 200,
            structural_type: referent_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        let backing = StructuralRuntimePlace::from(&referent);
        execution
            .structural_primitive_storage
            .insert(backing.clone(), TerminalScalarValue::Boolean(true));
        execution
            .reference_referents
            .insert(carrier.clone(), referent);
        execution
            .live_affine_frontier
            .insert(StructuralAffineDiscard {
                place: carrier_place,
                path: Vec::new(),
                structural_type: reference_type,
            });
        let target = execution.blocks.get_mut(&BlockId::new(2).unwrap()).unwrap();
        target.parameters.clear();
        target.structural_parameters.clear();
        let machine = execution
            .machines
            .get_mut(&execution.current_machine)
            .unwrap();
        machine.blocks = execution.blocks.clone();
        if scalar_return {
            machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: Default::default(),
            });
        }
        let mut meter = TerminalFuelMeter::with_allowance(0);
        let exhausted = execution.resume(&mut meter).unwrap();
        assert!(matches!(
            exhausted,
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(execution.resume(&mut meter).unwrap(), exhausted);
        assert!(execution.reference_referents.contains_key(&carrier));
        meter.replenish(1).unwrap();
        let _ = execution.resume(&mut meter).unwrap();
        assert!(execution.reference_referents.is_empty());
        assert!(!execution.structural_values.contains_key(&carrier_place));
        assert_eq!(
            execution.structural_primitive_storage.get(&backing),
            Some(&TerminalScalarValue::Boolean(true))
        );
        let usage = meter.usage().clone();
        let _ = execution.resume(&mut meter).unwrap();
        assert_eq!(meter.usage(), &usage);
        assert!(execution.reference_referents.is_empty());
    }
}
