//! Keep the complete implementation application and its checked method choices.
//! Historical report bytes can contain private names, so project retained typed
//! operands through the same portable context as the surrounding specialization.

use super::super::IndexIdentityArgument;
use super::super::constraint_identity::{atom, compound};
use super::super::coordinates::CoordinateTag;
use super::super::identity_context::TypeIdentityContext;
use super::super::substitution::index_argument;
use crate::TypedTrees;
use crate::expression::{StaticMachineArgument, StaticSymbolApplication};
use crate::name::Identifier;
use crate::typed_trees::{ClosedConformanceApplication, ClosedConformanceConstArgument};
use language_semantics::type_identity::vocabulary::Label;

pub(super) fn project(
    program: &TypedTrees,
    closed: &ClosedConformanceApplication,
    supplied: &StaticMachineArgument,
    context: &TypeIdentityContext<'_>,
) -> Result<String, String> {
    let declaration = program
        .conformance_by_symbol(closed.declaration)
        .ok_or("specialized callable lost its selected implementation declaration")?;
    if declaration.trait_symbol != closed.trait_definition {
        return Err("selected implementation changed its declared trait".into());
    }
    if declaration.rows.len() != closed.rows.len()
        || declaration
            .rows
            .iter()
            .zip(&closed.rows)
            .any(|(declared, selected)| {
                declared.declaring_trait != selected.declaring_trait
                    || declared.requirement != selected.requirement
                    || declared.realization_machine != selected.realization_machine
                    || declared.realization_state != selected.realization_state
            })
    {
        return Err(
            "selected implementation methods differ from its exact declaration rows".into(),
        );
    }
    let mut selected = StaticMachineArgument {
        symbol: closed.declaration,
        application: Some(Box::new(StaticSymbolApplication {
            member: Default::default(),
            lifetime_arguments: closed
                .lifetime_arguments
                .iter()
                .map(Identifier::generated)
                .collect(),
            arguments: closed.arguments.clone(),
        })),
        ..Default::default()
    };
    let mut supplied = supplied.clone();
    // An omitted empty telescope and an explicitly empty one name the same
    // nongeneric implementation. Keep every nonempty argument and region.
    if program.conformance_type_parameters(declaration).is_empty()
        && declaration.lifetime_parameters.is_empty()
    {
        for argument in [&mut supplied, &mut selected] {
            if argument.application.as_ref().is_some_and(|application| {
                !application.member.is_valid()
                    && application.arguments.is_empty()
                    && application.lifetime_arguments.is_empty()
            }) {
                argument.application = None;
            }
        }
    }
    let own_arguments = index_argument(
        program,
        IndexIdentityArgument::StaticArgument(&selected),
        context,
    );
    if own_arguments
        != index_argument(
            program,
            IndexIdentityArgument::StaticArgument(&supplied),
            context,
        )
    {
        return Err(
            "specialized callable conformance binding differs from its checked application".into(),
        );
    }
    let scope = closed
        .trait_scope
        .scope(program)
        .ok_or("selected implementation lost its prepared trait application")?;
    if scope.declaration.symbol != closed.trait_definition {
        return Err("selected implementation's prepared scope names another trait".into());
    }
    validate_selected_scope(program, declaration, closed, &scope, context)?;
    let trait_application = scope.with_identity_scopes(program, |scopes| {
        let mut arguments = Vec::new();
        for parameter in program.trait_type_parameters(scope.declaration) {
            let (reference, outer) = scope
                .parameter_application(parameter.symbol)
                .ok_or("selected implementation lost a target trait argument")?;
            let context = TypeIdentityContext {
                application_owner: scope.argument_owner,
                application_argument_owner: scope.argument_owner,
                application_scopes: &scopes[..outer],
                ..*context
            };
            let operand = if parameter.kind.is_const_or_value() {
                IndexIdentityArgument::TypeReference(reference)
            } else {
                IndexIdentityArgument::TypeArgument(reference)
            };
            arguments.push(index_argument(program, operand, &context));
        }
        let context = TypeIdentityContext {
            application_owner: scope.argument_owner,
            application_argument_owner: scope.argument_owner,
            application_scopes: scopes,
            ..*context
        };
        let lifetimes = scope
            .declaration
            .lifetime_parameters
            .iter()
            .map(|parameter| context.lifetime(program, parameter.as_str()));
        Ok::<_, String>(compound(
            CoordinateTag::TraitApplication,
            [
                context.name(program, closed.trait_definition, ""),
                compound(CoordinateTag::Arguments, arguments),
                compound(CoordinateTag::Lifetimes, lifetimes),
            ],
        ))
    })?;
    let type_identity = |reference| {
        index_argument(
            program,
            IndexIdentityArgument::TypeArgument(reference),
            context,
        )
    };
    let constants = closed.const_arguments.iter().map(|argument| {
        let fields = match argument {
            ClosedConformanceConstArgument::Evaluated {
                parameter_carrier,
                declared_carrier,
                value,
            } => vec![
                atom(Label::Kind, "evaluated"),
                type_identity(*parameter_carrier),
                type_identity(*declared_carrier),
                atom(CoordinateTag::Const, &value.encoding),
            ],
            ClosedConformanceConstArgument::CallerExpression {
                parameter_carrier,
                expression,
            } => vec![
                atom(Label::Kind, "expression"),
                type_identity(*parameter_carrier),
                index_argument(
                    program,
                    IndexIdentityArgument::TypeReference(*expression),
                    context,
                ),
            ],
            ClosedConformanceConstArgument::CallerBinder {
                parameter_carrier,
                binder,
                binder_carrier,
            } => vec![
                atom(Label::Kind, "binder"),
                type_identity(*parameter_carrier),
                type_identity(*binder_carrier),
                index_argument(program, IndexIdentityArgument::Subject(*binder), context),
            ],
        };
        compound(CoordinateTag::Const, fields)
    });
    let callable = |symbol| {
        index_argument(
            program,
            IndexIdentityArgument::StaticArgument(&StaticMachineArgument {
                symbol,
                ..Default::default()
            }),
            context,
        )
    };
    let mut rows = Vec::with_capacity(closed.rows.len());
    for row in &closed.rows {
        let (owner, requirement) = program
            .trait_machine_signature(row.requirement)
            .ok_or("selected implementation lost a checked requirement declaration")?;
        if owner.symbol != row.declaring_trait {
            return Err("selected implementation requirement belongs to another trait".into());
        }
        let machine = program
            .machine_by_symbol(row.realization_machine)
            .ok_or("selected implementation lost its checked method realization")?;
        if !program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == row.realization_state)
        {
            return Err(
                "selected implementation method and entry name different declarations".into(),
            );
        }
        rows.push(compound(
            CoordinateTag::Declaration,
            [
                context.name(program, row.declaring_trait, ""),
                context.qualify_non_binder_name(
                    program,
                    row.requirement,
                    program.trait_requirement_callable_identity_text(owner, requirement),
                ),
                callable(row.realization_machine),
                callable(row.realization_state),
            ],
        ));
    }
    Ok(compound(
        CoordinateTag::Declaration,
        [
            own_arguments,
            trait_application,
            compound(CoordinateTag::Const, constants),
            compound(
                CoordinateTag::Arguments,
                closed
                    .machine_arguments
                    .iter()
                    .map(|symbol| callable(*symbol)),
            ),
            compound(CoordinateTag::Declaration, rows),
        ],
    ))
}

/// Check the prepared substitutions against the selected arguments before using
/// them. A scope for another application of the same trait is not sufficient.
fn validate_selected_scope(
    program: &TypedTrees,
    declaration: &crate::trait_definition::Conformance,
    closed: &ClosedConformanceApplication,
    scope: &crate::trait_definition::AppliedTraitScope<'_>,
    context: &TypeIdentityContext<'_>,
) -> Result<(), String> {
    if scope.argument_owner != context.application_owner {
        return Err(
            "selected implementation scope belongs to another supplying declaration".into(),
        );
    }
    let parameters = program.conformance_type_parameters(declaration);
    let references = scope.with_identity_scopes(program, |scopes| {
        let own = scopes
            .first()
            .filter(|own| own.declaration == declaration.symbol)
            .ok_or("selected implementation scope lost its own argument frame")?;
        if own.arguments.len() != parameters.len()
            || closed.arguments.len() != parameters.len()
            || own.lifetimes.len() != declaration.lifetime_parameters.len()
            || closed.lifetime_arguments.len() != declaration.lifetime_parameters.len()
        {
            return Err("selected implementation scope has an incomplete own telescope".into());
        }
        for ((formal, actual), (parameter, supplied)) in own.lifetimes.iter().zip(
            declaration
                .lifetime_parameters
                .iter()
                .zip(&closed.lifetime_arguments),
        ) {
            if formal != parameter.as_str() || actual != supplied {
                return Err(
                    "selected implementation scope changed an own lifetime argument".into(),
                );
            }
        }
        let mut references = Vec::with_capacity(parameters.len());
        for (((binder, actual), parameter), supplied) in own
            .arguments
            .iter()
            .zip(parameters)
            .zip(closed.arguments.iter())
        {
            if *binder != parameter.symbol {
                return Err(
                    "selected implementation scope changed an own parameter declaration".into(),
                );
            }
            let reference = match actual {
                IndexIdentityArgument::TypeArgument(reference)
                | IndexIdentityArgument::TypeReference(reference) => *reference,
                _ => {
                    return Err(
                        "selected implementation scope lost its prepared type operand".into(),
                    );
                }
            };
            let actual = program
                .static_operand_from_type_reference(reference)
                .map_err(str::to_owned)?;
            if index_argument(
                program,
                IndexIdentityArgument::StaticArgument(&actual),
                context,
            ) != index_argument(
                program,
                IndexIdentityArgument::StaticArgument(supplied),
                context,
            ) {
                return Err("selected implementation scope changed an own static argument".into());
            }
            references.push(reference);
        }
        Ok::<_, String>(references)
    })?;
    let lifetimes = closed
        .lifetime_arguments
        .iter()
        .map(Identifier::generated)
        .collect::<Vec<_>>();
    let expected = crate::trait_definition::AppliedTraitScope::from_conformance_application(
        program,
        declaration,
        crate::trait_definition::TraitApplicationArguments {
            lexical_owner: context.application_owner,
            enclosing_data_applications: &[],
            arguments: &references,
            lifetimes: &lifetimes,
        },
    )
    .ok_or("selected implementation declaration has no matching prepared application")?;
    if !scope.same_application(program, &expected) {
        return Err("selected implementation's target trait arguments differ from its declaration application".into());
    }
    Ok(())
}
