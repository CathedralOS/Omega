//! An open static operation keeps its resolved syntax and the owner's exact arguments.
//! L4 supplies values for closed substitutions that require general evaluation.
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::type_error::SourceTypeError;
use crate::type_system::substitution::TypeInference;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{SpecializedIndexValue, StaticValue, StaticValueRequest, TypeError, ValueType};
use omega_resolved::DeclarationId;
use omega_typed::{ClosedArguments, StaticTemplate, SyntaxKey};
use std::collections::HashMap;
use std::sync::Arc;

fn syntax_key(
    unit: &ResolvedSourceFile<'_>,
    n: usize,
    owner: &DeclarationId,
) -> SyntaxKey {
    SyntaxKey::new(
        owner.clone(),
        (n..unit.file.nodes[n].end)
            .map(|i| {
                let node = &unit.file.nodes[i];
                (
                    node.kind,
                    node.children.len(),
                    node.leaves.iter().map(|t| t.text.to_owned()).collect(),
                    // Receipts identify the same resolved source before and after L3 selects names.
                    unit.tree.symbol(i).cloned(),
                )
            })
            .collect(),
    )
}
#[derive(Default)]
pub(crate) struct IndexSpecializations {
    allow_pending: bool,
    values: HashMap<(SyntaxKey, ValueType), Arc<HashMap<ClosedArguments, StaticValue>>>,
}
impl IndexSpecializations {
    pub fn build(
        units: &[ResolvedSourceFile<'_>],
        receipts: &[SpecializedIndexValue],
        allow_pending: bool,
    ) -> Result<Self, TypeError> {
        let mut prepared = Self {
            allow_pending,
            ..Self::default()
        };
        for receipt in receipts {
            let fail = |message: &str| TypeError::for_site(&receipt.request.expression, message);
            let n = receipt.request.node.node;
            let unit = units
                .get(receipt.request.node.file)
                .filter(|u| u.file.package == receipt.request.expression.package && u.file.path == receipt.request.expression.file)
                .ok_or_else(|| fail("evaluation result refers to an unadmitted file"))?;
            if n >= unit.file.nodes.len() || unit.file.span(n) != receipt.request.expression.span {
                return Err(fail("evaluation result refers to an unknown static expression"));
            }
            let owner = unit
                .specialization_owner(n)
                .ok_or_else(|| fail("evaluation result has no owning declaration"))?;
            let arguments = ClosedArguments::try_from(receipt.arguments.clone()).map_err(|_| fail("evaluation requires closed arguments"))?;
            let entry = prepared
                .values
                .entry((syntax_key(unit, n, owner), receipt.request.carrier.clone()))
                .or_default();
            if Arc::make_mut(entry)
                .insert(arguments, receipt.value.clone())
                .is_some_and(|v| v != receipt.value)
            {
                return Err(fail("conflicting evaluations of the same specialized expression"));
            }
        }
        Ok(prepared)
    }
    /// A receipt's owner tuple concatenates the formals of every specialization scope, outermost first.
    /// Each closed argument at an integer or Boolean static binder must be a value of that binder's carrier,
    /// as `raw_bound_argument` would form it for a written application. The tuple need not be one this pass uses.
    pub(super) fn check_receipt_arguments(
        units: &[ResolvedSourceFile<'_>],
        types: &TypeCatalog<'_>,
        receipt: &SpecializedIndexValue,
    ) -> Result<(), TypeError> {
        let request = &receipt.request;
        let owner = units
            .get(request.node.file)
            .and_then(|unit| unit.specialization_owner(request.node.node))
            .ok_or_else(|| TypeError::for_site(&request.expression, "evaluation result has no owning declaration"))?;
        let slots = types
            .specialization_scopes(owner)
            .flat_map(|scope| (0..types.arity(scope)).map(move |index| (scope, index)));
        for ((scope, index), argument) in slots.zip(&receipt.arguments) {
            argument.validate_closed_indices().map_err(|reason| {
                TypeError::for_site(
                    &request.expression,
                    format!("invalid evaluation argument {} of {}: {reason}", index + 1, scope.path),
                )
            })?;
            if !matches_scalar_binder(types, scope, index, argument) {
                return Err(TypeError::for_site(
                    &request.expression,
                    "specialized evaluation argument does not match its owner's static parameter",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn template(
        &self,
        unit: &ResolvedSourceFile<'_>,
        n: usize,
        owner: &DeclarationId,
        carrier: &ValueType,
    ) -> Arc<StaticTemplate> {
        let key = (syntax_key(unit, n, owner), carrier.clone());
        let values = self.values.get(&key).cloned().unwrap_or_default();
        Arc::new(StaticTemplate::new(
            key.0,
            StaticValueRequest::at(unit.file, n, carrier),
            self.allow_pending,
            values,
        ))
    }
}

/// Only integer and Boolean binder carriers are checked. Other binder kinds pass unchanged.
fn matches_scalar_binder(
    types: &TypeCatalog<'_>,
    scope: &DeclarationId,
    index: usize,
    argument: &ValueType,
) -> bool {
    let Some(bound) = types.bound(scope, index) else {
        return true;
    };
    match bound.carrier() {
        ValueType::Integer { .. } => match argument {
            ValueType::StaticInteger { carrier, value } => carrier.carrier() == bound.carrier() && bound.carrier().contains_integer(value),
            ValueType::Subject { .. } | ValueType::BoundSubject(_) => types.admits_runtime_index(scope, index),
            _ => false,
        },
        ValueType::Bool => matches!(
            argument,
            ValueType::StaticValue { carrier, value: StaticValue::Bool(_) } if carrier.carrier() == &ValueType::Bool
        ),
        _ => true,
    }
}
