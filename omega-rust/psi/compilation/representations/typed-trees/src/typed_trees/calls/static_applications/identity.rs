//! Complete semantic identity for one retained static application.
//!
//! Every authored occurrence has fresh payload handles. Duplicate supply and
//! alias traversal and instance production compare exact binders and complete
//! semantic operands within one retained owner. Type/index normalization uses
//! existing typed queries; selected declarations and lexical regions remain
//! handle-first. This is compiler-local equality, never a portable commitment,
//! bound admission, provider selection or receiving receipt.

use crate::TypedTrees;
use crate::expression::StaticMachineArgument;
use crate::static_applications::{
    StaticMachineApplicationHandle, StaticMachineBinding, StaticMachineBindingValue,
};
use crate::type_identity::{
    AppliedTypeIdentityOwner, IndexIdentityArgument, NormalizedTypeIdentity,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticApplicationIdentity {
    selected_interface: SymbolHandle,
    bindings: Vec<(SymbolHandle, BindingIdentity)>,
    lifetimes: Vec<StaticApplicationRegionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingIdentity {
    Type(NormalizedTypeIdentity),
    Const(String),
    Machine(ArgumentIdentity),
    Conformance(ArgumentIdentity),
    RuntimeValue {
        carrier: NormalizedTypeIdentity,
        subject: SymbolHandle,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ArgumentIdentity {
    Type(NormalizedTypeIdentity),
    Const(String),
    Declaration {
        symbol: SymbolHandle,
        arguments: Vec<ArgumentIdentity>,
        lifetimes: Vec<StaticApplicationRegionIdentity>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticApplicationRegionIdentity {
    Named {
        owner: SymbolHandle,
        ordinal: usize,
        capture_identity: u64,
    },
    Anonymous,
}

impl StaticApplicationIdentity {
    pub fn lifetimes(&self) -> &[StaticApplicationRegionIdentity] {
        &self.lifetimes
    }
}

pub fn static_application_identity(
    owner: &TypedTrees,
    application: StaticMachineApplicationHandle,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let lexical_owner = owner
        .expression_table
        .static_machine_application(application)
        .type_identity_owner(owner)
        .ok_or_else(|| Diagnostic::error("static application lost its original lexical capture"))?;
    let original = owner
        .expression_table
        .static_machine_application(application);
    let selected_interface = if original.selected_interface.is_valid() {
        original.selected_interface
    } else {
        owner
            .trait_machine_signature(original.binding_declaration)
            .map_or(SymbolHandle::invalid(), |(definition, _)| definition.symbol)
    };
    static_binding_identity(
        owner,
        selected_interface,
        owner
            .expression_table
            .static_machine_application_bindings(application),
        lexical_owner,
        owner
            .expression_table
            .static_machine_application_lifetimes(application)
            .iter()
            .map(|binding| &binding.argument),
    )
}

/// Canonical content of one original complete application. Local equality
/// identities deliberately retain arena/capture namespaces, so this projection
/// instead uses the existing exact-owner type and original-region queries.
/// This owner has no loaded toolchain source roster: a source-backed toolchain
/// operand without a managed owner therefore remains unavailable here.
pub(crate) fn static_application_identity_text(
    owner: &TypedTrees,
    application: StaticMachineApplicationHandle,
) -> Result<String, Diagnostic> {
    use crate::type_identity::{identity_atom as atom, identity_compound as compound};
    let view = super::StaticApplicationView::new(owner, application).map_err(Diagnostic::error)?;
    let lexical_owner = view
        .application()
        .type_identity_owner(owner)
        .ok_or_else(|| {
            Diagnostic::error("callable application lost its original lexical capture")
        })?;
    let arguments = view
        .bindings()
        .iter()
        .map(|binding| {
            let argument = match &binding.value {
                StaticMachineBindingValue::Type(reference) => {
                    IndexIdentityArgument::TypeArgument(*reference)
                }
                StaticMachineBindingValue::Const(reference) => {
                    IndexIdentityArgument::TypeReference(*reference)
                }
                StaticMachineBindingValue::Machine(argument)
                | StaticMachineBindingValue::Conformance(argument) => {
                    IndexIdentityArgument::StaticArgument(argument)
                }
                StaticMachineBindingValue::RuntimeValue { .. } => {
                    return Err(Diagnostic::error(
                        "runtime subject cannot identify a closed callable application",
                    ));
                }
            };
            Ok((binding.parameter, argument))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let lifetimes = view
        .lifetimes()
        .iter()
        .map(|(formal, binding)| (*formal, &binding.argument))
        .collect::<Vec<_>>();
    let mut bindings = Vec::with_capacity(view.bindings().len());
    // The view supplies all original binders, including conformance bounds
    // outside the type-parameter span, in declaration order.
    for (ordinal, binding) in view.bindings().iter().enumerate() {
        let value = match &binding.value {
            StaticMachineBindingValue::Type(reference) => compound(
                "type",
                [portable_type_identity_text(
                    owner,
                    *reference,
                    lexical_owner,
                )?],
            ),
            StaticMachineBindingValue::Const(reference) => {
                let parameter = view
                    .declaration()
                    .parameters()
                    .iter()
                    .find(|parameter| parameter.symbol == binding.parameter)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "callable const operand lost its original declaration slot",
                        )
                    })?;
                let (carrier, category) = match parameter.kind {
                    crate::data::TypeParameterKind::Const { type_reference } => {
                        (type_reference, "const")
                    }
                    crate::data::TypeParameterKind::Value { type_reference } => {
                        (type_reference, "value")
                    }
                    _ => {
                        return Err(Diagnostic::error(
                            "callable const operand changed its original declaration category",
                        ));
                    }
                };
                compound(
                    category,
                    [
                        atom(
                            "carrier",
                            &declared_carrier_identity_text(
                                owner,
                                carrier,
                                parameter.symbol,
                                lexical_owner,
                                &arguments,
                                &lifetimes,
                            )?,
                        ),
                        atom(
                            "value",
                            &portable_type_identity_text(owner, *reference, lexical_owner)?,
                        ),
                    ],
                )
            }
            StaticMachineBindingValue::Machine(argument) => compound(
                "machine",
                [argument_identity_text(
                    owner,
                    argument,
                    lexical_owner,
                    true,
                )?],
            ),
            StaticMachineBindingValue::Conformance(argument) => compound(
                "conformance",
                [argument_identity_text(
                    owner,
                    argument,
                    lexical_owner,
                    false,
                )?],
            ),
            StaticMachineBindingValue::RuntimeValue { .. } => {
                return Err(Diagnostic::error(
                    "runtime subject cannot identify a closed callable application",
                ));
            }
        };
        bindings.push(compound(
            "binding",
            [atom("ordinal", &ordinal.to_string()), value],
        ));
    }
    let selected_interface = if view.application().selected_interface.is_valid() {
        view.application().selected_interface
    } else {
        owner
            .trait_machine_signature(view.application().binding_declaration)
            .map_or(SymbolHandle::invalid(), |(definition, _)| definition.symbol)
    };
    let selected_interface = if selected_interface.is_valid() {
        nominal_identity_text(owner, selected_interface)?
    } else {
        String::new()
    };
    let regions = view
        .lifetimes()
        .iter()
        .map(|(_, binding)| region_identity_text(owner, lexical_owner, &binding.argument))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(compound(
        "static-application",
        [
            atom("interface", &selected_interface),
            compound("bindings", bindings),
            compound("regions", regions),
        ],
    ))
}

fn argument_identity_text(
    owner: &TypedTrees,
    argument: &StaticMachineArgument,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    machine: bool,
) -> Result<String, Diagnostic> {
    use crate::type_identity::{identity_atom as atom, identity_compound as compound};
    if !argument.symbol.is_valid()
        || argument.evidence_projection.is_some()
        || argument.type_reference.is_valid()
        || argument.const_literal.is_some()
    {
        return Err(Diagnostic::error(
            "callable application lost its exact static declaration operand",
        ));
    }
    let declaration = super::StaticApplicationDeclaration::for_symbol(owner, argument.symbol)
        .map_err(Diagnostic::error)?;
    let callable = if machine {
        // Nominal ownership alone cannot distinguish overloaded machines.
        // Reuse the existing canonical callable key for the exact selection;
        // the original declaration/application still owns nested operands.
        let identity = if let Some(selected) = owner
            .machine(argument.symbol)
            .or_else(|| owner.machine_holding_state(argument.symbol))
        {
            owner.machine_callable_identity_text(selected)
        } else if let Some((selected, signature)) = owner.trait_machine_signature(argument.symbol) {
            Some(owner.trait_requirement_callable_identity_text(selected, signature))
        } else if let Some(operator) =
            crate::operator::declaration_by_symbol(owner, argument.symbol)
        {
            Some(owner.callable_identity_text(owner.operator_callable_identity(operator)))
        } else {
            None
        };
        identity.filter(|text| !text.is_empty()).ok_or_else(|| {
            Diagnostic::error("callable application lost its exact selected callable identity")
        })?
    } else {
        owner
            .conformance_by_symbol(argument.symbol)
            .ok_or_else(|| {
                Diagnostic::error("callable application lost its original conformance declaration")
            })?;
        String::new()
    };
    let (arguments, regions) = if let Some(application) = &argument.application {
        let binders = declaration.binders().collect::<Vec<_>>();
        if binders.len() != application.arguments.len()
            || declaration.lifetimes().len() != application.lifetime_arguments.len()
        {
            return Err(Diagnostic::error(
                "nested callable operand has no complete original declaration tuple",
            ));
        }
        let type_substitutions = binders
            .iter()
            .zip(&application.arguments)
            .filter_map(|(binder, argument)| {
                let parameter = declaration
                    .parameters()
                    .iter()
                    .find(|parameter| parameter.symbol == *binder)?;
                matches!(parameter.kind, crate::data::TypeParameterKind::Type)
                    .then_some((*binder, argument.type_reference))
                    .filter(|(_, reference)| reference.is_valid())
            })
            .collect::<Vec<_>>();
        let original_arguments = binders
            .iter()
            .zip(&application.arguments)
            .map(|(binder, argument)| (*binder, IndexIdentityArgument::StaticArgument(argument)))
            .collect::<Vec<_>>();
        let original_lifetimes = declaration
            .lifetimes()
            .iter()
            .zip(&application.lifetime_arguments)
            .map(|(formal, argument)| (*formal, argument))
            .collect::<Vec<_>>();
        let arguments = binders
            .iter()
            .zip(&application.arguments)
            .map(|(binder, argument)| {
                let parameter = declaration
                    .parameters()
                    .iter()
                    .find(|parameter| parameter.symbol == *binder);
                match parameter.map(|parameter| &parameter.kind) {
                    Some(crate::data::TypeParameterKind::Type) => {
                        let identity = crate::type_identity::exact_owner_index_identity(
                            owner,
                            IndexIdentityArgument::StaticArgument(argument),
                            lexical_owner,
                            &[],
                        )
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "nested type operand has no exact source or region owner",
                            )
                        })?;
                        Ok(compound("type", [identity]))
                    }
                    Some(
                        crate::data::TypeParameterKind::Const { type_reference }
                        | crate::data::TypeParameterKind::Value { type_reference },
                    ) => {
                        let identity = crate::type_identity::declared_const_argument_identity(
                            owner,
                            IndexIdentityArgument::StaticArgument(argument),
                            *type_reference,
                            &type_substitutions,
                        )
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "nested const operand has no original closed carrier meaning",
                            )
                        })?;
                        let parameter = parameter.ok_or_else(|| {
                            Diagnostic::error(
                                "nested const operand lost its original declaration slot",
                            )
                        })?;
                        let category = if matches!(
                            parameter.kind,
                            crate::data::TypeParameterKind::Value { .. }
                        ) {
                            "value"
                        } else {
                            "const"
                        };
                        Ok(compound(
                            category,
                            [
                                atom(
                                    "carrier",
                                    &declared_carrier_identity_text(
                                        owner,
                                        *type_reference,
                                        parameter.symbol,
                                        lexical_owner,
                                        &original_arguments,
                                        &original_lifetimes,
                                    )?,
                                ),
                                atom("value", &identity),
                            ],
                        ))
                    }
                    Some(crate::data::TypeParameterKind::Machine { .. }) => Ok(compound(
                        "machine",
                        [argument_identity_text(
                            owner,
                            argument,
                            lexical_owner,
                            true,
                        )?],
                    )),
                    Some(crate::data::TypeParameterKind::Implementation) | None => Ok(compound(
                        "conformance",
                        [argument_identity_text(
                            owner,
                            argument,
                            lexical_owner,
                            false,
                        )?],
                    )),
                    _ => Err(Diagnostic::error(
                        "nested callable operand has an unsupported original binder category",
                    )),
                }
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let regions = application
            .lifetime_arguments
            .iter()
            .map(|region| region_identity_text(owner, lexical_owner, region))
            .collect::<Result<Vec<_>, _>>()?;
        (arguments, regions)
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(compound(
        "declaration",
        [
            atom("owner", &nominal_identity_text(owner, argument.symbol)?),
            atom("callable", &callable),
            compound("arguments", arguments),
            compound("regions", regions),
        ],
    ))
}

/// A canonical value's encoded type name is not its declared carrier owner.
/// Normalize the actual declaration carrier separately under original binder
/// handles and retained lifetime correspondence, before entering caller scope.
fn declared_carrier_identity_text<LifetimeArgument: super::StaticLifetimeOperand>(
    owner: &TypedTrees,
    carrier: crate::types::TypeReferenceHandle,
    parameter: SymbolHandle,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    arguments: &[(SymbolHandle, IndexIdentityArgument<'_>)],
    lifetimes: &[(super::DeclaredStaticLifetime<'_>, &LifetimeArgument)],
) -> Result<String, Diagnostic> {
    if !parameter.is_valid() {
        return Err(Diagnostic::error(
            "declared const carrier has no original parameter owner",
        ));
    }
    let declaration = owner.symbols.get(parameter).parent;
    if !declaration.is_valid() {
        return Err(Diagnostic::error(
            "declared const carrier lost its original lexical declaration",
        ));
    }
    // Selected inherited interface names do not stand in for the declaring
    // member's captured regions. Only a matching original declaration/ordinal
    // row licenses substitution of that source carrier's lifetime formal.
    let lifetimes = lifetimes
        .iter()
        .filter_map(|(formal, argument)| {
            (owner.declaration_lifetime_parameter(declaration, formal.name.as_str())
                == Some((formal.declaration, formal.ordinal)))
            .then(|| {
                (
                    formal.name.as_str().to_owned(),
                    argument
                        .named()
                        .map(|argument| argument.as_str().to_owned()),
                )
            })
        })
        .collect::<Vec<_>>();
    let scope = crate::type_identity::AppliedTypeIdentityScope {
        region_role: crate::type_identity::AppliedTypeIdentityRegionRole::Supplied,
        declaration,
        arguments,
        lifetimes: &lifetimes,
    };
    let identity = owner
        .exact_owner_scoped_applied_type_identity(
            crate::type_identity::ExactOwnerTypeIdentityRequest {
                type_reference: carrier,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: &[],
            },
            lexical_owner,
            &[scope],
        )
        .ok_or_else(|| {
            Diagnostic::error(
                "declared const carrier has no complete original source/type/region correspondence",
            )
        })?;
    owner
        .type_identity_text(identity)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| Diagnostic::error("declared const carrier lost its issuing identity"))
}

fn portable_type_identity_text(
    owner: &TypedTrees,
    reference: crate::types::TypeReferenceHandle,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
) -> Result<String, Diagnostic> {
    let identity = owner
        .exact_owner_applied_type_identity(
            crate::type_identity::ExactOwnerTypeIdentityRequest {
                type_reference: reference,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: &[],
            },
            lexical_owner,
        )
        .ok_or_else(|| {
            Diagnostic::error(
                "callable application type has no exact original source or region owner",
            )
        })?;
    owner
        .type_identity_text(identity)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| Diagnostic::error("callable application lost its issuing type identity"))
}

fn nominal_identity_text(owner: &TypedTrees, symbol: SymbolHandle) -> Result<String, Diagnostic> {
    let identity = owner
        .exact_owner_nominal_identity(symbol, &[])
        .ok_or_else(|| {
            Diagnostic::error("callable application declaration has no exact original source owner")
        })?;
    owner
        .type_identity_text(identity)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            Diagnostic::error("callable application lost its issuing declaration identity")
        })
}

fn region_identity_text<LifetimeArgument: super::StaticLifetimeOperand>(
    owner: &TypedTrees,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    lifetime: &LifetimeArgument,
) -> Result<String, Diagnostic> {
    use crate::type_identity::{identity_atom as atom, identity_compound as compound};
    let Some(lifetime) = lifetime.named() else {
        return Ok(compound("anonymous-invocation-region", []));
    };
    let (region_owner, declaration) = match lexical_owner {
        AppliedTypeIdentityOwner::Declaration(symbol) => {
            (owner, owner.lifetime_declaration_owner(symbol))
        }
        AppliedTypeIdentityOwner::Captured(capture) => (capture.owner(), capture.symbol()),
    };
    let (declaration, ordinal) = region_owner
        .original_declaration_lifetime_parameter(declaration, lifetime.as_str())
        .ok_or_else(|| {
            Diagnostic::error(
                "callable application region has no original declaration correspondence",
            )
        })?;
    Ok(compound(
        "region",
        [
            atom("owner", &nominal_identity_text(region_owner, declaration)?),
            atom("ordinal", &ordinal.to_string()),
        ],
    ))
}

/// The original selected interface tuple of one closed public header.
/// Member invocation operands remain in the complete application but do not
/// choose another sibling's interface. Original evidence binders and captured
/// lexical regions remain part of the outer tuple. This is identity only;
/// source checking and runtime provider admission remain separate owners.
pub fn requirement_interface_application_identity(
    owner: &TypedTrees,
    header: &super::InstantiatedRequirementSignature,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let selected = owner
        .trait_definition_by_symbol(header.selected_interface)
        .ok_or_else(|| {
            Diagnostic::error("closed requirement has no original selected interface")
        })?;
    if usize::try_from(header.captured_lifetime_count).ok()
        != Some(selected.lifetime_parameters.len())
    {
        return Err(Diagnostic::error(
            "closed requirement lost its complete captured interface regions",
        ));
    }
    super::interface_identity::retained_interface_identity(
        owner,
        header.requirement_template,
        selected.symbol,
        header.binding_application,
    )
}

/// Compare applications retained by independently copied source owners.
/// Type identity handles belong to their issuing table: equal slot numbers
/// across owners do not establish equal types, and different slots do not
/// establish different types. Resolve only those operands through the original
/// canonical tables; binder, declaration, runtime subject and region custody
/// remain exact. The owners must retain the same original declaration lineage.
/// This is application equality, never bound admission or provider selection.
pub fn static_application_identities_equal(
    left_owner: &TypedTrees,
    left_application: StaticMachineApplicationHandle,
    right_owner: &TypedTrees,
    right_application: StaticMachineApplicationHandle,
) -> Result<bool, Diagnostic> {
    if !left_owner
        .expression_table
        .static_machine_application_is_valid(left_application)
        || !right_owner
            .expression_table
            .static_machine_application_is_valid(right_application)
    {
        return Err(Diagnostic::error(
            "static application has no original retained payload",
        ));
    }
    let left_declaration = left_owner
        .expression_table
        .static_machine_application(left_application)
        .binding_declaration;
    let right_declaration = right_owner
        .expression_table
        .static_machine_application(right_application)
        .binding_declaration;
    if left_declaration != right_declaration {
        return Ok(false);
    }
    let left = static_application_identity(left_owner, left_application)?;
    let right = static_application_identity(right_owner, right_application)?;
    if std::ptr::eq(left_owner, right_owner) {
        return Ok(left == right);
    }
    if left.selected_interface != right.selected_interface
        || left.lifetimes != right.lifetimes
        || left.bindings.len() != right.bindings.len()
    {
        return Ok(false);
    }
    for ((left_binder, left_value), (right_binder, right_value)) in
        left.bindings.iter().zip(&right.bindings)
    {
        if left_binder != right_binder
            || !binding_identities_equal(left_owner, left_value, right_owner, right_value)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn binding_identities_equal(
    left_owner: &TypedTrees,
    left: &BindingIdentity,
    right_owner: &TypedTrees,
    right: &BindingIdentity,
) -> Result<bool, Diagnostic> {
    match (left, right) {
        (BindingIdentity::Type(left), BindingIdentity::Type(right)) => {
            type_identities_equal(left_owner, *left, right_owner, *right)
        }
        (BindingIdentity::Const(left), BindingIdentity::Const(right)) => Ok(left == right),
        (BindingIdentity::Machine(left), BindingIdentity::Machine(right))
        | (BindingIdentity::Conformance(left), BindingIdentity::Conformance(right)) => {
            argument_identities_equal(left_owner, left, right_owner, right)
        }
        (
            BindingIdentity::RuntimeValue {
                carrier: left,
                subject: left_subject,
            },
            BindingIdentity::RuntimeValue {
                carrier: right,
                subject: right_subject,
            },
        ) => Ok(left_subject == right_subject
            && type_identities_equal(left_owner, *left, right_owner, *right)?),
        _ => Ok(false),
    }
}

fn argument_identities_equal(
    left_owner: &TypedTrees,
    left: &ArgumentIdentity,
    right_owner: &TypedTrees,
    right: &ArgumentIdentity,
) -> Result<bool, Diagnostic> {
    match (left, right) {
        (ArgumentIdentity::Type(left), ArgumentIdentity::Type(right)) => {
            type_identities_equal(left_owner, *left, right_owner, *right)
        }
        (ArgumentIdentity::Const(left), ArgumentIdentity::Const(right)) => Ok(left == right),
        (
            ArgumentIdentity::Declaration {
                symbol: left_symbol,
                arguments: left_arguments,
                lifetimes: left_lifetimes,
            },
            ArgumentIdentity::Declaration {
                symbol: right_symbol,
                arguments: right_arguments,
                lifetimes: right_lifetimes,
            },
        ) => {
            if left_symbol != right_symbol
                || left_lifetimes != right_lifetimes
                || left_arguments.len() != right_arguments.len()
            {
                return Ok(false);
            }
            for (left, right) in left_arguments.iter().zip(right_arguments) {
                if !argument_identities_equal(left_owner, left, right_owner, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn type_identities_equal(
    left_owner: &TypedTrees,
    left: NormalizedTypeIdentity,
    right_owner: &TypedTrees,
    right: NormalizedTypeIdentity,
) -> Result<bool, Diagnostic> {
    let unavailable =
        || Diagnostic::error("static application lost an identity in its original type table");
    let left = left_owner
        .type_identity_text(left)
        .ok_or_else(unavailable)?;
    let right = right_owner
        .type_identity_text(right)
        .ok_or_else(unavailable)?;
    Ok(left == right)
}

/// Compare original binding rows without manufacturing an executable application.
/// Source declaration applications and retained call applications use the same
/// normalizer, binder coordinates and lexical region ownership. This operation
/// establishes equality only; the producing declaration owns tuple completeness.
pub fn static_binding_identity<
    'argument,
    LifetimeArgument: super::StaticLifetimeOperand + 'argument,
>(
    owner: &TypedTrees,
    selected_interface: SymbolHandle,
    bindings: &[StaticMachineBinding],
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    lifetime_arguments: impl IntoIterator<Item = &'argument LifetimeArgument>,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let mut bindings = bindings
        .iter()
        .map(|binding| {
            let value = match &binding.value {
                StaticMachineBindingValue::Type(reference) => {
                    BindingIdentity::Type(type_identity(owner, *reference, lexical_owner)?)
                }
                StaticMachineBindingValue::Const(reference) => BindingIdentity::Const(
                    index_identity(owner, IndexIdentityArgument::TypeReference(*reference))?,
                ),
                StaticMachineBindingValue::Machine(argument) => {
                    BindingIdentity::Machine(argument_identity(owner, argument, lexical_owner)?)
                }
                StaticMachineBindingValue::Conformance(argument) => {
                    BindingIdentity::Conformance(argument_identity(owner, argument, lexical_owner)?)
                }
                StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                } => BindingIdentity::RuntimeValue {
                    carrier: type_identity(owner, *type_reference, lexical_owner)?,
                    subject: *subject,
                },
            };
            Ok((binding.parameter, value))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    // Substitution rows carry original binder identity independently of their
    // storage order. Calls may publish grouped construction scratch; two rows
    // mean the same application only when each exact binder gets the same value.
    bindings
        .sort_unstable_by_key(|(parameter, _)| (parameter.arena_index(), parameter.generation()));
    if bindings.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Diagnostic::error(
            "static application repeats a declaration binder",
        ));
    }
    let lifetimes = lifetime_arguments
        .into_iter()
        .map(|argument| lexical_region(owner, lexical_owner, argument))
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(StaticApplicationIdentity {
        selected_interface,
        bindings,
        lifetimes,
    })
}

fn argument_identity(
    owner: &TypedTrees,
    argument: &StaticMachineArgument,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
) -> Result<ArgumentIdentity, Diagnostic> {
    if argument.type_reference.is_valid() {
        return Ok(ArgumentIdentity::Type(type_identity(
            owner,
            argument.type_reference,
            lexical_owner,
        )?));
    }
    if argument.const_literal.is_some()
        || (argument.symbol.is_valid()
            && owner.symbols.get(argument.symbol).kind == symbols::SymbolKind::Const)
    {
        return Ok(ArgumentIdentity::Const(index_identity(
            owner,
            IndexIdentityArgument::StaticArgument(argument),
        )?));
    }
    if !argument.symbol.is_valid() || argument.evidence_projection.is_some() {
        return Err(Diagnostic::error(
            "static application lost its exact static declaration operand",
        ));
    }
    let (arguments, lifetimes) = if let Some(application) = &argument.application {
        (
            application
                .arguments
                .iter()
                .map(|argument| argument_identity(owner, argument, lexical_owner))
                .collect::<Result<Vec<_>, Diagnostic>>()?,
            application
                .lifetime_arguments
                .iter()
                .map(|lifetime| lexical_region(owner, lexical_owner, lifetime))
                .collect::<Result<Vec<_>, Diagnostic>>()?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    Ok(ArgumentIdentity::Declaration {
        symbol: argument.symbol,
        arguments,
        lifetimes,
    })
}

fn type_identity(
    owner: &TypedTrees,
    reference: crate::types::TypeReferenceHandle,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
) -> Result<NormalizedTypeIdentity, Diagnostic> {
    crate::type_identity::applied_type_identity(
        owner,
        reference,
        lexical_owner,
        &[],
        crate::type_identity::TypeIdentityQualification::PackageQualified,
    )
    .ok_or_else(|| {
        Diagnostic::error("static application type operand has no exact applied identity")
    })
}

fn index_identity(
    owner: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
) -> Result<String, Diagnostic> {
    crate::type_identity::normalized_index_identity(
        owner,
        argument,
        &[],
        crate::type_identity::TypeIdentityQualification::PackageQualified,
    )
    .ok_or_else(|| {
        Diagnostic::error("static application const operand has no exact checked index identity")
    })
}

fn lexical_region<LifetimeArgument: super::StaticLifetimeOperand>(
    owner: &TypedTrees,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
    lifetime: &LifetimeArgument,
) -> Result<StaticApplicationRegionIdentity, Diagnostic> {
    let Some(lifetime) = lifetime.named() else {
        return Ok(StaticApplicationRegionIdentity::Anonymous);
    };
    let (symbol, parameters, capture_identity) = match lexical_owner {
        AppliedTypeIdentityOwner::Declaration(symbol) => (
            symbol,
            owner
                .machine(symbol)
                .map(|declaration| declaration.lifetime_parameters.as_slice())
                .or_else(|| {
                    owner
                        .data_definition_by_symbol(symbol)
                        .map(|declaration| declaration.lifetime_parameters.as_slice())
                })
                .or_else(|| {
                    owner
                        .conformance_by_symbol(symbol)
                        .map(|declaration| declaration.lifetime_parameters.as_slice())
                })
                .or_else(|| {
                    owner
                        .trait_definition_by_symbol(symbol)
                        .map(|declaration| declaration.lifetime_parameters.as_slice())
                }),
            0,
        ),
        AppliedTypeIdentityOwner::Captured(scope) => (
            scope.symbol(),
            Some(scope.lifetime_parameters()),
            scope.identity(),
        ),
    };
    parameters
        .and_then(|parameters| {
            parameters
                .iter()
                .position(|parameter| parameter == lifetime)
        })
        .map(|ordinal| StaticApplicationRegionIdentity::Named {
            owner: symbol,
            ordinal,
            capture_identity,
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "static application region lost its exact lexical declaration position",
            )
        })
}
