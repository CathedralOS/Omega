//! A recast read retains its representation re-view in the checked guard:
//! the viewed place keeps its declared carrier beneath the view carrier, and
//! a same-carrier recast reads the place directly.
use super::checked;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionRole,
};
use typed_trees::types::PrimitiveType;

const SOURCE: &str = r#"
    data Main { value: i32; small: i8; same: u32; }
    machine Main::main(&mut self) {
        let bits: &u32 = &self.value as &u32;
        let whole: &u32 = &self.same as &u32;
        transition bits == 5 && (&self.small as &u8) == 200 && whole == 7 {
            true -> good()
            _ -> bad()
        }
        state good(&mut self) {}
        state bad(&mut self) {}
    }
"#;

/// (view carrier, operand) for every re-view, plus every plain field read.
#[derive(Default)]
struct GuardReads {
    views: Vec<(PrimitiveType, CheckedScalarExpression)>,
    fields: Vec<PrimitiveType>,
}

fn scalar(expression: &CheckedScalarExpression, reads: &mut GuardReads) {
    match expression {
        CheckedScalarExpression::RepresentationView {
            primitive_type,
            operand,
        } => reads
            .views
            .push((*primitive_type, operand.as_ref().clone())),
        CheckedScalarExpression::StructuralParameterField { primitive_type, .. } => {
            reads.fields.push(*primitive_type);
        }
        CheckedScalarExpression::Boolean(expression) => boolean(expression, reads),
        _ => {}
    }
}

fn boolean(expression: &CheckedBooleanExpression, reads: &mut GuardReads) {
    match expression {
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right }
        | CheckedBooleanExpression::Equal { left, right } => {
            boolean(left, reads);
            boolean(right, reads);
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            scalar(left, reads);
            scalar(right, reads);
        }
        CheckedBooleanExpression::Not(inner) => boolean(inner, reads),
        _ => {}
    }
}

#[test]
fn recast_guard_reads_retain_the_declared_carrier_beneath_the_view() {
    let checked = checked(SOURCE);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("::main"))
        .expect("the authored Main::main machine");
    let state = &checked.machine_states(machine)[0];
    let guard = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(state.symbol, 2, CheckedScalarExpressionRole::Guard)
        .expect("the recast guard lowers to one pure expression");
    let mut reads = GuardReads::default();
    scalar(guard, &mut reads);
    let views = reads
        .views
        .iter()
        .map(|(view, operand)| {
            let CheckedScalarExpression::StructuralParameterField { primitive_type, .. } = operand
            else {
                panic!("a re-view observes one whole field place: {operand:?}");
            };
            (*view, *primitive_type)
        })
        .collect::<Vec<_>>();
    // The let-bound and inline views both keep their source carrier.
    assert_eq!(
        views,
        [
            (PrimitiveType::U32, PrimitiveType::I32),
            (PrimitiveType::U8, PrimitiveType::I8)
        ]
    );
    // The same-carrier recast is the place's own read, not a re-view.
    assert_eq!(reads.fields, [PrimitiveType::U32]);
}

#[test]
fn a_runtime_offset_byte_region_view_retains_its_first_byte_element() {
    let checked = checked(
        r#"
        data Main { buf: [u8; 12]; k: u32; }
        machine Main::main(&mut self) {
            self.k = 4;
            transition self.k <= 8 { true -> read(self.k) _ -> bad() }
            state read(&mut self, off: u32) {
                let word: &u32 = &self.buf[off] as &u32;
                transition word == 67305985 {
                    true -> good()
                    _ -> bad()
                }
            }
            state good(&mut self) {}
            state bad(&mut self) {}
        }
        "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with("::main"))
        .expect("the authored Main::main machine");
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str().ends_with("read"))
        .expect("the offset state");
    let guard = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(state.symbol, 1, CheckedScalarExpressionRole::Guard)
        .expect("the byte-region guard lowers to one pure expression");
    let mut reads = GuardReads::default();
    scalar(guard, &mut reads);
    let [(view, operand)] = reads.views.as_slice() else {
        panic!("one byte-region view: {guard:?}");
    };
    assert_eq!(*view, PrimitiveType::U32);
    assert!(
        matches!(
            operand,
            CheckedScalarExpression::StructuralParameterIndexedRead {
                primitive_type: PrimitiveType::U8,
                inner_indexes,
                element_path,
                ..
            } if inner_indexes.is_empty() && element_path.is_empty()
        ),
        "the view keeps the authored first-byte element read: {operand:?}"
    );
}
