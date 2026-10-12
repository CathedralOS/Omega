//! The exact bound boundary requirement owns its postconditions.
//!
//! Proof and range consumers use the same resolved callable identity, including
//! its applied telescope. Reconstructing a receiver field and matching leaf
//! names can select another package's trait, an inherited overload, or the
//! unapplied declaration. This metadata query grants no invocation authority;
//! callers must apply the checked write frame before publishing postconditions.

use crate::TypedTrees;
use crate::signature::StateSignature;
use crate::statement::TableCall;

pub fn called_boundary_signature<'program>(
    program: &'program TypedTrees,
    call: &TableCall,
) -> Option<&'program StateSignature> {
    // Evidence dispatch may rewrite the executable target. Its independently
    // retained requirement owns the public contract; never read stronger facts
    // from the private realizing state or fall back after a broken receipt.
    let (declaration, signature) = if let Some(dispatch) = &call.static_requirement_dispatch {
        let declaration = program.trait_definition_by_symbol(dispatch.declaring_trait)?;
        let signature =
            program.trait_machine_signature_by_symbol(declaration, dispatch.requirement)?;
        (declaration, signature)
    } else {
        program.trait_machine_signature(call.target_symbol)?
    };
    declaration.is_boundary.then_some(signature)
}
