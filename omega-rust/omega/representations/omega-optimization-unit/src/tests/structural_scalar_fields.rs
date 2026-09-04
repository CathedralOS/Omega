use super::fixtures::{id, structural_scalar_fields_plan};
use crate::{
    ObservationEventClass, PsiOptimizationUnit, PsiProvenance, ValueDefinitionSite,
    recompute_psi_optimization_unit_identity, reconstruct_psi_observation_model,
    reconstruct_psi_optimization_unit_seed,
};
use omega_abstract_operations::AbstractOperation;
use psi_core::{
    FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralFieldId, ValueId,
};
use psi_terminal::{StructuralAccess, StructuralPathSegment};

fn changed_identity(
    baseline: &PsiOptimizationUnit,
    mutate: impl FnOnce(&mut AbstractOperation),
) -> crate::OptimizationUnitIdentity {
    let mut changed = baseline.clone();
    mutate(&mut changed.functions[0].blocks[0].nodes[0].operation);
    recompute_psi_optimization_unit_identity(&changed)
}

#[test]
fn reconstruction_preserves_structural_scalar_field_derivatives_exactly() {
    let plan = structural_scalar_fields_plan();
    let unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(80).expect("nonzero schedule"),
    )
    .expect("structural scalar field plan reconstructs");
    let function = &unit.functions[0];
    let store = &function.blocks[0].nodes[0];
    let read = &function.blocks[0].nodes[1];

    assert_eq!(store.operation, plan.functions[0].operations[0]);
    assert_eq!(read.operation, plan.functions[0].operations[1]);
    assert!(store.definitions.is_empty());
    assert_eq!(store.uses.len(), 1);
    assert_eq!(store.uses[0].value, id(82, ValueId::new));
    assert_eq!(
        store.provenance,
        vec![PsiProvenance::Operation(id(87, OperationId::new))]
    );
    assert_eq!(read.definitions.len(), 1);
    assert_eq!(read.definitions[0].value, id(83, ValueId::new));
    assert_eq!(
        read.definitions[0].scalar_type,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    );
    assert_eq!(
        read.definitions[0].site,
        ValueDefinitionSite::Node {
            block: function.entry,
            node: 1,
        }
    );
    assert!(read.uses.is_empty());
    assert_eq!(
        read.provenance,
        vec![PsiProvenance::Operation(id(88, OperationId::new))]
    );
    assert!(function.declared_places.contains(&id(84, PlaceId::new)));

    let observations = reconstruct_psi_observation_model(&unit);
    for (observation, operation) in observations.nodes[..2]
        .iter()
        .zip(&plan.functions[0].operations[..2])
    {
        assert_eq!(&observation.operation, operation);
        assert_eq!(observation.events.len(), 1);
        assert_eq!(
            observation.events[0].class,
            ObservationEventClass::StructuralState
        );
        assert_eq!(&observation.events[0].operation, operation);
    }
}

#[test]
fn structural_scalar_field_store_identity_binds_every_payload() {
    let baseline = reconstruct_psi_optimization_unit_seed(
        &structural_scalar_fields_plan(),
        FuelScheduleIdentity::new(80).unwrap(),
    )
    .unwrap();
    let baseline_identity = baseline.identity;

    let mutations: [fn(&mut AbstractOperation); 6] = [
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { psi_operation, .. } => {
                *psi_operation = id(90, OperationId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { destination, .. } => {
                destination.access = StructuralAccess::WriteOnlyBorrow
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { path, .. } => {
                path.push(StructuralPathSegment::FixedIndex(2))
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { field, .. } => {
                *field = id(91, StructuralFieldId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { value, .. } => {
                value.value = id(92, ValueId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::StructuralScalarFieldStore { value, .. } => {
                value.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap())
            }
            _ => unreachable!(),
        },
    ];
    for mutate in mutations {
        assert_ne!(baseline_identity, changed_identity(&baseline, mutate));
    }
}

#[test]
fn integer_structural_field_identity_binds_every_payload() {
    let baseline = reconstruct_psi_optimization_unit_seed(
        &structural_scalar_fields_plan(),
        FuelScheduleIdentity::new(80).unwrap(),
    )
    .unwrap();
    let baseline_identity = baseline.identity;

    let changed_read_identity = |mutate: fn(&mut AbstractOperation)| {
        let mut changed = baseline.clone();
        mutate(&mut changed.functions[0].blocks[0].nodes[1].operation);
        recompute_psi_optimization_unit_identity(&changed)
    };
    let mutations: [fn(&mut AbstractOperation); 5] = [
        |operation| match operation {
            AbstractOperation::IntegerStructuralField { psi_operation, .. } => {
                *psi_operation = id(93, OperationId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::IntegerStructuralField { result, .. } => {
                result.value = id(94, ValueId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::IntegerStructuralField { result, .. } => {
                result.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::IntegerStructuralField { source, .. } => {
                source.place = id(95, PlaceId::new)
            }
            _ => unreachable!(),
        },
        |operation| match operation {
            AbstractOperation::IntegerStructuralField { field, .. } => {
                *field = id(96, StructuralFieldId::new)
            }
            _ => unreachable!(),
        },
    ];
    for mutate in mutations {
        assert_ne!(baseline_identity, changed_read_identity(mutate));
    }
}
