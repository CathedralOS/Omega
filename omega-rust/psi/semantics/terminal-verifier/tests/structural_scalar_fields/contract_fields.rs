use super::*;
use semantic_vocabulary::{
    ByteSequenceStructuralField, IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField,
};

#[derive(Clone, Copy, Debug)]
enum Clause {
    Requires,
    Ensures,
}

fn module() -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.machines.truncate(1);
    module.machines[0].blocks[0].operations.clear();
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    module
}

fn path() -> Vec<CanonicalStructuralPathSegment> {
    vec![CanonicalStructuralPathSegment::Field(id::<StructuralFieldId>(1)); 2]
}

fn integer_field(module: &TerminalModule) -> ScalarTerm {
    let ScalarType::Integer(integer_type) = integer_type() else {
        panic!("integer");
    };
    ScalarTerm::integer_field_path(
        module.machines[0].structural_parameters[0].place,
        path(),
        integer_type,
    )
}

fn install(module: &mut TerminalModule, clause: Clause, proposition: Proposition) {
    match clause {
        Clause::Requires => module.machines[0].contract.requires = vec![proposition],
        Clause::Ensures => {
            module.machines[0].contract.ensures = vec![terminal_psi::ContractClause {
                obligation: id::<ObligationId>(90),
                proposition,
            }]
        }
    }
}

fn field_mut(module: &mut TerminalModule) -> &mut StructuralFieldDeclaration {
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("nested record");
    };
    &mut fields[0]
}

#[test]
fn supplied_case_requirement_does_not_read_a_write_only_parameter() {
    let mut module = module();
    let case = semantic_vocabulary::StructuralCaseId::new(1).unwrap();
    module.structural_types[1].shape = StructuralTypeShape::Sum {
        cases: vec![
            terminal_psi::StructuralCaseDeclaration {
                id: case,
                identity: "Present".into(),
                fields: Vec::new(),
            },
            terminal_psi::StructuralCaseDeclaration {
                id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                identity: "Absent".into(),
                fields: Vec::new(),
            },
        ],
    };
    let root = module.machines[0].structural_parameters[0].place;
    module.machines[0]
        .contract
        .requires
        .push(Proposition::StructuralCaseMembership {
            subject: semantic_vocabulary::StructuralCaseSubject::new(
                root,
                vec![CanonicalStructuralPathSegment::Field(
                    id::<StructuralFieldId>(1),
                )],
            ),
            case,
        });
    validate_module(&module).expect("readable nested case requirement");
    module.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    terminal_verifier::verify_module(&module, &Default::default(), &Default::default())
        .expect("caller-supplied refinement introduces no tag read");
}

#[test]
fn contract_fields_validate_exact_paths_and_carriers_under_every_access() {
    for clause in [Clause::Requires, Clause::Ensures] {
        for access in [
            StructuralAccess::Owned,
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            let mut original = module();
            original.machines[0].structural_parameters[0].access = access;
            let field = integer_field(&original);
            install(
                &mut original,
                clause,
                Proposition::Equal(field.clone(), field),
            );
            validate_module(&original).expect("valid unused field clause");
            for mutation in 0..3 {
                let mut invalid = original.clone();
                match mutation {
                    0 => field_mut(&mut invalid).id = id::<StructuralFieldId>(2),
                    1 => {
                        field_mut(&mut invalid).field_type =
                            StructuralFieldType::Scalar(ScalarType::Boolean)
                    }
                    2 => {
                        field_mut(&mut invalid).field_type =
                            StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                            ))
                    }
                    _ => unreachable!(),
                }
                assert!(
                    matches!(
                        validate_module(&invalid),
                        Err(ModuleError::InvalidIntegerFieldTerm { .. })
                    ),
                    "{clause:?}, {access:?}, mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn integer_fields_are_checked_beneath_arithmetic_and_boolean_connectives() {
    for clause in [Clause::Requires, Clause::Ensures] {
        let mut original = module();
        let field = integer_field(&original);
        let ScalarType::Integer(carrier) = integer_type() else {
            panic!("integer");
        };
        let zero = ScalarTerm::integer(carrier, IntegerValue::Signed(0)).unwrap();
        let sum = ScalarTerm::WrappingIntegerAdd {
            scalar_type: carrier,
            left: Box::new(field),
            right: Box::new(zero),
        };
        let mut conjuncts = vec![
            Proposition::Equal(sum.clone(), sum.clone()),
            Proposition::LessOrEqual(sum.clone(), sum.clone()),
        ];
        conjuncts.sort();
        let mut disjuncts = vec![
            Proposition::Conjunction(conjuncts),
            Proposition::LessThan(sum.clone(), sum),
        ];
        disjuncts.sort();
        let proposition = Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(Proposition::Disjunction(disjuncts)),
        };
        install(&mut original, clause, proposition);
        validate_module(&original).expect("nested valid contract field");
        field_mut(&mut original).id = id::<StructuralFieldId>(2);
        assert!(matches!(
            validate_module(&original),
            Err(ModuleError::InvalidIntegerFieldTerm { .. })
        ));
    }
}

#[test]
fn contract_field_paths_reject_invalid_intermediate_steps_and_array_bounds() {
    for clause in [Clause::Requires, Clause::Ensures] {
        let mut original = module();
        let StructuralTypeShape::Record { fields } = &mut original.structural_types[0].shape else {
            panic!("root record");
        };
        fields[0].field_type = StructuralFieldType::Structural(id::<StructuralTypeId>(3));
        original.structural_types.push(StructuralTypeDeclaration {
            id: id::<StructuralTypeId>(3),
            identity: "Items".into(),
            shape: StructuralTypeShape::FixedArray {
                element: id::<StructuralTypeId>(2),
                length: 3,
            },
        });
        for index in [0, 2, 3, u64::MAX] {
            let mut candidate = original.clone();
            let mut field = integer_field(&candidate);
            let ScalarTerm::IntegerField { path, .. } = &mut field else {
                panic!("field");
            };
            path.insert(1, CanonicalStructuralPathSegment::FixedIndex(index));
            install(
                &mut candidate,
                clause,
                Proposition::Equal(field.clone(), field),
            );
            let result = validate_module(&candidate);
            if index < 3 {
                result.expect("in-range array field");
            } else {
                assert!(matches!(
                    result,
                    Err(ModuleError::InvalidIntegerFieldTerm { .. })
                ));
            }
        }
        let field = integer_field(&original);
        install(
            &mut original,
            clause,
            Proposition::Equal(field.clone(), field),
        );
        assert!(
            matches!(
                validate_module(&original),
                Err(ModuleError::InvalidIntegerFieldTerm { .. })
            ),
            "missing array index"
        );
    }
}

#[test]
fn boolean_ieee_and_byte_contract_fields_retain_their_own_leaf_kind() {
    for clause in [Clause::Requires, Clause::Ensures] {
        for kind in 0..3 {
            let mut original = module();
            let root = original.machines[0].structural_parameters[0].place;
            let proposition = match kind {
                0 => {
                    field_mut(&mut original).field_type =
                        StructuralFieldType::Scalar(ScalarType::Boolean);
                    let field = ScalarTerm::boolean_field_path(root, path());
                    Proposition::Equal(field.clone(), field)
                }
                1 => {
                    field_mut(&mut original).field_type =
                        StructuralFieldType::IeeeFloat(IeeeFloatFormat::Binary64);
                    let field = IeeeFloatStructuralField::new(root, path()).unwrap();
                    Proposition::IeeeFloatComparison {
                        format: IeeeFloatFormat::Binary64,
                        kind: IeeeFloatComparisonKind::Equal,
                        left: field.clone(),
                        right: field,
                    }
                }
                2 => {
                    field_mut(&mut original).field_type = StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    );
                    let field = ByteSequenceStructuralField::new(root, path()).unwrap();
                    Proposition::ByteSequenceEqual {
                        left: field.clone(),
                        right: field,
                    }
                }
                _ => unreachable!(),
            };
            install(&mut original, clause, proposition);
            validate_module(&original).expect("exact leaf kind");
            for access in [
                StructuralAccess::SharedBorrow,
                StructuralAccess::WriteOnlyBorrow,
            ] {
                let mut invalid = original.clone();
                invalid.machines[0].structural_parameters[0].access = access;
                validate_module(&invalid).expect("supplied contract is not an observation");
                field_mut(&mut invalid).field_type = StructuralFieldType::Scalar(integer_type());
                let error = validate_module(&invalid).expect_err("wrong leaf type");
                assert!(matches!(
                    (kind, error),
                    (0, ModuleError::InvalidBooleanFieldTerm { .. })
                        | (1, ModuleError::InvalidIeeeFloatFieldTerm { .. })
                        | (2, ModuleError::InvalidByteSequenceFieldTerm { .. })
                ));
            }
        }
    }
}
