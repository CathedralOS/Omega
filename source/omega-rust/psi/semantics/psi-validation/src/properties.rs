//! Declared type-property verification.
//!
//! Properties are lowercase facts in brackets on a data declaration
//! (`data Point [copy]`). The spelling set is closed at parse
//! time; this pass verifies the declared facts hold:
//!
//! - `copy`: structural — every field (and case payload field) must be a
//!   primitive or a data type that itself declares the property.
//! - `carry(...)`: the authored four-axis floor may not be more permissive
//!   than the policy derived from every stored field.
//! Whether zeroed storage establishes a type is derived from its default
//! domain, fields, and zero-case payload in `data`; semantic emptiness is not
//! a type property.

use crate::symbols::TopLevelSymbols;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, TypeParameter};
use psi_typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredPropertyRequirement {
    Copy,
    Linear,
    Carry(psi_language_semantics::CarryPolicy),
}

/// Closed property admitted for one exact opaque data declaration by an
/// external compiler-owned representation receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueDataPropertyReceiptKind {
    Copy,
}

/// Exact input to final opaque-property validation.
///
/// Construction grants nothing by itself. The orchestration owner must derive
/// this row from independently rechecked representation custody; validation
/// rejects missing, duplicate, stale, wrong-kind, and wrong-declaration rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueDataPropertyReceipt {
    declaration: psi_symbols::SymbolHandle,
    kind: OpaqueDataPropertyReceiptKind,
}

impl OpaqueDataPropertyReceipt {
    pub const fn copy(declaration: psi_symbols::SymbolHandle) -> Self {
        Self {
            declaration,
            kind: OpaqueDataPropertyReceiptKind::Copy,
        }
    }

    pub const fn declaration(self) -> psi_symbols::SymbolHandle {
        self.declaration
    }

    pub const fn kind(self) -> OpaqueDataPropertyReceiptKind {
        self.kind
    }
}

impl std::fmt::Display for DeclaredPropertyRequirement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Copy => formatter.write_str("copy"),
            Self::Linear => formatter.write_str("linear"),
            Self::Carry(policy) => write!(formatter, "{policy}"),
        }
    }
}

pub(crate) fn validate_data_properties(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    opaque_property_receipts: &[OpaqueDataPropertyReceipt],
    allow_pending_opaque_copy: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_opaque_property_receipts(program, opaque_property_receipts, diagnostics);
    for data_definition in program.data_definitions() {
        let properties = data_definition.properties;

        if data_definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
            validate_opaque_data_properties(
                data_definition,
                opaque_property_receipts,
                allow_pending_opaque_copy,
                diagnostics,
            );
            continue;
        }

        if properties.multiplicity == psi_language_semantics::Multiplicity::Unrestricted {
            validate_structural_property(program, symbols, data_definition, "copy", diagnostics);
        }
        if let Some(carry) = properties.carry {
            validate_carry_policy(program, data_definition, carry, diagnostics);
        }
    }
}

/// `copy` is compositional: the property holds when every stored
/// field holds it. Primitives qualify; named data must declare the property
/// itself; a type parameter qualifies when it declares the matching bound
/// (`data Box<T [copy]> [copy]`, frozen decision 13); everything else
/// (references, slices, owned text, dyn traits) is rejected until a ruling
/// extends the set.
fn validate_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    data_definition: &DataDefinition,
    property: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(
        program,
        data_definition,
        &mut |field, case: Option<&str>| {
            if type_satisfies_structural_property(
                program,
                symbols,
                type_parameters,
                field.type_reference,
                property,
            ) {
                return;
            }
            let place = match case {
                Some(case) => format!("case `{case}` payload field `{}`", field.name),
                None => format!("field `{}`", field.name),
            };
            if let Some(parameter) =
                referenced_type_parameter(program, type_parameters, field.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares `[{property}]` but {place} type parameter `{name}` does not declare `[{property}]` — add `{name} [{property}]`",
                data_definition.name,
                name = parameter.name
            )));
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
            "data `{}` declares `[{property}]` but {place} is not `{property}`: only primitives and data types declaring `[{property}]` qualify",
            data_definition.name
        )));
        },
    );
}

fn validate_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
    required: psi_language_semantics::CarryPolicy,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let type_parameters = program.data_type_parameters(data_definition);
    for_each_stored_field(
        program,
        data_definition,
        &mut |field, case: Option<&str>| {
            let actual =
                CarryDerivation::new(program, type_parameters).derive(field.type_reference);
            if actual.permits(required) {
                return;
            }
            let place = match case {
                Some(case) => format!("case `{case}` payload field `{}`", field.name),
                None => format!("field `{}`", field.name),
            };
            if let Some(parameter) =
                referenced_type_parameter(program, type_parameters, field.type_reference)
            {
                diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} type parameter `{}` has only `{actual}`; add a compatible `[carry(...)]` bound",
                data_definition.name, parameter.name
            )));
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` declares carry policy `{required}` but {place} permits only `{actual}`",
                data_definition.name
            )));
        },
    );
}

mod carry_derivation;
use carry_derivation::{CarryDerivation, for_each_stored_field};

/// Opaque declarations expose no representation the compiler could inspect.
/// Restrictive usage policy is safe (`[linear]`, or an explicitly strict carry
/// floor), while any claim that grants additional behavior must come from the
/// ordinary admission/receipt spine. Until that consumer lands, fail closed.
fn validate_opaque_data_properties(
    data_definition: &DataDefinition,
    receipts: &[OpaqueDataPropertyReceipt],
    allow_pending_copy: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let properties = data_definition.properties;
    if properties.multiplicity == psi_language_semantics::Multiplicity::Unrestricted
        && !allow_pending_copy
        && !receipts.iter().any(|receipt| {
            receipt.declaration == data_definition.symbol
                && receipt.kind == OpaqueDataPropertyReceiptKind::Copy
        })
    {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot claim `[copy]` without an admitted property receipt",
            data_definition.name
        )));
    }
    if properties
        .carry
        .is_some_and(|policy| policy != psi_language_semantics::CarryPolicy::STRICT)
    {
        diagnostics.push(Diagnostic::error(format!(
            "opaque boundary data `{}` cannot relax its carry policy without an admitted property receipt",
            data_definition.name
        )));
    }
}

fn validate_opaque_property_receipts(
    program: &TypedTrees,
    receipts: &[OpaqueDataPropertyReceipt],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, receipt) in receipts.iter().enumerate() {
        if receipts[..index].contains(receipt) {
            diagnostics.push(Diagnostic::error(
                "opaque data property receipts repeat one exact declaration and property",
            ));
            continue;
        }
        let definitions = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == receipt.declaration)
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            diagnostics.push(Diagnostic::error(format!(
                "opaque data property receipt maps to {} declarations; expected one",
                definitions.len(),
            )));
            continue;
        };
        if definition.supply_mode != psi_language_semantics::DataSupplyMode::BoundaryOpaque {
            diagnostics.push(Diagnostic::error(format!(
                "opaque data property receipt targets non-opaque declaration `{}`",
                definition.name,
            )));
            continue;
        }
        match receipt.kind {
            OpaqueDataPropertyReceiptKind::Copy
                if definition.properties.multiplicity
                    == psi_language_semantics::Multiplicity::Unrestricted => {}
            OpaqueDataPropertyReceiptKind::Copy => diagnostics.push(Diagnostic::error(format!(
                "opaque data property receipt grants `[copy]` to `{}`, but that declaration does not claim `[copy]`",
                definition.name,
            ))),
        }
    }
}

/// Derive the effective carry policy of a transparent data declaration from
/// its complete stored shape. This is the checker-owned result; an authored
/// `carry(...)` clause is only a minimum promise validated against it.
pub fn effective_data_carry_policy(
    program: &TypedTrees,
    data_definition: &DataDefinition,
) -> psi_language_semantics::CarryPolicy {
    if data_definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
        return psi_language_semantics::CarryPolicy::STRICT;
    }
    CarryDerivation::new(program, &[]).derive_named_data(
        data_definition.symbol,
        data_definition.name.as_str(),
        None,
    )
}

/// Derive the effective carry policy of an arbitrary type at one generic
/// declaration site. Live-set checking consumes this entry point so it uses
/// the same structural derivation and concrete-argument substitution as
/// property validation; carry policy is never re-inferred from a type name.
pub fn effective_type_carry_policy(
    program: &TypedTrees,
    type_parameters: &[TypeParameter],
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> psi_language_semantics::CarryPolicy {
    CarryDerivation::new(program, type_parameters).derive(type_reference)
}

/// One entry point for "does this type carry the declared property?", shared
/// with the instantiation-time bound check. Carry requirements use the
/// normalized four-axis comparison.
pub fn type_satisfies_declared_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    property: DeclaredPropertyRequirement,
) -> bool {
    match property {
        DeclaredPropertyRequirement::Carry(required) => {
            CarryDerivation::new(program, type_parameters)
                .derive(type_reference)
                .permits(required)
        }
        DeclaredPropertyRequirement::Copy => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            type_reference,
            "copy",
        ),
        DeclaredPropertyRequirement::Linear => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            type_reference,
            "linear",
        ),
    }
}

fn type_satisfies_structural_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    type_parameters: &[TypeParameter],
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    property: &str,
) -> bool {
    if program
        .type_reference_table
        .primitive_type(type_reference)
        .is_some()
    {
        return property != "linear";
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            if let Some(parameter) =
                type_parameter_for_reference(type_parameters, *symbol, name.as_str())
            {
                return type_parameter_declares_property(parameter, property);
            }
            named_type_declares_property(program, symbols, *symbol, name.as_str(), property)
        }
        TypeReferenceNode::Constrained { base_type, .. } => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            *base_type,
            property,
        ),
        TypeReferenceNode::FixedArray { element_type, .. } => type_satisfies_structural_property(
            program,
            symbols,
            type_parameters,
            *element_type,
            property,
        ),
        // An instantiated generic carries the property when its base data
        // declares it; the instantiation-time bound check separately verifies
        // the arguments uphold the base's parameter bounds.
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => named_type_declares_property(
            program,
            symbols,
            *base_symbol,
            base_name.as_str(),
            property,
        ),
        // References, slices, dyn traits, unit: not part of the verified set
        // yet.
        _ => false,
    }
}

/// The declared property names in canonical order, for diagnostics and for
/// iterating a parameter's bounds.
pub fn declared_property_requirements(
    properties: &psi_typed_trees::data::DataProperties,
) -> Vec<DeclaredPropertyRequirement> {
    let mut names = Vec::new();
    if properties.multiplicity == psi_language_semantics::Multiplicity::Unrestricted {
        names.push(DeclaredPropertyRequirement::Copy);
    }
    if properties.multiplicity == psi_language_semantics::Multiplicity::Linear {
        names.push(DeclaredPropertyRequirement::Linear);
    }
    if let Some(carry) = properties.carry {
        names.push(DeclaredPropertyRequirement::Carry(carry));
    }
    names
}

fn type_parameter_for_reference<'program>(
    type_parameters: &'program [TypeParameter],
    symbol: psi_symbols::SymbolHandle,
    name: &str,
) -> Option<&'program TypeParameter> {
    if symbol.is_valid() {
        return type_parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol);
    }
    type_parameters
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
}

fn type_parameter_declares_property(parameter: &TypeParameter, property: &str) -> bool {
    match property {
        "copy" => {
            parameter.bounds.multiplicity == psi_language_semantics::Multiplicity::Unrestricted
        }
        "linear" => parameter.bounds.multiplicity == psi_language_semantics::Multiplicity::Linear,
        _ => false,
    }
}

/// The type parameter a field or state-parameter type ultimately names, if
/// any — used to point the data-property diagnostic at the missing bound, and
/// by the machine-call bound check (decision 13) to find which parameter a
/// signature type like `&T` instantiates.
pub(crate) fn referenced_type_parameter<'program>(
    program: &TypedTrees,
    type_parameters: &'program [TypeParameter],
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<&'program TypeParameter> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { symbol, name } => {
            type_parameter_for_reference(type_parameters, *symbol, name.as_str())
        }
        TypeReferenceNode::Reference { referee, .. } => {
            referenced_type_parameter(program, type_parameters, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            referenced_type_parameter(program, type_parameters, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            referenced_type_parameter(program, type_parameters, *element_type)
        }
        _ => None,
    }
}

fn named_type_declares_property(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    symbol: psi_symbols::SymbolHandle,
    name: &str,
    property: &str,
) -> bool {
    // Builtin scalar spellings resolve as primitives above; a remaining named
    // type only qualifies when it is a data definition declaring the property.
    let _ = symbols;
    program
        .data_definitions()
        .iter()
        .find(|definition| {
            if symbol.is_valid() {
                definition.symbol == symbol
            } else {
                definition.name.as_str() == name
            }
        })
        .is_some_and(|definition| match property {
            "copy" => {
                definition.properties.multiplicity
                    == psi_language_semantics::Multiplicity::Unrestricted
            }
            "linear" => {
                definition.properties.multiplicity == psi_language_semantics::Multiplicity::Linear
            }
            _ => false,
        })
}
