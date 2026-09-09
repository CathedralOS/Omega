use super::*;
use semantic_vocabulary::BoundedIntegerType;
use terminal_codec::{
    build_terminal_obligation_ledger, current_terminal_trust_graph,
    decode_terminal_obligation_ledger, encode_terminal_obligation_ledger,
    validate_terminal_obligation_ledger,
};
use terminal_psi::{ScalarRangeInvariant, ScalarRangeInvariantArrival};
use terminal_verifier::ReconstructedTerminalObligationOwner;

fn invariant_fixture() -> TerminalModule {
    let mut module = ranked_countdown_fixture();
    module.scalar_range_invariants = vec![ScalarRangeInvariant {
        machine: module.machines[0].id,
        header: block_id(901),
        parameter: value_id(902),
        bounds: BoundedIntegerType::new(
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(u128::from(u32::MAX)),
        )
        .unwrap(),
        arrivals: vec![
            ScalarRangeInvariantArrival {
                edge: edge_id(900),
                obligation: obligation_id(910),
            },
            ScalarRangeInvariantArrival {
                edge: edge_id(903),
                obligation: obligation_id(911),
            },
        ],
    }];
    module
}

#[test]
fn scalar_range_invariant_semantics_and_independent_ledger_reload() {
    let module = invariant_fixture();
    let semantic_bytes =
        encode_module(&module).expect("invariant module encodes without certificates");
    let reloaded = decode_module(&semantic_bytes).expect("invariant semantic reload");
    assert_eq!(reloaded, module);
    assert_eq!(encode_module(&reloaded).unwrap(), semantic_bytes);

    let trust_graph = current_terminal_trust_graph().unwrap();
    let ledger = build_terminal_obligation_ledger(&reloaded, &trust_graph)
        .expect("reconstruct questions without a proof bundle");
    let arrivals: Vec<_> = ledger
        .obligations()
        .iter()
        .filter_map(|row| match row.owner {
            ReconstructedTerminalObligationOwner::ScalarRangeInvariant {
                machine,
                header,
                parameter,
                edge,
            } => Some((machine, header, parameter, edge, row.obligation.id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        arrivals,
        vec![
            (
                module.machines[0].id,
                block_id(901),
                value_id(902),
                edge_id(900),
                obligation_id(910)
            ),
            (
                module.machines[0].id,
                block_id(901),
                value_id(902),
                edge_id(903),
                obligation_id(911)
            ),
        ]
    );
    let ledger_bytes = encode_terminal_obligation_ledger(&ledger).unwrap();
    let reloaded_ledger = decode_terminal_obligation_ledger(&ledger_bytes).unwrap();
    assert_eq!(reloaded_ledger, ledger);
    validate_terminal_obligation_ledger(&reloaded_ledger, &reloaded, &trust_graph).unwrap();

    let mut changed = reloaded;
    changed.scalar_range_invariants[0].arrivals[1].obligation = obligation_id(912);
    assert_ne!(
        semantic_fingerprint(&changed).unwrap(),
        semantic_fingerprint(&module).unwrap()
    );
    assert_eq!(
        validate_terminal_obligation_ledger(&reloaded_ledger, &changed, &trust_graph),
        Err(CodecError::ObligationLedgerMismatch)
    );

    let mut stale = semantic_bytes;
    stale[8..10].copy_from_slice(&86_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale),
        Err(CodecError::UnsupportedFormatMarker(86))
    );
    let mut stale = ledger_bytes;
    stale[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_terminal_obligation_ledger(&stale),
        Err(CodecError::UnsupportedFormatMarker(1))
    );
}

#[test]
fn scalar_range_invariant_roster_rejects_noncanonical_or_incomplete_arrivals() {
    let original = invariant_fixture();
    let mut reordered = original.clone();
    reordered.scalar_range_invariants[0].arrivals.reverse();
    assert!(encode_module(&reordered).is_err());
    let mut duplicate = original.clone();
    duplicate
        .scalar_range_invariants
        .push(duplicate.scalar_range_invariants[0].clone());
    assert!(encode_module(&duplicate).is_err());
    let mut missing = original.clone();
    missing.scalar_range_invariants[0].arrivals.pop();
    assert!(encode_module(&missing).is_err());
    let mut wrong_parameter = original;
    wrong_parameter.scalar_range_invariants[0].parameter = value_id(906);
    assert!(encode_module(&wrong_parameter).is_err());
}
