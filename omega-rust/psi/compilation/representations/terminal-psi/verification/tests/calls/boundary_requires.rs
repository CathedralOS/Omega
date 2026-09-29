//! A boundary declaration's scalar `requires` rows: each call owes one
//! obligation per row, instantiated at its actuals in the declaration's
//! formal telescope, and a row may name only those formals.

use super::{
    AdmissionProfile, ModuleError, OperationKind, ProofBundle, Proposition, ScalarTerm, ScalarType,
    TerminalModule, boolean_declaration, boolean_value, boundary_call_module, obligation_id,
    operation_id, reconstruct_terminal_obligations, validate_module, value_id, verify_module,
};

/// Two Boolean formals, called with the caller's values swapped, and one
/// requires row naming the first formal.
fn required_boundary_module() -> TerminalModule {
    let mut module = boundary_call_module();
    module.boundary_machines[0].parameter_order =
        vec![terminal_psi::BoundaryParameterKind::Scalar; 2];
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Boolean; 2];
    module.boundary_machines[0].scalar_requires = vec![Proposition::Equal(
        boolean_value(1),
        ScalarTerm::boolean(true),
    )];
    let caller = &mut module.machines[0];
    caller.parameters = vec![
        boolean_declaration(value_id(1)),
        boolean_declaration(value_id(2)),
    ];
    caller.blocks[0].operations.remove(0);
    let OperationKind::BoundaryCall {
        arguments,
        requirement_obligations,
        ..
    } = &mut caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *arguments = vec![value_id(2), value_id(1)];
    *requirement_obligations = vec![obligation_id(1)];
    module
}

#[test]
fn boundary_requires_row_is_owed_at_the_call_actuals() {
    let module = required_boundary_module();
    validate_module(&module).expect("one obligation per requires row validates");
    let reconstructed = reconstruct_terminal_obligations(&module).expect("reconstructed");
    let call_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .expect("boundary call requires obligation");
    assert_eq!(
        call_site.obligation.proposition,
        Proposition::Equal(boolean_value(2), ScalarTerm::boolean(true)),
        "the first formal is replaced by the call's first actual, not a caller value of the same number",
    );
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "an unproven precondition cannot verify"
    );
}

#[test]
fn boundary_requires_rejects_missing_obligations_and_foreign_values() {
    let mut missing = required_boundary_module();
    let OperationKind::BoundaryCall {
        requirement_obligations,
        ..
    } = &mut missing.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    requirement_obligations.clear();
    assert_eq!(
        validate_module(&missing).unwrap_err(),
        ModuleError::CallRequirementArityMismatch {
            operation: operation_id(2),
            expected: 1,
            actual: 0,
        }
    );

    // A caller value cannot stand in for a third declaration-local formal.
    let mut foreign = required_boundary_module();
    foreign.machines[0]
        .parameters
        .push(boolean_declaration(value_id(3)));
    foreign.boundary_machines[0].scalar_requires = vec![Proposition::Equal(
        boolean_value(3),
        ScalarTerm::boolean(true),
    )];
    assert!(matches!(
        validate_module(&foreign),
        Err(ModuleError::MalformedProposition(_))
    ));
}

#[test]
fn ordered_ieee_boundary_requires_use_reconstructed_literal_bits() {
    use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, check_certificate};
    use semantic_vocabulary::{
        IeeeFloatComparisonKind as Kind, IeeeFloatFormat, IeeeFloatValue, PropositionContext,
    };
    let format = IeeeFloatFormat::Binary32;
    let scalar_type = ScalarType::IeeeFloat(format);
    let subject = ScalarTerm::value(value_id(1), scalar_type);
    let literal =
        |value: f32| ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(value.to_bits()));
    let relation = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    let mut clauses = vec![
        relation(Kind::Equal, subject.clone(), subject.clone()),
        relation(Kind::LessThan, literal(-129.0), subject.clone()),
        relation(Kind::LessThan, subject.clone(), literal(128.0)),
    ];
    clauses.sort();
    let mut module = boundary_call_module();
    module.boundary_machines[0].scalar_parameters = vec![scalar_type];
    module.boundary_machines[0].scalar_requires = vec![Proposition::Conjunction(clauses)];
    let operations = &mut module.machines[0].blocks[0].operations;
    operations[0].result = terminal_psi::OperationResult::Scalar(terminal_psi::ValueDeclaration {
        id: value_id(1),
        scalar_type,
        qualifications: Default::default(),
    });
    let OperationKind::BoundaryCall {
        requirement_obligations,
        ..
    } = &mut operations[1].kind
    else {
        panic!("boundary call");
    };
    *requirement_obligations = vec![obligation_id(1)];
    let context = PropositionContext::from_value_types([(value_id(1), scalar_type)]).unwrap();
    for (bits, accepted) in [
        ((-8.75f32).to_bits(), true),
        ((-129.0f32).to_bits(), false),
        (128.0f32.to_bits(), false),
        (0x7fc0_0001, false),
        (0x7f80_0000, false),
    ] {
        module.machines[0].blocks[0].operations[0].kind = OperationKind::IeeeFloatConstant {
            value: IeeeFloatValue::Binary32(bits),
        };
        let reconstructed = reconstruct_terminal_obligations(&module).expect("typed IEEE contract");
        let site = reconstructed
            .obligations()
            .iter()
            .find(|site| site.obligation.id == obligation_id(1))
            .unwrap();
        let equation = Proposition::Equal(
            subject.clone(),
            ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(bits)),
        );
        let index = site
            .semantic_axioms
            .iter()
            .position(|axiom| axiom == &equation)
            .expect("literal operation, not frontend facts, establishes the exact bits");
        let proof = ProofNode {
            conclusion: site.obligation.proposition.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                }),
                equalities: vec![ProofNode {
                    conclusion: equation,
                    rule: ProofRule::SemanticAxiom { index },
                }],
            },
        };
        assert_eq!(
            check_certificate(
                &context,
                &site.obligation.proposition,
                &site.requirements,
                &site.semantic_axioms,
                &proof
            )
            .is_ok(),
            accepted,
            "bits {bits:x}"
        );
    }
}
