//! A structural local bound by its initializer's call composes through the
//! statement sequence as the call's own result binding; the result type's
//! Unit shape is what admits or refuses it, at that statement.

use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

fn plan(source: &str, name: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked(source);
    let machine = machine_named(&checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    plans
        .for_machine(machine)
        .unwrap_or_else(|| {
            panic!(
                "`{name}` binds its call result as an ordinary structural local: {:?}",
                plans.omission_for_machine(machine)
            )
        })
        .operations
        .clone()
}

fn omission(source: &str, name: &str) -> checked_trees::CheckedUnitPlanOmissionStage {
    let checked = checked(source);
    let machine = machine_named(&checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    assert!(
        plans.for_machine(machine).is_none(),
        "`{name}` has no Unit shape for its bound result"
    );
    plans
        .omission_for_machine(machine)
        .unwrap_or_else(|| panic!("`{name}` records why its plan was omitted"))
        .stage
}

#[test]
fn call_bound_structural_local_composes_with_a_following_store() {
    let operations = plan(
        "data Inner { value: u64; }
         data Holder { inner: Inner; count: u64; }
         data Main { total: u64; }
         machine make() -> Holder { Holder { inner: Inner { value: 1 }, count: 2 } }
         machine Main::main(&mut self) { let h: Holder = make(); self.total = 5; }",
        "main",
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall {
            result, coordinate, ..
        },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("the call binds the local, the store follows, the body completes: {operations:#?}")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(result.statement_index, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(store.statement_index, 1);
}

#[test]
fn call_bound_structural_local_admits_record_array_members_and_a_literal_sibling() {
    // The bound result's shape is the same classifier a discarded structural
    // call answers to: a record carrying a fixed array of records binds, and
    // a record-literal local beside it keeps its own establishment.
    let operations = plan(
        "data Cell { value: u64; }
         data Holder { cells: [Cell; 2]; }
         data Pair { a: u64; b: u64; }
         data Main { total: u64; }
         machine make() -> Holder { Holder { cells: [Cell { value: 1 }, Cell { value: 2 }] } }
         machine Main::main(&mut self) {
             let h: Holder = make();
             let p: Pair = Pair { a: 3, b: 4 };
             self.total = 5;
         }",
        "main",
    );
    let [
        CheckedUnitEffectOperationPlan::StructuralCall { result: bound, .. },
        CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result: literal, ..
        },
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("both locals establish in authored order: {operations:#?}")
    };
    assert_eq!((bound.statement_index, bound.binding_ordinal), (0, 0));
    assert_eq!((literal.statement_index, literal.binding_ordinal), (1, 1));
}

#[test]
fn call_result_without_a_unit_shape_refuses_at_the_result_shape_phase() {
    // Neither control is a statement-kind refusal: the local is a call
    // binding by kind, and the result type's shape is what declines it.
    for (label, source) in [
        (
            "nominal drop",
            "data Wrapper { code: u64; }
             machine Wrapper::drop(&mut self) {}
             machine make() -> Wrapper { Wrapper { code: 2 } }
             data Main { total: u64; }
             machine Main::main(&mut self) { let w: Wrapper = make(); self.total = 5; }",
        ),
        (
            "domain-qualified byte field",
            "domain [u8; 4]::Utf8 requires valid_utf8(self);
             data Holder { text: [u8; 4] in Utf8; }
             machine make() -> Holder { Holder { text: \"abcd\" } }
             data Main { total: u64; }
             machine Main::main(&mut self) { let h: Holder = make(); self.total = 5; }",
        ),
    ] {
        let stage = omission(source, "main");
        assert!(
            matches!(
                stage,
                checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
                    phase: "statement sequence: local data: structural result shape",
                    statement_index: Some(0),
                    ..
                }
            ),
            "{label}: the bound result's shape refuses at its own statement: {stage:?}"
        );
    }
}

fn established_record_fields(
    checked: &checked_trees::CheckedTrees,
    name: &str,
    operation_index: usize,
) -> Vec<checked_trees::CheckedStructuralRecordField> {
    let machine = machine_named(checked, name);
    let plans = &checked.facts.flow.terminal_unit_effects;
    let operations = plans
        .for_machine(machine)
        .map(|plan| plan.operations.clone())
        .or_else(|| {
            plans
                .composed_for_machine(machine)
                .map(|plan| plan.states[0].operations.clone())
        })
        .unwrap_or_else(|| {
            panic!(
                "`{name}` composes: {:?}",
                plans.omission_for_machine(machine)
            )
        });
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } =
        &operations[operation_index]
    else {
        panic!("operation {operation_index} establishes a structural value: {operations:#?}")
    };
    let checked_trees::CheckedStructuralValueKind::Record { fields, .. } = &checked
        .facts
        .values
        .structural_values
        .nodes
        .get(*value)
        .kind
    else {
        panic!("the established value is a record literal")
    };
    checked
        .facts
        .values
        .structural_values
        .record_fields
        .span(*fields)
        .expect("record fields")
        .to_vec()
}

#[test]
fn partial_literal_local_establishes_omitted_fields_as_zeros() {
    // A declared member the literal omits still initializes: the record plan
    // covers every declared field in declared order, with a zero entry where
    // no authored initializer exists.
    let checked = checked(
        "data P3 { a: u64; b: u64; c: u64; }
         data Main { total: u64; }
         machine Main::main(&mut self) {
             let p: P3 = P3 { a: 1, c: 3 };
             self.total = 5;
         }",
    );
    let fields = established_record_fields(&checked, "main", 0);
    let [
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Scalar(_),
            ..
        },
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Zero,
            ..
        },
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Scalar(_),
            ..
        },
    ] = fields.as_slice()
    else {
        panic!("the omitted middle member is the zero entry in declared order: {fields:#?}")
    };
}

#[test]
fn empty_literal_local_establishes_every_field_as_a_zero() {
    let checked = checked(
        "data Pair { a: u64; b: u64; }
         data Main { total: u64; }
         machine Main::main(&mut self) {
             let p: Pair = Pair {};
             self.total = 5;
         }",
    );
    let fields = established_record_fields(&checked, "main", 0);
    assert!(
        fields.len() == 2
            && fields.iter().all(|field| matches!(
                field.value,
                checked_trees::CheckedStructuralRecordFieldValue::Zero
            )),
        "an empty literal zero-initializes every declared member: {fields:#?}"
    );
}

#[test]
fn partial_literal_local_with_a_mutable_borrow_composes() {
    // The OpenedProcessInfo shape: a mutable partial literal whose omitted
    // member is a closed array, followed by a &mut method statement and the
    // local's tail.
    let checked = checked(
        "data Info [copy] { id: u64; tag: [u8; 4]; len: u64; }
         machine Info::new(id: u64) -> Info {
             let mut info: Info = Info { id: id, len: 0 };
             info.set_len(id);
             info
         }
         machine Info::set_len(&mut self, v: u64) {
             self.len = v;
         }",
    );
    let fields = established_record_fields(&checked, "Info::new", 0);
    let [
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Scalar(_),
            ..
        },
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Zero,
            ..
        },
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Scalar(_),
            ..
        },
    ] = fields.as_slice()
    else {
        panic!("the omitted array member is the zero entry: {fields:#?}")
    };
}

#[test]
fn omitted_nested_record_member_establishes_a_zero_record() {
    let checked = checked(
        "data Inner { a: u64; b: u64; }
         data Outer { inner: Inner; count: u64; }
         data Main { total: u64; }
         machine Main::main(&mut self) {
             let o: Outer = Outer { count: 2 };
             self.total = 5;
         }",
    );
    let fields = established_record_fields(&checked, "main", 0);
    let [
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Zero,
            ..
        },
        checked_trees::CheckedStructuralRecordField {
            value: checked_trees::CheckedStructuralRecordFieldValue::Scalar(_),
            ..
        },
    ] = fields.as_slice()
    else {
        panic!("the omitted record member is the zero entry: {fields:#?}")
    };
}

#[test]
fn literal_refuses_an_omitted_field_with_no_zero_spelling() {
    // A sum member has no unique zero, so the omission cannot mint a field
    // value and the literal keeps its refusal.
    let stage = omission(
        "data Opt { case Some(v: u64); case Empty; }
         data P2 { a: u64; o: Opt; }
         machine make(a: u64) -> P2 {
             let p: P2 = P2 { a: a };
             p
         }",
        "make",
    );
    assert!(
        matches!(
            stage,
            checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
                statement_index: Some(0),
                ..
            }
        ),
        "the omitted sum member still refuses at its own statement: {stage:?}"
    );
}
