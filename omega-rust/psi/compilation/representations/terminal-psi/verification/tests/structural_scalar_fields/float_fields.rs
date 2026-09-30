//! IEEE field facts observe current bits; overwritten facts survive only through
//! an already captured immutable value, never as claims about the replacement.

use super::{
    CanonicalStructuralPathSegment, ModuleError, Operation, OperationKind, OperationResult,
    Proposition, ScalarTerm, ScalarType, StructuralAccess, StructuralFieldType,
    StructuralPathSegment, StructuralTypeShape, TerminalModule, ValueDeclaration, id,
    structural_scalar_field_module, validate_module,
};
use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
use terminal_psi::ContractClause;
use terminal_verifier::reconstruct_terminal_obligations;

fn path() -> Vec<CanonicalStructuralPathSegment> {
    vec![CanonicalStructuralPathSegment::Field(id(1)); 2]
}

fn field(format: IeeeFloatFormat) -> ScalarTerm {
    ScalarTerm::ieee_float_field_path(id(1), path(), format)
}

fn fixture(format: IeeeFloatFormat) -> TerminalModule {
    let mut module = structural_scalar_field_module();
    module.machines.truncate(1);
    module.machines[0].blocks[0].operations.clear();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        panic!("nested record");
    };
    fields[0].field_type = StructuralFieldType::IeeeFloat(format);
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation: id(90),
        proposition: Proposition::Equal(field(format), field(format)),
    }];
    module
}

fn scalar(identity: u64, format: IeeeFloatFormat) -> OperationResult {
    OperationResult::Scalar(ValueDeclaration {
        id: id(identity),
        scalar_type: ScalarType::IeeeFloat(format),
        qualifications: Default::default(),
    })
}

fn append(
    module: &mut TerminalModule,
    identity: u64,
    result: OperationResult,
    kind: OperationKind,
) {
    module.machines[0].blocks[0].operations.push(Operation {
        id: id(identity),
        result,
        kind,
        static_reach_binding: None,
        suspension_crossing: None,
    });
}

fn store(module: &mut TerminalModule, identity: u64, value: u64, primitive: bool) {
    let kind = if primitive {
        OperationKind::WriteOnlyPrimitiveStore {
            destination: id(1),
            path: vec![
                StructuralPathSegment::Field("item".into()),
                StructuralPathSegment::Field("value".into()),
            ],
            value: id(value),
        }
    } else {
        OperationKind::StructuralScalarFieldStore {
            destination: id(1),
            path: vec![StructuralPathSegment::Field("item".into())],
            field: id(1),
            value: id(value),
            range_obligation: None,
        }
    };
    append(module, identity, OperationResult::Unit, kind);
}

fn read(module: &mut TerminalModule, identity: u64, format: IeeeFloatFormat) {
    append(
        module,
        identity,
        scalar(identity, format),
        OperationKind::PrimitiveScalarRead {
            source: id(1),
            path: vec![
                StructuralPathSegment::Field("item".into()),
                StructuralPathSegment::Field("value".into()),
            ],
        },
    );
}

fn exit_facts(module: &TerminalModule) -> Vec<Proposition> {
    validate_module(module).expect("exact float storage is well formed");
    reconstruct_terminal_obligations(module)
        .unwrap()
        .obligations()[0]
        .semantic_axioms
        .clone()
}

fn bits(format: IeeeFloatFormat, poison: bool) -> IeeeFloatValue {
    match (format, poison) {
        (IeeeFloatFormat::Binary32, false) => IeeeFloatValue::Binary32(0x8000_0000),
        (IeeeFloatFormat::Binary32, true) => IeeeFloatValue::Binary32(0x7fc0_1234),
        (IeeeFloatFormat::Binary64, false) => IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
        (IeeeFloatFormat::Binary64, true) => IeeeFloatValue::Binary64(0x7ff8_0000_0000_1234),
    }
}

#[test]
fn ieee_field_reads_and_stores_preserve_nan_payloads_and_signed_zero() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for primitive in [false, true] {
            let mut module = fixture(format);
            append(
                &mut module,
                1,
                scalar(1, format),
                OperationKind::IeeeFloatConstant {
                    value: bits(format, true),
                },
            );
            store(&mut module, 2, 1, primitive);
            read(&mut module, 3, format);
            append(
                &mut module,
                4,
                scalar(4, format),
                OperationKind::IeeeFloatConstant {
                    value: bits(format, false),
                },
            );
            store(&mut module, 5, 4, primitive);
            read(&mut module, 6, format);
            let facts = exit_facts(&module);
            let value = |identity| ScalarTerm::value(id(identity), ScalarType::IeeeFloat(format));
            assert!(facts.contains(&Proposition::Equal(
                value(1),
                ScalarTerm::IeeeFloatLiteral(bits(format, true))
            )));
            assert!(facts.contains(&Proposition::Equal(field(format), value(4))));
            assert!(facts.contains(&Proposition::Equal(value(6), field(format))));
            assert!(!facts.contains(&Proposition::Equal(field(format), value(1))));
            assert!(!facts.contains(&Proposition::Equal(value(3), field(format))));
            assert!(
                facts.contains(&Proposition::Equal(value(3), value(1)))
                    || facts.contains(&Proposition::Equal(value(1), value(3)))
            );
        }
    }
}

#[test]
fn ieee_field_predicate_survives_poison_only_for_an_existing_read() {
    for captured in [false, true] {
        let format = IeeeFloatFormat::Binary64;
        let mut module = fixture(format);
        let predicate = |subject| Proposition::ScalarIeeeFloatComparison {
            format,
            kind: IeeeFloatComparisonKind::LessOrEqual,
            left: subject,
            right: ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000)),
        };
        module.machines[0].contract.requires = vec![predicate(field(format))];
        if captured {
            read(&mut module, 1, format);
        }
        append(
            &mut module,
            2,
            scalar(2, format),
            OperationKind::IeeeFloatConstant {
                value: bits(format, true),
            },
        );
        store(&mut module, 3, 2, false);
        let facts = exit_facts(&module);
        assert!(!facts.contains(&predicate(field(format))));
        assert_eq!(
            facts.contains(&predicate(ScalarTerm::value(
                id(1),
                ScalarType::IeeeFloat(format)
            ))),
            captured
        );
    }
}

#[test]
fn ieee_field_contracts_reject_wrong_format_path_and_root() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let original = fixture(format);
        validate_module(&original).unwrap();
        for mutation in 0..4 {
            let mut invalid = original.clone();
            let wrong = match mutation {
                0 => ScalarTerm::ieee_float_field_path(
                    id(1),
                    path(),
                    if format == IeeeFloatFormat::Binary32 {
                        IeeeFloatFormat::Binary64
                    } else {
                        IeeeFloatFormat::Binary32
                    },
                ),
                1 => ScalarTerm::ieee_float_field_path(id(2), path(), format),
                2 => ScalarTerm::ieee_float_field_path(
                    id(1),
                    vec![CanonicalStructuralPathSegment::Field(id(2))],
                    format,
                ),
                _ => ScalarTerm::ieee_float_field_path(
                    id(1),
                    vec![CanonicalStructuralPathSegment::Field(id(1))],
                    format,
                ),
            };
            invalid.machines[0].contract.ensures[0].proposition =
                Proposition::Equal(wrong.clone(), wrong);
            let error = validate_module(&invalid).expect_err("substituted field must reject");
            assert!(
                matches!(error, ModuleError::InvalidIeeeFloatFieldTerm { .. })
                    || mutation == 1
                        && matches!(
                            error,
                            ModuleError::MalformedProposition(
                                semantic_vocabulary::PropositionError::UnknownStructuralPlace(_)
                            )
                        ),
                "mutation {mutation}: {error:?}"
            );
        }
        let mut write_only = original;
        write_only.machines[0].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
        validate_module(&write_only).expect("a supplied clause grants no read permission");
        read(&mut write_only, 1, format);
        assert!(validate_module(&write_only).is_err());
    }
}

#[test]
fn ieee_field_store_discharges_restored_range_not_poisoned_range() {
    use proof_admission::{check_denotation_certificate, produce_denotation_certificates};
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        for primitive in [false, true] {
            let mut module = fixture(format);
            let literal = |value: f64| match format {
                IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32((value as f32).to_bits()),
                IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(value.to_bits()),
            };
            let predicate = |subject| Proposition::ScalarIeeeFloatComparison {
                kind: IeeeFloatComparisonKind::LessOrEqual,
                format,
                left: subject,
                right: ScalarTerm::IeeeFloatLiteral(literal(1.0)),
            };
            module.machines[0].contract.ensures[0].proposition = predicate(field(format));
            for (phase, (stored, holds)) in [
                (literal(0.5), true),
                (literal(2.0), false),
                (bits(format, true), false),
                (literal(0.5), true),
            ]
            .into_iter()
            .enumerate()
            {
                let identity = 1 + phase as u64 * 3;
                append(
                    &mut module,
                    identity,
                    scalar(identity, format),
                    OperationKind::IeeeFloatConstant { value: stored },
                );
                store(&mut module, identity + 1, identity, primitive);
                read(&mut module, identity + 2, format);
                let validated = validate_module(&module).unwrap();
                let context = validated.value_context(&module.machines[0]).unwrap();
                let questions = reconstruct_terminal_obligations(&module).unwrap();
                let question = &questions.obligations()[0];
                // Current field and the newest read follow the replacement.
                // The first read remains 0.5 through both invalid windows.
                for (subject, expected) in [
                    (field(format), holds),
                    (
                        ScalarTerm::value(id(identity + 2), ScalarType::IeeeFloat(format)),
                        holds,
                    ),
                    (
                        ScalarTerm::value(id(3), ScalarType::IeeeFloat(format)),
                        true,
                    ),
                ] {
                    let goal = predicate(subject);
                    let certificates = produce_denotation_certificates(
                        &context,
                        &goal,
                        &[],
                        &question.semantic_axioms,
                    );
                    assert_eq!(
                        certificates
                            .iter()
                            .any(|certificate| check_denotation_certificate(
                                &context,
                                &goal,
                                &[],
                                &question.semantic_axioms,
                                certificate
                            )),
                        expected,
                        "{format:?}, primitive={primitive}, phase={phase}, goal={goal:?}"
                    );
                }
            }
        }
    }
}
