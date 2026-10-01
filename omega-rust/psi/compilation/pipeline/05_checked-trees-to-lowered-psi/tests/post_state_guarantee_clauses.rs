//! Every fact of an authored `ensures` clause is either published with an
//! independently reconstructed exit proof or deliberately left to its owner.
//! A clause listing several facts publishes their conjunction, a guarantee
//! over a `Wrapping` field keeps the field's own arithmetic, and a named
//! borrowed parameter's post-state is a caller fact exactly as the receiver's
//! is. Changing the stored value an exit proof reads must fail verification.

use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};
use lowered_psi::LoweredPsi;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerValue, Proposition};
use terminal_psi::{OperationKind, TerminalModule};

fn lower(source: &str) -> LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .unwrap_or_else(|error| panic!("post-state guarantees lower: {error:?}"));
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("every published guarantee has a reconstructed exit proof");
    lowered
}

/// The conjuncts every published `ensures` roster carries, in order.
fn published_guarantees(module: &TerminalModule) -> Vec<Proposition> {
    let mut conjuncts = Vec::new();
    for clause in module
        .machines
        .iter()
        .flat_map(|machine| &machine.contract.ensures)
    {
        match &clause.proposition {
            Proposition::Conjunction(children) => conjuncts.extend(children.iter().cloned()),
            other => conjuncts.push(other.clone()),
        }
    }
    conjuncts
}

/// Replace every unsigned integer constant `from` with `to` and require that
/// the bundle no longer verifies: the exit proofs read the stored values.
fn assert_changed_store_rejects(lowered: &LoweredPsi, from: u128, to: u128) {
    let mut substituted = lowered.semantic_module.clone();
    let mut changed = 0;
    for operation in substituted
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
    {
        if operation.kind
            == (OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(from),
            })
        {
            operation.kind = OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(to),
            };
            changed += 1;
        }
    }
    assert!(changed > 0, "the fixture stores {from}");
    assert!(
        terminal_verifier::verify_module(
            &substituted,
            &lowered.proof_bundle,
            &AdmissionProfile::default()
        )
        .is_err(),
        "an authored guarantee cannot justify a different stored value"
    );
}

const COUNTER: &str = "data Counter { slots: [i32; 4]; count: u64; }
machine Counter::clear(&mut self) ensures self.count < self.slots.len { self.count = 0; }
";

#[test]
fn a_fact_list_publishes_each_authored_fact() {
    let lowered = lower(&format!(
        "{COUNTER}
machine Counter::bump(&mut self)
requires self.count < self.slots.len
ensures self.count > 0; self.count <= self.slots.len
{{ self.count = self.count + 1; }}
machine Counter::release(&mut self)
requires self.count > 0; self.count <= self.slots.len
{{ self.count = self.count - 1; }}
data Main {{ counter: Counter; }}
machine Main::main(&mut self) {{
    self.counter.clear();
    self.counter.bump();
    self.counter.release();
}}"
    ));
    // `clear` publishes one fact and `bump` both of its listed facts.
    assert_eq!(published_guarantees(&lowered.semantic_module).len(), 3);
    assert_changed_store_rejects(&lowered, 0, 9);
}

const RING: &str = "data Ring { cells: [i32; 4]; used: u64 in Wrapping; }
machine Ring::clear(&mut self) ensures self.used < self.cells.len { self.used = 0; }
";

#[test]
fn a_wrapping_field_guarantee_keeps_the_field_arithmetic() {
    let lowered = lower(&format!(
        "{RING}
machine Ring::fill(&mut self)
requires self.used < self.cells.len
ensures self.used > 0; self.used - 1 < self.cells.len
{{ self.used = self.used + 1; }}
machine Ring::last(&self) -> i32
requires self.used > 0; self.used - 1 < self.cells.len
{{
    let slots: &[i32] = self.cells.as_slice();
    let index: u64 in Wrapping = self.used - 1;
    slots[index]
}}
data Main {{ ring: Ring; }}
machine Main::main(&mut self) {{
    self.ring.clear();
    self.ring.fill();
    let value: i32 in Wrapping = self.ring.last();
}}"
    ));
    let published = published_guarantees(&lowered.semantic_module);
    assert_eq!(published.len(), 3);
    assert!(
        published
            .iter()
            .any(|proposition| format!("{proposition:?}").contains("WrappingIntegerSubtract")),
        "the guarantee keeps the field's wrapping subtraction: {published:?}"
    );
    assert_changed_store_rejects(&lowered, 0, 4);
}

#[test]
fn a_named_borrowed_parameter_guarantee_reaches_the_caller() {
    let lowered = lower(
        "data Folder { depth: u32; }
data Picker { limit: [i32; 4]; picks: u64; }
machine Picker::take(&mut self) requires self.picks < self.limit.len { self.picks = self.picks + 1; }
data Scope {}
machine Scope::pick(picker: &mut Picker) -> Folder
ensures picker.picks < picker.limit.len
{
    picker.picks = 0;
    Folder { depth: 1 }
}
data Main { picker: Picker; }
machine Main::main(&mut self) {
    let folder: Folder = Scope::pick(&mut self.picker);
    self.picker.take();
}",
    );
    assert_eq!(published_guarantees(&lowered.semantic_module).len(), 1);
    assert_changed_store_rejects(&lowered, 0, 7);
}
