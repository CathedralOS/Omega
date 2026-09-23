//! Element-view reads and subslices over the same cyclic view topology as
//! the byte-view fixture. Their index, endpoints and observed length are
//! `u64` element counts that must be defined before use. The in-bounds
//! obligation `index < length` is a bounds proof only over that unsigned
//! domain, so a signed, Boolean or undefined operand is refused at
//! validation rather than left to the proof.

use super::{
    AdmissionProfile, IntegerSign, IntegerType, ModuleError, ObligationId, OperationId,
    OperationKind, ProofBundle, ScalarType, StructuralTypeDeclaration, StructuralTypeId,
    StructuralTypeShape, TerminalModule, ValueDeclaration, ValueId, VerificationError, id,
    validate_module, verify_module_for_interpretation,
};

fn element_view_cycle() -> TerminalModule {
    let mut module = super::view_cycle();
    let element = id(9, StructuralTypeId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: element,
        identity: "test::Element".into(),
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        )),
    });
    module.structural_types[0].identity = "test::CyclicElements".into();
    module.structural_types[0].shape = StructuralTypeShape::ElementView { element };
    for block in &mut module.machines[0].blocks {
        for operation in &mut block.operations {
            operation.kind = match operation.kind.clone() {
                OperationKind::ByteSequenceLength { source } => {
                    OperationKind::ElementViewLength { source }
                }
                OperationKind::ByteSequenceRead {
                    source,
                    index,
                    length,
                    obligation,
                } => OperationKind::ElementViewRead {
                    source,
                    index,
                    length,
                    obligation,
                },
                OperationKind::ByteSequenceSubslice {
                    source,
                    start,
                    end,
                    length,
                    obligation,
                } => OperationKind::ElementViewSubslice {
                    source,
                    start,
                    end,
                    length,
                    obligation,
                },
                other => other,
            };
        }
    }
    module
}

fn with_operand(module: &mut TerminalModule, operation: u64, replace: impl Fn(&mut OperationKind)) {
    let target = module.machines[0]
        .blocks
        .iter_mut()
        .flat_map(|block| block.operations.iter_mut())
        .find(|candidate| candidate.id == id(operation, OperationId::new))
        .expect("fixture operation");
    replace(&mut target.kind);
}

fn signed_index(module: &mut TerminalModule) -> ValueId {
    let signed = id(21, ValueId::new);
    module.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: signed,
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
    });
    signed
}

#[test]
fn element_view_cycle_validates_and_owes_its_read_bound() {
    let module = element_view_cycle();
    validate_module(&module)
        .map(|_| ())
        .expect("u64 element counts over a cyclic element view validate");
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(VerificationError::MissingEvidence(obligation)) if obligation == id(10, ObligationId::new)
    ));
}

#[test]
fn element_read_index_must_be_a_defined_u64() {
    let mut signed = element_view_cycle();
    let index = signed_index(&mut signed);
    with_operand(&mut signed, 11, |kind| {
        if let OperationKind::ElementViewRead { index: slot, .. } = kind {
            *slot = index;
        }
    });
    assert_eq!(
        validate_module(&signed).map(|_| ()),
        Err(ModuleError::ElementViewReadOperandTypeMismatch {
            operation: id(11, OperationId::new),
            operand: index,
            actual: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        })
    );

    let mut boolean = element_view_cycle();
    with_operand(&mut boolean, 11, |kind| {
        if let OperationKind::ElementViewRead { index, .. } = kind {
            *index = id(20, ValueId::new);
        }
    });
    assert_eq!(
        validate_module(&boolean).map(|_| ()),
        Err(ModuleError::ElementViewReadOperandTypeMismatch {
            operation: id(11, OperationId::new),
            operand: id(20, ValueId::new),
            actual: ScalarType::Boolean,
        })
    );

    let mut undefined = element_view_cycle();
    with_operand(&mut undefined, 11, |kind| {
        if let OperationKind::ElementViewRead { index, .. } = kind {
            *index = id(99, ValueId::new);
        }
    });
    assert_eq!(
        validate_module(&undefined).map(|_| ()),
        Err(ModuleError::ValueUsedBeforeDefinition(id(99, ValueId::new)))
    );
}

#[test]
fn element_subslice_endpoints_must_be_defined_u64_values() {
    for (endpoint, is_start) in [("start", true), ("end", false)] {
        let mut signed = element_view_cycle();
        let value = signed_index(&mut signed);
        with_operand(&mut signed, 12, |kind| {
            if let OperationKind::ElementViewSubslice { start, end, .. } = kind {
                *(if is_start { start } else { end }) = value;
            }
        });
        assert_eq!(
            validate_module(&signed).map(|_| ()),
            Err(ModuleError::ElementViewSubsliceOperandTypeMismatch {
                operation: id(12, OperationId::new),
                operand: value,
                actual: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
            }),
            "a signed subslice {endpoint}"
        );
    }
}
