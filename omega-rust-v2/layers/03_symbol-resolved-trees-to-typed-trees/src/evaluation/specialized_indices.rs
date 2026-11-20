//! An open static operation keeps its resolved syntax and the owner's exact arguments.
//! L4 supplies values for closed substitutions that require general evaluation.
use super::static_value::StaticValueCarrier;
use crate::declarations::{declaration_catalog::DeclarationCatalog, resolved_source_file::ResolvedSourceFile};
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::{substitution::TypeInference, type_catalog::TypeCatalog};
use crate::{SpecializedIndexValue, StaticValue, StaticValueRequest, TypeError, ValueType};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
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
            let site = &receipt.request.expression;
            let n = receipt.request.node.node;
            let unit = units
                .get(receipt.request.node.file)
                .filter(|u| u.file.package == receipt.request.expression.package && u.file.path == receipt.request.expression.file)
                .ok_or_else(|| TypeError::for_site(site, "evaluation result refers to an unadmitted file"))?;
            if n >= unit.file.nodes.len() || unit.file.span(n) != receipt.request.expression.span {
                return Err(TypeError::for_site(site, "evaluation result refers to an unknown static expression"));
            }
            let owner = unit
                .specialization_owner(n)
                .ok_or_else(|| TypeError::for_site(site, "evaluation result has no owning declaration"))?;
            let arguments =
                ClosedArguments::try_from(receipt.arguments.clone()).map_err(|_| TypeError::for_site(site, "evaluation requires closed arguments"))?;
            let entry = prepared
                .values
                .entry((syntax_key(unit, n, owner), receipt.request.carrier.clone()))
                .or_default();
            if Arc::make_mut(entry)
                .insert(arguments, receipt.value.clone())
                .is_some_and(|v| v != receipt.value)
            {
                return Err(TypeError::for_site(site, "conflicting evaluations of the same specialized expression"));
            }
        }
        Ok(prepared)
    }
    /// Each receipt binds the owner's parameters from outermost to innermost.
    /// A stored result may be reused on another pass, so even unused tuples need
    /// valid arguments. Specialize each binder's carrier before checking its value.
    pub(super) fn check_receipt_arguments(
        units: &[ResolvedSourceFile<'_>],
        catalog: &DeclarationCatalog<'_>,
        receipt: &SpecializedIndexValue,
    ) -> Result<(), TypeError> {
        let request = &receipt.request;
        let owner = units
            .get(request.node.file)
            .and_then(|unit| unit.specialization_owner(request.node.node))
            .ok_or_else(|| TypeError::for_site(&request.expression, "evaluation result has no owning declaration"))?;
        Self::check_owner_arguments(catalog, request, owner, &receipt.arguments)
    }

    fn check_owner_arguments(
        catalog: &DeclarationCatalog<'_>,
        request: &StaticValueRequest,
        owner: &DeclarationId,
        mut rest: &[ValueType],
    ) -> Result<(), TypeError> {
        let types = &catalog.types;
        let mut scopes = Vec::new();
        for scope in types.specialization_scopes(owner) {
            let (arguments, remaining) = rest
                .split_at_checked(types.arity(scope))
                .ok_or_else(|| TypeError::for_site(&request.expression, "specialized type argument has the wrong number of arguments"))?;
            rest = remaining;
            scopes.push((scope, arguments));
            if !types.generics[scope].equations.is_empty() {
                types
                    .solve_arguments(scope, arguments.to_vec())
                    .map_err(|reason| TypeError::for_site(&request.expression, reason))?;
            }
            for (index, argument) in arguments.iter().enumerate() {
                argument.validate_closed_indices().map_err(|reason| {
                    TypeError::for_site(
                        &request.expression,
                        format!("invalid evaluation argument {} of {}: {reason}", index + 1, scope.path),
                    )
                })?;
                let parameter = &types.generics[scope].parameters[index];
                let bound = types.bound(scope, index);
                if parameter.kind == NodeKind::TypeParam || matches!(bound, Some(ValueType::Universe { .. })) {
                    if !is_type_argument(argument, catalog, request)? {
                        return Err(TypeError::for_site(
                            &request.expression,
                            "specialized evaluation argument does not match its owner's type parameter",
                        ));
                    }
                    types
                        .validate_bound(scope, index, argument)
                        .map_err(|reason| TypeError::for_site(&request.expression, reason))?;
                }
                let Some(bound) = bound else { continue };
                let carrier = scopes
                    .iter()
                    .fold(bound.clone(), |ty, (owner, args)| ty.substitute(owner, args));
                if matches!(carrier, ValueType::Universe { .. }) {
                    TypeCatalog::check_universe_member(argument, &carrier).map_err(|reason| TypeError::for_site(&request.expression, reason))?;
                }
                let valid = match carrier.carrier() {
                    ValueType::Integer { .. } => match argument {
                        ValueType::StaticInteger { carrier: actual, value } => {
                            actual.carrier() == carrier.carrier() && carrier.carrier().contains_integer(value)
                        }
                        ValueType::Subject { .. } | ValueType::BoundSubject(_) => types.admits_runtime_index(scope, index),
                        _ => false,
                    },
                    _ if carrier.supports_closed_snapshot() => matches!(argument, ValueType::StaticValue { carrier: actual, value }
                        if actual.carrier() == carrier.carrier() && value.matches_carrier(&carrier, catalog)),
                    _ => true,
                };
                if !valid {
                    return Err(TypeError::for_site(
                        &request.expression,
                        "specialized evaluation argument does not match its owner's static parameter",
                    ));
                }
            }
        }
        if !rest.is_empty() {
            return Err(TypeError::for_site(
                &request.expression,
                "specialized type argument has the wrong number of arguments",
            ));
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

/// Closed types include dependent families and erased mathematical types. A
/// closed value, callback selection or proof is an argument of another kind.
fn is_type_argument(
    argument: &ValueType,
    catalog: &DeclarationCatalog<'_>,
    request: &StaticValueRequest,
) -> Result<bool, TypeError> {
    use ValueType::*;
    let valid = |ty: &ValueType| is_type_argument(ty, catalog, request);
    Ok(match argument.carrier() {
        Tuple(items) => {
            for item in items {
                if !valid(item)? {
                    return Ok(false);
                }
            }
            true
        }
        Function(input, output) => valid(input)? && valid(output)?,
        Reference(_, item) | Slice(item) | Squash(item) | Array(item, _) | IndexedArray(item, _) | AtLifetime(item, _) => valid(item)?,
        Nominal(id) | Applied(id, _) => {
            if !catalog.types.scopes.contains_key(id.as_ref()) {
                return Ok(false);
            }
            let (_, args) = argument.carrier().nominal_parts().unwrap();
            IndexSpecializations::check_owner_arguments(catalog, request, id, args)?;
            true
        }
        Bool
        | Integer { .. }
        | IndexedInteger { .. }
        | Address
        | Float32
        | Float64
        | Witness { .. }
        | Callable(_)
        | Evidence(_)
        | LevelKind
        | Universe { .. }
        | FamilyApplication { .. } => true,
        _ => false,
    })
}
