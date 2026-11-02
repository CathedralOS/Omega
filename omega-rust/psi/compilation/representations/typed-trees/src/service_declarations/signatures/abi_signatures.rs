//! Exact declaration ABI applications, independently of callback selection.
//!
//! Each key retains the public overload, complete declaring application and
//! every non-callback local operand. All storage projections for that key are
//! retained for checking; this owner never chooses an ABI by equal shape or by
//! the first selected Handler. Uncalled open declarations retain their refusal.

use super::{BoundarySignatureDeclaration, PreparedBoundarySignatures};
use crate::TypedTrees;
use crate::data::{TypeParameter, TypeParameterKind};
use crate::types::TypeReferenceHandle;
use language_semantics::callback_declarations::BoundaryTelescope;
pub use semantic_vocabulary::CallingStaticOperand;
use semantic_vocabulary::{CallingGenericFormal, CallingOperandKind, CallingSignatureId};
use symbols::SymbolHandle;

#[derive(Clone)]
pub struct PreparedAbiStorage {
    pub parameters: Vec<TypeReferenceHandle>,
    pub result: TypeReferenceHandle,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

pub struct PreparedBoundaryAbiSignature {
    pub id: CallingSignatureId,
    pub template: SymbolHandle,
    /// Original declaration occurrences which contributed this exact key.
    /// Equal checked Build/Product copies may share its portable storage.
    pub source_templates: Vec<SymbolHandle>,
    pub requirement_identity: String,
    /// Empty only on a deferred open declaration, never a complete application.
    pub declaring_application: String,
    pub operands: Vec<CallingStaticOperand>,
    pub formals: Vec<CallingGenericFormal>,
    pub default_eligible: bool,
    pub telescope: Result<BoundaryTelescope, String>,
    pub projections: Vec<Result<PreparedAbiStorage, String>>,
}

pub(super) fn prepare(catalog: &mut PreparedBoundarySignatures, typed: &TypedTrees) {
    let mut rows = Vec::new();
    let mut boundaries = Vec::new();
    for declaration in catalog.declarations() {
        let (outer, open) = default_scope(typed, declaration);
        let projection = if open {
            Err("declaration ABI requires its complete declaring and non-callback local application".into())
        } else {
            declaration
                .projection
                .as_ref()
                .map_err(Clone::clone)
                .map(|prepared| PreparedAbiStorage {
                    parameters: prepared.parameter_types.clone(),
                    result: prepared.return_type,
                    substitutions: Vec::new(),
                })
        };
        let id = insert(typed, &mut rows, declaration, outer, Vec::new(), projection);
        boundaries.push((declaration.symbol, Ok(id)));
    }

    // Closed invocation contracts already retain exact actuals and declaring
    // scopes. Project only the ABI key; Handler identity remains on that
    // invocation. This also covers default ABI applications without Calling.
    for closed in typed.closed_requirement_applications() {
        let Some(declaration) = catalog
            .declarations()
            .find(|row| row.symbol == closed.template)
        else {
            continue;
        };
        let prepared = (|| {
            let outer = typed
                .type_identity_text(closed.portable_trait_application_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("declaration ABI lost its portable declaring application")?
                .to_owned();
            let (_, signature) = typed
                .trait_machine_signature(closed.template)
                .ok_or("declaration ABI lost its exact template")?;
            let parameters = typed.state_signature_type_parameters(signature);
            let bindings = typed.static_machine_application_bindings(closed.application);
            if parameters.len() != bindings.len()
                || parameters.len() != closed.portable_local_arguments.len()
                || parameters
                    .iter()
                    .zip(bindings)
                    .any(|(parameter, binding)| parameter.symbol != binding.parameter)
            {
                return Err(
                    "declaration ABI static operands differ from the complete declared telescope"
                        .to_owned(),
                );
            }
            let mut operands = Vec::new();
            for (ordinal, (parameter, identity)) in parameters
                .iter()
                .zip(closed.portable_local_arguments)
                .enumerate()
            {
                if callback_binder(declaration, parameter).is_some() {
                    continue;
                }
                operands.push(CallingStaticOperand {
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| "declaration ABI has too many static operands")?,
                    kind: operand_kind(&parameter.kind),
                    identity: typed
                        .type_identity_text(*identity)
                        .filter(|identity| !identity.is_empty())
                        .ok_or("declaration ABI lost a portable static operand")?
                        .to_owned(),
                });
            }
            let scope = closed
                .trait_scope
                .scope(typed)
                .ok_or("declaration ABI lost its retained declaring scope")?;
            let logical = typed
                .applied_callable_signature(&scope, closed.template)
                .or_else(|| {
                    (scope.substitutions.is_empty()
                        && scope.declaration.lifetime_parameters.is_empty())
                    .then_some(signature)
                })
                .ok_or("declaration ABI lost its callback-independent logical signature")?;
            let substitutions = parameters
                .iter()
                .zip(closed.local_arguments)
                .filter(|(parameter, _)| callback_binder(declaration, parameter).is_none())
                .map(|(parameter, actual)| {
                    let formal = typed
                        .applied_signature_parameter(signature, logical, parameter.symbol)
                        .ok_or("declaration ABI lost a non-callback formal correspondence")?;
                    Ok((formal, *actual))
                })
                .collect::<Result<Vec<_>, &str>>()?;
            Ok(insert(
                typed,
                &mut rows,
                declaration,
                outer,
                operands,
                Ok(PreparedAbiStorage {
                    parameters: typed
                        .state_signature_parameters(logical)
                        .iter()
                        .map(|parameter| parameter.type_reference)
                        .collect(),
                    result: logical.return_type,
                    substitutions,
                }),
            ))
        })();
        boundaries.push((closed.signature.symbol, prepared));
    }

    // A component can export a closed receiver without invoking its methods
    // or authoring Calling. Use the same checked application roots as service
    // preparation; do not infer an instance from its eventual native shape.
    let roots = typed.applied_trait_roots().into_iter().chain(
        typed
            .captured_machine_applications
            .iter()
            .filter_map(|(_, application)| {
                application.conformance_application.trait_scope.scope(typed)
            }),
    );
    for root in roots {
        root.visit(typed, &mut |scope| {
            if !scope.declaration.is_boundary {
                return;
            }
            for declared in typed.trait_machine_signatures(scope.declaration) {
                let Some(declaration) = catalog
                    .declarations()
                    .find(|row| row.symbol == declared.symbol)
                else {
                    continue;
                };
                if typed
                    .state_signature_type_parameters(declared)
                    .iter()
                    .any(|parameter| callback_binder(declaration, parameter).is_none())
                {
                    // Local actuals only come from checked applications. A
                    // receiver cannot invent an unused generic argument tuple.
                    continue;
                }
                let Some(outer) = scope
                    .portable_application_identity(typed)
                    .and_then(|identity| typed.type_identity_text(identity))
                    .filter(|identity| !identity.is_empty())
                else {
                    continue;
                };
                let signature = typed
                    .applied_callable_signature(scope, declaration.symbol)
                    .or_else(|| {
                        (scope.substitutions.is_empty()
                            && scope.declaration.lifetime_parameters.is_empty())
                        .then_some(declared)
                    });
                let projection = signature
                    .map(|signature| PreparedAbiStorage {
                        parameters: typed
                            .state_signature_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .collect(),
                        result: signature.return_type,
                        substitutions: Vec::new(),
                    })
                    .ok_or_else(|| "declaration ABI lost its applied logical signature".to_owned());
                insert(
                    typed,
                    &mut rows,
                    declaration,
                    outer.to_owned(),
                    Vec::new(),
                    projection,
                );
            }
        });
    }

    // An opaque `satisfies Requirement via binding` leaf inherits its governing
    // requirement's admitted contract, declaration ABI included
    // (language/effects.md, "Opaque supplies inherit"). A provider that calls
    // the leaf directly, rather than the trait member whose invocation edge its
    // own requirement does not publish, reaches the same exact ABI row. A
    // target family's synthesized interface is the boundary its callers see;
    // it takes the one row every member's governing requirement agrees on.
    let governing_application = |machine: &crate::machine::Machine| {
        let mut applications = typed
            .machine_trait_conformances(machine)
            .iter()
            .filter_map(|conformance| {
                crate::machine::resolve_satisfied_declaration(typed, machine, conformance)
            })
            .filter_map(|declaration| {
                boundaries
                    .iter()
                    .find(|(symbol, _)| *symbol == declaration.symbol())
                    .map(|(_, application)| application.clone())
            });
        match (applications.next(), applications.next()) {
            (Some(application), None) => Some(application),
            _ => None,
        }
    };
    let mut inherited = Vec::new();
    for machine in typed.machines() {
        if machine.body_is_present
            || boundaries
                .iter()
                .any(|(symbol, _)| *symbol == machine.symbol)
        {
            continue;
        }
        let application = if machine.target_family_interface {
            let members = typed
                .target_machine_families
                .iter()
                .find(|family| family.canonical == machine.symbol)
                .map_or(&[][..], |family| family.members.as_slice());
            let mut applications = members
                .iter()
                .filter_map(|member| typed.machine_by_symbol(member.machine))
                .map(governing_application);
            match applications.next() {
                Some(Some(first)) if applications.all(|other| other.as_ref() == Some(&first)) => {
                    Some(first)
                }
                _ => None,
            }
        } else if matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization { .. }
        ) {
            governing_application(machine)
        } else {
            None
        };
        if let Some(application) = application {
            inherited.push((machine.symbol, application));
        }
    }
    boundaries.extend(inherited);

    let mut applications = Vec::new();
    for (handle, application) in catalog.calling.applications() {
        let prepared = (|| {
            let declaration = catalog
                .applied_declaration(application)
                .ok_or("calling census lost its exact signature declaration")?;
            let scope = application
                .declaring_scope
                .as_ref()
                .and_then(|scope| scope.scope(typed))
                .ok_or("calling census lost its exact declaring scope")?;
            let outer = scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or("calling census lost its portable declaring application")?
                .to_owned();
            let (_, declared) = typed
                .trait_machine_signature(declaration.symbol)
                .ok_or("calling census lost its exact template")?;
            let locals = typed.state_signature_type_parameters(declared);
            if locals
                .iter()
                .all(|parameter| callback_binder(declaration, parameter).is_some())
            {
                let signature = typed
                    .applied_callable_signature(&scope, declaration.symbol)
                    .or_else(|| {
                        (scope.substitutions.is_empty()
                            && scope.declaration.lifetime_parameters.is_empty())
                        .then_some(declared)
                    });
                let projection = signature
                    .map(|signature| PreparedAbiStorage {
                        parameters: typed
                            .state_signature_parameters(signature)
                            .iter()
                            .map(|parameter| parameter.type_reference)
                            .collect(),
                        result: signature.return_type,
                        substitutions: Vec::new(),
                    })
                    .ok_or_else(|| "declaration ABI lost its applied logical signature".to_owned());
                let id = insert(typed, &mut rows, declaration, outer, Vec::new(), projection);
                return Ok(vec![id]);
            }
            // No concrete local tuple means no executable ABI application,
            // not permission to invent values for unused generic parameters.
            Ok(rows
                .iter()
                .filter(|row| {
                    row.requirement_identity == declaration.requirement_identity
                        && row.declaring_application == outer
                        && !row.operands.is_empty()
                })
                .map(|row| row.id)
                .collect())
        })();
        applications.push((handle, prepared));
    }
    for (application, ids) in applications {
        catalog.calling.set_abi_signatures(application, ids);
    }
    catalog.abi_signatures = rows;
    catalog.boundary_abis = boundaries;
}

fn default_scope(typed: &TypedTrees, declaration: &BoundarySignatureDeclaration) -> (String, bool) {
    let Some((owner, signature)) = typed.trait_machine_signature(declaration.symbol) else {
        return (declaration.requirement_identity.clone(), false);
    };
    let scope = crate::trait_definition::AppliedTraitScope::abstract_scope(owner);
    let outer = scope
        .portable_application_identity(typed)
        .and_then(|identity| typed.type_identity_text(identity))
        .unwrap_or_default()
        .to_owned();
    let open = !typed.trait_type_parameters(owner).is_empty()
        || !owner.lifetime_parameters.is_empty()
        || typed
            .state_signature_type_parameters(signature)
            .iter()
            .any(|parameter| callback_binder(declaration, parameter).is_none());
    (outer, open)
}

fn callback_binder(
    declaration: &BoundarySignatureDeclaration,
    parameter: &TypeParameter,
) -> Option<semantic_vocabulary::StaticMachineBinderId> {
    let prepared = declaration.projection.as_ref().ok()?;
    prepared
        .callback_sources
        .iter()
        .zip(&prepared.telescope.binders)
        .find(|(source, _)| source.parameter_symbol == parameter.symbol)
        .map(|(_, binder)| binder.binder)
}

fn operand_kind(kind: &TypeParameterKind) -> CallingOperandKind {
    match kind {
        TypeParameterKind::Type => CallingOperandKind::Type,
        TypeParameterKind::Const { .. } => CallingOperandKind::Const,
        TypeParameterKind::Value { .. } => CallingOperandKind::Value,
        TypeParameterKind::Machine { .. } => CallingOperandKind::Machine,
        TypeParameterKind::Implementation => CallingOperandKind::Implementation,
        TypeParameterKind::Proposition { .. } => CallingOperandKind::Proposition,
    }
}

fn insert(
    typed: &TypedTrees,
    rows: &mut Vec<PreparedBoundaryAbiSignature>,
    declaration: &BoundarySignatureDeclaration,
    declaring_application: String,
    operands: Vec<CallingStaticOperand>,
    projection: Result<PreparedAbiStorage, String>,
) -> CallingSignatureId {
    let id = CallingSignatureId::for_application(
        &declaration.requirement_identity,
        &declaring_application,
        operands
            .iter()
            .map(|operand| (operand.ordinal, operand.kind, operand.identity.as_str())),
    );
    if let Some(row) = rows.iter_mut().find(|row| {
        row.id == id
            && row.requirement_identity == declaration.requirement_identity
            && row.declaring_application == declaring_application
            && row.operands == operands
    }) {
        if !row.source_templates.contains(&declaration.symbol) {
            row.source_templates.push(declaration.symbol);
        }
        row.projections.push(projection);
        row.default_eligible |= declaration.default_eligible;
        return id;
    }
    let formals = typed
        .trait_machine_signature(declaration.symbol)
        .map(|(_, signature)| {
            // Signature parameter spans are bounded by the arena's u32 count.
            typed
                .state_signature_type_parameters(signature)
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| CallingGenericFormal {
                    ordinal: ordinal as u32,
                    kind: operand_kind(&parameter.kind),
                    callback: callback_binder(declaration, parameter),
                })
                .collect()
        })
        .unwrap_or_default();
    rows.push(PreparedBoundaryAbiSignature {
        id,
        template: declaration.symbol,
        source_templates: vec![declaration.symbol],
        requirement_identity: declaration.requirement_identity.clone(),
        declaring_application,
        operands,
        formals,
        default_eligible: declaration.default_eligible,
        telescope: declaration
            .projection
            .as_ref()
            .map(|prepared| prepared.telescope.clone())
            .map_err(Clone::clone),
        projections: vec![projection],
    });
    id
}
