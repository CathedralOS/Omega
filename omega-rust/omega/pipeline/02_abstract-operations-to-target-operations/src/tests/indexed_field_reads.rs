//! Indexed structural field reads: a literal `FixedIndex` may end the carrier
//! of a scalar field observation (`self.maps[1].value`), because the element
//! record owns the observed field. The lowering mirrors the producer's bounded
//! grammar — record fields followed by at most one literal, in-extent index —
//! and every other segment shape stays fail-closed.

use super::structural_borrows::source_plan;
use super::{AbstractOperation, AbstractOperationPlan, NativeTarget, TargetUnitOperation};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId};
use terminal_psi::{StructuralPathSegment, StructuralTypeShape};

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
fn terminal_fixed_index_carrier_lowers_the_exact_indexed_path() {
    let mut source = indexed_read_plan();
    let (maps, observed) = {
        let AbstractOperation::IntegerStructuralField { path, field, .. } = field_read(&mut source)
        else {
            unreachable!()
        };
        // The producer emits the bounded carrier this leg admits: one record
        // field followed by the literal element index.
        let [
            CanonicalStructuralPathSegment::Field(maps),
            CanonicalStructuralPathSegment::FixedIndex(1),
        ] = path.as_slice()
        else {
            panic!("bounded carrier path: {path:?}")
        };
        (*maps, *field)
    };
    let maps_identity = source
        .structural_types
        .iter()
        .flat_map(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter(),
            _ => [].iter(),
        })
        .find(|field| field.id == maps)
        .map(|field| field.identity.clone())
        .expect("maps field identity");
    let (place, access) = source
        .functions
        .iter()
        .flat_map(|function| &function.structural_parameters)
        .map(|parameter| (parameter.place, parameter.access))
        .next()
        .expect("receiver parameter");
    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
        let lowered =
            crate::lower_to_target_operations(&source, crate::TargetLoweringRequest::new(native))
                .expect("indexed carrier lowers");
        crate::validate_abstract_to_target_translation(&source, native, &lowered)
            .expect("indexed read replays independently");
        let (source, field) = lowered
            .functions
            .iter()
            .flat_map(|function| &function.graph.blocks)
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarFieldRead { source, field, .. } => {
                    Some((source, field))
                }
                _ => None,
            })
            .expect("structural scalar field read");
        assert_eq!(*field, observed);
        assert_eq!(source.place, place);
        assert_eq!(source.access, access);
        assert_eq!(
            source.path,
            vec![
                StructuralPathSegment::Field(maps_identity.clone()),
                StructuralPathSegment::FixedIndex(1),
            ]
        );
    }
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
