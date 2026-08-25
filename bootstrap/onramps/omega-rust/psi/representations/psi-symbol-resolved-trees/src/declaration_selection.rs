use psi_source::SourceSpan;
use psi_symbols::SymbolHandle;

/// Whether an authored declaration selection contributes only to an
/// implementation or is exposed through a package's published surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionExposure {
    PrivateImplementation,
    PublicInterface,
}

/// The authored syntax which selected a declaration.
///
/// These kinds describe source authority only. Compiler-planned layout,
/// movement, and automatic cleanup are semantic dependencies and do not belong
/// in this ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionKind {
    TypeReference,
    StaticPathSegment,
    MemberAccess,
    StructLiteralType,
    StructLiteralCase,
    StructLiteralField,
    CaseReference,
    CaseMembership,
    Call,
    Operator,
    Conformance,
    ExplicitCleanupCall,
}

/// The checked fact family which must supply a declaration selected too late
/// for symbol resolution to settle it.
///
/// A late-bound occurrence is explicit ledger state. It must not be encoded as
/// a resolved target containing an invalid symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionLateBinding {
    CheckedCall,
    CheckedOperator,
    CheckedConformance,
}

/// A symbol known to be valid when its authored selection row was recorded.
///
/// The private field prevents callers from constructing a resolved target with
/// `SymbolHandle::invalid()` and accidentally bypassing later finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAuthoredDeclarationSelection {
    selected_symbol: SymbolHandle,
}

impl ResolvedAuthoredDeclarationSelection {
    pub fn new(selected_symbol: SymbolHandle) -> Option<Self> {
        selected_symbol
            .is_valid()
            .then_some(Self { selected_symbol })
    }

    pub fn selected_symbol(self) -> SymbolHandle {
        self.selected_symbol
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredDeclarationSelectionTarget {
    Resolved(ResolvedAuthoredDeclarationSelection),
    LateBound(AuthoredDeclarationSelectionLateBinding),
}

/// One authored occurrence retained while its source location and exposure are
/// still exact. Package ownership is intentionally absent and is joined by a
/// later compiler integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDeclarationSelection {
    source_span: SourceSpan,
    exposure: AuthoredDeclarationSelectionExposure,
    kind: AuthoredDeclarationSelectionKind,
    target: AuthoredDeclarationSelectionTarget,
}

impl AuthoredDeclarationSelection {
    pub fn resolved(
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        selected_symbol: SymbolHandle,
    ) -> Option<Self> {
        Some(Self {
            source_span,
            exposure,
            kind,
            target: AuthoredDeclarationSelectionTarget::Resolved(
                ResolvedAuthoredDeclarationSelection::new(selected_symbol)?,
            ),
        })
    }

    pub fn late_bound(
        source_span: SourceSpan,
        exposure: AuthoredDeclarationSelectionExposure,
        kind: AuthoredDeclarationSelectionKind,
        late_binding: AuthoredDeclarationSelectionLateBinding,
    ) -> Self {
        Self {
            source_span,
            exposure,
            kind,
            target: AuthoredDeclarationSelectionTarget::LateBound(late_binding),
        }
    }

    pub fn source_span(self) -> SourceSpan {
        self.source_span
    }

    pub fn exposure(self) -> AuthoredDeclarationSelectionExposure {
        self.exposure
    }

    pub fn kind(self) -> AuthoredDeclarationSelectionKind {
        self.kind
    }

    pub fn target(self) -> AuthoredDeclarationSelectionTarget {
        self.target
    }
}

/// Stable insertion identity for one row in a resolved tree's ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredDeclarationSelectionHandle(usize);

impl AuthoredDeclarationSelectionHandle {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Append-only authored-selection custody in deterministic source traversal
/// order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredDeclarationSelections {
    rows: Vec<AuthoredDeclarationSelection>,
}

impl AuthoredDeclarationSelections {
    pub fn record(
        &mut self,
        selection: AuthoredDeclarationSelection,
    ) -> AuthoredDeclarationSelectionHandle {
        let handle = AuthoredDeclarationSelectionHandle(self.rows.len());
        self.rows.push(selection);
        handle
    }

    pub fn get(
        &self,
        handle: AuthoredDeclarationSelectionHandle,
    ) -> Option<&AuthoredDeclarationSelection> {
        self.rows.get(handle.0)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &AuthoredDeclarationSelection> {
        self.rows.iter()
    }

    pub fn as_slice(&self) -> &[AuthoredDeclarationSelection] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl<'ledger> IntoIterator for &'ledger AuthoredDeclarationSelections {
    type Item = &'ledger AuthoredDeclarationSelection;
    type IntoIter = std::slice::Iter<'ledger, AuthoredDeclarationSelection>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_source::{SourceId, Span};

    fn source_span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(SourceId(7), Span::new(start, end))
    }

    #[test]
    fn default_ledger_is_empty() {
        let selections = AuthoredDeclarationSelections::default();

        assert!(selections.is_empty());
        assert_eq!(selections.len(), 0);
        assert_eq!(selections.iter().len(), 0);
        assert_eq!(selections.as_slice(), &[]);
    }

    #[test]
    fn records_and_iterates_in_insertion_order() {
        let first = AuthoredDeclarationSelection::resolved(
            source_span(2, 6),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::MemberAccess,
            SymbolHandle::from_arena_index(3),
        )
        .expect("valid selected symbol");
        let second = AuthoredDeclarationSelection::late_bound(
            source_span(10, 11),
            AuthoredDeclarationSelectionExposure::PublicInterface,
            AuthoredDeclarationSelectionKind::Operator,
            AuthoredDeclarationSelectionLateBinding::CheckedOperator,
        );
        let mut selections = AuthoredDeclarationSelections::default();

        let first_handle = selections.record(first);
        let second_handle = selections.record(second);

        assert_eq!(first_handle.index(), 0);
        assert_eq!(second_handle.index(), 1);
        assert_eq!(selections.get(first_handle), Some(&first));
        assert_eq!(selections.get(second_handle), Some(&second));
        assert_eq!(
            selections.iter().copied().collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(
            (&selections).into_iter().copied().collect::<Vec<_>>(),
            [first, second]
        );
    }

    #[test]
    fn invalid_symbol_cannot_be_recorded_as_resolved() {
        let selection = AuthoredDeclarationSelection::resolved(
            source_span(4, 9),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::TypeReference,
            SymbolHandle::invalid(),
        );

        assert_eq!(selection, None);
    }

    #[test]
    fn rows_and_discriminators_are_copy_values() {
        fn assert_copy<Value: Copy>() {}

        assert_copy::<AuthoredDeclarationSelection>();
        assert_copy::<AuthoredDeclarationSelectionExposure>();
        assert_copy::<AuthoredDeclarationSelectionKind>();
        assert_copy::<AuthoredDeclarationSelectionLateBinding>();
        assert_copy::<AuthoredDeclarationSelectionTarget>();
        assert_copy::<AuthoredDeclarationSelectionHandle>();
        assert_copy::<ResolvedAuthoredDeclarationSelection>();

        let selection = AuthoredDeclarationSelection::late_bound(
            source_span(12, 18),
            AuthoredDeclarationSelectionExposure::PrivateImplementation,
            AuthoredDeclarationSelectionKind::Conformance,
            AuthoredDeclarationSelectionLateBinding::CheckedConformance,
        );
        let copied = selection;

        assert_eq!(selection, copied);
    }
}
