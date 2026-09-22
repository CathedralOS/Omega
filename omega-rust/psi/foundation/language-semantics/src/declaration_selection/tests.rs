//! Declaration selection tests.

use super::AuthoredDeclarationSelectionIntrinsic;
use super::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionFinalizationError,
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionLateBinding,
    AuthoredDeclarationSelectionOccurrenceId, AuthoredDeclarationSelectionRecordError,
    AuthoredDeclarationSelectionSuffixRebaseError, AuthoredDeclarationSelections, BuildOperation,
    CollectionMeasure, CollectionViewOperation, CompilerDerivedSelectionPartition, SourceSpan,
    SymbolHandle,
};
use source::{SourceId, Span};

fn source_span(start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(SourceId(7), Span::new(start, end))
}

fn record_fixture(selections: &mut AuthoredDeclarationSelections) {
    selections
        .record_resolved(
            source_span(2, 6),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(3),
        )
        .expect("valid resolved selection");
    selections
        .record_late_bound(
            source_span(2, 6),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        )
        .expect("ledger capacity");
}

#[test]
fn occurrence_identities_are_unique_and_deterministic() {
    let mut first_run = AuthoredDeclarationSelections::default();
    let mut second_run = AuthoredDeclarationSelections::default();
    record_fixture(&mut first_run);
    record_fixture(&mut second_run);

    let first_ids = first_run
        .iter()
        .map(|selection| selection.occurrence_id())
        .collect::<Vec<_>>();
    let second_ids = second_run
        .iter()
        .map(|selection| selection.occurrence_id())
        .collect::<Vec<_>>();

    assert_eq!(first_ids, second_ids);
    assert_eq!(first_ids.len(), 2);
    assert_ne!(first_ids[0], first_ids[1]);
    assert_eq!(first_ids[0].ordinal(), 0);
    assert_eq!(first_ids[1].ordinal(), 1);
    assert_eq!(first_run.get(first_ids[0]), first_run.as_slice().first());
    assert_eq!(first_run.get(first_ids[1]), first_run.as_slice().get(1));
}

#[test]
fn compiler_partitions_separate_applications_without_replacing_source_custody() {
    let mut selections = AuthoredDeclarationSelections::default();
    let source = source_span(12, 16);
    let first_partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(3);
    let second_partition = CompilerDerivedSelectionPartition::from_compiler_ordinal(7);
    let first = selections
        .record_resolved_in_partition(
            source,
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            Some(first_partition),
            SymbolHandle::from_arena_index(4),
        )
        .expect("first application");
    let second = selections
        .record_resolved_in_partition(
            source,
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            Some(second_partition),
            SymbolHandle::from_arena_index(8),
        )
        .expect("second application");

    assert_ne!(first, second);
    assert_eq!(selections.get(first).unwrap().source_span(), source);
    assert_eq!(selections.get(second).unwrap().source_span(), source);
    assert_eq!(
        selections.get(first).unwrap().compiler_partition(),
        Some(first_partition)
    );
    assert_eq!(
        selections.get(second).unwrap().compiler_partition(),
        Some(second_partition)
    );
}

#[test]
fn suffix_rebase_preserves_destination_prefix_and_shifts_only_extension_rows() {
    let mut combined = AuthoredDeclarationSelections::default();
    combined
        .record_late_bound(
            source_span(1, 2),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        )
        .expect("base row");
    let mut destination = combined.clone();
    destination
        .finalize_late_bound(
            AuthoredDeclarationSelectionOccurrenceId(0),
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
            SymbolHandle::from_arena_index(4),
        )
        .expect("typed base finalization");
    destination
        .record_resolved(
            source_span(3, 4),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(5),
        )
        .expect("typed-only base row");
    let extension = combined
        .record_resolved(
            source_span(5, 6),
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::from_arena_index(6),
        )
        .expect("extension row");

    let (joined, rebase) = combined
        .replace_prefix_and_rebase_suffix(1, &destination)
        .expect("compatible retained prefix");

    assert_eq!(
        &joined.as_slice()[..destination.len()],
        destination.as_slice()
    );
    assert_eq!(joined.len(), 3);
    assert_eq!(joined.as_slice()[2].occurrence_id().ordinal(), 2);
    assert_eq!(
        rebase.rebase_extension(extension).map(|id| id.ordinal()),
        Some(2)
    );
    assert_eq!(
        rebase.retain_base(AuthoredDeclarationSelectionOccurrenceId(0)),
        Some(AuthoredDeclarationSelectionOccurrenceId(0))
    );
    assert_eq!(
        rebase.retain_base(extension),
        None,
        "extension identity cannot be laundered through a base site"
    );
}

#[test]
fn suffix_rebase_rejects_prefix_identity_and_target_tampering() {
    let mut combined = AuthoredDeclarationSelections::default();
    combined
        .record_resolved(
            source_span(1, 2),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(4),
        )
        .expect("base row");

    let mut identity_tamper = AuthoredDeclarationSelections::default();
    identity_tamper
        .record_resolved(
            source_span(9, 10),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(4),
        )
        .expect("tampered row");
    assert_eq!(
        combined.replace_prefix_and_rebase_suffix(1, &identity_tamper),
        Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch)
    );

    let mut target_tamper = AuthoredDeclarationSelections::default();
    target_tamper
        .record_resolved(
            source_span(1, 2),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            SymbolHandle::from_arena_index(9),
        )
        .expect("retargeted row");
    assert_eq!(
        combined.replace_prefix_and_rebase_suffix(1, &target_tamper),
        Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixTargetMismatch)
    );

    let mut partitioned = AuthoredDeclarationSelections::default();
    partitioned
        .record_resolved_in_partition(
            source_span(1, 2),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            Some(CompilerDerivedSelectionPartition::from_compiler_ordinal(3)),
            SymbolHandle::from_arena_index(4),
        )
        .expect("partitioned row");
    assert_eq!(
        combined.replace_prefix_and_rebase_suffix(1, &partitioned),
        Err(AuthoredDeclarationSelectionSuffixRebaseError::PrefixIdentityMismatch),
        "compiler application custody cannot be changed while rebasing a retained row"
    );
}

#[test]
fn invalid_resolved_target_fails_closed_without_consuming_an_identity() {
    let mut selections = AuthoredDeclarationSelections::default();

    let error = selections
        .record_resolved(
            source_span(4, 9),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::invalid(),
        )
        .expect_err("invalid target must reject");

    assert_eq!(
        error,
        AuthoredDeclarationSelectionRecordError::InvalidSelectedSymbol
    );
    assert!(selections.is_empty());

    let first_valid = selections
        .record_late_bound(
            source_span(4, 9),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Call,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
        )
        .expect("ledger capacity");
    assert_eq!(first_valid.ordinal(), 0);
}

#[test]
fn late_finalization_is_exact_and_transactional() {
    let mut selections = AuthoredDeclarationSelections::default();
    let occurrence = selections
        .record_late_bound(
            source_span(9, 12),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Operator,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
        )
        .expect("record late operator");
    let before = selections.clone();

    assert_eq!(
        selections.finalize_late_bound(
            occurrence,
            AuthoredDeclarationSelectionLateBinding::CheckedCall,
            SymbolHandle::from_arena_index(7),
        ),
        Err(AuthoredDeclarationSelectionFinalizationError::LateBindingMismatch)
    );
    assert_eq!(selections, before);
    assert!(!selections.all_finalized());

    selections
        .finalize_late_bound(
            occurrence,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
            SymbolHandle::from_arena_index(7),
        )
        .expect("finalize exact operator occurrence");
    assert!(selections.all_finalized());
    assert_eq!(
        selections.finalize_late_bound(
            occurrence,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
            SymbolHandle::from_arena_index(8),
        ),
        Err(AuthoredDeclarationSelectionFinalizationError::AlreadyResolved)
    );
}

#[test]
fn collection_view_spellings_round_trip() {
    for operation in CollectionViewOperation::ALL {
        assert_eq!(
            CollectionViewOperation::from_authored_spelling(operation.authored_spelling()),
            Some(operation),
            "{} must select the operation it spells",
            operation.authored_spelling()
        );
    }
}

#[test]
fn collection_view_spellings_are_the_exact_authored_vocabulary() {
    assert_eq!(
        CollectionViewOperation::ALL.map(CollectionViewOperation::authored_spelling),
        ["as_slice", "as_mut_slice", "as_view", "bytes"]
    );
    for spelling in ["len", "capacity", "as_slices", "AS_SLICE", ""] {
        assert_eq!(
            CollectionViewOperation::from_authored_spelling(spelling),
            None,
            "`{spelling}` names no compiler-owned view"
        );
    }
}

#[test]
fn collection_measure_spellings_round_trip() {
    for measure in CollectionMeasure::ALL {
        assert_eq!(
            CollectionMeasure::from_authored_spelling(measure.authored_spelling()),
            Some(measure),
            "{} must select the measure it spells",
            measure.authored_spelling()
        );
    }
}

#[test]
fn collection_measure_spellings_are_the_exact_authored_vocabulary() {
    assert_eq!(
        CollectionMeasure::ALL.map(CollectionMeasure::authored_spelling),
        ["len", "capacity"]
    );
    for spelling in ["length", "Len", "LEN", "as_slice", "bytes", "cap", ""] {
        assert_eq!(
            CollectionMeasure::from_authored_spelling(spelling),
            None,
            "`{spelling}` names no compiler-owned measure"
        );
    }
}

#[test]
fn collection_measures_map_onto_their_intrinsic_selection_targets() {
    assert_eq!(
        CollectionMeasure::Length.intrinsic(),
        AuthoredDeclarationSelectionIntrinsic::CollectionLength
    );
    assert_eq!(
        CollectionMeasure::Capacity.intrinsic(),
        AuthoredDeclarationSelectionIntrinsic::CollectionCapacity
    );
}

#[test]
fn build_operation_spellings_round_trip() {
    for operation in BuildOperation::ALL {
        let target = if operation.carries_marker_operands() {
            format!(
                "{}{}pkg::symbol",
                operation.authored_spelling(),
                BuildOperation::MARKER_OPERAND_SEPARATOR
            )
        } else {
            operation.authored_spelling().to_owned()
        };
        assert_eq!(
            BuildOperation::from_call_target(&target),
            Some(operation),
            "`{target}` must select the operation it spells"
        );
    }
}

#[test]
fn build_operation_spellings_are_the_exact_authored_vocabulary() {
    assert_eq!(
        BuildOperation::ALL.map(BuildOperation::authored_spelling),
        [
            "select_provider",
            "select_representation",
            "exclude_service",
            "enable",
            "emit_report",
            "accept_boundary",
            "wire_compatibility",
            "include_source",
            "write_line",
        ]
    );
    assert_eq!(
        BuildOperation::ALL.map(BuildOperation::carries_marker_operands),
        [false, false, false, false, false, true, true, false, false]
    );
    assert_eq!(
        BuildOperation::from_call_target("accept_boundary#pkg::Console"),
        Some(BuildOperation::BoundaryAcceptance)
    );
    assert_eq!(
        BuildOperation::from_call_target("wire_compatibility#Edge#Lineage#Local#Peer#Readable"),
        Some(BuildOperation::WireCompatibilityRequest)
    );
    for spelling in [
        // A marker operation is never selected by its bare spelling, nor by
        // the authored method name the parser desugars, nor by a marker with
        // the wrong separator or a foreign spelling before it.
        "accept_boundary",
        "wire_compatibility",
        "require_wire_compatibility",
        "accept_boundary::Console",
        "select_provider#Console",
        "enable#Fast",
        // Neighbouring spellings from other vocabularies and near misses.
        "exclude_crash",
        "depend",
        "depend_as",
        "as_slice",
        "len",
        "select_providers",
        "Select_Provider",
        "SELECT_PROVIDER",
        "select_provider ",
        "write",
        "write_error_line",
        "",
    ] {
        assert_eq!(
            BuildOperation::from_call_target(spelling),
            None,
            "`{spelling}` names no toolchain-owned build operation"
        );
    }
}

#[test]
fn build_operations_map_onto_their_intrinsic_selection_targets() {
    use AuthoredDeclarationSelectionIntrinsic as Intrinsic;
    assert_eq!(
        BuildOperation::ALL.map(BuildOperation::intrinsic),
        [
            Intrinsic::BuildProviderSelection,
            Intrinsic::BuildRepresentationSelection,
            Intrinsic::BuildServiceExclusion,
            Intrinsic::BuildOptimizationSelection,
            Intrinsic::BuildOptimizationReportRequest,
            Intrinsic::BuildBoundaryAcceptance,
            Intrinsic::BuildWireCompatibilityRequest,
            Intrinsic::BuildIncludedSourceHandoff,
            Intrinsic::BuildLogWriteLine,
        ]
    );
}

#[test]
fn marker_operands_are_read_only_after_the_operation_spelling_and_separator() {
    use super::BuildOperation;
    assert_eq!(
        BuildOperation::BoundaryAcceptance.marker_operands("accept_boundary#pkg::Root"),
        Some("pkg::Root")
    );
    assert_eq!(
        BuildOperation::WireCompatibilityRequest.marker_operands("wire_compatibility#a#b"),
        Some("a#b")
    );
    assert_eq!(
        BuildOperation::BoundaryAcceptance.marker_operands("accept_boundary"),
        None
    );
    assert_eq!(
        BuildOperation::BoundaryAcceptance.marker_operands("wire_compatibility#x"),
        None
    );
    assert_eq!(
        BuildOperation::ProviderSelection.marker_operands("select_provider#x"),
        None
    );
}
