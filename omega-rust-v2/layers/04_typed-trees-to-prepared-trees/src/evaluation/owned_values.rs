use crate::evaluation::constant_evaluation::ConstantEvaluation;
use crate::evaluation::integer_arithmetic::IntegerRange;
use crate::invocation::{EvaluationError, UNSUPPORTED_BODY};
use crate::{PreparedValue, TargetObservation};
use omega_resolved::DeclarationId;
use omega_typed::{
    Multiplicity, NodeSemantics, ObligationKind, StaticValue, StaticValueDemand, TypedConstruction, TypedTree, TypedTrees, TypingObligation, ValueType,
};
use std::{cell::OnceCell, collections::HashSet, sync::Arc};

/// Static identity accepts a smaller set of values than constants do.
fn as_static(
    value: &PreparedValue,
    requires_proof: &impl Fn(&DeclarationId) -> bool,
) -> Option<StaticValue> {
    Some(match value {
        PreparedValue::Integer(value) => StaticValue::Integer(value.clone()),
        PreparedValue::Bool(value) => StaticValue::Bool(*value),
        PreparedValue::Float { .. } | PreparedValue::Tuple(_) => return None,
        PreparedValue::Array(values) => StaticValue::Array(static_fields(values, requires_proof)?),
        PreparedValue::Record { owner, fields } if !requires_proof(owner) => StaticValue::Record {
            owner: owner.clone(),
            fields: static_fields(fields, requires_proof)?,
        },
        PreparedValue::Sum { owner, case, fields } if !requires_proof(owner) => StaticValue::Sum {
            owner: owner.clone(),
            case: case.clone(),
            fields: static_fields(fields, requires_proof)?,
        },
        PreparedValue::Record { .. } | PreparedValue::Sum { .. } => return None,
    })
}

fn static_fields(
    values: &[PreparedValue],
    requires_proof: &impl Fn(&DeclarationId) -> bool,
) -> Option<Arc<[StaticValue]>> {
    values
        .iter()
        .map(|value| as_static(value, requires_proof))
        .collect()
}

#[derive(Default)]
pub(crate) struct NominalAdmission<'trees, 'text> {
    pub(crate) establishments: Vec<CheckedDataEstablishment<'trees, 'text>>,
    pub(crate) canonical: Vec<CheckedCanonicalIdentity<'trees, 'text>>,
    constrained: OnceCell<HashSet<&'trees DeclarationId>>,
}

/// A checked judgment that this demanded representation has a unique canonical
/// identity, including the required facts for its constrained components.
pub struct CheckedCanonicalIdentity<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    demand: StaticValueDemand,
    representative: PreparedValue,
    target_dependencies: Vec<TargetObservation>,
}

/// Construction can need a proof about the exact case and its initialized
/// fields. Evidence for another construction or typing pass cannot justify it.
pub struct CheckedDataEstablishment<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    obligation: &'trees TypingObligation,
    carrier: ValueType,
    fields: Arc<[PreparedValue]>,
    target_dependencies: Vec<TargetObservation>,
}

impl CheckedDataEstablishment<'_, '_> {
    fn covers(
        &self,
        trees: &TypedTrees<'_>,
        obligation: &TypingObligation,
        carrier: &ValueType,
    ) -> bool {
        std::ptr::eq(self.trees, trees) && std::ptr::eq(self.obligation, obligation) && &self.carrier == carrier
    }
}

const NEEDS_CONSTRUCTION_PROOF: &str = "the selected constructor requires proof outside the prepared value fragment";

fn construction_obligation<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    owner: &DeclarationId,
) -> Result<Option<&'a TypingObligation>, EvaluationError> {
    let mut establishment = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Expression(_) => {}
            NodeSemantics::Obligation(obligation)
                if matches!(&obligation.kind, ObligationKind::EstablishData(required) if required == owner) && establishment.is_none() =>
            {
                establishment = Some(obligation)
            }
            NodeSemantics::Obligation(obligation) if matches!(obligation.kind, ObligationKind::ZeroField { .. }) => {}
            _ => return Err(NEEDS_CONSTRUCTION_PROOF.into()),
        }
    }
    Ok(establishment)
}

impl ConstantEvaluation<'_, '_> {
    pub(crate) fn canonical_identity(
        &self,
        value: &PreparedValue,
        demand: &StaticValueDemand,
    ) -> Option<StaticValue> {
        let proof = OnceCell::new();
        let canonical = as_static(value, &|owner| {
            self.nominal
                .constrained
                .get_or_init(|| {
                    self.trees
                        .static_data_shapes
                        .iter()
                        .filter_map(|(carrier, shape)| match carrier {
                            ValueType::Nominal(id) | ValueType::Applied(id, _) if shape.requires_establishment => Some(id.as_ref()),
                            _ => None,
                        })
                        .collect()
                })
                .contains(owner)
                && proof
                    .get_or_init(|| {
                        self.nominal.canonical.iter().find(|checked| {
                            std::ptr::eq(checked.trees, self.trees)
                                && &checked.demand == demand
                                && &checked.representative == value
                                && checked.target_dependencies.iter().all(|used| {
                                    self.target
                                        .is_some_and(|target| target.matches_observation(used))
                                })
                        })
                    })
                    .is_none()
        })?;
        if let Some(Some(proof)) = proof.into_inner() {
            for dependency in &proof.target_dependencies {
                self.record_observation(dependency.clone());
            }
        }
        Some(canonical)
    }

    pub(crate) fn owned_field_position(
        &self,
        shape: &DeclarationId,
        field: &DeclarationId,
    ) -> Result<usize, EvaluationError> {
        self.trees
            .record_fields
            .get(shape)
            .or_else(|| self.trees.library?.record_fields.get(shape))
            .and_then(|fields| fields.iter().position(|entry| &entry.declaration == field))
            .ok_or_else(|| "the selected field is not in the active owned value".into())
    }

    pub(crate) fn construction<'a>(
        &'a self,
        tree: &'a TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        owner: &DeclarationId,
        instantiate: impl Fn(&ValueType) -> ValueType,
    ) -> Result<(&'a TypedConstruction, &'a [ValueType]), EvaluationError> {
        let construction = tree
            .construction(node)
            .ok_or("the selected nominal value has no supported initializer")?;
        if &construction.owner != owner || &instantiate(&construction.carrier) != carrier {
            return Err("the selected constructor has a different carrier".into());
        }
        let shape = self
            .trees
            .static_data_shapes
            .get(carrier)
            .ok_or(UNSUPPORTED_BODY)?;
        let fields = match &construction.case {
            Some(case) => shape
                .cases
                .iter()
                .find(|(declared, _)| declared == case)
                .map(|(_, payload)| payload),
            None if shape.cases.is_empty() => Some(&shape.fields),
            None => None,
        }
        .ok_or("the selected case does not belong to the carrier")?;
        let establishment = construction_obligation(tree, node, owner)?;
        if shape.requires_establishment != establishment.is_some() {
            return Err(NEEDS_CONSTRUCTION_PROOF.into());
        }
        if let Some(obligation) = establishment
            && !self
                .nominal
                .establishments
                .iter()
                .any(|proof| proof.covers(self.trees, obligation, carrier))
        {
            return Err(NEEDS_CONSTRUCTION_PROOF.into());
        }
        Ok((construction, fields))
    }

    pub(crate) fn check_construction_proof(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        fields: &[PreparedValue],
    ) -> Result<(), EvaluationError> {
        let construction = tree.construction(node).ok_or(UNSUPPORTED_BODY)?;
        if let Some(obligation) = construction_obligation(tree, node, &construction.owner)? {
            let proof = self
                .nominal
                .establishments
                .iter()
                .find(|proof| {
                    proof.covers(self.trees, obligation, carrier)
                        && proof.fields.as_ref() == fields
                        && proof.target_dependencies.iter().all(|used| {
                            self.target
                                .is_some_and(|target| target.matches_observation(used))
                        })
                })
                .ok_or(NEEDS_CONSTRUCTION_PROOF)?;
            for dependency in &proof.target_dependencies {
                self.record_observation(dependency.clone());
            }
        }
        Ok(())
    }

    pub(crate) fn construction_positions(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        construction: &TypedConstruction,
        fields: &[ValueType],
        instantiate: impl Fn(&ValueType) -> ValueType,
    ) -> Result<(Vec<usize>, Vec<usize>), EvaluationError> {
        let mut seen = vec![false; fields.len()];
        let mut authored = Vec::with_capacity(construction.initializers.len());
        for initializer in &construction.initializers {
            if std::mem::replace(seen.get_mut(initializer.position).ok_or(UNSUPPORTED_BODY)?, true) {
                return Err(UNSUPPORTED_BODY.into());
            }
            authored.push(initializer.position);
        }
        let mut zero_fields = Vec::new();
        let shape = construction.case.as_ref().unwrap_or(&construction.owner);
        for record in tree.semantics_at(node) {
            if let NodeSemantics::Obligation(obligation) = record
                && let ObligationKind::ZeroField { field, ty } = &obligation.kind
            {
                let position = self.owned_field_position(shape, field)?;
                if fields.get(position) != Some(&instantiate(ty)) || std::mem::replace(&mut seen[position], true) {
                    return Err(UNSUPPORTED_BODY.into());
                }
                zero_fields.push(position);
            }
        }
        if seen.iter().any(|&initialized| !initialized) {
            return Err("the selected constructor has a field without an initializer or typed zero request".into());
        }
        Ok((authored, zero_fields))
    }

    pub(crate) fn check_owned_type(
        &self,
        carrier: &ValueType,
        array: bool,
    ) -> Result<(), EvaluationError> {
        enum Step<'a> {
            Check(&'a ValueType),
            Complete(&'a ValueType),
        }
        let mut steps = vec![Step::Check(carrier)];
        let mut active = HashSet::new();
        let mut checked = HashSet::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Complete(carrier) => {
                    active.remove(carrier);
                    checked.insert(carrier);
                }
                Step::Check(ValueType::Array(element, _)) => steps.push(Step::Check(element)),
                Step::Check(ValueType::Tuple(fields)) => steps.extend(fields.iter().rev().map(Step::Check)),
                Step::Check(carrier @ (ValueType::Nominal(owner) | ValueType::Applied(owner, _))) => {
                    if checked.contains(carrier) {
                        continue;
                    }
                    if !active.insert(carrier) {
                        return Err("a recursive by-value type has no supported frozen value".into());
                    }
                    if !self
                        .trees
                        .data_properties
                        .iter()
                        .any(|(id, properties)| id == owner.as_ref() && properties.multiplicity == Multiplicity::Copy)
                    {
                        return Err(if array {
                            "the complete array element type must permit copying"
                        } else {
                            "the complete constant type must permit copying"
                        }
                        .into());
                    }
                    if self
                        .trees
                        .cleanups
                        .iter()
                        .chain(
                            self.trees
                                .library
                                .into_iter()
                                .flat_map(|library| &library.cleanups),
                        )
                        .any(|selection| &selection.owner == owner.as_ref())
                    {
                        return Err("the complete constant type has a cleanup obligation".into());
                    }
                    let shape = self
                        .trees
                        .static_data_shapes
                        .get(carrier)
                        .ok_or("the complete constant type has no finite typed field shape")?;
                    steps.push(Step::Complete(carrier));
                    steps.extend(
                        shape
                            .cases
                            .iter()
                            .rev()
                            .flat_map(|(_, fields)| fields.iter().rev())
                            .map(Step::Check),
                    );
                    steps.extend(shape.fields.iter().rev().map(Step::Check));
                }
                Step::Check(value) if matches!(value, ValueType::Bool | ValueType::Float32 | ValueType::Float64) || IntegerRange::for_type(value).is_some() => {
                }
                Step::Check(_) => {
                    let reason = if array {
                        "the complete array element type is outside the prepared value fragment"
                    } else {
                        "the complete constant type is outside the prepared value fragment"
                    };
                    return Err(reason.into());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn constant_reference(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
    ) -> Result<Option<&PreparedValue>, EvaluationError> {
        let mut selected = None;
        for record in tree.semantics_at(node) {
            if let NodeSemantics::Constant(id) = record
                && selected.replace(id).is_some()
            {
                return Err("ambiguous constant reference".into());
            }
        }
        let Some(id) = selected else { return Ok(None) };
        self.record_constant_read(id);
        if tree.semantics_at(node).any(|record| match record {
            NodeSemantics::Constant(_) | NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => false,
            NodeSemantics::Obligation(o) if matches!(o.kind, ObligationKind::TotalCrashGuard) => false,
            _ => true,
        }) {
            return Err(UNSUPPORTED_BODY.into());
        }
        let definition = self
            .definitions
            .get(id)
            .ok_or("the selected constant has no local typed initializer")?;
        if &definition.initializer.carrier != carrier {
            return Err("the selected constant has a different carrier".into());
        }
        self.values
            .get(id)
            .map(Some)
            .ok_or_else(|| EvaluationError::NeedsConstant(id.clone()))
    }
}

#[cfg(test)]
mod tests;
