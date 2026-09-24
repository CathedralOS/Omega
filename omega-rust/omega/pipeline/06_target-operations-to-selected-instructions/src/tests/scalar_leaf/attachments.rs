//! A nominal attachment neither adds a receiver nor disappears from native custody.

use super::{
    fixture, legalize_target_operations, select_instructions, selection_constraints,
    validate_legalized_operations, validate_selected_instructions,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use target::NativeTarget;
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[test]
fn attached_scalar_graph_retains_nominal_identity_without_an_implicit_receiver() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        // Exercise both a literal return and an ordinary scalar parameter.
        for immediate in [Some(7), None] {
            let (mut source, _, previous) = fixture(immediate, native);
            let attachment = StructuralTypeId::new(1).unwrap();
            let other = StructuralTypeId::new(2).unwrap();
            for (identity, name) in [(attachment, "Buffer"), (other, "OtherBuffer")] {
                source
                    .structural_types
                    .make_mut()
                    .push(StructuralTypeDeclaration {
                        id: identity,
                        identity: name.into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![StructuralFieldDeclaration {
                                id: StructuralFieldId::new(1).unwrap(),
                                identity: "stored".into(),
                                relevance: BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                                )),
                            }],
                        },
                    });
            }
            source.functions[0].attachment = Some(attachment);
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                previous.fuel_schedule,
            )
            .unwrap();
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let function = &legal.plan().scalar_functions[0];
            assert_eq!(function.attachment, Some(attachment));
            assert_eq!(function.parameters.len(), usize::from(immediate.is_none()));
            assert!(
                function.structural.is_none(),
                "a type is not receiver storage"
            );
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();

            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert_eq!(selected.plan().functions[0].attachment, Some(attachment));
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();

            // Same layout is not the same nominal owner. Neither omission nor
            // substitution may cross any of the independently checked joins.
            for replacement in [None, Some(other)] {
                let mut changed = target.clone();
                changed.functions[0].attachment = replacement;
                assert!(legalize_target_operations(&changed, &source, &unit).is_err());
                let mut changed = source.clone();
                changed.functions[0].attachment = replacement;
                assert!(legalize_target_operations(&target, &changed, &unit).is_err());
                let mut changed = unit.clone();
                changed.functions[0].attachment = replacement;
                changed.identity =
                    optimization_unit::recompute_psi_optimization_unit_identity(&changed);
                assert!(legalize_target_operations(&target, &source, &changed).is_err());
                let mut changed = legal.plan().clone();
                changed.scalar_functions[0].attachment = replacement;
                assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
                let mut changed = selected.plan().clone();
                changed.functions[0].attachment = replacement;
                assert!(
                    validate_selected_instructions(
                        &legal,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err()
                );
            }
        }
    }
}
