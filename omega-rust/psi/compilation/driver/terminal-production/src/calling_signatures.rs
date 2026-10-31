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
    sources: &[(
        symbols::SymbolHandle,
        semantic_vocabulary::BoundaryMachineId,
    )],
) -> Result<(), Diagnostic> {
    let services = inventory.services_for(checked)?;
    let signatures = services.signatures();
    let typed = &checked.typed;
    // Lowering already knows each declaration's exact source owner. Follow
    // that coordinate and its checked template link, rather than rebuilding
    // an inverse index from rendered callable identities across source scopes.
    let mut declarations = Vec::with_capacity(sources.len());
    for &(source, boundary) in sources {
        let template = typed
            .closed_requirement_application(source)
            .map_or(source, |application| application.template);
        let Some(declaration) = signatures
            .declarations()
            .find(|declaration| declaration.symbol == template)
        else {
            // Synthetic/runtime family interfaces are not declarations in
            // the authored calling census. Absence is not ABI permission.
            continue;
        };
        let boundary = module
            .boundary_machines
            .iter()
            .find(|row| row.id == boundary)
            .ok_or_else(|| {
                Diagnostic::error("calling publication lost an emitted source boundary")
            })?;
        declarations.push((boundary, declaration));
    }
    declarations.sort_by_key(|(boundary, _)| boundary.id);
    let mut identities = std::mem::take(&mut module.identities).into_builder();
    for (boundary, declaration) in declarations {
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
