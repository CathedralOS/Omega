//! Close prepared native telescopes over their emitted boundary declarations.
//!
//! The Product census already owns source parameter names, nominal callback
//! binders and native order. Publication retains those judgments once; Omega
//! later reads storage and contracts from the referenced canonical boundary.

use checked_trees::{CheckedTrees, PreparedProviderInventory};
use diagnostics::Diagnostic;
use terminal_psi::{
    CallingCallbackBinder, CallingDirectCallback, CallingRuntimeFormal,
    CallingSignatureDeclaration, CallingSignatureTelescope, TerminalModule,
};

pub(crate) fn capture(
    checked: &CheckedTrees,
    inventory: &PreparedProviderInventory,
    module: &mut TerminalModule,
) -> Result<(), Diagnostic> {
    let services = inventory.services_for(checked)?;
    let signatures = services.signatures();
    let typed = &checked.typed;
    // Bind the prepared census to the emitted identity namespace once. Source
    // application/template relationships never need to be searched for each
    // boundary, nor reconstructed by a target consumer.
    let mut declarations = signatures
        .declarations()
        .filter_map(|declaration| {
            module
                .identities
                .index_of(&declaration.requirement_identity)
                .map(|identity| (identity, declaration))
        })
        .collect::<Vec<_>>();
    for application in typed.closed_requirement_applications() {
        let Some(declaration) = signatures
            .declarations()
            .find(|declaration| declaration.symbol == application.template)
        else {
            continue;
        };
        let Some((owner, signature)) = typed.trait_machine_signature(application.signature.symbol)
        else {
            continue;
        };
        let callable = typed.trait_requirement_callable_identity_text(owner, signature);
        if let Some(identity) = module.identities.index_of(&callable) {
            declarations.push((identity, declaration));
        }
    }
    declarations.sort_by_key(|(identity, _)| *identity);
    // A closed application's identity can already equal the original row.
    // Deduplicate only the same checked declaration, not equal-looking rows.
    declarations.dedup_by(|left, right| left.0 == right.0 && std::ptr::eq(left.1, right.1));
    for boundary in &module.boundary_machines {
        if let Some(callable) = &boundary.declaration {
            let matching =
                declarations.partition_point(|(identity, _)| *identity < callable.source.callable);
            if declarations
                .get(matching..matching.saturating_add(2))
                .is_some_and(|pair| {
                    pair[0].0 == callable.source.callable && pair[1].0 == callable.source.callable
                })
            {
                return Err(Diagnostic::error(
                    "calling publication has ambiguous checked declaration correspondence",
                ));
            }
        }
    }
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    for boundary in &module.boundary_machines {
        let Some(callable) = &boundary.declaration else {
            continue;
        };
        // Closed requirement rows retain an exact applied callable, while
        // their native formal IDs belong to the declaration's original
        // telescope. Follow the source-issued template association, never an
        // overload leaf name or storage-shape match.
        let index =
            declarations.partition_point(|(identity, _)| *identity < callable.source.callable);
        let Some((_, declaration)) = declarations
            .get(index)
            .filter(|(identity, _)| *identity == callable.source.callable)
        else {
            // Synthetic/runtime family interfaces are not entries in the
            // authored boundary-calling census. Absence is not ABI permission.
            continue;
        };
        let requirement_identity = identities.intern(&declaration.requirement_identity);
        let telescope = declaration.projection.as_ref().map_err(Clone::clone).and_then(|prepared| {
            if prepared.telescope.parameters.len() != boundary.parameter_order.len() {
                return Err("native declaration telescope does not correspond to the emitted runtime parameter roster".into());
            }
            Ok(CallingSignatureTelescope {
                parameters: prepared
                    .telescope
                    .parameters
                    .iter()
                    .map(|formal| CallingRuntimeFormal {
                        identity: formal.identity,
                        native_ordinal: formal.native_ordinal,
                    })
                    .collect(),
                binders: prepared
                    .telescope
                    .binders
                    .iter()
                    .map(|binder| CallingCallbackBinder {
                        binder: binder.binder,
                        requirement: binder.requirement,
                        requirement_identity: identities.intern(&binder.requirement_identity),
                        static_machine_ordinal: binder.static_machine_ordinal,
                        invokes_synchronously: binder.invokes_synchronously,
                    })
                    .collect(),
                direct_callbacks: prepared
                    .telescope
                    .direct_callbacks
                    .iter()
                    .map(|callback| CallingDirectCallback {
                        identity: callback.identity,
                        native_ordinal: callback.native_ordinal,
                        binder: callback.binder,
                        requirement: callback.requirement,
                    })
                    .collect(),
            })
        });
        module.calling_signatures.push(CallingSignatureDeclaration {
            boundary: boundary.id,
            requirement_identity,
            telescope,
        });
    }
    module.identities = identities.build();
    Ok(())
}
