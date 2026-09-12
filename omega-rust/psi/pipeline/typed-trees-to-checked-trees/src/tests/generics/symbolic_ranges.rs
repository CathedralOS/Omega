use super::typed_source;
use crate::lower_typed_trees;
use typed_trees::statement::StatementNode;

#[test]
fn discarded_calls_in_open_templates_validate_inferred_const_bounds() {
    for (length, accepted) in [(2, true), (5, false)] {
        let source = format!("machine endpoint<const N: u64[0..=3]>(witness: &[u8; N]) -> u64 {{ N }}
            machine forward<Value>(unused: Value, witness: &[u8; {length}]) {{ _ = endpoint(witness); }}");
        let typed = typed_source(&source).expect("discarded generic call types");
        let result = lower_typed_trees(typed);
        assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        if let Ok(checked) = result {
            assert!(
                checked.machine_specializations.is_empty(),
                "the unused open caller remains generic"
            );
        }
    }
}

#[test]
fn open_mutable_symbolic_parameter_stores_preserve_the_declared_range() {
    let source = "machine store<const N: u64>(mut value: u64[0..=N]) { value = N; value = 0; }";
    let typed = typed_source(source).expect("symbolic mutable parameter types");
    assert_eq!(typed.machine_type_parameters(&typed.machines()[0]).len(), 1);
    let checked =
        lower_typed_trees(typed).expect("both assignments satisfy the declared symbolic range");
    assert!(checked.machine_specializations.is_empty());
}

#[test]
fn unrelated_const_cannot_enter_a_mutable_symbolic_parameter_range() {
    let source = "machine store<const N: u64, const M: u64>(mut value: u64[0..=N]) { value = M; }";
    let typed = typed_source(source).expect("distinct symbolic binder types");
    let errors = lower_typed_trees(typed).expect_err("M has no proof that it is at most N");
    assert!(
        errors.iter().any(|error| error.message.contains("range")),
        "{errors:?}"
    );
}

#[test]
fn explicit_const_forwarding_uses_the_callers_declared_bound() {
    let source = "machine endpoint<const N: u64[0..=3]>() -> u64[0..=3] { N }
        machine forward<const K: u64[0..=3]>() -> u64[0..=3] { endpoint<K>() }";
    let checked = lower_typed_trees(typed_source(source).expect("bounded forwarding types"))
        .expect("K supplies the complete declared interval required by N");
    assert!(checked.machine_specializations.is_empty());
}

#[test]
fn unrestricted_const_forwarding_cannot_borrow_the_callees_bound() {
    let source = "machine endpoint<const N: u64[0..=3]>() -> u64[0..=3] { N }
        machine forward<const K: u64>() -> u64[0..=3] { endpoint<K>() }";
    let errors = lower_typed_trees(typed_source(source).expect("unrestricted forwarding types"))
        .expect_err("the callee's declaration cannot prove the caller's unrestricted K");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "const parameter `N` of `endpoint` requires `u64[1 constraint]`, but `K` declares `u64`"),
        "{errors:?}"
    );
}

#[test]
fn authored_destination_cannot_narrow_an_explicit_generic_call_result() {
    let source = "machine endpoint<const N: u64>() -> u64[0..=N] { N }
        machine forward<Value>(unused: Value) -> u64 {
            let value: u64[0..=2] = endpoint<3>();
            value
        }";
    let errors = lower_typed_trees(typed_source(source).expect("authored result annotation types"))
        .expect_err("endpoint<3> does not promise the destination's upper bound 2");
    assert!(
        errors.iter().any(|error| error.message.contains("range")),
        "{errors:?}"
    );
}

#[test]
fn forged_inferred_destination_is_not_source_call_range_evidence() {
    let source = "machine endpoint<const N: u64>() -> u64[0..=N] { N }
        machine forward<Value>(unused: Value) -> u64[0..=2] { endpoint<3>() }";
    let mut typed = typed_source(source).expect("inferred result types");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "forward")
        .expect("open caller");
    let state = typed.machine_states(machine)[0].clone();
    let mut forged = false;
    for statement in typed.statement_table.statements_mut(state.statement_nodes) {
        if let StatementNode::LocalData(local) = statement
            && local.type_is_inferred
        {
            local.type_reference = state.return_type;
            forged = true;
        }
    }
    assert!(forged, "the source retains an inferred tail-call local");
    let errors = lower_typed_trees(typed)
        .expect_err("a forged inferred local cannot prove endpoint<3> returns at most 2");
    assert!(
        errors.iter().any(|error| error.message.contains("range")),
        "{errors:?}"
    );
}
