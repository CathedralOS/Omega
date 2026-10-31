//! Source-issued read plans for owned core locator snapshots.
//! These bind canonical fields/cases to the exact checked ForeignBinding type;
//! an arbitrary shape or label cannot produce this companion context.

use diagnostics::Diagnostic;
use semantic_vocabulary::{ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildNativeLocator {
    pub structural_type: StructuralTypeId,
    pub widths: [u64; 3],
    pub shape: Arc<TerminalBuildValueShape>,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum TerminalBuildValueShape {
    Scalar(ScalarType),
    Array { length: u64, element: Arc<Self> },
    Sum { cases: Vec<TerminalBuildValueCase> },
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildValueCase {
    pub id: StructuralCaseId,
    pub name: String,
    pub fields: Vec<TerminalBuildValueField>,
}
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildValueField {
    pub id: StructuralFieldId,
    pub name: String,
    pub shape: Arc<TerminalBuildValueShape>,
}

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    source_state: symbols::SymbolHandle,
    statement_index: usize,
    call: &typed_trees::expression::TableCallExpression,
    boundary: semantic_vocabulary::BoundaryMachineId,
    cache: &mut BTreeMap<StructuralTypeId, Arc<TerminalBuildValueShape>>,
) -> Result<TerminalBuildNativeLocator, Diagnostic> {
    let machine = checked
        .machine_holding_state(source_state)
        .ok_or_else(|| Diagnostic::error("native binding lost its exact source machine"))?;
    let state = checked
        .state_by_symbol(source_state)
        .ok_or_else(|| Diagnostic::error("native binding lost its exact source state"))?;
    let [_, _, value] = checked.expression_table.expression_handles(call.arguments) else {
        return Err(Diagnostic::error(
            "native binding lost its owned locator operand",
        ));
    };
    // The checked producer owns a contextually constructed value's carrier.
    // Expression normalization may clear its early inference annotation; an
    // expected formal is not a replacement for the completed value root.
    let reference = u32::try_from(statement_index)
        .ok()
        .and_then(|statement| {
            checked.facts.values.structural_values.root_for_expression(
                source_state,
                statement,
                *value,
            )
        })
        .filter(|root| root.machine == machine.symbol)
        .map(|root| root.type_reference)
        .or_else(|| validation::expression_result_type_reference(checked, machine, state, *value))
        .ok_or_else(|| Diagnostic::error("native binding locator has no exact produced type"))?;
    let vocabulary =
        validation::native_locators::vocabulary_for_owner(&checked.typed, machine.symbol).map_err(
            |errors| {
                errors.into_iter().next().unwrap_or_else(|| {
                    Diagnostic::error("native locator vocabulary failed without a diagnostic")
                })
            },
        )?;
    let widths =
        validation::native_locators::binding_widths(&checked.typed, vocabulary.binding, reference)
            .map_err(Diagnostic::error)?;
    let declaration = lowered
        .semantic_module
        .boundary_machines
        .iter()
        .find(|row| row.id == boundary)
        .ok_or_else(|| Diagnostic::error("native binding lost its emitted boundary signature"))?;
    let [parameter] = declaration.structural_parameters.as_slice() else {
        return Err(Diagnostic::error(
            "native binding must retain one owned structural locator",
        ));
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(Diagnostic::error(
            "native binding cannot retain borrowed locator storage",
        ));
    }
    let structural_type = parameter.structural_type;
    let row = lowered
        .semantic_module
        .structural_type(structural_type)
        .ok_or_else(|| Diagnostic::error("native locator has no emitted structural declaration"))?;
    if lowered.semantic_module.identities.text(row.identity)
        != Some(checked.normalized_type_identity_text(reference).as_str())
    {
        return Err(Diagnostic::error(
            "native locator source type and emitted storage identity disagree",
        ));
    }
    let shape = collect(
        &lowered.semantic_module,
        structural_type,
        cache,
        &mut BTreeSet::new(),
    )?;
    Ok(TerminalBuildNativeLocator {
        structural_type,
        widths,
        shape,
    })
}

fn collect(
    module: &TerminalModule,
    id: StructuralTypeId,
    cache: &mut BTreeMap<StructuralTypeId, Arc<TerminalBuildValueShape>>,
    visiting: &mut BTreeSet<StructuralTypeId>,
) -> Result<Arc<TerminalBuildValueShape>, Diagnostic> {
    if let Some(shape) = cache.get(&id) {
        return Ok(Arc::clone(shape));
    }
    if !visiting.insert(id) {
        return Err(Diagnostic::error(
            "native locator has a recursive owned shape",
        ));
    }
    let row = module
        .structural_type(id)
        .ok_or_else(|| Diagnostic::error("native locator child has no declaration"))?;
    // The boundary consumes the owned value. An affine carrier needs no
    // copying permission; ordinary source transfer/cleanup remains authoritative.
    if row.owns_linear_claim || row.quotient_carrier.is_some() {
        return Err(Diagnostic::error(
            "native locator cannot retain linear claims or quotient storage",
        ));
    }
    let shape = match &row.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => TerminalBuildValueShape::Scalar(*scalar),
        StructuralTypeShape::FixedArray { element, length } => TerminalBuildValueShape::Array {
            length: length.literal().ok_or_else(|| Diagnostic::error(
                "native locator requires a closed array extent",
            ))?,
            element: collect(module, *element, cache, visiting)?,
        },
        StructuralTypeShape::Sum { cases } => TerminalBuildValueShape::Sum {
            cases: cases.iter().map(|case| Ok(TerminalBuildValueCase {
                id: case.id,
                name: label(module, case.identity)?.to_owned(),
                fields: case.fields.iter().map(|field| {
                    if field.relevance.is_erased() { return Err(Diagnostic::error("native locator cannot consume erased fields")); }
                    let shape = match field.field_type {
                        StructuralFieldType::Scalar(scalar) => Arc::new(TerminalBuildValueShape::Scalar(scalar)),
                        StructuralFieldType::Structural(child) => collect(module, child, cache, visiting)?,
                        _ => return Err(Diagnostic::error("native locator field is not an owned scalar or structural value")),
                    };
                    Ok(TerminalBuildValueField { id: field.id, name: label(module, field.identity)?.to_owned(), shape })
                }).collect::<Result<_, Diagnostic>>()?,
            })).collect::<Result<_, Diagnostic>>()?,
        },
        _ => return Err(Diagnostic::error("native locator cannot retain references, views or opaque storage")),
    };
    visiting.remove(&id);
    let shape = Arc::new(shape);
    cache.insert(id, Arc::clone(&shape));
    Ok(shape)
}
fn label(
    module: &TerminalModule,
    identity: terminal_psi::TerminalIdentityIndex,
) -> Result<&str, Diagnostic> {
    module
        .identities
        .text(identity)
        .ok_or_else(|| Diagnostic::error("native locator descriptor label is absent"))
}
