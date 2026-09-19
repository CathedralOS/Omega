//! Exact recognition of the toolchain-owned routed service carrier.
//!
//! A same-named package declaration is ordinary opaque data. Compiler
//! privilege requires the complete core source, declaration shape, and one
//! closed boundary requirement. Service validity is intrinsic to the carrier:
//! `Service<R>` is already the exact closed identity, so any authored
//! qualification spelled on it is rejected. The retired `Bound` qualification
//! era survives only in these entry-point names, which still have callers that
//! have not migrated; the classifier itself consults no domain.

use crate::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub const SERVICE_CORE_SOURCE: &str = "service.omg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactServiceCarrier {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
}

/// Classify one type shell. `Ok(None)` means it is not the exact core
/// `Service` carrier; `Err` means it does name that carrier but violates the
/// deliberately narrow first-rung contract. The carrier is closed: an authored
/// `Constrained` shell over `Service<R>` is itself a violation, whatever
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
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol)
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
            "the core `Service` carrier is closed; it admits no authored qualification"
                .to_owned(),
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
    let TypeReferenceNode::Named {
        symbol: requirement,
        name,
    } = program.type_reference_table.type_reference(*requirement)
    else {
        return Err(
            "the first Service rung accepts only one closed, monomorphic boundary-trait requirement"
                .to_owned(),
        );
    };
    let Some(requirement_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == *requirement)
    else {
        return Err(format!(
            "`Service<{name}>` does not name an exact boundary-trait requirement"
        ));
    };
    if !requirement_definition.is_public {
        return Err(format!(
            "`Service<{name}>` requires a public boundary trait as its stable slot contract"
        ));
    }
    if !requirement_definition.is_boundary {
        return Err(format!(
            "`Service<{name}>` requires a boundary trait; ordinary traits are local interfaces"
        ));
    }
    if !requirement_definition.lifetime_parameters.is_empty()
        || !program
            .trait_type_parameters(requirement_definition)
            .is_empty()
    {
        return Err(format!(
            "the first Service rung accepts only a closed, nongeneric, lifetime-free boundary requirement; `{name}` remains open"
        ));
    }

    Ok(Some(ExactServiceCarrier {
        service_data: *base_symbol,
        requirement: *requirement,
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
    if !exact_toolchain_source(program, symbol, SERVICE_CORE_SOURCE) {
        return false;
    }
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(definition);
    definition.name.as_str() == "Service"
        && definition.is_public
        && definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
        && definition.properties.multiplicity == language_semantics::Multiplicity::Affine
        && definition.properties.carry.is_none()
        && definition.lifetime_parameters.is_empty()
        && matches!(parameters, [parameter] if matches!(parameter.kind, crate::data::TypeParameterKind::Type))
        && program.data_members(definition).is_empty()
}

fn exact_toolchain_source(program: &TypedTrees, symbol: SymbolHandle, relative: &str) -> bool {
    let Some(span) = program.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = program.symbols.source_file(span) else {
        return false;
    };
    source.origin == source::SourceOrigin::Toolchain
        && source
            .path
            .strip_prefix(&source.package_root)
            .ok()
            .is_some_and(|path| path == std::path::Path::new(relative))
}
