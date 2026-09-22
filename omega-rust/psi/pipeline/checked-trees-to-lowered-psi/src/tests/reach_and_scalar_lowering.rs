use super::{assert_source_direct_float_result, checked_float_projection_source};
use crate::TerminalMachineSelection;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use crate::proofs::crash_routes::checked_boolean_proposition;
use crate::retention::conformance_applications::lower_closed_conformance_applications;
use crate::scalar_graph::shared_runtime_parameters::normalize_shared_boolean_comparison_leaves;
use crate::terminal_identities::{operation_id, service_id, value_id};
use crate::unit::attached_unit::lower_root_service_reach;
use checked_trees::CheckedBooleanExpression;
use numerics::arithmetic::ArithmeticDomain;
use numerics::integer_policy::IntegerPolicyPrimitive;
use semantic_vocabulary::{IeeeFloatFormat, Proposition, PropositionContext, ScalarType};
use terminal_psi::{
    BoundaryMachineResult, OperationKind, OperationResult, StructuralMultiplicity,
    StructuralTypeShape, ValueDeclaration,
};
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn top_level_bounded_reach_lowers_normalized_machine_identity() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary machine InterruptAcknowledgement::complete()
        reaches <= MachineControl + PortIo;
    "#;
    let typed = crate::front_end::typed_program(source);
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("top-level completion requirement");
    let expected_identity = typed
        .normalized_machine_overload_identity(requirement)
        .expect("normalized top-level requirement")
        .identity();
    let requirement_symbol = requirement.symbol;
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let service_ids = ["MachineControl", "PortIo"]
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                checked
                    .facts
                    .service_reaches
                    .services
                    .id_for_name(name)
                    .expect("service exists"),
                service_id(u64::try_from(index).expect("service index") + 1),
            )
        })
        .collect::<Vec<_>>();
    let closure = lower_root_service_reach(&checked, requirement_symbol, &service_ids)
        .expect("lower top-level requirement reach");
    assert!(closure.concrete.is_empty());
    let [dependency] = closure.installation_dependencies.as_slice() else {
        panic!("top-level requirement must retain one installation dependency");
    };
    assert_eq!(dependency.requirement_identity, expected_identity);
    let bound_names = dependency
        .upper_bound
        .iter()
        .map(|id| {
            service_ids
                .iter()
                .find(|(_, terminal)| terminal == id)
                .and_then(|(source, _)| checked.facts.service_reaches.services.definition(*source))
                .expect("bound service is declared")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(bound_names, ["MachineControl", "PortIo"]);
}

#[test]
fn actual_float_meaning_calls_emit_deduplicated_source_free_module_rows() {
    let source = r#"
        machine prove_projection(value32: f32, value64: f64)
        requires
            Float::meaning32(value32) == Float::meaning32(value32);
            Float::meaning64(value64) == Float::meaning64(value64);
        { }

        machine terminal_root(value: bool) -> bool
        requires
            true == true;
        ensures
            true == true;
        { value }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered =
        lower_machine(&checked, TerminalMachineSelection::Name("terminal_root")).expect("lower");
    let projections = &lowered.semantic_module.float_meaning_projections;
    assert_eq!(projections.len(), 2);
    assert_eq!(projections[0].result.id, terminal_psi::ProofValueId(0));
    assert_eq!(
        projections[0].source,
        terminal_psi::FloatMeaningSource::TransitionalInput(terminal_psi::FloatProjectionInput {
            id: terminal_psi::FloatProjectionInputId(0),
            format: semantic_vocabulary::IeeeFloatFormat::Binary32,
        })
    );
    assert_eq!(
        projections[0].operation,
        terminal_psi::FloatMeaningProjectionOperation::Meaning32
    );
    assert_eq!(
        projections[1].operation,
        terminal_psi::FloatMeaningProjectionOperation::Meaning64
    );
    assert_eq!(
        projections[1].source,
        terminal_psi::FloatMeaningSource::TransitionalInput(terminal_psi::FloatProjectionInput {
            id: terminal_psi::FloatProjectionInputId(1),
            format: semantic_vocabulary::IeeeFloatFormat::Binary64,
        })
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![
            terminal_psi::FloatMeaningEqualityProposition {
                id: terminal_psi::ProofPropositionId(0),
                left: terminal_psi::ProofValueId(0),
                right: terminal_psi::ProofValueId(0),
            },
            terminal_psi::FloatMeaningEqualityProposition {
                id: terminal_psi::ProofPropositionId(1),
                left: terminal_psi::ProofValueId(1),
                right: terminal_psi::ProofValueId(1),
            },
        ]
    );
    terminal_verifier::validate_module(&lowered.semantic_module).expect("verify");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn emitted_direct_float_parameter_rejoins_terminal_owner_and_dense_scalar_parameter() {
    let source = r#"
        data Token { value: i32; }
        data Root {}

        machine Root::forward(token: Token, value: f32)
        requires
            Float::meaning32(value) == Float::meaning32(value);
        {
            transition { _ -> done(token, value) }
            state done(token: Token, value: f32) {}
        }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::forward"))
        .expect("lower direct float owner");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = machine.parameters.as_slice() else {
        panic!("the structural source position is excluded from the scalar parameter table")
    };
    assert_eq!(
        parameter.scalar_type,
        ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_projections[0].source,
        terminal_psi::FloatMeaningSource::DirectMachineParameter(
            terminal_psi::DirectMachineFloatParameter {
                owner: machine.id,
                parameter: parameter.id,
                format: IeeeFloatFormat::Binary32,
            }
        )
    );
    terminal_verifier::validate_module(&lowered.semantic_module).expect("verify direct source");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn emitted_direct_structural_float_leaf_rejoins_owner_root_and_member_path() {
    let source = r#"
        data Sample { value: f32; }
        data Root {}

        machine Root::structural_source(sample: Sample)
        requires
            Float::meaning32(sample.value) == Float::meaning32(sample.value);
        {
            transition { _ -> done(sample) }
            state done(sample: Sample) {}
        }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::structural_source"),
    )
    .expect("lower direct structural float owner");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [parameter] = machine.structural_parameters.as_slice() else {
        panic!("one structural parameter expected")
    };
    let [projection] = lowered.semantic_module.float_meaning_projections.as_slice() else {
        panic!("one structural FloatMeaning projection expected")
    };
    let terminal_psi::FloatMeaningSource::DirectStructuralLeaf(leaf) = &projection.source else {
        panic!("structural source should retain an exact Terminal leaf")
    };
    assert_eq!(leaf.owner, machine.id);
    assert_eq!(leaf.field.root(), parameter.place);
    assert_eq!(leaf.field.path().len(), 1);
    assert_eq!(leaf.format, IeeeFloatFormat::Binary32);
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("verify direct structural source");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );

    let mut path_drift = checked;
    let checked_trees::CheckedFloatProjectionSource::DirectStructuralLeaf(leaf) =
        &mut path_drift.facts.proof.float_meaning_projections[0].source
    else {
        panic!("checked structural source expected")
    };
    leaf.field.path[0] =
        checked_trees::CheckedStructuralPredicatePathSegment::Field("missing".to_owned());
    assert!(
        lower_machine(
            &path_drift,
            TerminalMachineSelection::Name("Root::structural_source")
        )
        .is_err()
    );
}

#[test]
fn source_direct_float_results_rejoin_terminal_owner_and_scalar_result() {
    assert_source_direct_float_result("f32", "meaning32", IeeeFloatFormat::Binary32);
    assert_source_direct_float_result("f64", "meaning64", IeeeFloatFormat::Binary64);
}

#[test]
fn direct_float_result_proof_only_contract_rejects_additional_value_clauses() {
    let checked = checked_float_projection_source(
        r#"
            machine result(value: f32) -> f32
            requires
                Float::meaning32(value) == Float::meaning32(value);
            ensures
                Float::meaning32(result) == Float::meaning32(result);
            { value }
        "#,
    );
    // A float-meaning clause is not a closed scalar contract clause: it keeps
    // an explicit `None` requires row, which `covered_requires` rejects before
    // the closed-literal contract shape is ever selected.
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("result")),
        Err(LoweringError::Unsupported(
            "scalar contract contains an unsupported clause"
        ))
    ));
}

#[test]
fn direct_float_result_proof_only_contract_replays_expression_and_owner() {
    let source = r#"
        machine result(value: f32) -> f32
        ensures
            Float::meaning32(result) == Float::meaning32(result);
        { value }

        machine other(value: f32) -> f32 { value }
    "#;
    let checked = checked_float_projection_source(source);

    let mut expression_drift = checked.clone();
    expression_drift.facts.proof.float_meaning_equalities[0].source_expression =
        typed_trees::expression::ExpressionHandle::invalid();
    assert!(lower_machine(&expression_drift, TerminalMachineSelection::Name("result")).is_err());

    let mut owner_drift = checked;
    let other = owner_drift
        .machines()
        .iter()
        .find(|machine| owner_drift.symbols.name(machine.symbol) == "other")
        .expect("other machine")
        .symbol;
    let checked_trees::CheckedFloatProjectionSource::DirectMachineResult(result) =
        &mut owner_drift.facts.proof.float_meaning_projections[0].source
    else {
        panic!("direct result source expected")
    };
    result.owner_machine = other;
    // A drifted checked-table owner never reaches the artifact as a stronger
    // source: emission re-derives the direct-result source from the machine
    // context, the mismatch demotes the row to `TransitionalInput`, and the
    // downgraded module still verifies.
    let drifted = lower_machine(&owner_drift, TerminalMachineSelection::Name("result"))
        .expect("lower drifted owner");
    assert!(
        drifted
            .semantic_module
            .float_meaning_projections
            .iter()
            .all(|projection| matches!(
                projection.source,
                terminal_psi::FloatMeaningSource::TransitionalInput(..)
            )),
        "a drifted owner demotes the row to TransitionalInput, never a forged direct result: {:?}",
        drifted
            .semantic_module
            .float_meaning_projections
            .iter()
            .map(|projection| projection.source.clone())
            .collect::<Vec<_>>()
    );
    terminal_verifier::validate_module(&drifted.semantic_module).expect("verify demoted owner");
}

#[test]
fn authored_float_meaning_equality_ensures_clause_cites_its_checked_row() {
    let source = r#"
        machine read() -> u64
        ensures
            FloatSemantics::add(
                FloatFormat::BINARY32,
                Float::meaning32(1.0f32),
                Float::meaning32(2.0f32)
            ) == Float::meaning32(3.0f32);
        { 7 }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("read")).expect("lower");
    // The authored `==` clause keeps its claim as a vocabulary `Atom` citing
    // the dense checked equality row — never erased toward `Truth`/`Empty` —
    // and the verifier discharges it as a semantic axiom of the module.
    let [clause] = lowered.semantic_module.machines[0]
        .contract
        .ensures
        .as_slice()
    else {
        panic!("the authored ensures clause lowers to one contract clause")
    };
    assert_eq!(
        clause.proposition,
        Proposition::Atom(terminal_psi::float_meaning_equality_proposition_id(0))
    );
    let [equality] = lowered.semantic_module.float_meaning_equalities.as_slice() else {
        panic!("the authored equality lowers to one module equality row")
    };
    assert_eq!(equality.id, terminal_psi::ProofPropositionId(0));
    assert_ne!(equality.left, equality.right);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verify");
    // A forged cite — a clause whose `Atom` identity names no reconstructed
    // equality axiom — stays unprovable: the replayed semantic axiom's
    // conclusion mismatches the obligation, so the verifier rejects the
    // module.
    let mut forged = lowered.semantic_module.clone();
    forged.machines[0].contract.ensures[0].proposition = Proposition::Atom(
        semantic_vocabulary::PropositionId::new(
            terminal_psi::float_meaning_equality_proposition_id(0).get() + 1,
        )
        .expect("nonzero"),
    );
    assert!(
        terminal_verifier::verify_module(
            &forged,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );

    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn exact_float_literals_cross_checked_terminal_codec_and_verifier_as_raw_bits() {
    let source = r#"
        machine prove_projection()
        requires
            Float::meaning32(0.0f32) == Float::meaning32(0.00f32);
            Float::meaning32(-0.0f32) == Float::meaning32(-0.00f32);
            Float::meaning64(0.1f64) == Float::meaning64(0.10f64);
        { }

        machine terminal_root(value: bool) -> bool
        requires
            true == true;
        ensures
            true == true;
        { value }
    "#;
    let checked = checked_float_projection_source(source);
    let lowered =
        lower_machine(&checked, TerminalMachineSelection::Name("terminal_root")).expect("lower");
    assert_eq!(
        lowered
            .semantic_module
            .float_meaning_projections
            .iter()
            .map(|projection| projection.source.clone())
            .collect::<Vec<_>>(),
        vec![
            terminal_psi::FloatMeaningSource::ExactBinary32Literal(0x0000_0000),
            terminal_psi::FloatMeaningSource::ExactBinary32Literal(0x8000_0000),
            terminal_psi::FloatMeaningSource::ExactBinary64Literal(0.1_f64.to_bits()),
        ]
    );
    assert_eq!(
        lowered.semantic_module.float_meaning_equalities,
        vec![
            terminal_psi::FloatMeaningEqualityProposition {
                id: terminal_psi::ProofPropositionId(0),
                left: terminal_psi::ProofValueId(0),
                right: terminal_psi::ProofValueId(0),
            },
            terminal_psi::FloatMeaningEqualityProposition {
                id: terminal_psi::ProofPropositionId(1),
                left: terminal_psi::ProofValueId(1),
                right: terminal_psi::ProofValueId(1),
            },
            terminal_psi::FloatMeaningEqualityProposition {
                id: terminal_psi::ProofPropositionId(2),
                left: terminal_psi::ProofValueId(2),
                right: terminal_psi::ProofValueId(2),
            },
        ]
    );
    terminal_verifier::validate_module(&lowered.semantic_module).expect("verify");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module).expect("encode");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module)
    );
}

#[test]
fn integer_operation_obligations_follow_the_shared_policy_catalog() {
    let operation = operation_id(10);
    let obligation_kinds = [
        LoweredIntegerBinaryKind::ExactShiftLeft,
        LoweredIntegerBinaryKind::ExactShiftRight,
        LoweredIntegerBinaryKind::ExactAdd,
        LoweredIntegerBinaryKind::ExactSubtract,
        LoweredIntegerBinaryKind::ExactMultiply,
        LoweredIntegerBinaryKind::ExactDivide,
        LoweredIntegerBinaryKind::ExactRemainder,
        LoweredIntegerBinaryKind::WrappingDivide,
        LoweredIntegerBinaryKind::WrappingRemainder,
        LoweredIntegerBinaryKind::SaturatingDivide,
        LoweredIntegerBinaryKind::SaturatingRemainder,
    ];
    for kind in obligation_kinds {
        assert!(kind.formation_obligation(operation).is_some(), "{kind:?}");
    }
    for kind in [
        LoweredIntegerBinaryKind::BitwiseAnd,
        LoweredIntegerBinaryKind::BitwiseOr,
        LoweredIntegerBinaryKind::BitwiseXor,
        LoweredIntegerBinaryKind::WrappingShiftLeft,
        LoweredIntegerBinaryKind::WrappingShiftRight,
        LoweredIntegerBinaryKind::WrappingAdd,
        LoweredIntegerBinaryKind::SaturatingAdd,
        LoweredIntegerBinaryKind::WrappingSubtract,
        LoweredIntegerBinaryKind::SaturatingSubtract,
        LoweredIntegerBinaryKind::WrappingMultiply,
        LoweredIntegerBinaryKind::SaturatingMultiply,
    ] {
        assert!(kind.formation_obligation(operation).is_none(), "{kind:?}");
    }
    assert_eq!(
        LoweredIntegerBinaryKind::ExactSubtract.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Subtract, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingDivide.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Divide, ArithmeticDomain::Saturating,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::ExactRemainder.integer_policy_binding(),
        Some((IntegerPolicyPrimitive::Remainder, ArithmeticDomain::Exact,)),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::WrappingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Wrapping,
        )),
    );
    assert_eq!(
        LoweredIntegerBinaryKind::SaturatingRemainder.integer_policy_binding(),
        Some((
            IntegerPolicyPrimitive::Remainder,
            ArithmeticDomain::Saturating,
        )),
    );
}

#[test]
fn shared_boolean_comparison_normalization_rejects_two_runtime_sides() {
    let comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Parameter { position: 0 }),
        right: Box::new(LoweredBooleanReturnExpression::Parameter { position: 1 }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&comparison).is_none());

    let local_comparison = LoweredBooleanReturnExpression::Equal {
        left: Box::new(LoweredBooleanReturnExpression::Local { position: 1 }),
        right: Box::new(LoweredBooleanReturnExpression::Constant { value: false }),
    };
    assert!(normalize_shared_boolean_comparison_leaves(&local_comparison).is_none());
}

#[test]
fn generic_conformance_application_crosses_terminal_scalar_closure() {
    let source = r#"
        trait Ranked<'rank, Context> {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        FieldOrder<'scope, Element>:
            Element satisfies Ranked<'scope, Borrow<'scope, Element>>
        {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<
            'call,
            Element,
            Order: Element satisfies Ranked<Borrow<'call, Element>>
        >(
            left: &'call Element,
            right: &'call Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller<'view>(left: &'view Card, right: &'view Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let owner = checked
        .machine_specializations
        .iter()
        .find(|specialization| !specialization.conformance_applications.is_empty())
        .expect("conformance specialization")
        .instance;

    let terminal_source = r#"
        machine terminal_root(value: bool) -> bool
        requires true == true
        ensures true == true
        { value }
    "#;
    let terminal_checked = crate::front_end::checked_program(terminal_source);
    let mut lowered = lower_machine(
        &terminal_checked,
        TerminalMachineSelection::Name("terminal_root"),
    )
    .expect("lower terminal");
    lower_closed_conformance_applications(&checked, &[owner], &mut lowered.semantic_module)
        .expect("lower closed application");
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("verify closed application");
    let [application] = lowered
        .semantic_module
        .closed_conformance_applications
        .as_slice()
    else {
        panic!("one closed application should cross terminal lowering")
    };
    assert!(application.telescope.iter().any(|binding| {
        binding.kind == terminal_psi::ClosedConformanceParameterKind::Type
            && binding.parameter == "Element"
            && binding.argument == "Card"
    }));
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
    assert_eq!(application.trait_lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
    assert_eq!(application.rows.len(), 1);
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .any(|machine| machine.id == application.owner)
    );
    let bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode closed application");
    let decoded = terminal_codec::decode_module(&bytes).expect("decode closed application");
    assert_eq!(decoded, lowered.semantic_module);

    let mut redirected_lifetime = decoded.clone();
    redirected_lifetime.closed_conformance_applications[0].trait_lifetime_arguments[0]
        .push_str("::redirected");
    assert!(matches!(
        terminal_verifier::validate_module(&redirected_lifetime),
        Err(terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));

    let mut redirected = decoded;
    redirected.closed_conformance_applications[0].rows[0]
        .realization_identity
        .push_str("::redirected");
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::ClosedConformanceFingerprintMismatch { .. })
    ));
}

#[test]
fn scalar_crash_disjunction_lowers_to_canonical_terminal_propositions() {
    let values = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let proposition = checked_boolean_proposition(
        &CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        },
        &values,
    )
    .expect("scalar disjunction lowers");
    let Proposition::Disjunction(disjuncts) = &proposition else {
        panic!("scalar disjunction retains proposition structure")
    };
    assert_eq!(disjuncts.len(), 2);
    let keys = disjuncts
        .iter()
        .map(|disjunct| terminal_codec::canonical_proposition_order_key(disjunct).unwrap())
        .collect::<Vec<_>>();
    assert!(keys[0] < keys[1]);
    PropositionContext::from_value_types(values.iter().map(|value| (value.id, value.scalar_type)))
        .unwrap()
        .validate(&proposition)
        .expect("scalar disjunction is well typed");
}

#[test]
fn payloadless_sum_equality_lowers_to_case_membership_equivalence() {
    let source = r#"
        data Mode {
            case Off;
            case On;
        }

        data Root {}
        machine Root::enter(left: Mode, right: Mode)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Mode, right: Mode)
        crashes Abort
            left != right
        {}
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower terminal");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-less sum retains a sum shape");
    assert_eq!(
        cases
            .iter()
            .map(|case| case.identity.as_str())
            .collect::<Vec<_>>(),
        ["Off", "On"]
    );
    let [bucket] = lowered.semantic_module.machines[0]
        .contract
        .crash_routes
        .as_slice()
    else {
        panic!("one crash bucket")
    };
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] = bucket.alternatives.as_slice()
    else {
        panic!("one predicate")
    };
    let Proposition::Conjunction(equivalences) = predicate.proposition() else {
        panic!("sum equality is one canonical conjunction")
    };
    assert_eq!(equivalences.len(), 4);
    assert!(equivalences.iter().all(|equivalence| matches!(
        equivalence,
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::StructuralCaseMembership { .. })
                && matches!(conclusion.as_ref(), Proposition::StructuralCaseMembership { .. })
    )));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("case-membership equality validates");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("case-membership module encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let different = lower_machine(&checked, TerminalMachineSelection::Name("Root::different"))
        .expect("lower inequality");
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Conjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
}

#[test]
fn structural_boundary_result_lowers_and_round_trips() {
    let checked = crate::front_end::checked_program(
        r#"
        data ByteRead {
            case Eof;
            case Byte(value: i32);
        }
        boundary trait Console {
            machine read_byte() -> ByteRead
            reaches Console;
        }
        data Root {}
        machine Root::enter()
        reaches Console
        {
            let result: ByteRead = Console::read_byte();
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("lower terminal");
    let module = &lowered.semantic_module;
    let [boundary] = module.boundary_machines.as_slice() else {
        panic!("one structural-result boundary")
    };
    let BoundaryMachineResult::Structural(signature) = &boundary.result else {
        panic!("boundary result should retain its structural signature")
    };
    assert_eq!(signature.multiplicity, StructuralMultiplicity::Affine);
    let operation = &module.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &operation.result else {
        panic!("boundary call should produce one structural result")
    };
    assert_eq!(result.structural_type, signature.structural_type);
    assert_eq!(result.multiplicity, signature.multiplicity);
    assert!(matches!(operation.kind, OperationKind::BoundaryCall { .. }));
    let bytes = terminal_codec::encode_module(module).expect("encode terminal module");
    let decoded = terminal_codec::decode_module(&bytes).expect("decode terminal module");
    assert_eq!(&decoded, module);
}

#[test]
fn payload_bearing_sum_equality_uses_exact_case_payload_paths() {
    let source = r#"
        trait Equatable {
            machine equals(&self, rhs: &Self) -> bool;
        }

        data Message {
            case Empty;
            case Data(value: i32);
        }
        MessageEquatable: Message satisfies Equatable;

        data Root {}
        machine Root::enter(left: Message, right: Message)
        crashes Abort
            left == right
        {}

        machine Root::different(left: Message, right: Message)
        crashes Abort
            left != right
        {}
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("payload-bearing equality has exact case-payload paths");
    let cases = lowered
        .semantic_module
        .structural_types
        .iter()
        .find_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Sum { cases } => Some(cases),
            _ => None,
        })
        .expect("payload-bearing sum shape");
    assert_eq!(cases.len(), 2);
    assert!(cases[0].fields.is_empty());
    assert_eq!(cases[1].fields.len(), 1);
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
        lowered.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one equality predicate")
    };
    let Proposition::Disjunction(arms) = predicate.proposition() else {
        panic!("payload-bearing equality is a per-case disjunction")
    };
    assert_eq!(arms.len(), 2);
    assert!(format!("{arms:?}").contains("Case(StructuralCaseId(2))"));
    assert!(format!("{arms:?}").contains("Field(StructuralFieldId(1))"));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("exact case-payload paths validate");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("payload-bearing sum module encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes),
        Ok(lowered.semantic_module.clone())
    );
    let mut redirected = lowered.semantic_module.clone();
    let payload_field = redirected
        .structural_types
        .iter_mut()
        .find_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Sum { cases } => {
                cases.iter_mut().find_map(|case| case.fields.first_mut())
            }
            _ => None,
        })
        .expect("payload field");
    payload_field.id = semantic_vocabulary::StructuralFieldId::new(99).expect("redirected field");
    assert!(matches!(
        terminal_verifier::validate_module(&redirected),
        Err(terminal_verifier::ModuleError::InvalidIntegerFieldTerm { .. })
    ));

    let different = lower_machine(&checked, TerminalMachineSelection::Name("Root::different"))
        .expect("lower inequality");
    let [terminal_psi::CrashRouteGuard::Predicate(predicate)] =
        different.semantic_module.machines[0].contract.crash_routes[0]
            .alternatives
            .as_slice()
    else {
        panic!("one inequality predicate")
    };
    assert!(matches!(
        predicate.proposition(),
        Proposition::Implication { premise, conclusion }
            if matches!(premise.as_ref(), Proposition::Disjunction(_))
                && matches!(conclusion.as_ref(), Proposition::Falsehood)
    ));
}
