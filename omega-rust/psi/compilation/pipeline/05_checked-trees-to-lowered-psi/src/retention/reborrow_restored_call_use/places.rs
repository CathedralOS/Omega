//! Place and lifecycle coordinates retained by restored-call publication.
//!
//! Source checking owns reborrow admission. These identities only join a later
//! operation to its checked occurrence; they do not establish access authority.

use checked_trees::{BorrowAccessKind, BorrowLoanOwnerSegment, FlowInvalidationSource};
use terminal_psi::{
    StructuralAccess, TerminalBorrowBoundarySource, TerminalBorrowOwnerSegment,
    TerminalBorrowPlace, TerminalBorrowPlaceSegment,
};

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;

pub(crate) fn identity(
    checked: &CheckedTrees,
    symbol: symbols::SymbolHandle,
) -> Result<String, LoweringError> {
    if let Ok(identity) = checked.typed.normalized_hermetic_symbol_identity(symbol) {
        return Ok(identity);
    }
    if !symbol.is_valid() {
        return unsupported("reborrow custody contains an unresolved symbol");
    }
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega-terminal-reborrow-symbol-v1");
    digest.raw(checked.typed.symbols.display_path(symbol, "::").as_bytes());
    digest.raw(&symbol.arena_index().to_le_bytes());
    digest.raw(&symbol.generation().to_le_bytes());
    Ok(format!(
        "terminal-borrow:{}",
        digest
            .finish()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn ordinal(value: usize) -> Result<u64, LoweringError> {
    value
        .try_into()
        .map_err(|_| LoweringError::Unsupported("reborrow custody coordinate is too large"))
}

/// Name the exact storage occurrence in this artifact. Equal computed values
/// need not share backing, so denotation or display text cannot identify a root.
/// The occurrence key is opaque custody vocabulary, not permission evidence;
/// source checking has already judged its formation and lifetime.
pub(crate) fn storage_root_identity(
    checked: &CheckedTrees,
    root: facts::PlaceRoot,
) -> Result<String, LoweringError> {
    match root {
        facts::PlaceRoot::Symbol(symbol) => identity(checked, symbol),
        facts::PlaceRoot::Expression(expression) if expression.is_valid() => {
            // Checking owns arena validity. This retained occurrence names
            // custody; encoding it must not reopen or revalidate source nodes.
            let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
                b"omega-terminal-computed-backing-v1",
            );
            digest.raw(&expression.arena_index().to_le_bytes());
            digest.raw(&expression.generation().to_le_bytes());
            Ok(format!(
                "terminal-borrow-storage:{}",
                digest
                    .finish()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ))
        }
        _ => unsupported("reborrow custody contains an unresolved storage root"),
    }
}

pub(crate) fn boundary(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    source: FlowInvalidationSource,
) -> Result<TerminalBorrowBoundarySource, LoweringError> {
    Ok(match source {
        FlowInvalidationSource::Statement { statement_index } => {
            TerminalBorrowBoundarySource::Statement {
                statement_index: ordinal(statement_index)?,
            }
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => TerminalBorrowBoundarySource::Call {
            statement_index: ordinal(statement_index)?,
            call_ordinal: ordinal(call_ordinal)?,
            target_identity: identities.intern(&identity(checked, target_symbol)?),
        },
    })
}

pub(crate) fn owner_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    path: &[BorrowLoanOwnerSegment],
) -> Result<Vec<TerminalBorrowOwnerSegment>, LoweringError> {
    path.iter()
        .map(|segment| {
            Ok(match segment {
                BorrowLoanOwnerSegment::Field(symbol) => TerminalBorrowOwnerSegment::Field(
                    identities.intern(&identity(checked, *symbol)?),
                ),
                BorrowLoanOwnerSegment::Case(symbol) => TerminalBorrowOwnerSegment::Case(
                    identities.intern(&identity(checked, *symbol)?),
                ),
                BorrowLoanOwnerSegment::FixedIndex(index) => {
                    TerminalBorrowOwnerSegment::FixedIndex(ordinal(*index)?)
                }
                BorrowLoanOwnerSegment::DynamicIndex => TerminalBorrowOwnerSegment::DynamicIndex,
            })
        })
        .collect()
}

/// Static publication coordinate of one loan's evaluated selectors. The
/// loan handle distinguishes formations even when prepared source expressions
/// are shared. Runtime invocations remain scoped by the containing restored call.
#[derive(Clone, Copy)]
pub(crate) struct Capture {
    pub state: symbols::SymbolHandle,
    pub loan: arena::Handle<checked_trees::BorrowLoanFact>,
}

impl Capture {
    fn selector_identity(
        self,
        checked: &CheckedTrees,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Result<String, LoweringError> {
        let loan = checked.facts.borrow.loans.get(self.loan);
        if !expression.is_valid() {
            return unsupported("reborrow custody selector has no captured occurrence");
        }
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"omega-terminal-captured-borrow-selector-v1",
        );
        digest.raw(identity(checked, self.state)?.as_bytes());
        digest.raw(&self.loan.arena_index().to_le_bytes());
        digest.raw(&self.loan.generation().to_le_bytes());
        digest.raw(&ordinal(loan.statement_index)?.to_le_bytes());
        digest.raw(&loan.formation_expression.arena_index().to_le_bytes());
        digest.raw(&loan.formation_expression.generation().to_le_bytes());
        digest.raw(&expression.arena_index().to_le_bytes());
        digest.raw(&expression.generation().to_le_bytes());
        Ok(format!(
            "terminal-borrow-selector:{}",
            digest
                .finish()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ))
    }
}

pub(crate) fn place_segments(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    segments: &[facts::PlaceSegment],
    capture: Capture,
) -> Result<Vec<TerminalBorrowPlaceSegment>, LoweringError> {
    segments
        .iter()
        .map(|segment| {
            Ok(match segment {
                facts::PlaceSegment::Field { symbol } => TerminalBorrowPlaceSegment::Field(
                    identities.intern(&identity(checked, *symbol)?),
                ),
                facts::PlaceSegment::Case { variant } => TerminalBorrowPlaceSegment::Case(
                    identities.intern(&identity(checked, *variant)?),
                ),
                facts::PlaceSegment::FixedIndex { index } => {
                    TerminalBorrowPlaceSegment::FixedIndex(ordinal(*index)?)
                }
                facts::PlaceSegment::FixedRange { start, end } => {
                    TerminalBorrowPlaceSegment::FixedRange {
                        start: ordinal(*start)?,
                        end: ordinal(*end)?,
                    }
                }
                facts::PlaceSegment::Index { expression } => {
                    TerminalBorrowPlaceSegment::CapturedSelector(
                        identities.intern(&capture.selector_identity(checked, *expression)?),
                    )
                }
                facts::PlaceSegment::OpaqueRange => {
                    return unsupported("reborrow custody window has no captured occurrence");
                }
            })
        })
        .collect()
}

pub(crate) fn place(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    place: &checked_trees::CapturedPlace,
    capture: Capture,
) -> Result<TerminalBorrowPlace, LoweringError> {
    Ok(TerminalBorrowPlace {
        root_identity: identities.intern(&storage_root_identity(checked, place.root)?),
        segments: place_segments(identities, checked, &place.segments, capture)?,
    })
}

/// Inherited selector coordinates retain their parent's capture identity.
/// Only the newly evaluated suffix belongs to the child's formation.
pub(crate) fn projected_place(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    parent: &checked_trees::CapturedPlace,
    published_parent: &TerminalBorrowPlace,
    child: &checked_trees::CapturedPlace,
    capture: Capture,
) -> Result<(TerminalBorrowPlace, Vec<TerminalBorrowPlaceSegment>), LoweringError> {
    if parent.root != child.root || parent.incoming_reference != child.incoming_reference {
        return unsupported("reborrow custody child changes its captured root");
    }
    let remainder = child
        .segments
        .strip_prefix(parent.segments.as_slice())
        .ok_or(LoweringError::Unsupported(
            "reborrow custody child place is not under its parent",
        ))?;
    let projection = place_segments(identities, checked, remainder, capture)?;
    let mut place = published_parent.clone();
    place.segments.extend_from_slice(&projection);
    Ok((place, projection))
}

pub(crate) fn access(kind: BorrowAccessKind) -> StructuralAccess {
    match kind {
        BorrowAccessKind::Read => StructuralAccess::SharedBorrow,
        BorrowAccessKind::Mutable => StructuralAccess::MutableBorrow,
        BorrowAccessKind::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    }
}
