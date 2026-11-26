//! Report the slots and providers that execution actually received.
use product_publication::CompletedProduct;
use target_vocabulary::{BoundarySettlementRealization, NormalizedForeignCallMechanism};

pub(super) fn describe(report: &CompletedProduct) -> Result<String, &'static str> {
    let Some(runtime) = report.runtime_provisions().map(|p| p.execution()) else {
        return Ok("bindings: no runtime binding receipt".into());
    };
    let artifact = report
        .retained_native_program()
        .map(|p| p.psi_artifact())
        .or_else(|| report.artifact())
        .ok_or("the test lost its retained Psi binding inventory")?;
    let root = (artifact, runtime.host_provisions());
    let components = runtime.components()?.iter().map(|component| {
        (
            component.artifact().as_ref(),
            component.runtime().host_provisions(),
        )
    });
    let mut bindings = Vec::new();
    for (artifact, provisions) in std::iter::once(root).chain(components) {
        let module = artifact.semantic_module();
        let program = artifact.view().identity().program_fingerprint;
        let boundary = |id| {
            module
                .boundary_machine(id)
                .and_then(|row| module.identities.text(row.identity))
                .map_or_else(|| format!("boundary #{}", id.get()), str::to_owned)
        };
        if let Some(installation) = provisions.installation() {
            for (slot, machine) in installation.providers() {
                let name = module
                    .machine(machine)
                    .and_then(|body| body.declaration.as_ref())
                    .and_then(|decl| module.identities.text(decl.machine.identity()))
                    .unwrap_or("machine");
                bindings.push(format!(
                    "{program} {} => {name} #{}",
                    boundary(slot),
                    machine.get()
                ));
            }
        }
        for host in provisions.host() {
            let selected = match &host.realization {
                BoundarySettlementRealization::Builtin(realization) => format!("{realization:?}"),
                BoundarySettlementRealization::NormalizedForeignCall(binding) => {
                    match &binding.mechanism {
                        NormalizedForeignCallMechanism::SystemCall(call) => {
                            format!("syscall {}", call.number)
                        }
                        NormalizedForeignCallMechanism::Component {
                            provider,
                            candidate,
                            ..
                        } => format!(
                            "component {} machine #{}",
                            provider.program_fingerprint,
                            candidate.get()
                        ),
                        NormalizedForeignCallMechanism::BuildImport { locator, .. }
                        | NormalizedForeignCallMechanism::Import { locator, .. } => {
                            describe_import(locator.locator())
                        }
                        NormalizedForeignCallMechanism::CompilerImport { import, .. } => {
                            format!("compiler import {import:?}")
                        }
                        NormalizedForeignCallMechanism::Table { byte_offset, .. } => {
                            format!("table byte {byte_offset}")
                        }
                    }
                }
            };
            bindings.push(format!(
                "{program} {} => {selected}",
                boundary(host.boundary)
            ));
        }
    }
    bindings.sort_unstable();
    Ok(if bindings.is_empty() {
        "bindings: none".into()
    } else {
        format!("bindings: {}", bindings.join(", "))
    })
}

fn describe_import(locator: &semantic_vocabulary::ForeignLocatorCandidate) -> String {
    use semantic_vocabulary::ForeignLocatorCandidate::*;
    match locator {
        PeByName { library, export } => format!(
            "import {}!{}",
            library.escape_ascii(),
            export.escape_ascii()
        ),
        PeByOrdinal { library, ordinal } => {
            format!("import {} ordinal {ordinal}", library.escape_ascii())
        }
        ElfVersioned {
            object,
            symbol,
            version,
        } => format!(
            "import {} {}@{}",
            object.escape_ascii(),
            symbol.escape_ascii(),
            version.escape_ascii()
        ),
        MachODylibSymbol {
            install_name,
            symbol,
        } => format!(
            "import {} {}",
            install_name.escape_ascii(),
            symbol.escape_ascii()
        ),
    }
}
