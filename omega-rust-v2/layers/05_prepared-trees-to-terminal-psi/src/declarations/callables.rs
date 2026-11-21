//! Runtime callable contracts. A machine value is a descriptor of a closed
//! machine application; its structural contract is declared once, independently
//! of every machine that may satisfy it, and owns the full call shape.
//!
//! A contract's declaration site is the signature on the right-hand side of
//! its transparent alias. Every use of one alias application shares that
//! contract. Distinct applications of a generic alias keep the alias site but
//! remain distinct structural contracts.
use crate::lowering::types::{TypeError, scalar_carrier};
use crate::{CallableContractId, LoweringError, TemplateCallable};
use omega_prepared::PreparedTrees;
use omega_typed::{CallableType, ValueType};

/// The descriptor carries its contract's complete argument and result shapes.
/// A direct borrowed result additionally needs a public lifetime relation;
/// its implementation still owes backing and access at each return.
pub(crate) fn supported(callable: &CallableType) -> bool {
    callable.closed_runtime
        && callable.requirement.is_none()
        && callable.reach.as_ref().is_some_and(Vec::is_empty)
        && callable.crash_ceiling.as_ref().is_some_and(Vec::is_empty)
        && !callable.pause.block
        && callable.parameters.iter().all(supported_formal)
        && (callable.result == ValueType::NoResult
            || scalar_carrier(&callable.result).is_ok()
            || result_lifetime_input(&callable.result, callable.parameters.iter()).is_some())
}

pub(crate) fn supported_formal(ty: &ValueType) -> bool {
    use omega_typed::Access::{Mut, Shared};
    scalar_carrier(ty).is_ok()
        || matches!(ty, ValueType::Reference(Shared | Mut, inner) if scalar_carrier(inner).is_ok() || matches!(inner.as_ref(), ValueType::Nominal(_)))
}

/// With one direct scalar reference among otherwise scalar inputs, elision
/// relates a shared scalar reference result to that input. An aggregate or
/// open input would need its entire reference frontier. An explicit lifetime
/// is a distinct typed constructor and never enters this rule.
pub(super) fn result_lifetime_input<'a>(
    result: &ValueType,
    inputs: impl Iterator<Item = &'a ValueType>,
) -> Option<usize> {
    if !matches!(result, ValueType::Reference(omega_typed::Access::Shared, inner) if scalar_carrier(inner).is_ok()) {
        return None;
    }
    let mut input = None;
    for (index, ty) in inputs.enumerate() {
        if scalar_carrier(ty).is_ok() {
            continue;
        }
        if !matches!(ty, ValueType::Reference(omega_typed::Access::Shared | omega_typed::Access::Mut, inner) if scalar_carrier(inner).is_ok())
            || input.is_some()
        {
            return None;
        }
        input = Some(index);
    }
    input
}

/// Collect each distinct runtime callable carrier. A contract is identified by
/// its declaration site and its complete structural type. Rows are ordered by
/// site; applications of one generic alias keep their first-use order.
pub(crate) fn collect<'a>(carriers: impl Iterator<Item = &'a ValueType>) -> Result<Vec<CallableType>, (usize, TypeError)> {
    let mut found: Vec<&CallableType> = Vec::new();
    for (index, carrier) in carriers.enumerate() {
        let ValueType::Callable(callable) = carrier else { continue };
        if !supported(callable) {
            return Err((index, TypeError::UnsupportedCarrier(format!("runtime callable {carrier}"))));
        }
        found.push(callable);
    }
    // A stable sort keeps the encounter order among applications of one site.
    found.sort_by(|a, b| a.contract.cmp(&b.contract));
    let mut table: Vec<CallableType> = Vec::with_capacity(found.len());
    for callable in found {
        let start = table.partition_point(|row| row.contract < callable.contract);
        if !table[start..].contains(callable) {
            table.push(callable.clone());
        }
    }
    Ok(table)
}

pub(crate) fn find(
    table: &[CallableType],
    callable: &CallableType,
) -> Result<CallableContractId, TypeError> {
    let missing = || TypeError::UnsupportedCarrier(format!("runtime callable {}", ValueType::Callable(Box::new(callable.clone()))));
    let start = table.partition_point(|row| row.contract < callable.contract);
    let index = table[start..]
        .iter()
        .take_while(|row| row.contract == callable.contract)
        .position(|row| row == callable)
        .ok_or_else(missing)?;
    CallableContractId::from_index(start + index).ok_or_else(missing)
}

/// The independent Psi declaration of one admitted contract. Its predicates
/// come from the signature that declares the contract, through the same
/// interface lowering an alias expansion uses, so a contract and its alias
/// never publish different obligations.
pub(crate) fn contract(
    prepared: &PreparedTrees<'_>,
    callable: &CallableType,
) -> Result<TemplateCallable, LoweringError> {
    let site = &callable.contract;
    if !supported(callable) {
        return Err(LoweringError::new(site, "runtime callable contract requires unsupported obligation lowering"));
    }
    let (origin, node) = prepared
        .typed()
        .callable_origin(site)
        .ok_or_else(|| LoweringError::new(site, "runtime callable contract lacks its checked structural signature"))?;
    super::type_expression::callable_interface(prepared, &origin, node, callable)
}
