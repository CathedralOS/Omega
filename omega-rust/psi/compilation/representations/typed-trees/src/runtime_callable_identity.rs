//! Changing the selected machine must not change a callable type.
//!
//! Compare the signature's own parameters by position while keeping surrounding
//! generic arguments in scope. Structured identities let nested types and
//! contract facts retain their declaration references.

use crate::TypedTrees;
use crate::callable_values::RuntimeCallableContractHandle;
use crate::contract_structure::{
    self, ContractProjection, DeclaredContractFact, DeclaredContractKind,
};
use crate::data::{MachineParameterContract, TypeParameterKind};
use crate::domain::ProofFact;
use crate::expression::StaticMachineArgument;
use crate::proposition::{PropositionApplication, PropositionBody, PropositionFormula};
use crate::signature::{AuthoredInvocationTarget, SignatureContractKind, StateSignature};
use crate::type_identity::{
    AppliedTypeIdentityScope, IndexIdentityArgument, LifetimeParameterIdentity, TypeIdentityRequest,
};
use crate::type_system::type_identity::constraint_identity::{atom, compound};
use crate::type_system::type_identity::identity_context::{
    TypeIdentityContext, normalize_type_reference,
};
use crate::type_system::type_identity::substitution;
use crate::types::TypeReferenceHandle;
use language_semantics::type_identity::vocabulary::{CallableForm, IndexForm, Label, TypeForm};
use std::cell::Cell;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCallableIdentityError {
    MissingContract,
    MissingSignature,
    RecursiveSignature,
    UnresolvedType,
    UnresolvedSymbol,
    UnresolvedContractFact,
    UnresolvedStaticContract,
    UnresolvedServiceReach,
    UnresolvedInvocation,
    UnresolvedProgress,
}

impl RuntimeCallableIdentityError {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::MissingContract => "callable identity names no retained contract",
            Self::MissingSignature => "callable identity has no complete signature owner",
            Self::RecursiveSignature => "callable identity contains a cyclic signature",
            Self::UnresolvedType => "callable identity contains an unresolved type or lifetime",
            Self::UnresolvedSymbol => "callable identity contains an unresolved declaration",
            Self::UnresolvedContractFact => {
                "callable identity contains an unresolved contract fact"
            }
            Self::UnresolvedStaticContract => {
                "callable identity contains an unresolved static binder contract"
            }
            Self::UnresolvedServiceReach => {
                "callable identity contains an unresolved service ceiling"
            }
            Self::UnresolvedInvocation => {
                "callable identity contains an unresolved invocation target"
            }
            Self::UnresolvedProgress => {
                "callable identity contains an unresolved termination premise"
            }
        }
    }
}

impl std::fmt::Display for RuntimeCallableIdentityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.reason())
    }
}

impl std::error::Error for RuntimeCallableIdentityError {}

type IdentityResult<T> = Result<T, RuntimeCallableIdentityError>;

fn record(role: &str, children: impl IntoIterator<Item = String>) -> String {
    compound(
        CallableForm::Record,
        std::iter::once(atom(Label::Kind, role)).chain(children),
    )
}

fn scalar(role: &str, value: &str) -> String {
    compound(
        CallableForm::Atom,
        [atom(Label::Kind, role), atom(Label::Encoding, value)],
    )
}

fn name(role: &str, value: &str) -> String {
    compound(
        CallableForm::Name,
        [atom(Label::Kind, role), atom(Label::Symbol, value)],
    )
}

fn index(role: &str, value: String) -> String {
    compound(CallableForm::Index, [atom(Label::Kind, role), value])
}

fn typed(role: &str, value: String) -> String {
    compound(CallableForm::Type, [atom(Label::Kind, role), value])
}

fn symbol(
    program: &TypedTrees,
    selected: SymbolHandle,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    if !selected.is_valid() {
        return Err(RuntimeCallableIdentityError::UnresolvedSymbol);
    }
    Ok(context.name(program, selected, ""))
}

fn type_text(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    if !program.contains_type_reference(reference) {
        return Err(RuntimeCallableIdentityError::UnresolvedType);
    }
    Ok(normalize_type_reference(program, reference, context))
}

pub(crate) fn normalize_contract(
    program: &TypedTrees,
    contract: RuntimeCallableContractHandle,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    with_contract_scope(program, contract, context, |signature, context| {
        normalize_signature(program, signature, context)
            .map(|signature| compound(TypeForm::Callable, [signature]))
    })
}

fn with_contract_scope<T>(
    program: &TypedTrees,
    contract: RuntimeCallableContractHandle,
    context: &TypeIdentityContext<'_>,
    use_scope: impl FnOnce(&StateSignature, &TypeIdentityContext<'_>) -> IdentityResult<T>,
) -> IdentityResult<T> {
    if !program.runtime_callable_contracts.is_valid(contract) {
        return Err(RuntimeCallableIdentityError::MissingContract);
    }
    let invalid = Cell::new(false);
    let retained = program.runtime_callable_contract(contract);
    let mut owners = context.callable_scope_owners.to_vec();
    owners.push((retained.signature.symbol, retained.lexical_owner));
    let context = TypeIdentityContext {
        structural_callable_identity: true,
        callable_scope_owners: &owners,
        invalid_application: Some(&invalid),
        invalid_index_argument: Some(&invalid),
        missing_exact_nominal_owner: Some(&invalid),
        ..*context
    };
    let signature = &retained.signature;
    let result = use_scope(signature, &context)?;
    if invalid.get() {
        return Err(RuntimeCallableIdentityError::UnresolvedType);
    }
    Ok(result)
}

fn with_signature_scope<T>(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
    use_scope: impl FnOnce(&TypeIdentityContext<'_>) -> IdentityResult<T>,
) -> IdentityResult<T> {
    if !signature.symbol.is_valid() {
        return Err(RuntimeCallableIdentityError::MissingSignature);
    }
    if context
        .active_callable_signatures
        .iter()
        .any(|active| std::ptr::eq(*active, signature))
    {
        return Err(RuntimeCallableIdentityError::RecursiveSignature);
    }
    let generics = program.state_signature_type_parameters(signature);
    let parameters = program.state_signature_parameters(signature);
    let own_symbols = generics
        .iter()
        .map(|parameter| parameter.symbol)
        .chain(parameters.iter().map(|parameter| parameter.symbol))
        .chain(
            signature
                .conformance_bounds
                .iter()
                .map(|bound| bound.binder),
        )
        .collect::<Vec<_>>();
    if own_symbols.iter().any(|symbol| !symbol.is_valid()) {
        return Err(RuntimeCallableIdentityError::MissingSignature);
    }
    let depth = context.active_callable_signatures.len();
    let mut binders = generics
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| (parameter.symbol, format!("$C{depth}G{ordinal}")))
        .chain(
            parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| (parameter.symbol, format!("$C{depth}V{ordinal}"))),
        )
        .chain(
            signature
                .conformance_bounds
                .iter()
                .enumerate()
                .map(|(ordinal, bound)| (bound.binder, format!("$C{depth}E{ordinal}"))),
        )
        .collect::<Vec<_>>();
    binders.extend(
        context
            .binders
            .iter()
            .filter(|(symbol, _)| !own_symbols.contains(symbol))
            .cloned(),
    );
    let substitutions = context
        .substitutions
        .iter()
        .filter(|(symbol, _)| !own_symbols.contains(symbol))
        .copied()
        .collect::<Vec<_>>();
    let starts = context
        .substitution_scope_starts
        .iter()
        .map(|start| {
            context.substitutions[..(*start).min(context.substitutions.len())]
                .iter()
                .filter(|(symbol, _)| !own_symbols.contains(symbol))
                .count()
        })
        .collect::<Vec<_>>();
    let index_substitutions = context
        .index_substitutions
        .iter()
        .filter(|(symbol, _)| !own_symbols.contains(symbol))
        .copied()
        .collect::<Vec<_>>();
    let scope_arguments = context
        .application_scopes
        .iter()
        .map(|scope| {
            scope
                .arguments
                .iter()
                .filter(|(symbol, _)| !own_symbols.contains(symbol))
                .copied()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let scopes = context
        .application_scopes
        .iter()
        .zip(&scope_arguments)
        .filter(|(scope, _)| scope.declaration != signature.symbol)
        .map(|(scope, arguments)| AppliedTypeIdentityScope {
            arguments,
            ..*scope
        })
        .collect::<Vec<_>>();
    let first_lifetime = context
        .lifetime_binders
        .iter()
        .map(|(_, ordinal)| *ordinal)
        .max()
        .map_or(0, |ordinal| ordinal + 1);
    let mut lifetimes = signature
        .lifetime_parameters
        .iter()
        .enumerate()
        .map(|(ordinal, _)| {
            (
                LifetimeParameterIdentity {
                    declaration: signature.symbol,
                    ordinal,
                },
                first_lifetime + ordinal,
            )
        })
        .collect::<Vec<_>>();
    lifetimes.extend(
        context
            .lifetime_binders
            .iter()
            .filter(|(identity, _)| identity.declaration != signature.symbol)
            .copied(),
    );
    let resolved_lifetimes = context
        .resolved_lifetimes
        .iter()
        .filter(|(identity, _)| identity.declaration != signature.symbol)
        .copied()
        .collect::<Vec<_>>();
    let mut active = context.active_callable_signatures.to_vec();
    active.push(signature);
    let nested = TypeIdentityContext {
        binders: &binders,
        substitutions: &substitutions,
        substitution_scope_starts: &starts,
        index_substitutions: &index_substitutions,
        application_scopes: &scopes,
        application_owner: signature.symbol,
        lifetime_binders: &lifetimes,
        resolved_lifetimes: &resolved_lifetimes,
        active_callable_signatures: &active,
        ..*context
    };
    use_scope(&nested)
}

fn normalize_signature(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    with_signature_scope(program, signature, context, |nested| {
        let generics = program.state_signature_type_parameters(signature);
        let parameters = program.state_signature_parameters(signature);
        let depth = nested.active_callable_signatures.len() - 1;
        let static_binders = generics
            .iter()
            .map(|parameter| {
                let kind = match &parameter.kind {
                    TypeParameterKind::Type => scalar("kind", "type"),
                    TypeParameterKind::Implementation => scalar("kind", "implementation"),
                    TypeParameterKind::Const { type_reference } => record(
                        "const",
                        [typed("type", type_text(program, *type_reference, &nested)?)],
                    ),
                    TypeParameterKind::Value { type_reference } => record(
                        "value",
                        [typed("type", type_text(program, *type_reference, &nested)?)],
                    ),
                    TypeParameterKind::Machine { contract } => {
                        machine_contract(program, contract, &nested)?
                    }
                    TypeParameterKind::Proposition { contract } => {
                        let formals = program.state_parameters.span_or_empty(contract.parameters);
                        let mut local = formals
                            .iter()
                            .enumerate()
                            .map(|(ordinal, formal)| {
                                (formal.symbol, format!("$C{depth}P{ordinal}"))
                            })
                            .collect::<Vec<_>>();
                        local.extend(nested.binders.iter().cloned());
                        let context = TypeIdentityContext {
                            binders: &local,
                            ..*nested
                        };
                        record(
                            "proposition",
                            formals
                                .iter()
                                .map(|formal| parameter_shape(program, formal, &context))
                                .collect::<IdentityResult<Vec<_>>>()?,
                        )
                    }
                };
                let mut values = vec![kind];
                if !nested.callable_operand_shape {
                    values.push(properties(parameter.bounds));
                }
                Ok(record("static-binder", values))
            })
            .collect::<IdentityResult<Vec<_>>>()?;
        let parameter_shapes = parameters
            .iter()
            .map(|parameter| parameter_shape(program, parameter, &nested))
            .collect::<IdentityResult<Vec<_>>>()?;
        let mut parts = vec![
            record("static-binders", static_binders),
            record("parameters", parameter_shapes),
            typed(
                "result",
                type_text(program, signature.return_type, &nested)?,
            ),
        ];
        if !nested.callable_operand_shape {
            parts.push(scalar(
                "lifetime-binders",
                &signature.lifetime_parameters.len().to_string(),
            ));
            parts.push(conformances(program, signature, &nested)?);
            parts.push(clauses(program, signature, &nested)?);
            parts.push(reach(program, signature, &nested)?);
            parts.push(invocations(program, signature, &nested)?);
            parts.push(scalar(
                "suspends",
                if signature.suspends { "yes" } else { "no" },
            ));
            parts.push(scalar(
                "blocks",
                if signature.blocks { "yes" } else { "no" },
            ));
            parts.push(termination(program, signature, &nested)?);
            parts.push(callbacks(program, signature, &nested)?);
        }
        Ok(record("signature", parts))
    })
}

fn parameter_shape(
    program: &TypedTrees,
    parameter: &crate::signature::StateParameter,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    Ok(record(
        "parameter",
        [
            scalar("mutable", if parameter.is_mutable { "yes" } else { "no" }),
            scalar("constant", if parameter.is_const { "yes" } else { "no" }),
            scalar(
                "relevance",
                if parameter.relevance.is_erased() {
                    "erased"
                } else {
                    "relevant"
                },
            ),
            typed(
                "type",
                type_text(program, parameter.type_reference, context)?,
            ),
        ],
    ))
}

fn properties(bounds: crate::data::DataProperties) -> String {
    use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension, Multiplicity};
    let multiplicity = match bounds.multiplicity {
        Multiplicity::Unrestricted => "copy",
        Multiplicity::Affine => "affine",
        Multiplicity::Linear => "linear",
    };
    let mut parts = vec![scalar("multiplicity", multiplicity)];
    if let Some(carry) = bounds.carry {
        parts.extend([
            scalar(
                "suspension",
                match carry.suspension {
                    CarrySuspension::Forbidden => "forbidden",
                    CarrySuspension::Allowed => "allowed",
                },
            ),
            scalar(
                "cpu",
                match carry.cpu {
                    CarryCpu::Origin => "origin",
                    CarryCpu::Any => "any",
                },
            ),
            scalar(
                "thread",
                match carry.host_thread {
                    CarryHostThread::Origin => "origin",
                    CarryHostThread::Any => "any",
                },
            ),
            scalar(
                "address",
                match carry.address {
                    CarryAddress::Stable => "stable",
                    CarryAddress::Movable => "movable",
                },
            ),
        ]);
    } else {
        parts.push(scalar("carry", "omitted"));
    }
    record("properties", parts)
}

fn machine_contract(
    program: &TypedTrees,
    contract: &MachineParameterContract,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    Ok(match contract {
        MachineParameterContract::RequirementIdentity => {
            scalar("machine-binder", "requirement-identity")
        }
        MachineParameterContract::Structural(signature) => record(
            "machine-binder",
            [normalize_signature(program, signature, context)?],
        ),
        MachineParameterContract::Nominal {
            receiving_trait,
            receiving_arguments,
            trait_definition,
            requirement,
            trait_arguments,
            instantiated,
        } => {
            let signature = instantiated
                .as_deref()
                .ok_or(RuntimeCallableIdentityError::UnresolvedStaticContract)?;
            if context.callable_operand_shape {
                return Ok(record(
                    "machine-binder",
                    [normalize_signature(program, signature, context)?],
                ));
            }
            let mut parts = vec![
                name(
                    "receiving-trait",
                    &symbol(program, *receiving_trait, context)?,
                ),
                name(
                    "declaring-trait",
                    &symbol(program, *trait_definition, context)?,
                ),
                name("requirement", &symbol(program, *requirement, context)?),
            ];
            parts.push(record(
                "receiving-arguments",
                receiving_arguments
                    .iter()
                    .map(|reference| {
                        Ok(typed("argument", type_text(program, *reference, context)?))
                    })
                    .collect::<IdentityResult<Vec<_>>>()?,
            ));
            parts.push(record(
                "trait-arguments",
                trait_arguments
                    .iter()
                    .map(|reference| {
                        Ok(typed("argument", type_text(program, *reference, context)?))
                    })
                    .collect::<IdentityResult<Vec<_>>>()?,
            ));
            parts.push(normalize_signature(program, signature, context)?);
            record("nominal-machine-binder", parts)
        }
    })
}

fn conformances(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let mut bounds =
        signature
            .conformance_bounds
            .iter()
            .map(|bound| {
                let mut parts = vec![
                    name("binder", &symbol(program, bound.binder, context)?),
                    name("trait", &symbol(program, bound.trait_symbol, context)?),
                ];
                parts.extend(bound.lifetime_arguments.iter().map(|lifetime| {
                    index("lifetime", context.lifetime(program, lifetime.as_str()))
                }));
                parts.extend(
                    bound
                        .arguments
                        .iter()
                        .map(|reference| {
                            Ok(typed("argument", type_text(program, *reference, context)?))
                        })
                        .collect::<IdentityResult<Vec<_>>>()?,
                );
                Ok(record("conformance", parts))
            })
            .collect::<IdentityResult<Vec<_>>>()?;
    bounds.sort();
    bounds.dedup();
    Ok(record("conformances", bounds))
}

fn clauses(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let mut projection = ContractProjection {
        type_identity: |reference, _: &[(SymbolHandle, String)]| {
            Ok(program
                .type_identities
                .intern(&type_text(program, reference, context)?))
        },
        fact_identity: |source: &ProofFact, _: &[(SymbolHandle, String)]| {
            fact(program, source, context)
        },
        symbol_identity: |selected, _: &[(SymbolHandle, String)]| {
            context.name(program, selected, "")
        },
        crash_cause: |cause| match cause {
            crate::signature::CrashCause::Trap => semantic_vocabulary::CrashCause::Trap,
            crate::signature::CrashCause::Abort => semantic_vocabulary::CrashCause::Abort,
        },
    };
    let projected = contract_structure::declared_callable_with(
        program,
        program.state_signature_parameters(signature),
        signature.return_type,
        program.state_signature_contracts(signature),
        context.binders,
        true,
        &mut projection,
    )?;
    let mut clauses = projected
        .clauses
        .into_iter()
        .map(|clause| {
            let mut parts = Vec::new();
            let role = match clause.kind {
                DeclaredContractKind::Requires => "requires",
                DeclaredContractKind::Ensures => "ensures",
                DeclaredContractKind::EnsuresForResultCase { data, case } => {
                    parts.extend([name("data", &data), name("case", &case)]);
                    "ensures-case"
                }
                DeclaredContractKind::Crashes(cause) => {
                    parts.push(scalar(
                        "cause",
                        match cause {
                            semantic_vocabulary::CrashCause::Trap => "trap",
                            semantic_vocabulary::CrashCause::Abort => "abort",
                        },
                    ));
                    "crashes"
                }
            };
            parts.push(match clause.fact {
                Some(fact) => encode_fact(program, fact)?,
                None => scalar("guard", "unconditional"),
            });
            Ok(record(role, parts))
        })
        .collect::<IdentityResult<Vec<_>>>()?;
    clauses.sort();
    clauses.dedup();
    let routes = program
        .state_signature_contracts(signature)
        .iter()
        .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }));
    if routes && signature.declares_no_crash_route {
        return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
    }
    clauses.push(scalar(
        "crash-interface",
        if signature.declares_no_crash_route {
            "none"
        } else if routes {
            "routes"
        } else {
            "open"
        },
    ));
    Ok(record("clauses", clauses))
}

fn fact(
    program: &TypedTrees,
    fact: &ProofFact,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<DeclaredContractFact> {
    if let ProofFact::Proposition(application) = fact {
        return Ok(DeclaredContractFact::Proposition(proposition(
            program,
            application,
            context,
            &[],
        )?));
    }
    contract_structure::project_declared_fact_with(
        program,
        fact,
        |expression| {
            program
                .expression_table
                .expression_is_valid(expression)
                .then(|| {
                    substitution::index_argument(
                        program,
                        IndexIdentityArgument::Expression(expression),
                        context,
                    )
                })
        },
        |reference| {
            type_text(program, reference, context)
                .ok()
                .map(|text| program.type_identities.intern(&text))
        },
        |selected| {
            if !selected.is_valid() {
                substitution::rejected(context);
            }
            context.name(program, selected, "")
        },
        |_| None,
    )
    .ok_or(RuntimeCallableIdentityError::UnresolvedContractFact)
}

fn encode_fact(program: &TypedTrees, fact: DeclaredContractFact) -> IdentityResult<String> {
    Ok(match fact {
        DeclaredContractFact::Expression(expression) => index("expression", expression),
        DeclaredContractFact::Membership {
            value,
            domain,
            arguments,
        } => {
            let mut parts = vec![index("subject", value), name("domain", &domain)];
            for argument in arguments {
                parts.push(typed(
                    "argument",
                    program
                        .type_identity_text(argument)
                        .ok_or(RuntimeCallableIdentityError::UnresolvedType)?,
                ));
            }
            record("membership", parts)
        }
        DeclaredContractFact::Proposition(value) => value,
        DeclaredContractFact::ContentConservation(_) => {
            return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
        }
    })
}

fn validate_proposition_operand_roles(
    program: &TypedTrees,
    declaration: &crate::proposition::PropositionDefinition,
    application: &PropositionApplication,
) -> IdentityResult<()> {
    use crate::proposition::{
        PropositionBinderArgumentKind as Actual, PropositionBinderKind as Formal,
    };
    let formals = program.proposition_binders(declaration);
    if formals.len() != application.binder_arguments.len()
        || formals
            .iter()
            .zip(&application.binder_arguments)
            .any(|(formal, actual)| {
                !matches!(
                    (&formal.kind, actual.kind),
                    (Formal::Type, Actual::Type)
                        | (Formal::Const { .. }, Actual::Const)
                        | (Formal::Machine, Actual::Machine)
                )
            })
    {
        return Err(RuntimeCallableIdentityError::UnresolvedStaticContract);
    }
    Ok(())
}

fn with_proposition_scope<T>(
    program: &TypedTrees,
    declaration: &crate::proposition::PropositionDefinition,
    operands: &[StaticMachineArgument],
    arguments: &[crate::expression::ExpressionHandle],
    context: &TypeIdentityContext<'_>,
    use_scope: impl FnOnce(&TypeIdentityContext<'_>) -> IdentityResult<T>,
) -> IdentityResult<T> {
    let binders = program.proposition_binders(declaration);
    let parameters = program.proposition_parameters(declaration);
    if binders.len() != operands.len() || parameters.len() != arguments.len() {
        return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
    }
    let replacements = binders
        .iter()
        .zip(operands)
        .map(|(binder, argument)| {
            (
                binder.symbol,
                IndexIdentityArgument::StaticArgument(argument),
            )
        })
        .chain(
            parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, argument)| {
                    (
                        parameter.symbol,
                        IndexIdentityArgument::Expression(*argument),
                    )
                }),
        )
        .collect::<Vec<_>>();
    let mut scopes = context.application_scopes.to_vec();
    scopes.push(AppliedTypeIdentityScope {
        declaration: declaration.symbol,
        arguments: &replacements,
        lifetimes: &[],
    });
    let nested = TypeIdentityContext {
        application_scopes: &scopes,
        ..*context
    };
    use_scope(&nested)
}

fn nominal_proposition_endpoint(
    program: &TypedTrees,
    application: &PropositionApplication,
    context: &TypeIdentityContext<'_>,
    active: &[SymbolHandle],
) -> IdentityResult<crate::proposition::NormalizedPropositionApplicationIdentity> {
    use crate::proposition::{
        NormalizedEvidenceInterfaceIdentity, NormalizedPropositionApplicationIdentity,
        NormalizedPropositionBinderArgument, PropositionBinderArgumentKind, PropositionBinderKind,
        PropositionEvidenceClassification,
    };
    if !application.proposition.is_valid() || active.contains(&application.proposition) {
        return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
    }
    let declaration = program
        .proposition_definition_by_symbol(application.proposition)
        .ok_or(RuntimeCallableIdentityError::UnresolvedContractFact)?;
    validate_proposition_operand_roles(program, declaration, application)?;
    let operands = application
        .binder_arguments
        .iter()
        .map(|argument| StaticMachineArgument {
            symbol: argument.symbol,
            path: argument.path.clone(),
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.clone(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    with_proposition_scope(
        program,
        declaration,
        &operands,
        arguments,
        context,
        |scope| {
            if let PropositionBody::Transparent {
                proposition: PropositionFormula::Application(expansion),
            } = &declaration.body
            {
                let mut active = active.to_vec();
                active.push(application.proposition);
                return nominal_proposition_endpoint(program, expansion, scope, &active);
            }
            let classification = match &declaration.body {
                PropositionBody::Primitive => PropositionEvidenceClassification::FactOnly,
                PropositionBody::Witness { evidence } => {
                    let evidence_identity = type_text(program, *evidence, scope)?;
                    let head = match program.type_reference_table.type_reference(*evidence) {
                        crate::types::TypeReferenceNode::Named { symbol, .. } => {
                            Some((*symbol, &[][..]))
                        }
                        crate::types::TypeReferenceNode::Generic {
                            base_symbol,
                            arguments,
                            ..
                        } => Some((*base_symbol, program.type_reference_handles(*arguments))),
                        _ => None,
                    };
                    let interface = if let Some((trait_symbol, arguments)) =
                        head.filter(|(symbol, _)| {
                            symbol.is_valid()
                                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Trait
                        }) {
                        let root_arguments = arguments
                            .iter()
                            .map(|argument| type_text(program, *argument, scope))
                            .collect::<IdentityResult<Vec<_>>>()?;
                        let requirements = crate::proposition::normalized_evidence_requirements(
                            program,
                            trait_symbol,
                            &root_arguments,
                        )?;
                        Some(NormalizedEvidenceInterfaceIdentity {
                            trait_symbol,
                            arguments: root_arguments
                                .iter()
                                .map(|argument| program.type_identities.intern(argument))
                                .collect(),
                            requirements,
                        })
                    } else {
                        None
                    };
                    PropositionEvidenceClassification::Witness {
                        evidence: evidence_identity,
                        interface,
                    }
                }
                PropositionBody::Transparent { .. } => {
                    return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
                }
            };
            Ok(NormalizedPropositionApplicationIdentity {
                declaration: declaration.symbol,
                name: declaration.name.as_str().to_owned(),
                binder_arguments: program
                    .proposition_binders(declaration)
                    .iter()
                    .zip(&operands)
                    .map(|(binder, operand)| NormalizedPropositionBinderArgument {
                        kind: match binder.kind {
                            PropositionBinderKind::Type => PropositionBinderArgumentKind::Type,
                            PropositionBinderKind::Const { .. } => {
                                PropositionBinderArgumentKind::Const
                            }
                            PropositionBinderKind::Machine => {
                                PropositionBinderArgumentKind::Machine
                            }
                        },
                        identity: substitution::index_argument(
                            program,
                            IndexIdentityArgument::StaticArgument(operand),
                            context,
                        ),
                    })
                    .collect(),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        substitution::index_argument(
                            program,
                            IndexIdentityArgument::Expression(*argument),
                            context,
                        )
                    })
                    .collect(),
                classification,
            })
        },
    )
}

fn proposition(
    program: &TypedTrees,
    application: &PropositionApplication,
    context: &TypeIdentityContext<'_>,
    active: &[SymbolHandle],
) -> IdentityResult<String> {
    if !application.proposition.is_valid() || active.contains(&application.proposition) {
        return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
    }
    let operands = application
        .binder_arguments
        .iter()
        .map(|argument| StaticMachineArgument {
            symbol: argument.symbol,
            path: argument.path.clone(),
            const_literal: argument.const_literal.clone(),
            evidence_projection: argument.evidence_projection.clone(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let arguments = program
        .expression_table
        .expression_handles(application.arguments);
    let mut parts = vec![index(
        "declaration",
        substitution::index_argument(
            program,
            IndexIdentityArgument::Subject(application.proposition),
            context,
        ),
    )];
    parts.extend(operands.iter().map(|argument| {
        index(
            "static-argument",
            substitution::index_argument(
                program,
                IndexIdentityArgument::StaticArgument(argument),
                context,
            ),
        )
    }));
    parts.extend(arguments.iter().map(|argument| {
        index(
            "argument",
            substitution::index_argument(
                program,
                IndexIdentityArgument::Expression(*argument),
                context,
            ),
        )
    }));
    let Some(declaration) = program.proposition_definition_by_symbol(application.proposition)
    else {
        if program.symbols.get(application.proposition).kind
            != symbols::SymbolKind::PropositionParameter
        {
            return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
        }
        let parameter = program
            .first_data_type_parameter_by_symbol(application.proposition)
            .ok_or(RuntimeCallableIdentityError::UnresolvedContractFact)?;
        let TypeParameterKind::Proposition { contract } = &parameter.kind else {
            return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
        };
        if !operands.is_empty() || contract.parameters.len() != arguments.len() {
            return Err(RuntimeCallableIdentityError::UnresolvedContractFact);
        }
        return Ok(record("proposition-parameter", parts));
    };
    validate_proposition_operand_roles(program, declaration, application)?;
    with_proposition_scope(
        program,
        declaration,
        &operands,
        arguments,
        context,
        |nested| match &declaration.body {
            PropositionBody::Primitive => Ok(record("primitive-proposition", parts)),
            PropositionBody::Witness { evidence } => {
                parts.push(typed("evidence", type_text(program, *evidence, &nested)?));
                Ok(record("witness-proposition", parts))
            }
            PropositionBody::Transparent { proposition: body } => {
                let mut active = active.to_vec();
                active.push(application.proposition);
                match body {
                    PropositionFormula::BooleanExpression(expression) => Ok(record(
                        "boolean-proposition",
                        [index(
                            "expression",
                            substitution::index_argument(
                                program,
                                IndexIdentityArgument::Expression(*expression),
                                &nested,
                            ),
                        )],
                    )),
                    PropositionFormula::Application(expansion) => {
                        proposition(program, expansion, &nested, &active)
                    }
                }
            }
        },
    )
}

fn reach(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let mut row = signature.service_reach_row;
    let mut visited = Vec::new();
    let open = row.0 == 0;
    while let Some(bound) = program.service_reach_rows.abstract_bound(row) {
        if visited.contains(&row) {
            return Err(RuntimeCallableIdentityError::UnresolvedServiceReach);
        }
        visited.push(row);
        row = bound;
    }
    if !open && (row.0 == 0 || row.0 as usize > program.service_reach_rows.len()) {
        return Err(RuntimeCallableIdentityError::UnresolvedServiceReach);
    }
    let mut parts = program
        .service_reach_rows
        .services(row)
        .iter()
        .map(|service| {
            let definition = program
                .service_reaches
                .definition(*service)
                .ok_or(RuntimeCallableIdentityError::UnresolvedServiceReach)?;
            Ok(name(
                "service",
                &symbol(program, definition.symbol, context)?,
            ))
        })
        .collect::<IdentityResult<Vec<_>>>()?;
    for authored in program.authored_service_reach_rows_for(signature.symbol) {
        for target in &authored.targets {
            if !target.service.is_valid() {
                return Err(RuntimeCallableIdentityError::UnresolvedServiceReach);
            }
            if program
                .service_reaches
                .id_for_symbol(target.service)
                .is_none()
            {
                parts.push(index(
                    "static-dependency",
                    substitution::index_argument(
                        program,
                        IndexIdentityArgument::Subject(target.service),
                        context,
                    ),
                ));
            }
        }
    }
    parts.sort();
    parts.dedup();
    parts.insert(
        0,
        scalar(
            "ceiling",
            if open {
                "open"
            } else if signature.service_reach_is_installation_bound || !visited.is_empty() {
                "installation-bound"
            } else {
                "finite"
            },
        ),
    );
    Ok(record("reach", parts))
}

fn invocations(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let parameters = program.state_signature_parameters(signature);
    let mut targets = program
        .state_signature_invokes(signature)
        .iter()
        .map(|invocation| {
            Ok(match invocation.target {
                AuthoredInvocationTarget::Unresolved => {
                    return Err(RuntimeCallableIdentityError::UnresolvedInvocation);
                }
                AuthoredInvocationTarget::Parameter {
                    ordinal,
                    symbol: selected,
                } => {
                    if parameters
                        .get(ordinal as usize)
                        .is_none_or(|parameter| parameter.symbol != selected)
                    {
                        return Err(RuntimeCallableIdentityError::UnresolvedInvocation);
                    }
                    name("parameter", &symbol(program, selected, context)?)
                }
                AuthoredInvocationTarget::Service(selected) => {
                    name("service", &symbol(program, selected, context)?)
                }
            })
        })
        .collect::<IdentityResult<Vec<_>>>()?;
    targets.sort();
    targets.dedup();
    Ok(record("invokes", targets))
}

fn termination(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let language_semantics::TerminationGuarantee::Terminates { premises } =
        &signature.termination_guarantee
    else {
        return Ok(scalar("termination", "none"));
    };
    let mut encoded = premises
        .iter()
        .map(|premise| {
            let mut definitions = program
                .domain_definitions()
                .iter()
                .filter(|definition| definition.semantic_id == premise.profile);
            let definition = definitions
                .next()
                .ok_or(RuntimeCallableIdentityError::UnresolvedProgress)?;
            if definitions.next().is_some() {
                return Err(RuntimeCallableIdentityError::UnresolvedProgress);
            }
            let mut parts = vec![
                name("profile", &symbol(program, definition.symbol, context)?),
                name("subject", &symbol(program, premise.subject.root, context)?),
            ];
            parts.extend(
                premise
                    .subject
                    .projections
                    .iter()
                    .map(|projection| {
                        Ok(name("projection", &symbol(program, *projection, context)?))
                    })
                    .collect::<IdentityResult<Vec<_>>>()?,
            );
            Ok(record("premise", parts))
        })
        .collect::<IdentityResult<Vec<_>>>()?;
    encoded.sort();
    encoded.dedup();
    Ok(record("terminates", encoded))
}

fn callbacks(
    program: &TypedTrees,
    signature: &StateSignature,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    let binders = program.state_signature_type_parameters(signature);
    Ok(record(
        "native-callbacks",
        signature
            .native_callback_parameters
            .iter()
            .map(|callback| {
                let mut matching = binders.iter().filter(|binder| {
                    binder.name == callback.binder
                        && matches!(binder.kind, TypeParameterKind::Machine { .. })
                });
                let binder = matching
                    .next()
                    .ok_or(RuntimeCallableIdentityError::UnresolvedStaticContract)?;
                if matching.next().is_some() {
                    return Err(RuntimeCallableIdentityError::UnresolvedStaticContract);
                }
                Ok(record(
                    "callback",
                    [
                        scalar("native-ordinal", &callback.native_ordinal.to_string()),
                        name("binder", &symbol(program, binder.symbol, context)?),
                    ],
                ))
            })
            .collect::<IdentityResult<Vec<_>>>()?,
    ))
}

pub(crate) fn normalize_machine_value(
    program: &TypedTrees,
    value: &crate::callable_values::MachineValue,
    context: &TypeIdentityContext<'_>,
) -> IdentityResult<String> {
    use crate::static_applications::StaticMachineBindingValue;
    let invalid = Cell::new(false);
    let context = &TypeIdentityContext {
        structural_callable_identity: true,
        invalid_application: Some(&invalid),
        invalid_index_argument: Some(&invalid),
        missing_exact_nominal_owner: Some(&invalid),
        ..*context
    };
    if !value.selected.symbol.is_valid() {
        return Err(RuntimeCallableIdentityError::UnresolvedSymbol);
    }
    let mut parts = vec![
        index(
            "selection",
            substitution::index_argument(
                program,
                IndexIdentityArgument::StaticArgument(&value.selected),
                context,
            ),
        ),
        typed(
            "contract",
            type_text(program, value.type_reference, context)?,
        ),
    ];
    if value.application.is_valid() {
        let application = program
            .expression_table
            .static_machine_applications()
            .find_map(|(handle, row)| (handle == value.application).then_some(row))
            .ok_or(RuntimeCallableIdentityError::UnresolvedStaticContract)?;
        parts.push(name(
            "declaration",
            &symbol(program, application.binding_declaration, context)?,
        ));
        parts.push(name(
            "entry",
            &symbol(program, application.registration_operation, context)?,
        ));
        let bindings = program
            .static_machine_application_bindings(value.application)
            .iter()
            .enumerate()
            .map(|(ordinal, binding)| {
                let supplied = match &binding.value {
                    StaticMachineBindingValue::Type(reference) => {
                        typed("type", type_text(program, *reference, context)?)
                    }
                    StaticMachineBindingValue::Const(reference) => index(
                        "const",
                        substitution::index_argument(
                            program,
                            IndexIdentityArgument::TypeReference(*reference),
                            context,
                        ),
                    ),
                    StaticMachineBindingValue::Machine(argument)
                    | StaticMachineBindingValue::Conformance(argument) => index(
                        "selection",
                        substitution::index_argument(
                            program,
                            IndexIdentityArgument::StaticArgument(argument),
                            context,
                        ),
                    ),
                    StaticMachineBindingValue::RuntimeValue { .. } => {
                        return Err(RuntimeCallableIdentityError::UnresolvedStaticContract);
                    }
                };
                Ok(record(
                    "binding",
                    [scalar("ordinal", &ordinal.to_string()), supplied],
                ))
            })
            .collect::<IdentityResult<Vec<_>>>()?;
        parts.push(record("application", bindings));
    }
    match (value.conformance.is_valid(), value.requirement.is_valid()) {
        (true, true) => parts.extend([
            name("conformance", &symbol(program, value.conformance, context)?),
            name("requirement", &symbol(program, value.requirement, context)?),
        ]),
        (false, false) => {}
        _ => return Err(RuntimeCallableIdentityError::UnresolvedStaticContract),
    }
    if invalid.get() {
        return Err(RuntimeCallableIdentityError::UnresolvedType);
    }
    Ok(compound(
        IndexForm::MachineValue,
        [record("machine-selection", parts)],
    ))
}

impl TypedTrees {
    pub fn runtime_machine_value_identity(
        &self,
        value: &crate::callable_values::MachineValue,
        binders: &[(SymbolHandle, String)],
    ) -> IdentityResult<String> {
        normalize_machine_value(
            self,
            value,
            &TypeIdentityContext {
                binders,
                ..Default::default()
            },
        )
    }

    /// Normalize an operand under this exact signature's own positional
    /// binders while retaining surrounding declaration/application identity.
    pub fn runtime_callable_proposition_application(
        &self,
        contract: RuntimeCallableContractHandle,
        application: &PropositionApplication,
        binders: &[(SymbolHandle, String)],
    ) -> IdentityResult<crate::proposition::NormalizedPropositionApplicationIdentity> {
        with_contract_scope(
            self,
            contract,
            &TypeIdentityContext {
                binders,
                ..Default::default()
            },
            |signature, context| {
                with_signature_scope(self, signature, context, |scope| {
                    nominal_proposition_endpoint(self, application, scope, &[])
                })
            },
        )
    }

    pub fn runtime_callable_index_identity(
        &self,
        contract: RuntimeCallableContractHandle,
        argument: IndexIdentityArgument<'_>,
        binders: &[(SymbolHandle, String)],
    ) -> IdentityResult<String> {
        with_contract_scope(
            self,
            contract,
            &TypeIdentityContext {
                binders,
                ..Default::default()
            },
            |signature, context| {
                with_signature_scope(self, signature, context, |scope| {
                    Ok(substitution::index_argument(self, argument, scope))
                })
            },
        )
    }

    pub fn runtime_callable_contract_identity(
        &self,
        contract: RuntimeCallableContractHandle,
        binders: &[(SymbolHandle, String)],
    ) -> IdentityResult<String> {
        normalize_contract(
            self,
            contract,
            &TypeIdentityContext {
                binders,
                ..Default::default()
            },
        )
    }

    pub fn runtime_callable_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> IdentityResult<String> {
        runtime_type_identity(self, request, false)
    }

    pub fn runtime_callable_operand_shape(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> IdentityResult<String> {
        runtime_type_identity(self, request, true)
    }
}

fn runtime_type_identity(
    program: &TypedTrees,
    request: TypeIdentityRequest<'_>,
    operand_shape: bool,
) -> IdentityResult<String> {
    normalize_type_in_context(
        program,
        request.type_reference,
        &TypeIdentityContext {
            binders: request.binders,
            substitutions: request.substitutions,
            qualification: request.qualification,
            ..Default::default()
        },
        operand_shape,
    )
}

pub(crate) fn normalize_type_in_context(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
    operand_shape: bool,
) -> IdentityResult<String> {
    if !program.contains_type_reference(reference) {
        return Err(RuntimeCallableIdentityError::UnresolvedType);
    }
    let invalid = Cell::new(false);
    let text = normalize_type_reference(
        program,
        reference,
        &TypeIdentityContext {
            callable_operand_shape: operand_shape,
            structural_callable_identity: true,
            invalid_application: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            missing_exact_nominal_owner: Some(&invalid),
            ..*context
        },
    );
    if invalid.get() {
        Err(RuntimeCallableIdentityError::UnresolvedType)
    } else {
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callable_values::RuntimeCallableContract;
    use crate::expression::{Expression, ExpressionHandle, NamePath};
    use crate::name::Identifier;
    use crate::proposition::{PropositionDefinition, PropositionEvidenceClassification};
    use crate::signature::StateParameter;
    use crate::trait_definition::{TraitDefinition, TraitRequirement};
    use crate::types::TypeReferenceNode;
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    fn name_expression(program: &mut TypedTrees, symbol: SymbolHandle) -> ExpressionHandle {
        program
            .expression_table
            .insert_tree(&Expression::Name(NamePath::resolved(
                vec![Identifier::generated("operand")],
                symbol,
                symbol,
            )))
    }

    fn application(
        program: &mut TypedTrees,
        proposition: SymbolHandle,
        symbols: &[SymbolHandle],
    ) -> PropositionApplication {
        let expressions = symbols
            .iter()
            .map(|symbol| name_expression(program, *symbol))
            .collect::<Vec<_>>();
        PropositionApplication {
            proposition,
            name: Identifier::generated("application"),
            binder_arguments: Box::new([]),
            arguments: program
                .expression_table
                .insert_expression_handles(expressions),
        }
    }

    struct AliasFixture {
        program: TypedTrees,
        contracts: [RuntimeCallableContractHandle; 2],
        subjects: [Vec<SymbolHandle>; 2],
        base: SymbolHandle,
        alias: SymbolHandle,
        witness: arena::Handle<TraitDefinition>,
        missing_trait: SymbolHandle,
        type_binder: SymbolHandle,
    }

    fn alias_fixture() -> AliasFixture {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("test"));
        let declarations = symbols
            .insert_children(
                root,
                [
                    (SymbolKind::Proposition, SymbolNameRef::Static("Base")),
                    (SymbolKind::Proposition, SymbolNameRef::Static("Flip")),
                    (SymbolKind::Trait, SymbolNameRef::Static("Witness")),
                    (SymbolKind::Trait, SymbolNameRef::Static("Missing")),
                    (SymbolKind::CallableContract, SymbolNameRef::Static("first")),
                    (
                        SymbolKind::CallableContract,
                        SymbolNameRef::Static("second"),
                    ),
                ],
            )
            .handles()
            .collect::<Vec<_>>();
        let type_binder = symbols
            .insert_children(
                declarations[0],
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
            )
            .handles()
            .next()
            .unwrap();
        let witness_parameter = symbols
            .insert_children(
                declarations[2],
                [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
            )
            .handles()
            .next()
            .unwrap();
        let mut parameters = Vec::new();
        for owner in [
            declarations[0],
            declarations[1],
            declarations[4],
            declarations[5],
        ] {
            parameters.push(
                symbols
                    .insert_children(
                        owner,
                        [
                            (SymbolKind::Parameter, SymbolNameRef::Static("left")),
                            (SymbolKind::Parameter, SymbolNameRef::Static("right")),
                        ],
                    )
                    .handles()
                    .collect::<Vec<_>>(),
            );
        }
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let witness_arguments = program
            .type_reference_table
            .insert_type_reference_handles([unit]);
        let witness_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: declarations[2],
                base_name: Identifier::generated("Witness"),
                arguments: witness_arguments,
                lifetime_arguments: Vec::new(),
            });
        let witness_parameters =
            program
                .data_type_parameters
                .insert_many([crate::data::TypeParameter {
                    symbol: witness_parameter,
                    name: Identifier::generated("T"),
                    ..Default::default()
                }]);
        let witness = program.traits.insert(TraitDefinition {
            symbol: declarations[2],
            name: Identifier::generated("Witness"),
            type_parameters: witness_parameters,
            ..Default::default()
        });
        let mut spans = Vec::new();
        for subjects in &parameters {
            spans.push(
                program
                    .state_parameters
                    .insert_many(subjects.iter().map(|symbol| StateParameter {
                        symbol: *symbol,
                        name: Identifier::generated("parameter"),
                        type_reference: unit,
                        ..Default::default()
                    })),
            );
        }
        program.propositions.insert(PropositionDefinition {
            symbol: declarations[0],
            name: Identifier::generated("Base"),
            parameters: spans[0],
            body: PropositionBody::Witness {
                evidence: witness_type,
            },
            ..Default::default()
        });
        let expansion = application(
            &mut program,
            declarations[0],
            &[parameters[1][1], parameters[1][0]],
        );
        program.propositions.insert(PropositionDefinition {
            symbol: declarations[1],
            name: Identifier::generated("Flip"),
            parameters: spans[1],
            body: PropositionBody::Transparent {
                proposition: PropositionFormula::Application(expansion),
            },
            ..Default::default()
        });
        let contracts = [0, 1].map(|index| {
            program
                .runtime_callable_contracts
                .insert(RuntimeCallableContract {
                    signature: StateSignature {
                        symbol: declarations[index + 4],
                        parameters: spans[index + 2],
                        return_type: unit,
                        ..Default::default()
                    },
                    lexical_owner: root,
                    resolved_source: None,
                })
        });
        AliasFixture {
            program,
            contracts,
            subjects: [parameters[2].clone(), parameters[3].clone()],
            base: declarations[0],
            alias: declarations[1],
            witness,
            missing_trait: declarations[3],
            type_binder,
        }
    }

    #[test]
    fn callable_alias_projection_preserves_reordered_operands_under_alpha_renaming() {
        let mut fixture = alias_fixture();
        let first = application(&mut fixture.program, fixture.alias, &fixture.subjects[0]);
        let second = application(&mut fixture.program, fixture.alias, &fixture.subjects[1]);
        let direct = application(
            &mut fixture.program,
            fixture.base,
            &[fixture.subjects[0][1], fixture.subjects[0][0]],
        );
        let a = fixture
            .program
            .runtime_callable_proposition_application(fixture.contracts[0], &first, &[])
            .unwrap();
        let b = fixture
            .program
            .runtime_callable_proposition_application(fixture.contracts[1], &second, &[])
            .unwrap();
        let direct = fixture
            .program
            .runtime_callable_proposition_application(fixture.contracts[0], &direct, &[])
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a, direct);
        assert_ne!(a.arguments[0], a.arguments[1]);
        let PropositionEvidenceClassification::Witness {
            interface: Some(interface),
            ..
        } = a.classification
        else {
            panic!("the alias must retain its witness interface");
        };
        assert_eq!(interface.arguments.len(), 1);
        let missing = application(
            &mut fixture.program,
            fixture.alias,
            &[fixture.subjects[0][0]],
        );
        assert_eq!(
            fixture.program.runtime_callable_proposition_application(
                fixture.contracts[0],
                &missing,
                &[]
            ),
            Err(RuntimeCallableIdentityError::UnresolvedContractFact)
        );
    }

    #[test]
    fn callable_alias_projection_refuses_unresolved_inherited_witness_contracts() {
        let mut fixture = alias_fixture();
        let call = application(&mut fixture.program, fixture.alias, &fixture.subjects[0]);

        let requires = fixture
            .program
            .trait_requirements
            .insert_many([TraitRequirement {
                symbol: fixture.missing_trait,
                name: Identifier::generated("Missing"),
                ..Default::default()
            }]);
        fixture.program.traits.get_mut(fixture.witness).requires = requires;
        assert_eq!(
            fixture.program.runtime_callable_proposition_application(
                fixture.contracts[0],
                &call,
                &[]
            ),
            Err(RuntimeCallableIdentityError::UnresolvedContractFact)
        );
    }
    #[test]
    fn callable_alias_projection_refuses_wrong_static_roles_and_cycles() {
        use crate::proposition::{
            PropositionBinder, PropositionBinderArgument, PropositionBinderArgumentKind,
        };
        let mut fixture = alias_fixture();
        let mut call = application(&mut fixture.program, fixture.base, &fixture.subjects[0]);
        let binders = fixture
            .program
            .proposition_binders
            .insert_many([PropositionBinder {
                symbol: fixture.type_binder,
                name: Identifier::generated("T"),
                ..Default::default()
            }]);
        let base = fixture
            .program
            .propositions
            .iter()
            .find(|(_, row)| row.symbol == fixture.base)
            .unwrap()
            .0;
        fixture.program.propositions.get_mut(base).binders = binders;
        call.binder_arguments = vec![PropositionBinderArgument {
            kind: PropositionBinderArgumentKind::Machine,
            path: Box::new([]),
            const_literal: None,
            evidence_projection: None,
            symbol: fixture.missing_trait,
        }]
        .into_boxed_slice();
        assert_eq!(
            fixture.program.runtime_callable_proposition_application(
                fixture.contracts[0],
                &call,
                &[]
            ),
            Err(RuntimeCallableIdentityError::UnresolvedStaticContract)
        );
        let call = application(&mut fixture.program, fixture.alias, &fixture.subjects[0]);
        let alias = fixture
            .program
            .propositions
            .iter()
            .find(|(_, row)| row.symbol == fixture.alias)
            .unwrap()
            .0;
        let parameters = fixture
            .program
            .proposition_parameters(fixture.program.propositions.get(alias))
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let expansion = application(&mut fixture.program, fixture.alias, &parameters);
        fixture.program.propositions.get_mut(alias).body = PropositionBody::Transparent {
            proposition: PropositionFormula::Application(expansion),
        };
        assert_eq!(
            fixture.program.runtime_callable_proposition_application(
                fixture.contracts[0],
                &call,
                &[]
            ),
            Err(RuntimeCallableIdentityError::UnresolvedContractFact)
        );
    }
}
