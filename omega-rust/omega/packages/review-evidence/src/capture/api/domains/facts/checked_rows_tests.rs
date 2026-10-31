//! The production requirement against its literal original whole-arena scan.
use super::*;
use facts::evidence::FactTable;
use facts::{DataDefinitionFactRecord, Fact, FactPayload, ProgramPoint};
use typed_trees::{TypedTrees, domain::ProofFact, expression::ExpressionNode};

fn original_require<'a, R: 'a>(
    typed: &TypedTrees,
    semantic_facts: &FactTable,
    symbol: SymbolHandle,
    fact_handle: arena::Handle<typed_trees::domain::ProofFact>,
    identity: &PackageReviewNominalIdentity,
    kind: &'static str,
    noun: &'static str,
    origin: impl Fn(SymbolHandle) -> facts::FactOrigin,
    retained_facts: impl Iterator<Item = &'a R>,
    record_fields: impl Fn(
        &R,
    ) -> (
        SymbolHandle,
        arena::Handle<typed_trees::domain::ProofFact>,
        facts::FactHandle,
    ),
) -> Result<(), Vec<Diagnostic>> {
    let point = facts::ProgramPoint::Definition { symbol };
    let matching_rows = semantic_facts
        .iter()
        .filter_map(|(handle, fact)| {
            (fact.point == point
                && fact.origin == origin(symbol)
                && fact.evidence == facts::QualificationEvidence::default()
                && semantic_fact_matches_definition_fact(typed, fact, fact_handle))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    if matching_rows.len() != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public {kind} `{}` {noun} fact has {} exact checked definition rows; expected one",
            identity.path,
            matching_rows.len()
        ))]);
    }
    let retained_records = retained_facts
        .filter(|record| {
            let (record_symbol, record_fact, _) = record_fields(record);
            record_symbol == symbol && record_fact == fact_handle
        })
        .collect::<Vec<_>>();
    let matching_records = retained_records
        .iter()
        .filter(|record| record_fields(record).2 == matching_rows[0])
        .count();
    if retained_records.len() != 1 || matching_records != 1 {
        return Err(vec![Diagnostic::error(format!(
            "public {kind} `{}` {noun} fact has {matching_records} exact checked ownership records among {} retained records; expected exactly one retained record",
            identity.path,
            retained_records.len(),
        ))]);
    }
    Ok(())
}

fn messages(result: Result<(), Vec<Diagnostic>>) -> Result<(), Vec<String>> {
    result.map_err(|errors| errors.into_iter().map(|error| error.message).collect())
}

fn compare(
    typed: &TypedTrees,
    rows: &FactTable,
    records: &[DataDefinitionFactRecord],
    symbol: SymbolHandle,
    fact: arena::Handle<ProofFact>,
) -> Result<(), Vec<String>> {
    let identity = PackageReviewNominalIdentity {
        owner: crate::record::PackageReviewNominalOwner::Unresolved,
        path: "RawCheckedData".to_owned(),
    };
    let fields =
        |record: &DataDefinitionFactRecord| (record.data_symbol, record.fact, record.semantic_fact);
    let actual = messages(require_exact_checked_fact(
        typed,
        rows,
        symbol,
        fact,
        &identity,
        "data",
        "invariant",
        |data_symbol| facts::FactOrigin::DataDefinition { data_symbol },
        records.iter(),
        fields,
    ));
    let original = messages(original_require(
        typed,
        rows,
        symbol,
        fact,
        &identity,
        "data",
        "invariant",
        |data_symbol| facts::FactOrigin::DataDefinition { data_symbol },
        records.iter(),
        fields,
    ));
    assert_eq!(actual, original);
    actual
}

#[test]
fn exact_checked_facts_keep_original_rows_ownership_and_refusal_order() {
    let mut typed = TypedTrees::default();
    let expression = typed.expression_table.insert(ExpressionNode::Boolean(true));
    let other = typed
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let proof = typed.proof_facts.append(ProofFact::Expression(expression));
    let different_proof = typed.proof_facts.append(ProofFact::Expression(other));
    let symbol = SymbolHandle::from_parts(7, 3);
    let newer = SymbolHandle::from_parts(7, 4);
    for symbol in [symbol, newer, SymbolHandle::invalid()] {
        let expected = Fact {
            point: ProgramPoint::Definition { symbol },
            origin: facts::FactOrigin::DataDefinition {
                data_symbol: symbol,
            },
            payload: FactPayload::BooleanExpression(expression),
            ..Default::default()
        };
        let mut rows = FactTable::default();
        // Deliberately no contexts/refs: the raw table, not context membership,
        // is the original authority for complete row selection at this point.
        let handle = rows.append(expected);
        let record = DataDefinitionFactRecord {
            data_symbol: symbol,
            fact: proof,
            semantic_fact: handle,
            ..Default::default()
        };
        assert!(compare(&typed, &rows, &[record.clone()], symbol, proof).is_ok());
        // Each residual predicate remains active even inside the selected bucket.
        let mut wrong = expected;
        wrong.origin = facts::FactOrigin::Unknown;
        rows.append(wrong);
        wrong = expected;
        wrong.evidence.source_symbol = newer;
        rows.append(wrong);
        wrong = expected;
        wrong.payload = FactPayload::BooleanExpression(other);
        rows.append(wrong);
        wrong = expected;
        wrong.point = ProgramPoint::Global;
        let elsewhere = rows.append(wrong);
        assert!(compare(&typed, &rows, &[record.clone()], symbol, proof).is_ok());

        let missing = compare(&typed, &FactTable::default(), &[], symbol, proof).unwrap_err();
        assert!(missing[0].contains("0 exact checked definition rows; expected one"));
        let owner_missing = compare(&typed, &rows, &[], symbol, proof).unwrap_err();
        assert!(owner_missing[0].contains("0 exact checked ownership records among 0 retained"));
        let mut wrong_owner = record.clone();
        wrong_owner.semantic_fact = elsewhere;
        let wrong = compare(&typed, &rows, &[wrong_owner], symbol, proof).unwrap_err();
        assert!(wrong[0].contains("0 exact checked ownership records among 1 retained"));
        let mut wrong_generation = record.clone();
        wrong_generation.semantic_fact =
            arena::Handle::from_parts(handle.arena_index(), handle.generation() + 1);
        assert!(
            compare(&typed, &rows, &[wrong_generation], symbol, proof).unwrap_err()[0]
                .contains("0 exact checked ownership records among 1 retained")
        );
        let duplicated_owner = compare(
            &typed,
            &rows,
            &[record.clone(), record.clone()],
            symbol,
            proof,
        )
        .unwrap_err();
        assert!(duplicated_owner[0].contains("2 exact checked ownership records among 2 retained"));
        let mut other_fact = record.clone();
        other_fact.fact = different_proof;
        assert!(
            compare(&typed, &rows, &[other_fact], symbol, proof).unwrap_err()[0]
                .contains("among 0 retained")
        );
        rows.append(expected);
        // Semantic duplicate failure precedes absent/wrong ownership failure.
        let duplicate = compare(&typed, &rows, &[], symbol, proof).unwrap_err();
        assert!(duplicate[0].contains("2 exact checked definition rows; expected one"));
    }
}
