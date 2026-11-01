//! Exact returning declarations survive source release and artifact ingress.

use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::{ScalarDomainEstablishmentRoute, ScalarReturnOrigin};

pub(super) fn inspect(artifact: &CanonicalTerminalArtifact) -> Result<(), String> {
    let module = artifact.semantic_module();
    for returned in &module.scalar_qualifications.returns {
        let Some(declaration) = module
            .machine(returned.machine)
            .and_then(|machine| machine.declaration.as_ref())
        else {
            continue;
        };
        let Some(domain) = module
            .scalar_qualifications
            .domains
            .iter()
            .find(|domain| domain.id == returned.domain)
        else {
            continue;
        };
        for origin in &returned.origins {
            let ScalarReturnOrigin::Established { route } = origin else {
                continue;
            };
            let Some(ScalarDomainEstablishmentRoute::ExactMachine {
                declaration: issuer,
                ..
            }) = domain.establishment_routes.get(*route as usize)
            else {
                continue;
            };
            if issuer.identity() != declaration.semantic_declaration {
                continue;
            }
            let Some((other, _)) =
                domain
                    .establishment_routes
                    .iter()
                    .enumerate()
                    .find(|(_, route)| {
                        route.declaration().identity() != issuer.identity()
                            && declaration
                                .specialization
                                .as_ref()
                                .is_none_or(|specialization| {
                                    route.declaration().identity()
                                        != specialization.template_semantic_declaration
                                })
                    })
            else {
                continue;
            };
            let mut forged = module.clone();
            let forged_return = forged
                .scalar_qualifications
                .returns
                .iter_mut()
                .find(|row| {
                    row.machine == returned.machine
                        && row.edge == returned.edge
                        && row.domain == returned.domain
                })
                .ok_or("issuer control lost its return")?;
            // Keep the exact callable and returned SSA occurrence. Another
            // checked declaration (including the family head) cannot impersonate
            // this returning body just because its physical signature agrees.
            forged_return.origins = vec![ScalarReturnOrigin::Established {
                route: u32::try_from(other).map_err(|_| "issuer control route exceeds u32")?,
            }];
            let bytes =
                terminal_codec::encode_module(&forged).map_err(|error| error.to_string())?;
            let error = terminal_codec::decode_module(&bytes)
                .err()
                .ok_or("scalar issuer control accepted another declaration")?;
            if !error.to_string().contains("scalar return") {
                return Err(format!(
                    "scalar issuer control reached an unrelated refusal: {error}"
                ));
            }
            return Ok(());
        }
    }
    Err("scalar issuer control needs an exact application and another retained declaration".into())
}
