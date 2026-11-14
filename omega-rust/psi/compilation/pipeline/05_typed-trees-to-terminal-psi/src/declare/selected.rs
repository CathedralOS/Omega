//! Publish the selected map's complete declaration data with its actual calls.
//! These records describe the choice; the normal conformance witnesses still
//! establish refinement, and invocation admission still establishes permission.

use super::{nominal, requirement_application};
use semantic_vocabulary::ContractId;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceParameterBinding, ClosedConformanceParameterKind,
    ClosedConformanceRealizationCallable, ClosedConformanceRow, ClosedConformanceSchema,
    ClosedConformanceSchemaKind, ClosedConformanceStaticApplication, NominalDeclarationIdentity,
    StaticRequirementDispatch, TargetMachineCallable, TerminalIdentityTableBuilder,
    TerminalMachineResult, TerminalModule,
};
use typed_trees::TypedTrees;

type HeaderLink = (ContractId, usize, StaticRequirementDispatch);

pub(super) fn publish(
    typed: &TypedTrees,
    program: &crate::read::Program<'_>,
    sources: &[(source::SourceId, [u8; 32])],
    module: &mut TerminalModule,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<Vec<HeaderLink>, &'static str> {
    let mut demands = Vec::new();
    for machine in &module.machines {
        for invocation in machine.module_invocations() {
            let Some(required) = invocation.inputs.required_contract else {
                continue;
            };
            let original = *program
                .declaration_representatives
                .get(required)
                .ok_or("selected public contract lost its construction representative")?;
            let (_, body, _, operand, owner) = program
                .declaration_selections
                .get(original)
                .and_then(Option::as_ref)
                .ok_or("selected public contract lost its source application")?;
            if *body != invocation.inputs.callee {
                return Err("selected public contract names another physical implementation");
            }
            let identity = typed
                .declared_static_operand_identity(
                    typed_trees::type_identity::DeclaredStaticOperandIdentityRequest {
                        operand: typed_trees::type_identity::IndexIdentityArgument::StaticArgument(
                            operand,
                        ),
                        lexical_owner: *owner,
                        binders: &[],
                        lifetime_binders: &[],
                    },
                )
                .ok_or("selected application has no complete argument identity")?;
            let key = (machine.id, identity);
            demands.push((key, required, original, *body, *owner, operand));
        }
    }
    demands.sort_unstable_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    demands.dedup_by(|right, left| right.0 == left.0 && right.1 == left.1);
    let mut links = Vec::with_capacity(demands.len());
    let mut start = 0;
    while start < demands.len() {
        let mut end = start + 1;
        while end < demands.len() && demands[end].0 == demands[start].0 {
            end += 1;
        }
        let group = &demands[start..end];
        let operand = group[0].5;
        let source_owner = group[0].4;
        let selected = typed
            .conformance_by_symbol(operand.symbol)
            .ok_or("selected map lost its actual conformance declaration")?;
        let parameters = typed.conformance_type_parameters(selected);
        let raw_arguments = operand
            .application
            .as_ref()
            .map_or(&[][..], |application| application.arguments.as_ref());
        let raw_lifetimes = operand.application.as_ref().map_or(&[][..], |application| {
            application.lifetime_arguments.as_ref()
        });
        if operand
            .application
            .as_ref()
            .is_some_and(|application| application.member.is_valid())
            || parameters.len() != raw_arguments.len()
            || selected.lifetime_parameters.len() != raw_lifetimes.len()
        {
            return Err("selected map does not bind its complete declared argument list");
        }
        let references = raw_arguments
            .iter()
            .map(|argument| {
                if typed
                    .type_reference_table
                    .contains_type_reference(argument.type_reference)
                {
                    Ok(argument.type_reference)
                } else {
                    Err("selected map argument has no retained source reference")
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let scope = typed_trees::trait_definition::AppliedTraitScope::from_conformance_application(
            typed,
            selected,
            typed_trees::trait_definition::TraitApplicationArguments {
                lexical_owner: source_owner,
                enclosing_data_applications: &[],
                arguments: &references,
                lifetimes: raw_lifetimes,
            },
        )
        .ok_or("selected map lost its complete applied trait scope")?;
        let mut telescope = Vec::with_capacity(parameters.len() + raw_lifetimes.len());
        for (parameter, argument) in selected.lifetime_parameters.iter().zip(raw_lifetimes) {
            let argument = typed_trees::type_identity::portable_applied_lifetime_identity(
                typed,
                argument.as_str(),
                source_owner,
                &[],
            )
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or("selected map has an unresolved lifetime argument")?;
            telescope.push(ClosedConformanceParameterBinding {
                parameter: parameter.as_str().to_owned(),
                kind: ClosedConformanceParameterKind::Lifetime,
                argument,
            });
        }
        for (parameter, argument) in parameters.iter().zip(raw_arguments) {
            use typed_trees::data::TypeParameterKind;
            let kind = match parameter.kind {
                TypeParameterKind::Type => ClosedConformanceParameterKind::Type,
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    ClosedConformanceParameterKind::Const
                }
                TypeParameterKind::Machine { .. } | TypeParameterKind::Implementation => {
                    ClosedConformanceParameterKind::Machine
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err("selected map needs its complete proposition parameter contract");
                }
            };
            let argument = typed
                .declared_static_operand_identity(
                    typed_trees::type_identity::DeclaredStaticOperandIdentityRequest {
                        operand: typed_trees::type_identity::IndexIdentityArgument::StaticArgument(
                            argument,
                        ),
                        lexical_owner: source_owner,
                        binders: &[],
                        lifetime_binders: &[],
                    },
                )
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or("selected map argument has no exact declared identity")?;
            telescope.push(ClosedConformanceParameterBinding {
                parameter: parameter.name.as_str().to_owned(),
                kind,
                argument,
            });
        }
        let trait_lifetime_arguments = scope
            .portable_lifetime_arguments(typed)
            .ok_or("selected map trait has unresolved lifetime arguments")?
            .into_iter()
            .map(|identity| {
                typed
                    .type_identity_text(identity)
                    .filter(|identity| !identity.is_empty())
                    .ok_or("selected trait lifetime has no exact identity")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let trait_arguments = typed
            .trait_type_parameters(scope.declaration)
            .iter()
            .map(|parameter| {
                scope
                    .portable_parameter_identity(typed, parameter.symbol)
                    .and_then(|identity| typed.type_identity_text(identity))
                    .filter(|identity| !identity.is_empty())
                    .ok_or("selected trait argument has no exact identity")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut application = ClosedConformanceApplication {
            owner: group[0].0.0,
            declaration_identity: nominal(typed, sources, selected.symbol, identities)?,
            telescope,
            trait_identity: nominal(typed, sources, selected.trait_symbol, identities)?,
            trait_lifetime_arguments,
            trait_arguments,
            realization_callables: Vec::new(),
            schemas: Vec::new(),
            static_applications: Vec::new(),
            rows: Vec::new(),
            report_fingerprint: 0,
            commitment: Default::default(),
        };
        let mut source_schemas = Vec::with_capacity(selected.rows.len());
        for row in &selected.rows {
            let (declaring, required) = typed
                .trait_machine_signature(row.requirement)
                .filter(|(owner, _)| owner.symbol == row.declaring_trait)
                .ok_or("selected map row lost its exact public declaration")?;
            let declaring_scope = scope
                .declaring_scope(typed, row.declaring_trait)
                .ok_or("selected map row lost its applied declaring scope")?;
            let applied = typed
                .applied_callable_signature(&declaring_scope, required.symbol)
                .or_else(|| {
                    (declaring.type_parameters.is_empty()
                        && declaring.lifetime_parameters.is_empty())
                    .then_some(required)
                })
                .ok_or("selected map row has no complete applied requirement signature")?;
            let required_identity =
                typed.trait_requirement_callable_identity_text(declaring, applied);
            let public_identity =
                typed.trait_requirement_callable_identity_text(declaring, required);
            if required_identity.is_empty() || public_identity.is_empty() {
                return Err("selected map row has no exact public callable identity");
            }
            let (definition, parameter_kinds) =
                typed.requirement_schema_definition(declaring, required)?;
            let generic = !required.lifetime_parameters.is_empty() || !parameter_kinds.is_empty();
            let schema = ClosedConformanceSchema {
                requirement_declaration: NominalDeclarationIdentity::new(nominal(
                    typed,
                    sources,
                    required.symbol,
                    identities,
                )?),
                declaring_trait_identity: nominal(typed, sources, declaring.symbol, identities)?,
                public_requirement_identity: identities.intern(&public_identity),
                requirement_identity: identities.intern(&required_identity),
                realization_identity: nominal(typed, sources, row.realization_machine, identities)?,
                definition,
                kind: if generic {
                    ClosedConformanceSchemaKind::Open
                } else {
                    ClosedConformanceSchemaKind::Nongeneric
                },
                lifetime_parameter_count: u32::try_from(required.lifetime_parameters.len())
                    .map_err(|_| "requirement lifetime count overflows")?,
                parameter_kinds,
                finite_tuples: None,
            };
            if !generic {
                application.rows.push(ClosedConformanceRow {
                    requirement_declaration: schema.requirement_declaration.clone(),
                    declaring_trait_identity: schema.declaring_trait_identity,
                    public_requirement_identity: schema.public_requirement_identity,
                    family_tuple: Vec::new(),
                    requirement_identity: schema.requirement_identity,
                    realization_identity: schema.realization_identity,
                    realization_callable_identity: None,
                });
            }
            source_schemas.push((
                row.declaring_trait,
                row.requirement,
                schema.declaring_trait_identity,
                schema.public_requirement_identity,
            ));
            application.schemas.push(schema);
        }
        source_schemas
            .sort_unstable_by_key(|(declaring, requirement, _, _)| (*declaring, *requirement));
        if source_schemas
            .windows(2)
            .any(|pair| (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1))
        {
            return Err("selected map repeats an original requirement row");
        }
        application.schemas.sort_unstable_by(|left, right| {
            (
                identities.text(left.declaring_trait_identity),
                identities.text(left.public_requirement_identity),
            )
                .cmp(&(
                    identities.text(right.declaring_trait_identity),
                    identities.text(right.public_requirement_identity),
                ))
        });
        application.rows.sort_unstable_by(|left, right| {
            (
                identities.text(left.declaring_trait_identity),
                identities.text(left.public_requirement_identity),
            )
                .cmp(&(
                    identities.text(right.declaring_trait_identity),
                    identities.text(right.public_requirement_identity),
                ))
        });
        let application_index = module.closed_conformance_applications.len();
        for (_, required, original, body, _, _) in group {
            let (_, _, dispatch, _, _) = program
                .declaration_selections
                .get(*original)
                .and_then(Option::as_ref)
                .ok_or("selected header lost its source dispatch")?;
            let source_schema = source_schemas
                .binary_search_by_key(
                    &(dispatch.declaring_trait, dispatch.requirement),
                    |(declaring, requirement, _, _)| (*declaring, *requirement),
                )
                .ok()
                .and_then(|position| source_schemas.get(position))
                .ok_or("selected call has no exact original requirement row")?;
            let schema_key = (
                identities.text(source_schema.2),
                identities.text(source_schema.3),
            );
            let schema = application
                .schemas
                .binary_search_by(|schema| {
                    (
                        identities.text(schema.declaring_trait_identity),
                        identities.text(schema.public_requirement_identity),
                    )
                        .cmp(&schema_key)
                })
                .ok()
                .and_then(|position| application.schemas.get(position))
                .ok_or("selected call lost its prepared requirement schema")?;
            let machine = module
                .machine(*body)
                .ok_or("selected map has no emitted implementation")?;
            let declared = machine
                .declaration
                .as_ref()
                .ok_or("selected implementation has no exact source declaration")?;
            let callable = identities
                .text(declared.callable)
                .filter(|identity| !identity.is_empty())
                .ok_or("selected implementation lost its callable identity")?;
            let callable = terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                callable,
                declared
                    .specialization
                    .as_ref()
                    .map(|specialization| &specialization.commitment),
            );
            let callable = identities.intern(&callable);
            let source = *program
                .declaration_sources
                .get(*original)
                .ok_or("selected header lost its applied source signature")?;
            let arguments = match typed.closed_requirement_application(source.symbol) {
                Some(closed) => requirement_application(typed, sources, &closed, identities)?.1,
                None if schema.parameter_kinds.is_empty()
                    && schema.lifetime_parameter_count == 0 =>
                {
                    Vec::new()
                }
                None => return Err("selected generic member lost its actual closed application"),
            };
            let result = match &machine.result {
                TerminalMachineResult::Unit => ClosedConformanceCallableResult::Unit,
                TerminalMachineResult::Scalar(result) => {
                    ClosedConformanceCallableResult::Scalar(result.scalar_type)
                }
                TerminalMachineResult::Structural(result) => {
                    ClosedConformanceCallableResult::Structural(result.structural_type)
                }
            };
            application
                .realization_callables
                .push(ClosedConformanceRealizationCallable {
                    source_callable_identity: callable,
                    callable: TargetMachineCallable::Machine(*body),
                    result,
                });
            application
                .static_applications
                .push(ClosedConformanceStaticApplication {
                    public_requirement_identity: schema.public_requirement_identity,
                    realization_identity: schema.realization_identity,
                    source_callable_identity: callable,
                    callable: TargetMachineCallable::Machine(*body),
                    application_commitment: declared
                        .specialization
                        .as_ref()
                        .map(|specialization| specialization.commitment),
                    method_arguments: arguments,
                });
            links.push((
                *required,
                application_index,
                StaticRequirementDispatch {
                    conformance_application_report_fingerprint: 0,
                    conformance_application_commitment: Default::default(),
                    public_requirement_identity: schema.public_requirement_identity,
                    declaring_trait_identity: schema.declaring_trait_identity,
                    requirement_identity: schema.requirement_identity,
                    realization_identity: schema.realization_identity,
                    realization_callable_identity: callable,
                    realization: *body,
                },
            ));
        }
        application
            .realization_callables
            .sort_unstable_by(|left, right| {
                identities
                    .text(left.source_callable_identity)
                    .cmp(&identities.text(right.source_callable_identity))
            });
        application.realization_callables.dedup();
        let order = {
            let mut keys = application
                .static_applications
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    terminal_semantics::closed_conformances::static_application_order_key(
                        identities.as_table(),
                        row,
                    )
                    .map(|key| (key, index))
                })
                .collect::<Result<Vec<_>, _>>()?;
            keys.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            keys.into_iter().map(|(_, index)| index).collect::<Vec<_>>()
        };
        let mut rows = std::mem::take(&mut application.static_applications)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        for index in order {
            application.static_applications.push(
                rows.get_mut(index)
                    .and_then(Option::take)
                    .ok_or("selected method order lost its emitted row")?,
            );
        }
        application.static_applications.dedup();
        application.commitment = terminal_psi::closed_conformance_application_commitment(
            identities.as_table(),
            &application,
        );
        application.report_fingerprint =
            terminal_psi::closed_conformance_application_report_fingerprint(
                identities.as_table(),
                &application,
            );
        module.closed_conformance_applications.push(application);
        start = end;
    }
    Ok(links)
}
