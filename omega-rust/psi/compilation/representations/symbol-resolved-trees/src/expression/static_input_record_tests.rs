use crate::expression::StaticMachineArgument;
use compiler_storage::{RecordLimits, decode, encode};
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
    AuthoredDeclarationSelections,
};
use symbols::SymbolHandle;

#[test]
fn exact_static_input_attachment_round_trips_and_refuses_the_old_marker() {
    let selected = SymbolHandle::from_parts(3, 5);
    let mut ledger = AuthoredDeclarationSelections::default();
    let occurrence = ledger
        .record_resolved(
            source::SourceSpan::new(source::SourceId(0), source::Span::new(1, 2)),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::StaticArgument,
            selected,
        )
        .unwrap();
    let argument = StaticMachineArgument {
        authored_selection: Some(occurrence),
        authored_selection_target: Some(selected),
        symbol: selected,
        signature_free_requirement: Some(Ok(crate::expression::ResolvedSignatureFreeRequirement {
            trait_definition: SymbolHandle::from_parts(4, 5),
            declaring_trait: SymbolHandle::from_parts(6, 5),
            requirement: selected,
        })),
        ..Default::default()
    };
    let mut bytes = encode(&argument, RecordLimits::default()).unwrap();
    assert_eq!(
        decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).unwrap(),
        argument
    );
    bytes[8] ^= 1;
    assert!(decode::<StaticMachineArgument>(&bytes, RecordLimits::default()).is_err());
}
