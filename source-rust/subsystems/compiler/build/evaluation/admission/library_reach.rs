//! Admit the invoked exports of incoming Build libraries against their default empty reach.
use diagnostics::Diagnostic;
use psi_provider_selections::{AdmittedProviderSelections, logical_reach::boundary_call_reach};
use semantic_vocabulary::{PackageKeyIdentity, TargetMachineCallable};
use terminal_psi::TerminalModule;
use terminal_semantics::{
    body_services::body_service_reaches, callable_values::CallableBoundaryImplementation,
};

pub(super) fn check(
    module: &TerminalModule,
    entry: TargetMachineCallable,
    providers: &AdmittedProviderSelections,
    edges: &[(PackageKeyIdentity, String)],
) -> Result<(), Vec<Diagnostic>> {
    if edges.is_empty() {
        return Ok(());
    }
    let fail = |message: &str| vec![Diagnostic::error(message)];
    let TargetMachineCallable::Machine(entry) = entry else {
        return Err(fail(
            "library reach admission needs an executable Build entry",
        ));
    };
    let implementations = providers
        .providers()
        .chain(
            providers
                .assumed_bindings()
                .iter()
                .map(|row| (row.required, row.implementation)),
        )
        .map(|(boundary, machine)| CallableBoundaryImplementation { boundary, machine })
        .collect::<Vec<_>>();
    let reached =
        boundary_call_reach(module, entry, &implementations, None).map_err(|error| vec![error])?;
    let body_reaches = body_service_reaches(module).map_err(|reason| fail(&reason))?;
    let scope = &module.scalars.scalar_applications.proof_scope;
    let callables = reached
        .reached_machines
        .into_iter()
        .map(TargetMachineCallable::Machine)
        .chain(
            reached
                .boundaries
                .into_iter()
                .map(TargetMachineCallable::Boundary),
        );
    for callable in callables {
        let declaration = scope.callable_declaration(callable).ok_or_else(|| {
            fail("reached Build callable lost its declaration during library reach admission")
        })?;
        let header = scope
            .callable_headers
            .iter()
            .find_map(|(_, header)| (header.declaration == declaration).then_some(header))
            .ok_or_else(|| {
                fail("reached Build callable lost its package during library reach admission")
            })?;
        let Some((_, alias)) = edges
            .iter()
            .find(|(package, _)| Some(*package) == header.package)
        else {
            continue;
        };
        let (published, body) = match callable {
            TargetMachineCallable::Machine(id) => {
                let machine = module
                    .machine(id)
                    .ok_or_else(|| fail("reached Build library machine is absent"))?;
                (
                    &machine.published_service_ceiling,
                    body_reaches.get(id).map_err(fail)?,
                )
            }
            TargetMachineCallable::Boundary(id) => {
                let boundary = module
                    .boundary_machine(id)
                    .ok_or_else(|| fail("reached Build library boundary is absent"))?;
                (&boundary.published_service_ceiling, &[][..])
            }
        };
        if let Some(service) = published.first().or_else(|| body.first()) {
            let service = module
                .service(*service)
                .and_then(|service| module.identities.text(service.identity))
                .ok_or_else(|| fail("reached Build library service lost its identity"))?;
            let name = header
                .callable
                .as_ref()
                .ok()
                .and_then(|identity| module.identities.text(*identity))
                .ok_or_else(|| fail("reached Build library callable lost its identity"))?;
            return Err(fail(&format!(
                "Build dependency `{alias}` has an empty reach ceiling, but invoking `{name}` admits service `{service}`"
            )));
        }
    }
    Ok(())
}
