//! Original field and public-header interface application correspondence.
//!
//! A trait declaration alone cannot distinguish sibling applied provider fields.
//! Both sides retain original binders, all operand categories and lexical regions
//! under their issuing owner, then use the existing static identity normalizer.
//! This read-only join grants no receiver, provider, bound or installation authority.

use super::{
    StaticApplicationIdentity, StaticApplicationView, StaticMachineApplicationHandle,
    StaticMachineBinding, StaticMachineBindingValue, static_argument_from_type_reference,
    static_binding_identity,
};
use crate::TypedTrees;
use crate::data::TypeParameterKind;
use crate::name::Identifier;
use crate::type_identity::AppliedTypeIdentityOwner;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// Resolve the exact original interface row carried by a selected public header.
/// An inherited member may retain Child presentation and Parent declaration;
/// the receiver must name one of the actual retained interface applications.
/// A direct authored member has a complete empty tuple only if its declaration
/// has no remaining operand, evidence or region slots.
pub fn requirement_provider_application_identity(
    owner: &TypedTrees,
    requirement: SymbolHandle,
    interface: SymbolHandle,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let mut headers = owner
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, header)| {
            owner.trait_machine_signatures.get(header.signature).symbol == requirement
        });
    if let Some((_, header)) = headers.next() {
        if headers.next().is_some() {
            return Err(Diagnostic::error(
                "public requirement has ambiguous header custody",
            ));
        }
        let applications = owner
            .requirement_interface_applications
            .span(header.interface_applications)
            .ok_or_else(|| {
                Diagnostic::error("public requirement lost its interface application roster")
            })?;
        let mut rows = applications.iter().filter(|row| row.interface == interface);
        let row = rows.next().ok_or_else(|| {
            Diagnostic::error("provider field has no original public interface application")
        })?;
        if rows.next().is_some()
            || (interface == header.selected_interface
                && row.binding_application != header.binding_application)
            || (interface == header.declaring_trait
                && row.binding_application != header.declaring_binding_application)
        {
            return Err(Diagnostic::error(
                "provider field has ambiguous interface application custody",
            ));
        }
        return retained_interface_identity(
            owner,
            header.requirement_template,
            interface,
            row.binding_application,
        );
    }
    if owner
        .trait_machine_signature(requirement)
        .is_none_or(|(declaration, _)| declaration.symbol != interface)
    {
        return Err(Diagnostic::error(
            "provider field has no original declaring requirement",
        ));
    }
    interface_arguments_identity(
        owner,
        interface,
        &[],
        &[],
        AppliedTypeIdentityOwner::Declaration(interface),
    )
}

/// Read the original receiver tuple of a caller-owned public contract. This
/// logical view consumed its complete original telescope but owns no executable
/// header or provider roster. Both are independent of this identity operation.
pub fn requirement_call_interface_identity(
    owner: &TypedTrees,
    requirement: SymbolHandle,
    application: StaticMachineApplicationHandle,
    interface: SymbolHandle,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let selected = owner
        .selected_public_trait_requirement_signature(requirement, application)
        .ok_or_else(|| Diagnostic::error("public call lost its original selected requirement"))?;
    let contract = owner
        .public_requirement_contract(application, selected.declaration.symbol)
        .ok_or_else(|| Diagnostic::error("public call has no retained applied contract"))?;
    if !std::ptr::eq(contract, selected.contract) {
        return Err(Diagnostic::error(
            "public call lost its exact applied contract custody",
        ));
    }
    retained_interface_identity(owner, selected.declaration.symbol, interface, application)
}

pub(super) fn retained_interface_identity(
    owner: &TypedTrees,
    requirement_template: SymbolHandle,
    interface: SymbolHandle,
    application: StaticMachineApplicationHandle,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let declaration = owner
        .trait_definition_by_symbol(interface)
        .ok_or_else(|| Diagnostic::error("public header lost its original interface"))?;
    if !owner
        .expression_table
        .static_machine_application_is_valid(application)
    {
        return Err(Diagnostic::error(
            "public interface lost its original application payload",
        ));
    }
    let application = StaticApplicationView::new(owner, application).map_err(Diagnostic::error)?;
    // Zero is the documented ordinary direct declaring interface. It cannot
    // choose a descendant or stand in for an absent application payload.
    let selected_interface = if application.application().selected_interface.is_valid() {
        application.application().selected_interface
    } else {
        owner
            .trait_machine_signature(application.declaration().symbol())
            .map_or(SymbolHandle::invalid(), |(declaration, _)| {
                declaration.symbol
            })
    };
    if selected_interface != interface || application.declaration().symbol() != requirement_template
    {
        return Err(Diagnostic::error(
            "public header lost its original interface/member association",
        ));
    }
    let parameters = owner.trait_type_parameters(declaration);
    let bindings = application
        .bindings()
        .iter()
        .filter(|binding| {
            parameters
                .iter()
                .any(|parameter| parameter.symbol == binding.parameter)
                || declaration
                    .conformance_bounds
                    .iter()
                    .any(|bound| bound.binder == binding.parameter)
        })
        .map(|binding| (**binding).clone())
        .collect::<Vec<_>>();
    if parameters.iter().any(|parameter| {
        bindings
            .iter()
            .filter(|binding| binding.parameter == parameter.symbol)
            .count()
            != 1
    }) || declaration.conformance_bounds.iter().any(|bound| {
        bindings
            .iter()
            .filter(|binding| binding.parameter == bound.binder)
            .count()
            != 1
    }) {
        return Err(Diagnostic::error(
            "public header lost its complete original interface operands",
        ));
    }
    let captured = declaration.lifetime_parameters.len();
    if application.lifetimes().len() < captured {
        return Err(Diagnostic::error(
            "public header lost its original interface regions",
        ));
    }
    static_binding_identity(
        owner,
        interface,
        &bindings,
        application
            .application()
            .type_identity_owner(owner)
            .ok_or_else(|| Diagnostic::error("public header lost its original lexical capture"))?,
        application.lifetimes()[..captured]
            .iter()
            .map(|(_, binding)| &binding.argument),
    )
}

/// Read a trait tuple from its genuine field type payload. Reference and
/// qualification shells are transparent to identity only; access and placement
/// consumers must still establish their independent receiver permissions.
/// A core Binding contributes its nominal requirement and the same complete
/// operand/region tuple as a direct trait presentation. This identity query
/// preserves captured lexical scope; it does not admit a receiving occurrence.
pub fn trait_type_application_identity(
    owner: &TypedTrees,
    mut reference: TypeReferenceHandle,
    lexical_owner: AppliedTypeIdentityOwner<'_>,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    loop {
        if !reference.is_valid()
            || !owner
                .type_reference_table
                .contains_type_reference(reference)
        {
            return Err(Diagnostic::error(
                "provider field lost its original type payload",
            ));
        }
        if let Some(carrier) = crate::service::inspect_service_carrier_contract(owner, reference)
            .map_err(Diagnostic::error)?
        {
            return interface_arguments_identity(
                owner,
                carrier.requirement,
                carrier.arguments(owner),
                carrier.lifetimes(owner),
                lexical_owner,
            );
        }
        match owner.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                return interface_arguments_identity(owner, *symbol, &[], &[], lexical_owner);
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } => {
                return interface_arguments_identity(
                    owner,
                    *base_symbol,
                    owner
                        .type_reference_table
                        .type_reference_handles(*arguments),
                    lifetime_arguments,
                    lexical_owner,
                );
            }
            TypeReferenceNode::DynamicTrait {
                symbol, arguments, ..
            } => {
                return interface_arguments_identity(
                    owner,
                    *symbol,
                    owner
                        .type_reference_table
                        .type_reference_handles(*arguments),
                    &[],
                    lexical_owner,
                );
            }
            _ => {
                return Err(Diagnostic::error(
                    "provider field has no original interface type application",
                ));
            }
        }
    }
}

fn interface_arguments_identity(
    owner: &TypedTrees,
    interface: SymbolHandle,
    arguments: &[TypeReferenceHandle],
    lifetime_arguments: &[Identifier],
    lexical_owner: AppliedTypeIdentityOwner<'_>,
) -> Result<StaticApplicationIdentity, Diagnostic> {
    let declaration = owner
        .trait_definition_by_symbol(interface)
        .ok_or_else(|| Diagnostic::error("provider field has no original trait declaration"))?;
    let parameters = owner.trait_type_parameters(declaration);
    if parameters.len() != arguments.len()
        || declaration.lifetime_parameters.len() != lifetime_arguments.len()
        || declaration.conformance_bounds.iter().any(|bound| {
            !parameters
                .iter()
                .any(|parameter| parameter.symbol == bound.binder)
        })
    {
        return Err(Diagnostic::error(
            "provider field lost its complete original interface telescope",
        ));
    }
    let bindings = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, reference)| {
            if !reference.is_valid()
                || !owner
                    .type_reference_table
                    .contains_type_reference(*reference)
            {
                return Err(Diagnostic::error(
                    "provider field lost an original interface operand",
                ));
            }
            let value = match &parameter.kind {
                TypeParameterKind::Type => StaticMachineBindingValue::Type(*reference),
                TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. } => {
                    StaticMachineBindingValue::Const(*reference)
                }
                TypeParameterKind::Machine { .. } | TypeParameterKind::Implementation => {
                    let argument = static_argument_from_type_reference(owner, *reference)
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "provider field lost an original static declaration operand",
                            )
                        })?;
                    if matches!(parameter.kind, TypeParameterKind::Machine { .. }) {
                        StaticMachineBindingValue::Machine(argument)
                    } else {
                        StaticMachineBindingValue::Conformance(argument)
                    }
                }
                TypeParameterKind::Proposition { .. } => {
                    return Err(Diagnostic::error(
                        "provider field has no retained proposition operand",
                    ));
                }
            };
            Ok(StaticMachineBinding {
                parameter: parameter.symbol,
                value,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    static_binding_identity(
        owner,
        interface,
        &bindings,
        lexical_owner,
        lifetime_arguments,
    )
}
