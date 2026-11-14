use super::*;
use crate::body::{Borrowed, BorrowedReferent, Regions};
use language_core::ReferenceAccess;
use terminal_psi::{ArrayExtent, ByteSequenceCarrier, ElementViewRegion};

fn array_types(access: StructuralAccess) -> IdTable<StructuralTypeId, RecordType> {
    let mut records = IdTable::default();
    let mut add = |identity: &str, copyable, shape| {
        records.push(RecordType {
            owns_linear_claim: false,
            copyable,
            identity: identity.to_owned(),
            shape,
            requires: Vec::new(),
        });
    };
    add(
        "u8",
        true,
        crate::StorageShape::PrimitiveScalar {
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
            policy: None,
        },
    );
    add(
        "[u8;3]",
        true,
        crate::StorageShape::FixedArray {
            element: id(1),
            length: ArrayExtent::Literal(3),
        },
    );
    add(
        "byte view",
        access == StructuralAccess::SharedBorrow,
        crate::StorageShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(access),
        }),
    );
    records
}

#[test]
fn explicit_array_views_retain_backing_and_a_separate_loan() {
    for (access, structural) in [
        (ReferenceAccess::Shared, StructuralAccess::SharedBorrow),
        (ReferenceAccess::Mutable, StructuralAccess::MutableBorrow),
    ] {
        let array = ValueType::record(id(2));
        let view = ValueType::record(id(3));
        let formation = of_type(
            ExpressionKind::ArrayByteView {
                base: Box::new(of_type(ExpressionKind::Slot(0), array)),
                access,
                element: id(1),
            },
            view,
        );
        // Shared formation is bound to a local; exclusive formation is a
        // fresh call actual. Both retain the original array operand.
        let (slots, statements, actual) = if access == ReferenceAccess::Shared {
            (
                vec![array, view],
                vec![bind(1, formation)],
                of_type(ExpressionKind::Slot(1), view),
            )
        } else {
            (vec![array], Vec::new(), formation)
        };
        let argument = of_type(
            ExpressionKind::Borrow {
                access,
                place: Box::new(actual),
                home: None,
            },
            view,
        );
        let mut statements = statements;
        statements.push(Statement::Evaluate(call(2, vec![argument], None)));
        statements.push(Statement::Arm {
            guard: None,
            target: Target::Unit,
            source: nowhere(),
        });
        let mut signatures = IdTable::default();
        signatures.push(shaped(vec![array], None));
        let mut callee = shaped(vec![view], None);
        callee.borrowed.push(Borrowed {
            position: 0,
            access,
            referent: BorrowedReferent::Slice,
            regions: Regions::NONE,
            home: None,
        });
        signatures.push(callee);
        let mut outcomes = IdTable::default();
        outcomes.push(returns_normally());
        outcomes.push(returns_normally());
        let lowered = lower_records(
            Body { slots, statements },
            &signatures,
            &outcomes,
            &array_types(structural),
        )
        .unwrap();
        assert_module_and_tables_agree(&lowered.machine, &lowered.tables);
        let operation = lowered
            .machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishElementView { .. }
                        | OperationKind::EstablishMutableElementView { .. }
                )
            })
            .unwrap();
        let (destination, source, element, region) = match &operation.kind {
            OperationKind::EstablishElementView {
                destination,
                source,
                element,
                region,
            } => {
                assert_eq!(access, ReferenceAccess::Shared);
                (*destination, source, *element, *region)
            }
            OperationKind::EstablishMutableElementView {
                destination,
                source,
                element,
                region,
            } => {
                assert_eq!(access, ReferenceAccess::Mutable);
                (*destination, source, *element, *region)
            }
            _ => unreachable!(),
        };
        assert_eq!(destination, produced(operation));
        assert_eq!(source.access, structural);
        assert!(source.path.is_empty());
        assert_eq!(element, id(1));
        assert_eq!(region, ElementViewRegion::WholeCollection);
        let captured = *lowered.tables.roots.get(source.place).unwrap();
        assert!(obligations(&lowered.tables).iter().any(|obligation|
            matches!(obligation, ObligationKind::Loan { captured: exact, access: loan_access, .. }
                if *exact == captured && *loan_access == access)));
    }
}
