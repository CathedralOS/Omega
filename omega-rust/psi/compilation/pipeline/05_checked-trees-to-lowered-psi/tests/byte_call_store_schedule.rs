//! A byte-result consumer completes its exact call occurrence, not a second
//! statement. Equal-capacity results cannot replace one another in that pair.

use checked_trees::{CheckedByteSequenceValue, CheckedUnitEffectOperationPlan};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

const SOURCE: &str = r#"
    domain [u8; 16]::Utf8 requires valid_utf8(self);
    data Holder { text: [u8; 16] in Utf8; marker: u64; }
    machine label(short: bool) -> [u8; 16] in Utf8 {
        transition short { true -> "Gate" false -> "Branch Room" }
    }
    machine fill(holder: &mut Holder) {
        holder.marker = 1;
        holder.text = label(false);
        holder.text = label(true);
        holder.marker = 2;
    }
"#;

#[test]
fn ordinary_byte_call_store_schedule_keeps_exact_completed_results() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("fill"))
        .expect("ordinary call and byte store compose between scalar stores");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("completed owned byte results retain their predicates and bounds");
    let fill = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "fill")
        .expect("authored fill")
        .symbol;
    let plan_index = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .position(|plan| plan.machine == fill)
        .expect("straight-line fill uses the ordinary operation schedule");
    let operations = &checked.facts.flow.terminal_unit_effects.machines[plan_index].operations;
    let stores = operations
        .iter()
        .enumerate()
        .filter_map(|(position, operation)| match operation {
            CheckedUnitEffectOperationPlan::ByteSequenceStore(store) => {
                let CheckedByteSequenceValue::CallResult {
                    binding_ordinal, ..
                } = store.value.nodes.get(store.value.root)
                else {
                    panic!("byte store observes its completed call");
                };
                Some((position, *binding_ordinal))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 2);
    assert_ne!(stores[0].1, stores[1].1);
    for corruption in [
        "earlier binding",
        "consumer before producer",
        "duplicate consumer",
        "reordered call pairs",
    ] {
        let mut changed = checked.clone();
        let plan = &mut changed.facts.flow.terminal_unit_effects.machines[plan_index];
        let operations = &mut plan.operations;
        match corruption {
            "consumer before producer" => operations.swap(stores[1].0 - 1, stores[1].0),
            "duplicate consumer" => {
                operations.insert(stores[1].0 + 1, operations[stores[1].0].clone());
            }
            "reordered call pairs" => {
                operations.swap(stores[0].0 - 1, stores[1].0 - 1);
                operations.swap(stores[0].0, stores[1].0);
            }
            "earlier binding" => {
                let CheckedUnitEffectOperationPlan::ByteSequenceStore(store) =
                    &mut operations[stores[1].0]
                else {
                    panic!("second consumer");
                };
                let CheckedByteSequenceValue::CallResult {
                    binding_ordinal, ..
                } = store.value.nodes.get_mut(store.value.root)
                else {
                    panic!("completed source");
                };
                *binding_ordinal = stores[0].1;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, TerminalMachineSelection::Name("fill")).is_err(),
            "reject {corruption} without accepting a repeated coordinate"
        );
    }
}
