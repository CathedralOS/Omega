use super::canonical_receiver_place_for_call_site;
use crate::flow::canonical_place_from_symbol;
use crate::semantic_calls::{CallSite, find_call_site};
use crate::tests::front_end::typed_program;

/// `carrier.context.increment_counter()` is a projected statement receiver:
/// `receiver_root_symbol` names the `carrier` parameter and `receiver_symbol`
/// the projected `context` field, so the canonical place is the parameter
/// root plus the field segment — reachable only through the statement's
/// recorded position. `_ = helper(1)` is a distinct call statement and the
/// tail `0` is not a call, giving coordinates that must decline the projected
/// derivation.
const SOURCE: &str = "data Context { counter: u64; }
    data Carrier { context: &mut Context; }
    machine Context::increment_counter(&mut self) -> u64 { self.counter = 1; 0 }
    machine helper(value: u64) -> u64 { value }
    machine inspect(mut carrier: Carrier) -> u64 {
        _ = carrier.context.increment_counter();
        _ = helper(1);
        0
    }";

fn typed(source: &str) -> typed_trees::TypedTrees {
    let mut program = typed_program(source);
    crate::lookup::resolve_projected_receiver_calls(&mut program)
        .expect("projected receivers resolve");
    program
}

/// The projected-receiver scope boundary is the occurrence's recorded
/// statement coordinate. A call site rebuilt from a copied row — the same
/// payload at the same recorded coordinate, without the arena address —
/// resolves the identical place. Before the bound moved to recorded identity,
/// the copy could not be located by pointer equality and fell back to a bare
/// `receiver_symbol` place that drops the `carrier` root.
#[test]
fn a_replayed_call_site_resolves_the_same_projected_receiver_place() {
    let program = typed(SOURCE);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .expect("caller machine");
    let state = &program.machine_states(machine)[0];
    let site = find_call_site(&program, machine.symbol, state.symbol, 0, 0)
        .expect("the projected statement call site");
    let CallSite::Statement(call) = site else {
        panic!("statement call site")
    };
    let carrier = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "carrier")
        .expect("carrier parameter")
        .symbol;

    let projected =
        canonical_receiver_place_for_call_site(&program, machine.symbol, state.symbol, &site, 0)
            .expect("projected receiver place");
    assert!(
        matches!(projected.root, facts::PlaceRoot::Symbol(root) if root == carrier),
        "the projected receiver keeps the parameter root: {projected:?}"
    );
    assert_eq!(
        projected.segments.as_slice(),
        [facts::PlaceSegment::Field {
            symbol: call.receiver_symbol
        }],
        "the projected receiver keeps the exact field segment"
    );
    // The fixture is only meaningful while the projected place differs from
    // the receiver-symbol fallback — keep that separation explicit.
    let fallback = canonical_place_from_symbol(call.receiver_symbol);
    assert_ne!(Some(projected.clone()), fallback);

    // Replay: an identical payload at the same recorded coordinate resolves
    // the identical place.
    let copied = call.clone();
    let replayed = CallSite::Statement(&copied);
    assert_eq!(
        canonical_receiver_place_for_call_site(
            &program,
            machine.symbol,
            state.symbol,
            &replayed,
            0
        ),
        Some(projected.clone()),
    );

    // A coordinate that names a different statement cannot borrow the
    // recorded scope: index 1 is a different call, index 2 is not a call.
    for wrong_index in [1usize, 2] {
        assert_eq!(
            canonical_receiver_place_for_call_site(
                &program,
                machine.symbol,
                state.symbol,
                &site,
                wrong_index,
            ),
            fallback,
            "statement index {wrong_index} declines the projected derivation"
        );
    }
}
