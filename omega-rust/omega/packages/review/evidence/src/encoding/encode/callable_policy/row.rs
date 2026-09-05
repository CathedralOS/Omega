//! Complete named callable fields, sharing the enclosing policy budget.

use super::*;

pub(in crate::encoding) fn encode_callable(
    encoder: &mut Encoder,
    callable: &PackagePolicyCallable,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("role", |encoder| {
        match callable.role {
            PackagePolicyCallableRole::Boundary => encoder.tag("boundary", 0),
            PackagePolicyCallableRole::Public => encoder.tag("public", 1),
            PackagePolicyCallableRole::Build => encoder.tag("build", 2),
            PackagePolicyCallableRole::PrivateAssumption => encoder.tag("private_assumption", 3),
            PackagePolicyCallableRole::PrivateExternal => encoder.tag("private_external", 4),
        }
        Ok(())
    })?;
    encoder.field("identity", |encoder| {
        encode_nominal(encoder, &callable.identity)
    })?;
    encoder.field("supply", |encoder| encode_supply(encoder, callable.supply))?;
    encoder.field("lifetime_parameter_count", |encoder| {
        encoder.usize(callable.lifetime_parameter_count)
    })?;
    encoder.field("type_parameters", |encoder| {
        encoder.sequence(&callable.type_parameters, encode_type_parameter)
    })?;
    encoder.field("conformance_bounds", |encoder| {
        encoder.sequence(&callable.conformance_bounds, encode_conformance_bound)
    })?;
    encoder.field("parameters", |encoder| {
        encoder.sequence(&callable.parameters, |encoder, parameter| {
            encoder.field("name", |encoder| encoder.string(&parameter.name))?;
            encoder.field("type", |encoder| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
            encoder.field("const", |encoder| {
                encoder.boolean(parameter.is_const);
                Ok(())
            })?;
            encoder.field("mutable", |encoder| {
                encoder.boolean(parameter.is_mutable);
                Ok(())
            })?;
            encoder.field("self", |encoder| {
                encoder.boolean(parameter.is_self);
                Ok(())
            })
        })
    })?;
    encoder.field("return_type", |encoder| {
        encoder.option(callable.return_type.as_ref(), encode_type_identity)
    })?;
    encoder.field("conformances", |encoder| {
        encoder.sequence(&callable.conformances, encode_callable_conformance)
    })?;
    encoder.field("operator_realizations", |encoder| {
        encoder.sequence(&callable.operator_realizations, |encoder, realization| {
            encoder.field("coordinate", |encoder| {
                encode_operator_coordinate(encoder, &realization.coordinate)
            })?;
            encoder.field("alias", |encoder| {
                encoder.option(realization.alias.as_deref(), |encoder, alias| {
                    encoder.string(alias)
                })
            })
        })
    })?;
    encoder.field("contracts", |encoder| {
        encoder.sequence(&callable.contracts, encode_callable_contract)
    })?;
    encoder.field("declared_service_reach", |encoder| {
        encoder.option(
            callable.declared_service_reach.as_deref(),
            |encoder, row| encoder.sequence(row, encode_nominal),
        )
    })?;
    encoder.field("checked_service_reach", |encoder| {
        match &callable.checked_service_reach {
            PackageReviewCheckedServiceReach::NoCheckedBody => encoder.tag("no_checked_body", 0),
            PackageReviewCheckedServiceReach::CheckedBody { realized, concrete } => {
                encoder.tag("checked_body", 1);
                encoder.field("realized", |encoder| {
                    encoder.sequence(realized, encode_nominal)
                })?;
                encoder.field("concrete", |encoder| {
                    encoder.sequence(concrete, encode_nominal)
                })?;
            }
        }
        Ok(())
    })?;
    encoder.field("unresolved_installation_reaches", |encoder| {
        encoder.sequence(
            &callable.unresolved_installation_reaches,
            encode_installation_reach,
        )
    })?;
    encoder.field("declared_synchronous_invocations", |encoder| {
        encoder.option(
            callable.declared_synchronous_invocations.as_deref(),
            |encoder, row| encoder.sequence(row, encode_synchronous_invocation),
        )
    })?;
    encoder.field("realized_synchronous_invocations", |encoder| {
        encoder.sequence(
            &callable.realized_synchronous_invocations,
            encode_synchronous_invocation,
        )
    })?;
    encoder.field("capability_flows", |encoder| {
        encoder.sequence(&callable.capability_flows, behavior::capability)
    })?;
    encoder.field("reachable_capability_flows", |encoder| {
        encoder.sequence(&callable.reachable_capability_flows, behavior::capability)
    })?;
    encoder.field("declared_may_suspend", |encoder| {
        encoder.option(callable.declared_may_suspend.as_ref(), |encoder, value| {
            encoder.boolean(*value);
            Ok(())
        })
    })?;
    encoder.field("declared_may_block", |encoder| {
        encoder.option(callable.declared_may_block.as_ref(), |encoder, value| {
            encoder.boolean(*value);
            Ok(())
        })
    })?;
    encoder.field("declared_termination", |encoder| {
        encoder.option(
            callable.declared_termination.as_ref(),
            behavior::termination,
        )
    })?;
    encoder.field("checked_may_suspend", |encoder| {
        encoder.boolean(callable.checked_may_suspend);
        Ok(())
    })?;
    encoder.field("checked_may_block", |encoder| {
        encoder.boolean(callable.checked_may_block);
        Ok(())
    })?;
    encoder.field("checked_termination", |encoder| {
        behavior::termination(encoder, &callable.checked_termination)
    })?;
    encoder.field("checked_crash", |encoder| {
        behavior::crash(encoder, &callable.checked_crash)
    })?;
    encoder.field("mutation", |encoder| {
        behavior::mutation(encoder, &callable.mutation)
    })
}
