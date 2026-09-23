use super::*;
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn stores(source: &str, state_index: usize) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program.machines().iter().next().unwrap();
    let state = &program.machine_states(machine)[state_index];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached state signature");
    build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
}

/// A whole owned parameter copies into an initialized payload-free sum field:
/// the store keeps the destination's proven write custody, and the value is an
/// ordinary structural argument of the field's declared sum type rather than
/// a scalar the field could not name.
#[test]
fn whole_parameter_stores_into_a_sum_field() {
    let source = r#"
        data Format [copy] { case Plain; case Hex; }
        data T { format: Format; }
        machine T::set_format(&mut self, format: Format) {
            self.format = format;
        }
    "#;
    let stores = stores(source, 0).expect("sum field store");
    let Some(CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(plan)) = stores.first()
    else {
        panic!("case field store leads the sequence");
    };
    assert_eq!(plan.field_identity, "format");
    assert!(plan.carrier_path.is_empty());
    assert!(matches!(
        plan.destination,
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter { position: 0 }
    ));
    assert!(matches!(
        plan.value.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
    ));
    assert!(plan.value.path.is_empty());
    assert_eq!(
        plan.value.access,
        checked_trees::CheckedStructuralAccess::Owned
    );
}

/// The same store admits inside a whole-sequence frame: a case store keeps its
/// statement position beside an ordinary scalar field store.
#[test]
fn case_store_sequences_with_scalar_stores() {
    let source = r#"
        data Format [copy] { case Plain; case Hex; }
        data T { format: Format; n: u64; }
        machine T::relabel(&mut self, format: Format) {
            self.format = format;
            self.n = 3;
        }
    "#;
    let stores = stores(source, 0).expect("case store beside scalar store");
    assert!(matches!(
        stores.as_slice(),
        [
            CheckedUnitEffectOperationPlan::StructuralCaseFieldStore(_),
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        ]
    ));
}

/// A member read is not a whole owned place: `self.format = source.format`
/// would need a path below the source root, which this store's value lane
/// does not carry, so it keeps declining.
#[test]
fn member_read_value_still_declines() {
    let source = r#"
        data Format [copy] { case Plain; case Hex; }
        data T { format: Format; }
        machine T::copy_format(&mut self, source: T) {
            self.format = source.format;
        }
    "#;
    assert!(stores(source, 0).is_none());
}

/// A record-typed field is not a payload-free sum: storing a whole record
/// parameter moves the record's own member custody, which this op does not
/// claim, so the store keeps declining.
#[test]
fn record_field_store_still_declines() {
    let source = r#"
        data Rec [copy] { x: u64; }
        data T { rec: Rec; }
        machine T::set_rec(&mut self, rec: Rec) {
            self.rec = rec;
        }
    "#;
    assert!(stores(source, 0).is_none());
}

/// A data mixing record fields with cases is not a payload-free sum either —
/// the mixed shape owns scalar members with their own write custody — so the
/// store keeps declining.
#[test]
fn mixed_field_store_still_declines() {
    let source = r#"
        data Mixed [copy] { n: u64; case Flag; }
        data T { m: Mixed; }
        machine T::set_mixed(&mut self, m: Mixed) {
            self.m = m;
        }
    "#;
    assert!(stores(source, 0).is_none());
}
