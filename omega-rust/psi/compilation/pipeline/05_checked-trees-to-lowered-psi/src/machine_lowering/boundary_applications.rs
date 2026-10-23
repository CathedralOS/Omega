//! Publish exact boundary requirements while checked application identities
//! and emission joins are still available. Optimization and native realization
//! consume these rows, rather than reconstructing them from source after publication.

use checked_trees::{CheckedBoundaryOperatorApplicationUseSite, CheckedTrees};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::OperationId;
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryOperatorRequirement,
    BoundaryTypeIdentity, NominalDeclarationIdentity, TerminalBoundaryApplicationDemand,
};
use typed_trees::type_identity::ExactOwnerTypeIdentityRequest;

mod candidates;

struct LoweredBoundaryApplicationOccurrence {
    application_index: usize,
    terminal_operation: OperationId,
}

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    lowered: &mut LoweredPsi,
) -> Result<(), &'static str> {
    let mut occurrences = Vec::new();
    for comparison in &lowered.selected_comparison_occurrences {
        occurrences.push(application_occurrence(
            checked,
            comparison.application_site,
            comparison.requirement_operator,
            comparison.terminal_operation,
        )?);
    }
    // A named operator is an ordinary boundary call. The checked call
    // coordinate supplies its selected application, not the provider choice.
    let mut source_calls = lowered
        .source_call_occurrences
        .iter()
        .filter_map(|source| {
            source
                .terminal_site
                .operation()
                .map(|operation| (operation, source))
        })
        .collect::<Vec<_>>();
    source_calls.sort_unstable_by_key(|(operation, _)| *operation);
    for operation in lowered.semantic_module.operations() {
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            continue;
        };
        let declaration = lowered
            .semantic_module
            .boundary_machine(boundary)
            .ok_or("boundary application has no Terminal declaration")?;
        if declaration.kind != terminal_psi::BoundaryRequirementKind::Operator {
            continue;
        }
        let operation_id = operation.id;
        let ordinal = source_calls.partition_point(|(id, _)| *id < operation_id);
        let Some((id, source)) = source_calls.get(ordinal) else {
            return Err("named boundary application has no source-call occurrence");
        };
        if *id != operation_id
            || source_calls
                .get(ordinal + 1)
                .is_some_and(|(next, _)| *next == operation_id)
        {
            return Err("named boundary application has no unique source-call occurrence");
        }
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) = source.source_site
        else {
            return Err("named boundary application has no exact expression occurrence");
        };
        let mut uses = checked
            .facts
            .operators
            .named_uses
            .iter()
            .filter(|(_, usage)| {
                usage.expression == expression
                    && usage.selected_operator_symbol == source.source_target
                    && source.matches_checked_origin(checked, usage.origin)
            });
        let (Some((_, usage)), None) = (uses.next(), uses.next()) else {
            return Err("named boundary application has no unique checked operator use");
        };
        occurrences.push(application_occurrence(
            checked,
            CheckedBoundaryOperatorApplicationUseSite::Expression {
                expression,
                origin: usage.origin,
            },
            source.source_target,
            operation_id,
        )?);
    }
    occurrences.sort_unstable_by_key(|occurrence| occurrence.terminal_operation.get());
    if occurrences
        .windows(2)
        .any(|pair| pair[0].terminal_operation == pair[1].terminal_operation)
    {
        return Err("multiple boundary applications name one emitted operation");
    }
    let mut rows = Vec::with_capacity(occurrences.len());
    let mut candidates = Vec::new();
    let mut retained_applications = std::collections::BTreeSet::new();
    for occurrence in &occurrences {
        let application =
            &checked.facts.operators.boundary_applications[occurrence.application_index];
        let declaration = checked
            .exact_owner_nominal_identity(application.requirement_symbol, exact_toolchain_sources)
            .ok_or("boundary requirement has no exact package or toolchain owner")?;
        let requirement = BoundaryOperatorRequirement::new(
            NominalDeclarationIdentity::new(
                identities.intern(&checked.type_identity_text(declaration).unwrap_or_default()),
            ),
            checked
                .typed
                .callable_identity_text(application.requirement_identity),
            checked
                .symbols
                .symbol_package_identity(application.requirement_symbol),
        )?;
        let arguments =
            project_application(identities, checked, exact_toolchain_sources, application)?;
        if retained_applications.insert((requirement.clone(), arguments.clone())) {
            candidates::retain(
                identities,
                checked,
                exact_toolchain_sources,
                application,
                &requirement,
                &arguments,
                &mut candidates,
            )?;
        }
        rows.push(TerminalBoundaryApplicationDemand::new(
            occurrence.terminal_operation,
            requirement,
            arguments,
        ));
    }
    lowered.semantic_module.boundary_application_demands = rows;
    candidates.sort();
    candidates.dedup();
    lowered.semantic_module.boundary_application_candidates = candidates;
    Ok(())
}

fn application_occurrence(
    checked: &CheckedTrees,
    site: CheckedBoundaryOperatorApplicationUseSite,
    requirement: symbols::SymbolHandle,
    terminal_operation: OperationId,
) -> Result<LoweredBoundaryApplicationOccurrence, &'static str> {
    let mut applications = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .enumerate()
        .filter(|(_, application)| {
            application.site == site && application.requirement_symbol == requirement
        });
    let (Some((application_index, _)), None) = (applications.next(), applications.next()) else {
        return Err("emitted boundary operation has no unique checked application");
    };
    Ok(LoweredBoundaryApplicationOccurrence {
        application_index,
        terminal_operation,
    })
}

fn project_application(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<BoundaryApplication, &'static str> {
    if application.arguments.is_empty() {
        return Ok(BoundaryApplication::Empty);
    }
    let mut type_identity = |type_reference| {
        let identity = checked
            .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
                type_reference,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources,
            })
            .ok_or("boundary application type has no exact package or toolchain owner")?;
        Ok(BoundaryTypeIdentity::new(identities.intern(
            &checked.type_identity_text(identity).unwrap_or_default(),
        )))
    };
    let mut arguments = Vec::with_capacity(application.arguments.len());
    for (ordinal, argument) in application.arguments.iter().enumerate() {
        let expected = u32::try_from(ordinal)
            .map_err(|_| "boundary application exceeds the supported ordinal range")?;
        arguments.push(match argument {
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                type_reference,
                ..
            } if *binder_owner == application.requirement_symbol && *binder_ordinal == expected => {
                BoundaryApplicationArgument::type_argument(
                    *binder_ordinal,
                    type_identity(*type_reference)?,
                )
            }
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                declared_carrier,
                value,
                ..
            } if *binder_owner == application.requirement_symbol && *binder_ordinal == expected => {
                BoundaryApplicationArgument::const_argument(
                    *binder_ordinal,
                    type_identity(*declared_carrier)?,
                    value.type_name.clone(),
                    value.encoding.clone(),
                )?
            }
            _ => {
                return Err(
                    "boundary application does not match its binder owner, category and ordinal",
                );
            }
        });
    }
    BoundaryApplication::exact(arguments)
}
