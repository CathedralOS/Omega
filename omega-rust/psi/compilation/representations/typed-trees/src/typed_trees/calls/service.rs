//! Exact recognition of the toolchain-owned routed service carrier.
//!
//! A same-named package declaration is ordinary opaque data. Compiler
//! privilege requires the complete core source, declaration shape, and one
//! closed boundary requirement. Service validity is intrinsic to the carrier:
//! `Binding<R>` is already the exact closed identity, so any authored
//! qualification spelled on it is rejected. The retired `Bound` qualification
//! era survives only in these entry-point names, which still have callers that
//! have not migrated; the classifier itself consults no domain.

use crate::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub const SERVICE_CORE_SOURCE: &str = "binding.omg";

#[derive(Debug, Clone, Copy)]
pub struct ExactServiceCarrier {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Original exact application, not the enclosing Binding or an ABI view.
    pub requirement_reference: TypeReferenceHandle,
    pub requirement_identity: crate::type_identity::TypeIdentityId,
}

impl PartialEq for ExactServiceCarrier {
    fn eq(&self, other: &Self) -> bool {
        self.service_data == other.service_data
            && self.requirement == other.requirement
            && self.requirement_identity == other.requirement_identity
    }
}

impl Eq for ExactServiceCarrier {}

impl ExactServiceCarrier {
    pub fn arguments<'program>(
        self,
        program: &'program TypedTrees,
    ) -> &'program [TypeReferenceHandle] {
        match program
            .type_reference_table
            .type_reference(self.requirement_reference)
        {
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_table
                .type_reference_handles(*arguments),
            _ => &[],
        }
    }

    pub fn application(
        self,
        program: &TypedTrees,
    ) -> Option<crate::trait_definition::AppliedTraitScope<'_>> {
        crate::trait_definition::AppliedTraitScope::new(
            program,
            program.trait_definition_by_symbol(self.requirement)?,
            crate::trait_definition::TraitApplicationArguments {
                // Region-bearing carriers still require occurrence custody;
                // the classifier refuses them before this view is produced.
                lexical_owner: SymbolHandle::invalid(),
                arguments: self.arguments(program),
                lifetimes: &[],
            },
        )
    }
}

/// A parameter's exact underlying binding and its separately retained borrow
/// presentation. Unwrapping this shell is valid for parameter planning, not
/// for deciding whether a value may be moved out of borrowed storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactServiceParameterCarrier {
    pub carrier: ExactServiceCarrier,
    pub carrier_type: TypeReferenceHandle,
    pub access: Option<language_semantics::ReferenceAccess>,
}

pub fn classify_exact_service_parameter_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ExactServiceParameterCarrier>, String> {
    let (carrier_type, access) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } if matches!(
            access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        ) =>
        {
            (*referee, Some(*access))
        }
        _ => (type_reference, None),
    };
    Ok(
        classify_exact_bound_service_carrier(program, carrier_type)?.map(|carrier| {
            ExactServiceParameterCarrier {
                carrier,
                carrier_type,
                access,
            }
        }),
    )
}

/// Classify one type shell. `Ok(None)` means it is not the exact core
/// `Service` carrier; `Err` means it does name that carrier but violates the
/// deliberately narrow first-rung contract. The carrier is closed: an authored
/// `Constrained` shell over `Binding<R>` is itself a violation, whatever
/// domain or membership the constraint names.
pub fn classify_exact_bound_service_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ExactServiceCarrier>, String> {
    let mut current = type_reference;
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(current)
    {
        qualified = true;
        current = *base_type;
    }

    let generic_origin = match program.type_reference_table.type_reference(current) {
        TypeReferenceNode::Generic { .. } => current,
        TypeReferenceNode::Named { symbol, .. } => {
            let Some(origin) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            else {
                return Ok(None);
            };
            origin
        }
        _ => return Ok(None),
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(generic_origin)
    else {
        return Ok(None);
    };
    if !is_exact_service_data_symbol(program, *base_symbol) {
        return Ok(None);
    }
    if qualified {
        return Err(
            "the core `Binding` carrier is closed; it admits no authored qualification".to_owned(),
        );
    }
    if !lifetime_arguments.is_empty() {
        return Err("the core `Service` carrier takes no lifetime arguments".to_owned());
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let [requirement] = arguments else {
        return Err(format!(
            "the core `Service` carrier requires exactly one closed boundary requirement, but {} arguments were supplied",
            arguments.len()
        ));
    };
    let requirement_reference = *requirement;
    let (requirement, name, arguments) = match program.type_reference_table.type_reference(requirement_reference) {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name, &[][..]),
        TypeReferenceNode::Generic { base_symbol, base_name, lifetime_arguments, arguments, .. }
            if lifetime_arguments.is_empty() => (*base_symbol, base_name,
                program.type_reference_table.type_reference_handles(*arguments)),
        _ => return Err("the core `Binding` carrier requires one complete closed boundary-trait application with retained exact regions".to_owned()),
    };
    let Some(requirement_definition) = program.trait_definition_by_symbol(requirement) else {
        return Err(format!(
            "`Binding<{name}>` does not name an exact boundary-trait requirement"
        ));
    };
    if !requirement_definition.is_public {
        return Err(format!(
            "`Binding<{name}>` requires a public boundary trait as its stable slot contract"
        ));
    }
    if !requirement_definition.is_boundary {
        return Err(format!(
            "`Binding<{name}>` requires a boundary trait; ordinary traits are local interfaces"
        ));
    }
    if !requirement_definition.lifetime_parameters.is_empty()
        || program.trait_type_parameters(requirement_definition).len() != arguments.len()
    {
        return Err(format!(
            "the core `Binding` carrier requires a complete closed boundary application with retained exact regions; `{name}` remains open"
        ));
    }
    program
        .require_closed_application_operands(arguments)
        .map_err(str::to_owned)?;

    Ok(Some(ExactServiceCarrier {
        service_data: *base_symbol,
        requirement,
        requirement_reference,
        requirement_identity: program.normalized_type_identity(requirement_reference),
    }))
}

pub fn exact_bound_service_requirement(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    classify_exact_bound_service_carrier(program, type_reference)
        .ok()
        .flatten()
        .map(|carrier| carrier.requirement)
}

pub fn is_exact_service_data_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    let Some(definition) = program.data_definition_by_symbol(symbol) else {
        return false;
    };
    // One carrier, one source. `ForeignBinding` was listed here as a second
    // row, which is an artifact of the rename that turned `Service<R>` into
    // `Binding<R>`: the name it took over had belonged to a routed-service
    // carrier, but `ForeignBinding` is the compile-time foreign LOCATOR in
    // `core/external_binding.omg` and is not `boundary data` at all, so it
    // could never satisfy the shape checked below.
    let carrier_source = match definition.name.as_str() {
        "Binding" => SERVICE_CORE_SOURCE,
        _ => return false,
    };
    if !exact_toolchain_source(program, symbol, carrier_source) {
        return false;
    }
    let parameters = program.data_type_parameters(definition);
    definition.is_public
        && definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
        && definition.properties.multiplicity == language_semantics::Multiplicity::Affine
        && definition.properties.carry.is_none()
        && definition.lifetime_parameters.is_empty()
        && matches!(parameters, [parameter] if matches!(parameter.kind, crate::data::TypeParameterKind::Type))
        && program.data_members(definition).is_empty()
}

fn exact_toolchain_source(program: &TypedTrees, symbol: SymbolHandle, relative: &str) -> bool {
    program
        .symbol_toolchain_relative_path(symbol)
        .is_some_and(|path| path == std::path::Path::new(relative))
}
