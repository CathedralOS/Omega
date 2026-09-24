//! Function structural-root uniqueness and operation replay tests.
use crate::tests::fixtures::dominance::{
    OperationResultCfgShape, operation_result_cfg_unit, refresh_function_derivatives,
};
use crate::tests::fixtures::structural_catalog::{
    boolean_structural_field_unit, content_entry_claim,
    direct_realization_boolean_structural_field_unit,
    direct_realization_integer_structural_field_unit, install_content_owner, structural_domain,
    structural_result_call_unit, structural_scalar_field_store_unit,
};
use crate::tests::support::{id, refresh_identity, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    ClaimId, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralDomainId, StructuralPlaceKind, StructuralTypeId, ValueId,
};

#[test]
fn projected_case_membership_replays_exact_path_case_access_and_identity() {
    use terminal_psi::{StructuralPathSegment as Path, StructuralTypeShape as Shape};
    let mut baseline = direct_realization_boolean_structural_field_unit();
    let array = id(4_800, StructuralTypeId::new);
    let sum = id(4_801, StructuralTypeId::new);
    let case = id(4_802, semantic_vocabulary::StructuralCaseId::new);
    let Shape::Record { fields } = &mut baseline.structural_types.make_mut()[0].shape else {
        unreachable!()
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Structural(array);
    baseline.structural_types.make_mut().extend([
        terminal_psi::StructuralTypeDeclaration {
            id: array,
            identity: "validation::colors".into(),
            shape: Shape::FixedArray {
                element: sum,
                length: 2,
            },
        },
        terminal_psi::StructuralTypeDeclaration {
            id: sum,
            identity: "validation::Color".into(),
            shape: Shape::Sum {
                cases: vec![terminal_psi::StructuralCaseDeclaration {
                    id: case,
                    identity: "validation::Color::Blue".into(),
                    fields: Vec::new(),
                }],
            },
        },
    ]);
    let O::BooleanStructuralField {
        psi_operation,
        result,
        source,
        ..
    } = baseline.functions[0].blocks[0].nodes[0].operation.clone()
    else {
        unreachable!()
    };
    baseline.functions[0].blocks[0].nodes[0].operation = O::StructuralCaseMembership {
        psi_operation,
        result: abstract_operations::AbstractResult {
            value: result,
            scalar_type: ScalarType::Boolean,
        },
        source,
        path: vec![Path::Field("ready".into()), Path::FixedIndex(1)],
        case,
    };
    refresh_function_derivatives(&mut baseline, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("nested fixed-array case observation validates");

    let mut other_element = baseline.clone();
    let O::StructuralCaseMembership { path, .. } =
        &mut other_element.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    path[1] = Path::FixedIndex(0);
    refresh_function_derivatives(&mut other_element, 0);
    validate_psi_optimization_unit(&other_element)
        .expect("the other in-bounds element also validates");
    assert_ne!(
        baseline.identity, other_element.identity,
        "projection participates in identity"
    );

    for corruption in 0..5 {
        let mut candidate = baseline.clone();
        let O::StructuralCaseMembership {
            path,
            case: selected_case,
            ..
        } = &mut candidate.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        match corruption {
            0 => path[1] = Path::FixedIndex(2),
            1 => path[0] = Path::Field("missing".into()),
            2 => *selected_case = id(4_803, semantic_vocabulary::StructuralCaseId::new),
            3 => {
                path.pop();
            }
            4 => {
                candidate.functions[0].structural_parameters[0].access =
                    terminal_psi::StructuralAccess::WriteOnlyBorrow
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut candidate, 0);
        assert!(
            validate_psi_optimization_unit(&candidate).is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn logical_structural_roots_are_unique_beyond_place_identity() {
    let mut duplicate = structural_result_call_unit();
    let first_call = duplicate.functions[0].blocks[0].nodes[0].clone();
    let (psi_operation, result_type) = match &first_call.operation {
        O::CallStructural {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.structural_type),
        _ => panic!("fixture begins with one structural call"),
    };
    let duplicate_place = id(4_712, PlaceId::new);
    let mut duplicate_call = first_call;
    let O::CallStructural {
        result: duplicate_result,
        ..
    } = &mut duplicate_call.operation
    else {
        unreachable!()
    };
    duplicate_result.place = duplicate_place;
    duplicate.functions[0].blocks[0]
        .nodes
        .insert(1, duplicate_call);
    duplicate.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: duplicate_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: psi_operation,
                structural_type: result_type,
            },
        });
    refresh_function_derivatives(&mut duplicate, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate),
        Err(
            OptimizationUnitValidationError::DuplicateStructuralPlaceRoot {
                machine: _,
                kind: StructuralPlaceKind::OperationResult { .. },
            }
        )
    ));
}

#[test]
fn boolean_structural_field_replays_exact_readable_root_and_field() {
    let baseline = boolean_structural_field_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("exact affine readable Boolean observation validates");
    let invalid = |mut candidate: PsiOptimizationUnit| {
        refresh_identity(&mut candidate);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
        ));
    };

    let mut write_only = baseline.clone();
    write_only.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    invalid(write_only);

    let mut qualified = baseline.clone();
    let domain = id(1, StructuralDomainId::new);
    qualified.structural_domains =
        vec![structural_domain(1, 1, qualified.structural_types[0].id)].into();
    qualified.functions[0].structural_parameters[0]
        .qualifications
        .push(domain);
    invalid(qualified);

    let mut claimed = baseline.clone();
    let claim = id(1, ClaimId::new);
    let source = claimed.functions[0].structural_parameters[0].place;
    claimed.functions[0]
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim,
            input: source,
            path: Vec::new(),
        });
    claimed.functions[0].entry_claims.insert(claim);
    invalid(claimed);

    let mut content_claimed = baseline.clone();
    install_content_owner(&mut content_claimed);
    content_claimed.functions[0]
        .content_entry_claims
        .push(content_entry_claim(claim, source));
    invalid(content_claimed);

    let mut missing_cleanup = baseline.clone();
    let O::Return {
        cleanup_actions, ..
    } = &mut missing_cleanup.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture ends in a scalar return")
    };
    cleanup_actions.clear();
    refresh_function_derivatives(&mut missing_cleanup, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_cleanup),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));

    let mut wrong_field = baseline.clone();
    let O::BooleanStructuralField { field, .. } =
        &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with its observation")
    };
    *field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    invalid(wrong_field);

    let mut non_boolean_field = baseline.clone();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut non_boolean_field.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    invalid(non_boolean_field);

    let mut differing_observation = baseline;
    let mut second = differing_observation.functions[0].blocks[0].nodes[0].clone();
    let second_field = id(4_713, semantic_vocabulary::StructuralFieldId::new);
    let O::BooleanStructuralField {
        psi_operation,
        result,
        field,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(4_714, OperationId::new);
    *result = id(4_715, ValueId::new);
    *field = second_field;
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut differing_observation.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    fields.push(terminal_psi::StructuralFieldDeclaration {
        id: second_field,
        identity: "validation::other-ready".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    differing_observation.functions[0].blocks[0]
        .nodes
        .insert(1, second);
    refresh_function_derivatives(&mut differing_observation, 0);
    validate_psi_optimization_unit(&differing_observation)
        .expect("each exact Boolean field is independently readable on an owned record");
}

#[test]
fn owned_boolean_fields_compose_without_entry_or_scalar_parameter_restrictions() {
    for multiplicity in [
        terminal_psi::StructuralMultiplicity::Affine,
        terminal_psi::StructuralMultiplicity::Unrestricted,
    ] {
        let mut candidate = boolean_structural_field_unit();
        candidate.entry = candidate.functions[1].machine;
        let function = &mut candidate.functions[0];
        function.parameters.clear();
        function.structural_parameters[0].multiplicity = multiplicity;
        let O::Return {
            cleanup_actions, ..
        } = &mut function.blocks[0].nodes[1].operation
        else {
            panic!("fixture ends in a scalar return");
        };
        cleanup_actions.clear();
        if multiplicity == terminal_psi::StructuralMultiplicity::Affine {
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                function.structural_parameters[0].place,
            ));
        }
        refresh_function_derivatives(&mut candidate, 0);
        validate_psi_optimization_unit(&candidate)
            .expect("owned Boolean reads use exact roots and ordinary cleanup");
    }
}

#[test]
fn owned_boolean_field_read_rejects_observation_after_transfer() {
    let mut candidate = boolean_structural_field_unit();
    let parameter = candidate.functions[0].structural_parameters[0].clone();
    let source = parameter.place;
    let callee = candidate.functions[1].machine;
    let mut call = candidate.functions[0].blocks[0].nodes[0].clone();
    call.operation = O::CallUnit {
        psi_operation: id(4_720, OperationId::new),
        callee,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: source,
            access: terminal_psi::StructuralAccess::Owned,
            path: Vec::new(),
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    let O::Return {
        cleanup_actions, ..
    } = &mut candidate.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("fixture ends in a scalar return");
    };
    cleanup_actions.clear();
    candidate.functions[0].blocks[0].nodes.insert(1, call);

    let consumer = &mut candidate.functions[1];
    consumer.attachment = None;
    consumer.declared_places.insert(source);
    consumer
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        });
    consumer.structural_parameters.push(parameter);
    let O::ReturnUnit {
        cleanup_actions, ..
    } = &mut consumer.blocks[0].nodes[0].operation
    else {
        panic!("consumer ends in a Unit return");
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
        source,
    ));
    refresh_function_derivatives(&mut candidate, 0);
    refresh_function_derivatives(&mut candidate, 1);
    validate_psi_optimization_unit(&candidate)
        .expect("a Boolean observation before owned transfer remains available afterward");

    candidate.functions[0].blocks[0].nodes.swap(0, 1);
    refresh_function_derivatives(&mut candidate, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::CurrentOwnedPlaceNotLive { place, .. })
            if place == source
    ));
}

fn assert_invalid_direct_realization_observation(mut candidate: PsiOptimizationUnit) {
    refresh_identity(&mut candidate);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidBooleanStructuralField { .. })
    ));
}

#[test]
fn unrestricted_shared_boolean_structural_field_direct_realization_validates() {
    validate_psi_optimization_unit(&direct_realization_boolean_structural_field_unit())
        .expect("an unqualified unrestricted shared direct realization validates");
}

#[test]
fn shared_boolean_observations_validate_each_declared_field_independently() {
    let mut candidate = direct_realization_boolean_structural_field_unit();
    let mut second = candidate.functions[0].blocks[0].nodes[0].clone();
    let O::BooleanStructuralField {
        psi_operation,
        result,
        field,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(4_714, OperationId::new);
    *result = id(4_715, ValueId::new);
    *field = id(4_713, semantic_vocabulary::StructuralFieldId::new);
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut candidate.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    fields.push(terminal_psi::StructuralFieldDeclaration {
        id: *field,
        identity: "validation::second-boolean".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    candidate.functions[0].blocks[0].nodes.insert(1, second);
    refresh_function_derivatives(&mut candidate, 0);
    validate_psi_optimization_unit(&candidate)
        .expect("independent exact Boolean fields on one shared record");
}

#[test]
fn unrestricted_shared_integer_structural_field_direct_realization_validates() {
    validate_psi_optimization_unit(&direct_realization_integer_structural_field_unit())
        .expect("an unqualified unrestricted shared integer field read validates");
}

#[test]
fn bounded_integer_field_reads_require_the_exact_declared_carrier_and_path() {
    use semantic_vocabulary::BoundedIntegerType;
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    let mut baseline = direct_realization_integer_structural_field_unit();
    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let StructuralTypeShape::Record { fields } = &mut baseline.structural_types.make_mut()[0].shape
    else {
        panic!("integer observation carrier is a record");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(integer, IntegerValue::Signed(-7), IntegerValue::Signed(17))
            .expect("bounded i32"),
    );
    refresh_function_derivatives(&mut baseline, 0);
    validate_psi_optimization_unit(&baseline)
        .expect("a bounded field read retains its declared integer carrier");

    for (sign, bits, minimum, maximum) in [
        (
            IntegerSign::Unsigned,
            32,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(17),
        ),
        (
            IntegerSign::Signed,
            64,
            IntegerValue::Signed(-7),
            IntegerValue::Signed(17),
        ),
    ] {
        let mut candidate = baseline.clone();
        let StructuralTypeShape::Record { fields } =
            &mut candidate.structural_types.make_mut()[0].shape
        else {
            unreachable!()
        };
        fields[0].field_type = StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(sign, bits).expect("integer carrier"),
                minimum,
                maximum,
            )
            .expect("valid declared interval"),
        );
        refresh_function_derivatives(&mut candidate, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&candidate),
            Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
        ));
    }

    let mut wrong_path = baseline;
    let O::IntegerStructuralField { path, field, .. } =
        &mut wrong_path.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    // A scalar leaf cannot be reused as a record carrier, even with the same ID.
    path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
        *field,
    ));
    refresh_function_derivatives(&mut wrong_path, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_path),
        Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
    ));
}

#[test]
fn bounded_integer_field_store_rejects_an_exact_carrier_without_range_authority() {
    use semantic_vocabulary::BoundedIntegerType;
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    let mut candidate = structural_scalar_field_store_unit();
    validate_psi_optimization_unit(&candidate).expect("unbounded projected store validates");
    let O::StructuralScalarFieldStore { field, value, .. } =
        &candidate.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    let selected_field = *field;
    let ScalarType::Integer(integer) = value.scalar_type else {
        panic!("integer store fixture");
    };
    let declaration = candidate
        .structural_types
        .make_mut()
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Record { fields } => {
                fields.iter_mut().find(|field| field.id == selected_field)
            }
            _ => None,
        })
        .expect("exact projected scalar field");
    declaration.field_type = StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(integer, IntegerValue::Signed(-7), IntegerValue::Signed(17))
            .expect("interval contains the fixture's stored value"),
    );
    refresh_function_derivatives(&mut candidate, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&candidate),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));
}

fn bounded_scalar_store_authority_unit() -> PsiOptimizationUnit {
    use semantic_vocabulary::{BoundedIntegerType, ObligationId, Proposition, ScalarTerm};
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    let mut unit = structural_scalar_field_store_unit();
    let O::StructuralScalarFieldStore {
        psi_operation,
        destination,
        field,
        value,
        range_obligation,
        ..
    } = &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("projected store fixture");
    };
    let obligation = id(4_730, ObligationId::new);
    *range_obligation = Some(obligation);
    let operation = *psi_operation;
    let owner_type = destination.structural_type;
    let selected_field = *field;
    let stored_value = *value;
    let ScalarType::Integer(integer) = stored_value.scalar_type else {
        panic!("integer store");
    };
    let bounds = |maximum| {
        BoundedIntegerType::new(
            integer,
            IntegerValue::Signed(-7),
            IntegerValue::Signed(maximum),
        )
        .unwrap()
    };
    let declarations = unit.structural_types.make_mut();
    let child = declarations.iter_mut().find(|declaration| matches!(
        &declaration.shape,
        StructuralTypeShape::Record { fields } if fields.iter().any(|field| field.id == selected_field)
    )).expect("selected field's record");
    let StructuralTypeShape::Record { fields } = &mut child.shape else {
        panic!("child record");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(bounds(17));
    let mut other_field = fields[0].clone();
    other_field.id = id(4_731, semantic_vocabulary::StructuralFieldId::new);
    other_field.identity = "narrower".into();
    other_field.field_type = StructuralFieldType::BoundedInteger(bounds(16));
    fields.push(other_field);
    let mut other_child = child.clone();
    other_child.id = id(4_732, StructuralTypeId::new);
    other_child.identity = "validation::narrower-store-child".into();
    let StructuralTypeShape::Record { fields } = &mut other_child.shape else {
        panic!("other child");
    };
    fields[0].field_type = StructuralFieldType::BoundedInteger(bounds(16));
    let owner = declarations
        .iter_mut()
        .find(|declaration| declaration.id == owner_type)
        .unwrap();
    let StructuralTypeShape::Record { fields } = &mut owner.shape else {
        panic!("owner");
    };
    let mut other_path = fields[0].clone();
    other_path.id = id(4_733, semantic_vocabulary::StructuralFieldId::new);
    other_path.identity = "other".into();
    other_path.field_type = StructuralFieldType::Structural(other_child.id);
    fields.push(other_path);
    declarations.push(other_child);
    declarations.sort_by_key(|declaration| declaration.id);

    // The replacement RHS is available and has the correct width but not the
    // destination's range. Failure must therefore check current proof custody.
    let mut other_value = unit.functions[0].blocks[0].nodes[0].clone();
    other_value.operation = O::IntegerConstant {
        psi_operation: id(4_734, OperationId::new),
        result: id(4_735, ValueId::new),
        scalar_type: stored_value.scalar_type,
        value: IntegerValue::Signed(18),
    };
    unit.functions[0].blocks[0].nodes.insert(1, other_value);
    refresh_function_derivatives(&mut unit, 0);

    let value = ScalarTerm::value(stored_value.value, stored_value.scalar_type);
    let endpoint = |value| ScalarTerm::Integer {
        scalar_type: integer,
        value: IntegerValue::Signed(value),
    };
    let mut clauses = vec![
        Proposition::LessOrEqual(endpoint(-7), value.clone()),
        Proposition::LessOrEqual(value, endpoint(17)),
    ];
    clauses.sort();
    let proposition =
        terminal_codec::canonical_proposition_order_key(&Proposition::Conjunction(clauses))
            .unwrap();
    let fact = optimization_unit::AcceptedObligationFact::new(
        unit.psi,
        [42; 32],
        unit.functions[0].machine,
        operation,
        obligation,
        proposition,
    );
    optimization_unit::attach_accepted_obligation_facts(unit, vec![fact]).unwrap()
}

#[test]
fn bounded_integer_field_store_rechecks_current_value_declaration_and_obligation() {
    use semantic_vocabulary::{BoundedIntegerType, ObligationId};
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};

    let original = bounded_scalar_store_authority_unit();
    validate_psi_optimization_unit(&original).expect("exact bounded store authority");
    for corruption in [
        "rhs",
        "bound",
        "path",
        "field",
        "missing obligation",
        "wrong obligation",
    ] {
        let mut changed = original.clone();
        let O::StructuralScalarFieldStore {
            path,
            field,
            value,
            range_obligation,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[2].operation
        else {
            panic!("store");
        };
        match corruption {
            "rhs" => value.value = id(4_735, ValueId::new),
            "path" => path[0] = terminal_psi::StructuralPathSegment::Field("other".into()),
            "field" => *field = id(4_731, semantic_vocabulary::StructuralFieldId::new),
            "missing obligation" => *range_obligation = None,
            "wrong obligation" => *range_obligation = Some(id(4_736, ObligationId::new)),
            "bound" => {
                let selected_field = *field;
                let declaration = changed
                    .structural_types
                    .make_mut()
                    .iter_mut()
                    .find_map(|declaration| match &mut declaration.shape {
                        StructuralTypeShape::Record { fields } => {
                            fields.iter_mut().find(|field| field.id == selected_field)
                        }
                        _ => None,
                    })
                    .unwrap();
                let StructuralFieldType::BoundedInteger(bounds) = declaration.field_type else {
                    panic!("bound");
                };
                declaration.field_type = StructuralFieldType::BoundedInteger(
                    BoundedIntegerType::new(
                        bounds.integer_type(),
                        bounds.minimum(),
                        IntegerValue::Signed(16),
                    )
                    .unwrap(),
                );
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        if corruption == "missing obligation" {
            assert!(result.is_err(), "{corruption}");
        } else {
            assert!(
                matches!(
                    result,
                    Err(OptimizationUnitValidationError::AcceptedObligationFactIndexMismatch)
                ),
                "{corruption}: {result:?}"
            );
        }
    }
}

#[test]
fn bounded_integer_field_store_rejects_missing_and_forged_accepted_facts() {
    use semantic_vocabulary::ObligationId;
    let original = bounded_scalar_store_authority_unit();
    validate_psi_optimization_unit(&original).expect("exact bounded store authority");
    for corruption in [
        "absent",
        "operation",
        "obligation",
        "proposition",
        "identity",
    ] {
        let mut changed = original.clone();
        if corruption == "absent" {
            changed.accepted_obligation_facts.clear();
        } else {
            let fact = &mut changed.accepted_obligation_facts[0];
            let proposition = if corruption == "proposition" {
                terminal_codec::canonical_proposition_order_key(
                    &semantic_vocabulary::Proposition::Truth,
                )
                .unwrap()
            } else {
                fact.proposition.clone()
            };
            *fact = optimization_unit::AcceptedObligationFact::new(
                fact.psi,
                fact.proof_bundle_fingerprint,
                fact.machine,
                if corruption == "operation" {
                    id(4_737, OperationId::new)
                } else {
                    fact.operation
                },
                if corruption == "obligation" {
                    id(4_736, ObligationId::new)
                } else {
                    fact.obligation
                },
                proposition,
            );
            if corruption == "identity" {
                fact.proof_bundle_fingerprint[0] ^= 1;
            }
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn shared_integer_observations_validate_each_declared_field_independently() {
    let mut candidate = direct_realization_integer_structural_field_unit();
    let mut second = candidate.functions[0].blocks[0].nodes[0].clone();
    let O::IntegerStructuralField {
        psi_operation,
        result,
        field,
        ..
    } = &mut second.operation
    else {
        unreachable!()
    };
    *psi_operation = id(4_714, OperationId::new);
    result.value = id(4_715, ValueId::new);
    *field = id(4_713, semantic_vocabulary::StructuralFieldId::new);
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut candidate.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    fields.push(terminal_psi::StructuralFieldDeclaration {
        id: *field,
        identity: "validation::second-integer".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: terminal_psi::StructuralFieldType::Scalar(result.scalar_type),
    });
    candidate.functions[0].blocks[0].nodes.insert(1, second);
    refresh_function_derivatives(&mut candidate, 0);
    validate_psi_optimization_unit(&candidate)
        .expect("independent exact fields on one shared record");
}

#[test]
fn direct_integer_structural_field_rejects_access_type_and_field_corruption() {
    let mut access = direct_realization_integer_structural_field_unit();
    access.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    refresh_identity(&mut access);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
    ));

    let mut result_type = direct_realization_integer_structural_field_unit();
    let O::IntegerStructuralField { result, .. } =
        &mut result_type.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Boolean;
    refresh_node_derivatives(&mut result_type, 0, 0, 0);
    refresh_identity(&mut result_type);
    assert!(validate_psi_optimization_unit(&result_type).is_err());

    let mut field = direct_realization_integer_structural_field_unit();
    let O::IntegerStructuralField {
        field: selected_field,
        ..
    } = &mut field.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    *selected_field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut field, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&field),
        Err(OptimizationUnitValidationError::InvalidIntegerStructuralField { .. })
    ));
}

#[test]
fn projected_structural_scalar_field_store_validates_and_rejects_corruption() {
    validate_psi_optimization_unit(&structural_scalar_field_store_unit())
        .expect("an exact mutable projected scalar store validates");

    let mut affine = structural_scalar_field_store_unit();
    affine.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    let O::StructuralScalarFieldStore { destination, .. } =
        &mut affine.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    destination.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut affine, 0, 0, 1);
    refresh_identity(&mut affine);
    validate_psi_optimization_unit(&affine)
        .expect("an exact affine mutable loan retains the same store authority");

    let mut access = structural_scalar_field_store_unit();
    access.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::SharedBorrow;
    let O::StructuralScalarFieldStore { destination, .. } =
        &mut access.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    destination.access = terminal_psi::StructuralAccess::SharedBorrow;
    refresh_node_derivatives(&mut access, 0, 0, 1);
    refresh_identity(&mut access);
    assert!(matches!(
        validate_psi_optimization_unit(&access),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));

    let mut path = structural_scalar_field_store_unit();
    let O::StructuralScalarFieldStore { path: selected, .. } =
        &mut path.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    selected.clear();
    refresh_node_derivatives(&mut path, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&path),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));

    let mut field = structural_scalar_field_store_unit();
    let O::StructuralScalarFieldStore {
        field: selected, ..
    } = &mut field.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *selected = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut field, 0, 0, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&field),
        Err(OptimizationUnitValidationError::InvalidStructuralScalarFieldStore { .. })
    ));
}

#[test]
fn direct_realization_boolean_structural_field_requires_readable_access() {
    let mut readable_but_exclusive = direct_realization_boolean_structural_field_unit();
    readable_but_exclusive.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::MutableBorrow;
    refresh_identity(&mut readable_but_exclusive);
    validate_psi_optimization_unit(&readable_but_exclusive)
        .expect("a mutable reference permits an exact Boolean observation");

    let mut write_only = direct_realization_boolean_structural_field_unit();
    write_only.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    assert_invalid_direct_realization_observation(write_only);
}

#[test]
fn direct_realization_boolean_structural_field_rejects_linear_observation() {
    let mut linear = direct_realization_boolean_structural_field_unit();
    linear.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Linear;
    // A linear parameter without its required entry claim fails the root
    // catalog before the Boolean field-operation contract is checked.
    refresh_identity(&mut linear);
    let result = validate_psi_optimization_unit(&linear);
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::StructuralCatalogMismatch {
                machine: Some(machine),
            }) if machine == linear.functions[0].machine
        ),
        "unclaimed linear parameter must fail its exact root catalog: {result:?}"
    );
}

#[test]
fn direct_realization_boolean_structural_field_rejects_type_corruption() {
    let mut non_boolean = direct_realization_boolean_structural_field_unit();
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut non_boolean.structural_types.make_mut()[0].shape
    else {
        panic!("direct realization carrier is a record")
    };
    fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
    ));
    assert_invalid_direct_realization_observation(non_boolean);
}

#[test]
fn direct_realization_boolean_structural_field_rejects_field_corruption() {
    let mut wrong_field = direct_realization_boolean_structural_field_unit();
    let O::BooleanStructuralField { field, .. } =
        &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("direct realization begins with a Boolean field observation")
    };
    *field = id(4_799, semantic_vocabulary::StructuralFieldId::new);
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    assert_invalid_direct_realization_observation(wrong_field);
}

#[test]
fn structural_returns_reject_non_source_roots_and_signature_drift() {
    let mut result_root = structural_result_call_unit();
    let result_place = result_root.functions[1]
        .result
        .structural()
        .expect("structural result")
        .place;
    let return_node = result_root.functions[1].blocks[0].nodes.len() - 1;
    let O::ReturnStructural { source, .. } =
        &mut result_root.functions[1].blocks[0].nodes[return_node].operation
    else {
        panic!("fixture returns structurally")
    };
    *source = result_place;
    refresh_node_derivatives(&mut result_root, 1, 0, return_node);
    assert!(matches!(
        validate_psi_optimization_unit(&result_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut literal_root = structural_result_call_unit();
    let literal_type = terminal_psi::StructuralTypeDeclaration {
        id: id(4_716, StructuralTypeId::new),
        identity: "validation::return-source-literal".into(),
        shape: terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView,
        ),
    };
    let literal = terminal_psi::StructuralPlaceDeclaration {
        id: id(4_717, PlaceId::new),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type: literal_type.id,
        },
    };
    literal_root
        .structural_types
        .make_mut()
        .push(literal_type.clone());
    literal_root.functions[1].structural_places.push(literal);
    let establishment_node = literal_root.functions[1].blocks[0].nodes[0].clone();
    literal_root.functions[1].blocks[0]
        .nodes
        .insert(0, establishment_node);
    literal_root.functions[1].blocks[0].nodes[0].operation = O::EstablishByteSequenceLiteral {
        psi_operation: id(4_718, OperationId::new),
        place: literal,
        structural_type: literal_type,
        bytes: b"return-source".to_vec(),
        qualifications: Vec::new(),
    };
    let O::ReturnStructural { source, .. } =
        &mut literal_root.functions[1].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *source = literal.id;
    refresh_function_derivatives(&mut literal_root, 1);
    assert!(matches!(
        validate_psi_optimization_unit(&literal_root),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));

    let mut wrong_signature =
        operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    let O::CallStructural { result, .. } =
        &mut wrong_signature.functions[0].blocks[3].nodes[0].operation
    else {
        panic!("non-topological fixture stores its call in the entry block")
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_signature, 0, 3, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_signature),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}
