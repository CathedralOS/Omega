//! `initialize(player: &mut Player, name: [u8; 16] in Utf8)` copies the
//! owned formal's live bytes into `player.name`, without borrowing its ABI.
//! This native consumer fixture supplies the exact admitted capacity fact;
//! source checking and proof production are covered by the Stage05 regression.

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarTerm,
    ScalarType,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet,
    StructuralAccess, StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn id<T: PsiSemanticId>(value: u64) -> T {
    T::new(value).unwrap()
}

#[test]
fn owned_qualified_byte_parameter_replaces_field_through_four_native_abis() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        predicates.insert(ByteSequencePredicate::ValidUtf8);
        let carrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
        source.structural_types = vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "Player".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: id(1),
                        identity: "name".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier,
                            predicates,
                        },
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: "[u8;16]".into(),
                shape: StructuralTypeShape::ByteSequence(carrier),
            },
        ]
        .into();
        let function = &mut source.functions[0];
        function.structural_parameters = vec![
            StructuralParameterDeclaration {
                place: id(1),
                position: 0,
                is_self: false,
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: id(2),
                position: 1,
                is_self: false,
                structural_type: id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                qualifications: vec![id(1)],
                projected_qualifications: Vec::new(),
            },
        ];
        let returned = function.operations.pop().unwrap();
        function.operations = vec![
            AbstractOperation::ByteSequenceLength {
                psi_operation: id(1),
                source: id(2),
                result: AbstractResult {
                    value: id(1),
                    scalar_type: ScalarType::Integer(integer),
                },
            },
            AbstractOperation::StructuralByteSequenceFieldStore {
                psi_operation: id(2),
                destination: id(1),
                path: Vec::new(),
                field: id(1),
                source: id(2),
                length: id(1),
                obligation: id(1),
            },
            returned,
        ];
        let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("reconstruct the actual owned parameter and byte operations");
        unit.structural_domains = vec![StructuralDomainDeclaration {
            id: id(1),
            semantic_domain: id(1),
            identity: "Utf8<[u8;16]>".into(),
            carrier: id(2),
            byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
            content_projection: None,
            establishment_routes: Vec::new(),
        }]
        .into();
        let capacity_goal = semantic_vocabulary::Proposition::LessOrEqual(
            ScalarTerm::value(id(1), ScalarType::Integer(integer)),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(16)).unwrap(),
        );
        let accepted = optimization_unit::AcceptedObligationFact::new(
            unit.psi,
            [4; 32],
            source.functions[0].machine,
            id(2),
            id(1),
            terminal_codec::canonical_proposition_order_key(&capacity_goal).unwrap(),
        );
        let unit = optimization_unit::attach_accepted_obligation_facts(unit, vec![accepted])
            .expect("retain the exact store capacity goal");
        optimization_unit_semantics::validate_psi_optimization_unit(&unit)
            .expect("independently validate the qualified owned source before native lowering");
        for access in [
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut changed = unit.clone();
            changed.functions[0].structural_parameters[1].access = access;
            changed.identity =
                optimization_unit::recompute_psi_optimization_unit_identity(&changed);
            assert!(
                optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
                "{access:?} cannot replace immutable owned copy-source authority"
            );
        }
        let mut unqualified = unit.clone();
        unqualified.functions[0].structural_parameters[1]
            .qualifications
            .clear();
        unqualified.identity =
            optimization_unit::recompute_psi_optimization_unit_identity(&unqualified);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&unqualified).is_err(),
            "the capacity fact cannot supply the source's missing Utf8 declaration"
        );
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .unwrap_or_else(|error| {
            panic!("owned byte formal target lowering on {native:?}: {error:?}")
        });
        let parameter = &target.functions[0].graph.parameters[1];
        assert_eq!(parameter.access, StructuralAccess::Owned);
        assert_eq!(
            parameter.shape,
            calling_conventions::ValueShape::integer(24, 8)
        );
        let legalized =
            crate::legalize_target_operations(&target, &source, &unit).unwrap_or_else(|error| {
                panic!("owned byte formal legalization on {native:?}: {error:?}")
            });
        crate::validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
            .expect("independently replay the owned source and qualified destination");
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legalized, &environment);
        let selected = crate::select_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("owned byte formal selection on {native:?}: {error:?}"));
        crate::validate_selected_instructions(
            &legalized,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .expect("replay live-length copying and owned backing geometry");

        let mut borrowed_signature = target.clone();
        borrowed_signature.functions[0].graph.parameters[1].access = StructuralAccess::SharedBorrow;
        assert!(
            crate::legalize_target_operations(&borrowed_signature, &source, &unit).is_err(),
            "an owned byte payload cannot be reinterpreted as a borrowed parameter"
        );
    }
}
