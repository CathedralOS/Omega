use super::super::{
    calling, declarations::encode_representation_target, public_api, representation,
    selected_providers, terminal_permissions, values::identity::encode_nominal,
};
use super::*;

pub(super) fn terminal(
    builder: &mut Builder,
    policy: &PackagePolicyTerminalPermissions,
) -> Result<(), PackageReviewEncodingError> {
    let PackagePolicyTerminalPermissions {
        package: _,
        target: _,
        services,
    } = policy;
    for value in services {
        let PackagePolicyTerminalService {
            service,
            static_parameters,
            lifetime_parameter_count,
            methods,
            permissions,
        } = value;
        builder.push(
            PackagePolicyRowKind::TerminalService,
            false,
            false,
            |encoder| encode_nominal(encoder, service),
            |encoder| {
                encoder.field("service", |encoder| encode_nominal(encoder, service))?;
                encoder.field("static_parameters", |encoder| {
                    encoder.sequence(static_parameters, public_api::type_parameter)
                })?;
                encoder.field("lifetime_parameter_count", |encoder| {
                    encoder.u32(*lifetime_parameter_count);
                    Ok(())
                })?;
                encoder.field("methods", |encoder| {
                    encoder.sequence(methods, selected_providers::encode_service_method)
                })
            },
        )?;
        for permission in permissions {
            builder.push(
                PackagePolicyRowKind::TerminalPermission,
                true,
                false,
                |encoder| {
                    encode_nominal(encoder, service)?;
                    encode_nominal(encoder, &permission.requirement)
                },
                |encoder| {
                    encoder.field("service", |encoder| encode_nominal(encoder, service))?;
                    encoder.field("permission", |encoder| {
                        terminal_permissions::permission(encoder, permission)
                    })
                },
            )?;
        }
    }
    Ok(())
}

pub(super) fn representation(
    builder: &mut Builder,
    policy: &PackagePolicyRepresentation,
) -> Result<(), PackageReviewEncodingError> {
    let PackagePolicyRepresentation {
        package: _,
        target,
        declarations,
        producer_availability,
        selected_availability,
        demands,
    } = policy;
    builder.push(
        PackagePolicyRowKind::RepresentationTarget,
        false,
        false,
        |_| Ok(()),
        |encoder| {
            encode_representation_target(encoder, *target);
            Ok(())
        },
    )?;
    for declaration in declarations {
        builder.push(
            PackagePolicyRowKind::RepresentationDeclaration,
            false,
            false,
            |encoder| encode_nominal(encoder, declaration),
            |encoder| encode_nominal(encoder, declaration),
        )?;
    }
    for value in producer_availability {
        let PackagePolicyRepresentationAvailability {
            opaque,
            conformance,
            carrier,
        } = value;
        builder.push(
            PackagePolicyRowKind::RepresentationAvailability,
            false,
            false,
            |encoder| encode_nominal(encoder, &conformance.identity),
            |encoder| {
                encoder.field("opaque", |encoder| encode_nominal(encoder, opaque))?;
                encoder.field("conformance", |encoder| {
                    public_api::conformance_shape(encoder, conformance)
                })?;
                encoder.field("carrier", |encoder| encode_nominal(encoder, carrier))
            },
        )?;
    }
    for selection in selected_availability {
        builder.push(
            PackagePolicyRowKind::RepresentationSelection,
            false,
            false,
            |encoder| encode_nominal(encoder, &selection.opaque),
            |encoder| representation::selection(encoder, selection),
        )?;
    }
    for value in demands {
        let PackagePolicyRepresentationDemand { opaque, calling } = value;
        builder.push(
            PackagePolicyRowKind::RepresentationDemand,
            false,
            false,
            |encoder| {
                encode_nominal(encoder, &calling.boundary_trait)?;
                encoder.sequence(
                    &calling.boundary_arguments,
                    super::super::declarations::encode_type_identity,
                )?;
                encode_nominal(encoder, &calling.requirement)?;
                encode_nominal(encoder, opaque)
            },
            |encoder| {
                encoder.field("opaque", |encoder| encode_nominal(encoder, opaque))?;
                encoder.field("calling", |encoder| {
                    calling::encode_application(encoder, calling)
                })
            },
        )?;
    }
    Ok(())
}
