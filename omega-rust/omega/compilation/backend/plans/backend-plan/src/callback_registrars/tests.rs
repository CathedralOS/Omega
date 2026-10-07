use super::CallbackRegistrarOccurrences;
use checked_trees::NominalMachineUseSite;
use lowered_psi::LoweredSourceCallOccurrence;
use symbols::SymbolHandle;

fn expression(generation: u32) -> NominalMachineUseSite {
    NominalMachineUseSite::Expression(checked_trees::expression::ExpressionHandle::from_parts(
        9, generation,
    ))
}

fn occurrence(
    site: Option<NominalMachineUseSite>,
    target: SymbolHandle,
    operation: u64,
) -> LoweredSourceCallOccurrence {
    LoweredSourceCallOccurrence {
        source_site: site,
        source_state: SymbolHandle::invalid(),
        statement_index: 0,
        call_ordinal: 0,
        terminal_site: semantic_vocabulary::BoundaryInvocationSite::Operation(
            semantic_vocabulary::OperationId::new(operation).unwrap(),
        ),
        source_target: target,
        source_values_before_call: Vec::new(),
    }
}

#[test]
fn registrar_keys_distinguish_site_kind_and_both_handle_generations() {
    let target = SymbolHandle::from_parts(3, 1);
    let next_target = SymbolHandle::from_parts(3, 2);
    let statement = NominalMachineUseSite::Statement(
        checked_trees::statement::StatementHandle::from_parts(9, 1),
    );
    let calls = [
        occurrence(Some(expression(1)), target, 1),
        occurrence(Some(expression(2)), target, 2),
        occurrence(Some(statement), target, 3),
        occurrence(Some(expression(1)), next_target, 4),
    ];
    let joined = CallbackRegistrarOccurrences::new(
        calls
            .iter()
            .rev()
            .map(|call| (call.source_site.unwrap(), call.source_target)),
        &calls,
    );
    for call in &calls {
        let found = joined
            .find(call.source_site.unwrap(), call.source_target)
            .unwrap();
        assert!(std::ptr::eq(found, call));
    }
    assert_eq!(joined.find(expression(3), target), Err(0));
    assert_eq!(
        joined.find(expression(1), SymbolHandle::from_parts(3, 3)),
        Err(0)
    );
}

#[test]
fn registrar_lookup_counts_all_matches_but_only_for_requested_sites() {
    let target = SymbolHandle::from_parts(3, 1);
    let calls = [
        occurrence(Some(expression(1)), target, 1),
        occurrence(None, target, 2),
        occurrence(Some(expression(1)), target, 3),
        occurrence(Some(expression(2)), target, 4),
        occurrence(Some(expression(3)), target, 5),
    ];
    let joined = CallbackRegistrarOccurrences::new(
        [
            (expression(1), target),
            (expression(1), target),
            (expression(2), target),
            (expression(4), target),
        ]
        .into_iter(),
        &calls,
    );
    // Duplicate requested placements share one lookup, not duplicate counts.
    // A later unique call can be queried before the earlier ambiguous call;
    // the producer decides when to surface each failure while producing bodies.
    assert!(std::ptr::eq(
        joined.find(expression(2), target).unwrap(),
        &calls[3]
    ));
    assert_eq!(joined.find(expression(1), target), Err(2));
    assert_eq!(joined.find(expression(4), target), Err(0));
    assert_eq!(joined.find(expression(3), target), Err(0));
    let empty = CallbackRegistrarOccurrences::new(std::iter::empty(), &calls);
    assert_eq!(empty.find(expression(1), target), Err(0));
}
