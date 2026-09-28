//! Indexed structural field reads: a literal `FixedIndex` may end the carrier
//! of a scalar field observation (`self.maps[1].value`), because the element
//! record owns the observed field. The lowering mirrors the producer's bounded
//! grammar — record fields followed by at most one literal, in-extent index —
//! and every other segment shape stays fail-closed.

use super::structural_borrows::source_plan;
use super::{AbstractOperation, AbstractOperationPlan, NativeTarget};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId};
use terminal_psi::StructuralTypeShape;

fn indexed_read_plan() -> AbstractOperationPlan {
    source_plan(
        r#"
            data Map { value: i32; }
            data Main { maps: [Map; 2]; }
            machine Main::run(&mut self) -> i32 { self.maps[1].value }
        "#,
    )
}

fn field_read(plan: &mut AbstractOperationPlan) -> &mut AbstractOperation {
    plan.functions
        .iter_mut()
        .flat_map(|function| &mut function.operations)
        .find(|operation| matches!(operation, AbstractOperation::IntegerStructuralField { .. }))
        .expect("integer structural field read")
}

#[test]
fn field_read_carrier_rejects_unbounded_and_malformed_index_segments() {
    for mutation in [
        "out of range",
        "non-terminal index",
        "index into a record",
        "case segment",
        "element without the field",
    ] {
        let mut source = indexed_read_plan();
        match mutation {
            // The literal must stay below the declared extent even though the
            // producer's bound was already proven.
            "out of range" => {
                let AbstractOperation::IntegerStructuralField { path, .. } =
                    field_read(&mut source)
                else {
                    unreachable!()
                };
                path[1] = CanonicalStructuralPathSegment::FixedIndex(2);
            }
            // The index only ends the carrier; a deeper segment is malformed.
            "non-terminal index" => {
                let AbstractOperation::IntegerStructuralField { path, .. } =
                    field_read(&mut source)
                else {
                    unreachable!()
                };
                path.push(CanonicalStructuralPathSegment::FixedIndex(0));
            }
            // The receiver record is not an array carrier.
            "index into a record" => {
                let AbstractOperation::IntegerStructuralField { path, .. } =
                    field_read(&mut source)
                else {
                    unreachable!()
                };
                *path = vec![CanonicalStructuralPathSegment::FixedIndex(0)];
            }
            // Case segments never belong to a scalar field carrier.
            "case segment" => {
                let AbstractOperation::IntegerStructuralField { path, .. } =
                    field_read(&mut source)
                else {
                    unreachable!()
                };
                path[1] = CanonicalStructuralPathSegment::Case(StructuralCaseId::new(1).unwrap());
            }
            // A scalar element owns no field for the terminal read.
            _ => {
                let element = source
                    .structural_types
                    .iter()
                    .find_map(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, .. } => Some(element),
                        _ => None,
                    })
                    .expect("fixed array element");
                source
                    .structural_types
                    .make_mut()
                    .iter_mut()
                    .find(|declaration| declaration.id == element)
                    .unwrap()
                    .shape =
                    StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Boolean);
            }
        }
        assert!(
            crate::lower_to_target_operations(
                &source,
                crate::TargetLoweringRequest::new(NativeTarget::macos_arm64())
            )
            .is_err(),
            "accepted {mutation}"
        );
    }
}
