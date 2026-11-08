//! Project a checked applied field back to its authored public declaration.
//! The producer owns the correspondence; review never rebuilds a type view.

use super::AuthoredMember;
use crate::capture::SourceDeclarationInput;
use crate::capture::contracts::facts::ContractProjectionContext;
use checked_trees::ContractProofFactOwner;
use facts::FactOrigin;
use symbols::{SymbolHandle, SymbolKind};

pub(super) fn authored_projection(
    compilation: &SourceDeclarationInput<'_>,
    context: &ContractProjectionContext<'_>,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    source: AuthoredMember,
    applied: (Option<SymbolHandle>, SymbolHandle),
) -> Option<(Option<SymbolHandle>, SymbolHandle)> {
    let mut declarations = compilation
        .facts
        .proof
        .contract_facts
        .iter()
        .filter(|(_, row)| row.owner == context.owner && row.fact == fact);
    declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let origin = match context.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            FactOrigin::MachineContract { machine_symbol }
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => FactOrigin::StateContract {
            machine_symbol,
            state_symbol,
        },
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => FactOrigin::StateSignatureContract {
            owner_symbol,
            state_symbol,
        },
        _ => return None,
    };
    let mut projections = compilation
        .facts
        .semantic
        .contract_member_projections
        .iter()
        .filter(|(_, row)| {
            row.origin == origin && row.fact == fact && row.expression == source.expression
        });
    let (_, selected) = projections.next()?;
    // A shared traversal can encounter the same projection twice. Identical
    // records agree; two different applications/coordinates never do.
    if projections.any(|(_, row)| row != selected)
        || selected.authored_field != source.field
        || selected.applied_field != applied.1
        || selected.applied_case != applied.0.unwrap_or_default()
        || !selected.receiver_type.is_valid()
        || !compilation
            .type_reference_table
            .contains_type_reference(selected.receiver_type)
        || !valid_field(compilation, selected.authored_field, selected.authored_case)
        || !valid_field(compilation, selected.applied_field, selected.applied_case)
    {
        return None;
    }
    Some((
        selected
            .authored_case
            .is_valid()
            .then_some(selected.authored_case),
        selected.authored_field,
    ))
}

fn valid_field(
    compilation: &SourceDeclarationInput<'_>,
    field: SymbolHandle,
    case: SymbolHandle,
) -> bool {
    if !field.is_valid() || compilation.symbols.get(field).kind != SymbolKind::Field {
        return false;
    }
    let parent = compilation.symbols.get(field).parent;
    if case.is_valid() {
        parent == case && compilation.symbols.get(case).kind == SymbolKind::Variant
    } else {
        parent.is_valid() && compilation.symbols.get(parent).kind == SymbolKind::Data
    }
}
