//! Original schemas and exact executable applications are distinct contracts.
//!
//! Source checking establishes the complete original roster and generic body
//! judgment. Ordinary consumption trusts that semantic projection, as it does
//! closed reach; commitments do not authenticate source without an opening.
//! Receiving independently checks canonical schema shape, finite coverage and
//! exact application/declaration/tuple joins. None supplies foreign authority.

use terminal_psi::{
    ClosedConformanceSchema, ClosedConformanceSchemaKind, ClosedReachArgument,
    ClosedReachParameter, TerminalIdentityIndex, TerminalModule,
};

fn identity(
    module: &TerminalModule,
    identity: TerminalIdentityIndex,
) -> Result<&str, &'static str> {
    module
        .identities
        .text(identity)
        .filter(|text| !text.is_empty())
        .ok_or("closed conformance contains an invalid or empty identity")
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], &'static str> {
    let value = bytes
        .get(..count)
        .ok_or("conformance schema definition is truncated")?;
    *bytes = &bytes[count..];
    Ok(value)
}

fn length(bytes: &mut &[u8]) -> Result<usize, &'static str> {
    let raw: [u8; 8] = take(bytes, 8)?
        .try_into()
        .map_err(|_| "invalid schema length")?;
    u64::from_le_bytes(raw)
        .try_into()
        .map_err(|_| "schema length exceeds host positions")
}

fn validate_definition(schema: &ClosedConformanceSchema) -> Result<(), &'static str> {
    let mut bytes = schema.definition.as_slice();
    let domain = b"omega.requirement-schema.v1\0";
    if take(&mut bytes, domain.len())? != domain {
        return Err("conformance schema has an unknown definition domain");
    }
    let signature = length(&mut bytes)?;
    if signature == 0 {
        return Err("conformance schema has no original signature");
    }
    take(&mut bytes, signature)?;
    if length(&mut bytes)? != schema.lifetime_parameter_count as usize
        || length(&mut bytes)? != schema.parameter_kinds.len()
    {
        return Err("conformance schema telescope differs from its original definition");
    }
    for expected in &schema.parameter_kinds {
        let kind = take(&mut bytes, 1)?[0];
        if !(1..=6).contains(&kind) || kind != *expected {
            return Err("conformance schema binder category differs from its definition");
        }
        if !(1..=3).contains(&take(&mut bytes, 1)?[0]) {
            return Err("conformance schema has an invalid multiplicity");
        }
        let carry = bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("schema carry demand is unterminated")?;
        std::str::from_utf8(take(&mut bytes, carry)?)
            .map_err(|_| "schema carry demand is not canonical text")?;
        take(&mut bytes, 1)?;
        let contract = length(&mut bytes)?;
        take(&mut bytes, contract)?;
    }
    take(&mut bytes, 32)?;
    if !bytes.is_empty() {
        return Err("conformance schema definition has trailing bytes");
    }
    let generic = schema.lifetime_parameter_count != 0 || !schema.parameter_kinds.is_empty();
    match (schema.kind, generic, &schema.finite_tuples) {
        (ClosedConformanceSchemaKind::Nongeneric, false, None)
        | (ClosedConformanceSchemaKind::Open, true, None) => Ok(()),
        (ClosedConformanceSchemaKind::Finite, true, Some(tuples))
            if schema.lifetime_parameter_count == 0
                && schema
                    .parameter_kinds
                    .iter()
                    .all(|kind| matches!(kind, 2 | 3))
                && tuples
                    .iter()
                    .all(|tuple| tuple.len() == schema.parameter_kinds.len()) =>
        {
            Ok(())
        }
        _ => Err("conformance schema classification contradicts its retained telescope or roster"),
    }
}

fn argument_matches(
    module: &TerminalModule,
    expected: &ClosedReachArgument,
    actual: &ClosedReachParameter,
) -> bool {
    match (expected, actual) {
        (ClosedReachArgument::Type(identity), ClosedReachParameter::Type { argument })
        | (ClosedReachArgument::Const(identity), ClosedReachParameter::Const { argument })
        | (
            ClosedReachArgument::Proposition(identity),
            ClosedReachParameter::Proposition { argument },
        ) => module.identities.text(*identity) == Some(argument.as_str()),
        (
            ClosedReachArgument::Machine {
                identity,
                contract_commitment,
            },
            ClosedReachParameter::Machine(binding),
        ) => {
            *identity == binding.selected_identity
                && *contract_commitment == binding.selected_contract_commitment
        }
        _ => false,
    }
}

/// Receiving replay over immutable, producer-trusted semantic inputs. Run
/// before realization or interpreter execution; this never grants host access.
pub fn validate_closed_conformances(module: &TerminalModule) -> Result<(), &'static str> {
    for application in &module.closed_conformance_applications {
        identity(module, application.declaration_identity)?;
        identity(module, application.trait_identity)?;
        if module.machine(application.owner).is_none() {
            return Err("closed conformance owner is absent");
        }
        let mut previous = None;
        for schema in &application.schemas {
            let key = (
                identity(module, schema.declaring_trait_identity)?,
                identity(module, schema.public_requirement_identity)?,
            );
            identity(module, schema.requirement_identity)?;
            identity(module, schema.realization_identity)?;
            if previous.is_some_and(|previous| previous >= key) {
                return Err("conformance schemas are repeated or not canonical");
            }
            previous = Some(key);
            validate_definition(schema)?;
            let rows = application
                .rows
                .iter()
                .filter(|row| {
                    row.declaring_trait_identity == schema.declaring_trait_identity
                        && row.public_requirement_identity == schema.public_requirement_identity
                        && row.requirement_identity == schema.requirement_identity
                })
                .collect::<Vec<_>>();
            match schema.kind {
                ClosedConformanceSchemaKind::Open if !rows.is_empty() => {
                    return Err("open conformance schema advertises finite execution coverage");
                }
                ClosedConformanceSchemaKind::Nongeneric
                    if rows.len() != 1
                        || !rows[0].family_tuple.is_empty()
                        || rows[0].realization_identity != schema.realization_identity =>
                {
                    return Err("nongeneric conformance schema lost its original row");
                }
                ClosedConformanceSchemaKind::Finite => {
                    let tuples = schema
                        .finite_tuples
                        .as_ref()
                        .ok_or("finite schema lost its declared roster")?;
                    for tuple in tuples {
                        for argument in tuple {
                            identity(module, *argument)?;
                        }
                    }
                    let tuple_keys = tuples
                        .iter()
                        .map(|tuple| {
                            tuple
                                .iter()
                                .map(|argument| identity(module, *argument))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if tuple_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
                        return Err(
                            "finite conformance schema tuples are repeated or not canonical",
                        );
                    }
                    if tuples
                        .iter()
                        .enumerate()
                        .any(|(index, tuple)| tuples[..index].contains(tuple))
                        || rows.len() != tuples.len()
                        || tuples.iter().any(|tuple| {
                            rows.iter().filter(|row| &row.family_tuple == tuple).count() != 1
                        })
                    {
                        return Err(
                            "finite conformance schema has partial or duplicate tuple coverage",
                        );
                    }
                }
                _ => {}
            }
        }
        for row in &application.rows {
            identity(module, row.realization_identity)?;
            if application
                .schemas
                .iter()
                .filter(|schema| {
                    schema.declaring_trait_identity == row.declaring_trait_identity
                        && schema.public_requirement_identity == row.public_requirement_identity
                        && schema.requirement_identity == row.requirement_identity
                })
                .count()
                != 1
            {
                return Err("conformance row has no unique original schema");
            }
        }
        let mut registry_identities = Vec::new();
        let mut registry_machines = Vec::new();
        for callable in &application.realization_callables {
            let callable_identity = identity(module, callable.source_callable_identity)?;
            if registry_identities
                .last()
                .is_some_and(|previous| *previous >= callable_identity)
                || registry_machines.contains(&callable.machine)
            {
                return Err(
                    "conformance callable registry has duplicate identities or machine bindings, or is not canonical",
                );
            }
            registry_identities.push(callable_identity);
            registry_machines.push(callable.machine);
            let machine = module
                .machine(callable.machine)
                .ok_or("conformance callable registry names an absent machine")?;
            let matches = match callable.result {
                terminal_psi::ClosedConformanceCallableResult::Unit => {
                    matches!(machine.result, terminal_psi::TerminalMachineResult::Unit)
                }
                terminal_psi::ClosedConformanceCallableResult::Scalar(expected) => machine
                    .result
                    .scalar()
                    .is_some_and(|result| result.scalar_type == expected),
                terminal_psi::ClosedConformanceCallableResult::Structural(expected) => machine
                    .result
                    .structural()
                    .is_some_and(|result| result.structural_type == expected),
            };
            if !matches {
                return Err(
                    "conformance callable registry result differs from its emitted declaration",
                );
            }
        }
        let application_keys = application
            .static_applications
            .iter()
            .map(|row| static_application_order_key(&module.identities, row))
            .collect::<Result<Vec<_>, _>>()?;
        if application_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("conformance static applications are repeated or not canonical");
        }
        for applied in &application.static_applications {
            identity(module, applied.public_requirement_identity)?;
            identity(module, applied.realization_identity)?;
            identity(module, applied.source_callable_identity)?;
            let schemas = application
                .schemas
                .iter()
                .filter(|schema| {
                    schema.public_requirement_identity == applied.public_requirement_identity
                        && schema.realization_identity == applied.realization_identity
                })
                .collect::<Vec<_>>();
            let [schema] = schemas.as_slice() else {
                return Err("static conformance application has no unique original schema");
            };
            if applied.method_arguments.len() != schema.parameter_kinds.len() {
                return Err(
                    "static conformance method tuple differs from its independent telescope",
                );
            }
            for (kind, argument) in schema.parameter_kinds.iter().zip(&applied.method_arguments) {
                let (actual, argument_identity) = match argument {
                    ClosedReachArgument::Type(identity) => (1, *identity),
                    ClosedReachArgument::Const(identity) => (2, *identity),
                    ClosedReachArgument::Proposition(identity) => (6, *identity),
                    ClosedReachArgument::Machine { identity, .. } => (4, *identity),
                };
                identity(module, argument_identity)?;
                if actual != *kind && !(actual == 2 && *kind == 3) {
                    return Err("static conformance method argument has the wrong binder category");
                }
            }
            if let Some(tuples) = &schema.finite_tuples {
                let values = applied
                    .method_arguments
                    .iter()
                    .map(|argument| match argument {
                        ClosedReachArgument::Const(identity) => Some(*identity),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or("finite conformance method application contains a non-value argument")?;
                if !tuples.contains(&values) {
                    return Err(
                        "static conformance method tuple is outside its original finite roster",
                    );
                }
            }
            let machine = module
                .machine(applied.machine)
                .ok_or("static conformance application target is absent")?;
            let declaration = machine
                .declaration
                .as_ref()
                .ok_or("static conformance target has no checked declaration")?;
            if declaration
                .specialization
                .as_ref()
                .map(|specialization| specialization.commitment)
                != applied.application_commitment
            {
                return Err(
                    "static conformance application differs from its checked declaration commitment",
                );
            }
            if application
                .realization_callables
                .iter()
                .filter(|callable| {
                    callable.source_callable_identity == applied.source_callable_identity
                        && callable.machine == applied.machine
                })
                .count()
                != 1
            {
                return Err("static conformance application has no unique callable registry entry");
            }
            if !applied.method_arguments.is_empty() {
                let invocation = machine.closed_reach_application.as_ref().ok_or(
                    "static conformance method tuple has no independent retained invocation",
                )?;
                if Some(invocation.specialization_commitment) != applied.application_commitment
                    || invocation.telescope.len() != applied.method_arguments.len()
                    || !applied
                        .method_arguments
                        .iter()
                        .zip(&invocation.telescope)
                        .all(|(expected, actual)| argument_matches(module, expected, actual))
                {
                    return Err(
                        "static conformance method tuple differs from its retained executable invocation",
                    );
                }
            }
        }
        if terminal_psi::closed_conformance_application_commitment(&module.identities, application)
            != application.commitment
            || terminal_psi::closed_conformance_application_report_fingerprint(
                &module.identities,
                application,
            ) != application.report_fingerprint
        {
            return Err(
                "closed conformance application commitment differs from its semantic contents",
            );
        }
    }
    for invocation in &module.proof_output_calls {
        let Some(dispatch) = &invocation.static_requirement_dispatch else {
            continue;
        };
        let applications = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == invocation.caller
                    && application.commitment == dispatch.conformance_application_commitment
                    && application.report_fingerprint
                        == dispatch.conformance_application_report_fingerprint
            })
            .collect::<Vec<_>>();
        let [application] = applications.as_slice() else {
            return Err("static requirement dispatch has no unique closed conformance application");
        };
        if application
            .schemas
            .iter()
            .filter(|schema| {
                schema.declaring_trait_identity == dispatch.declaring_trait_identity
                    && schema.public_requirement_identity == dispatch.public_requirement_identity
                    && schema.requirement_identity == dispatch.requirement_identity
                    && schema.realization_identity == dispatch.realization_identity
            })
            .count()
            != 1
        {
            return Err("static requirement dispatch has no exact original schema");
        }
        if application
            .static_applications
            .iter()
            .filter(|applied| {
                applied.public_requirement_identity == dispatch.public_requirement_identity
                    && applied.realization_identity == dispatch.realization_identity
                    && applied.source_callable_identity == dispatch.realization_callable_identity
                    && applied.machine == dispatch.realization
            })
            .count()
            != 1
        {
            return Err("static requirement dispatch has no exact executable method application");
        }
        if invocation
            .runtime_call
            .as_ref()
            .is_none_or(|call| call.callee != dispatch.realization)
        {
            return Err("static requirement dispatch differs from its ordinary runtime call");
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MethodArgumentOrderKey<'a> {
    Type(&'a str),
    Const(&'a str),
    Proposition(&'a str),
    Machine(&'a str, [u8; 32]),
}

/// Portable application order resolves semantic identities before comparison.
/// Arena mint positions are never an application-order coordinate.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaticApplicationOrderKey<'a> {
    requirement: &'a str,
    realization: &'a str,
    callable: &'a str,
    arguments: Vec<MethodArgumentOrderKey<'a>>,
    commitment: Option<[u8; 32]>,
}

pub fn static_application_order_key<'a>(
    identities: &'a terminal_psi::TerminalIdentityTable,
    application: &terminal_psi::ClosedConformanceStaticApplication,
) -> Result<StaticApplicationOrderKey<'a>, &'static str> {
    let text = |identity| {
        identities
            .text(identity)
            .filter(|value| !value.is_empty())
            .ok_or("static application order contains an invalid identity")
    };
    let arguments = application
        .method_arguments
        .iter()
        .map(|argument| {
            Ok(match argument {
                ClosedReachArgument::Type(identity) => {
                    MethodArgumentOrderKey::Type(text(*identity)?)
                }
                ClosedReachArgument::Const(identity) => {
                    MethodArgumentOrderKey::Const(text(*identity)?)
                }
                ClosedReachArgument::Proposition(identity) => {
                    MethodArgumentOrderKey::Proposition(text(*identity)?)
                }
                ClosedReachArgument::Machine {
                    identity,
                    contract_commitment,
                } => MethodArgumentOrderKey::Machine(text(*identity)?, *contract_commitment),
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    Ok(StaticApplicationOrderKey {
        requirement: text(application.public_requirement_identity)?,
        realization: text(application.realization_identity)?,
        callable: text(application.source_callable_identity)?,
        arguments,
        commitment: application.application_commitment,
    })
}
