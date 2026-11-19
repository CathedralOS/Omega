use crate::TargetObservation;
use crate::constant_evaluation::ConstantEvaluation;
use crate::floating_point::PreparedFloat;
use crate::integer_arithmetic::IntegerRange;
use crate::invocation::{EvaluationError, UNSUPPORTED_BODY};
use num_bigint::BigInt;
use omega_resolver::DeclarationId;
use omega_typer::{
    Multiplicity, NodeSemantics, ObligationKind, Site, StaticValue, StaticValueDemand, TypedConstruction, TypedTree, TypedTrees, TypingObligation, ValueType,
};
use std::{cell::OnceCell, collections::HashSet, sync::Arc};

/// A frozen semantic value. Aggregate fields are in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreparedValue {
    Integer(BigInt),
    Bool(bool),
    Float {
        value: PreparedFloat,
        origin: Site,
    },
    Array(Arc<[PreparedValue]>),
    Tuple(Arc<[PreparedValue]>),
    Record {
        owner: DeclarationId,
        fields: Arc<[PreparedValue]>,
    },
    Sum {
        owner: DeclarationId,
        case: DeclarationId,
        fields: Arc<[PreparedValue]>,
    },
}

impl PreparedValue {
    pub fn float(&self) -> Option<PreparedFloat> {
        match self {
            Self::Float { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Static identity accepts a smaller set of values than constants do.
    fn as_static(
        &self,
        requires_proof: &impl Fn(&DeclarationId) -> bool,
    ) -> Option<StaticValue> {
        Some(match self {
            Self::Integer(value) => StaticValue::Integer(value.clone()),
            Self::Bool(value) => StaticValue::Bool(*value),
            Self::Float { .. } | Self::Tuple(_) => return None,
            Self::Array(values) => StaticValue::Array(static_fields(values, requires_proof)?),
            Self::Record { owner, fields } if !requires_proof(owner) => StaticValue::Record {
                owner: owner.clone(),
                fields: static_fields(fields, requires_proof)?,
            },
            Self::Sum { owner, case, fields } if !requires_proof(owner) => StaticValue::Sum {
                owner: owner.clone(),
                case: case.clone(),
                fields: static_fields(fields, requires_proof)?,
            },
            Self::Record { .. } | Self::Sum { .. } => return None,
        })
    }

    /// Check that the active semantic components determine their observable values.
    /// A consumer still needs a selected layout before writing runtime bytes.
    pub fn check_materializable(&self) -> Result<(), MaterializationError> {
        enum Component<'a> {
            Element(usize),
            Field(&'a DeclarationId, usize),
        }
        let mut stack = vec![(self, None, 0)];
        let mut path = Vec::new();
        while let Some((value, component, depth)) = stack.pop() {
            path.truncate(depth);
            path.extend(component);
            let fields = match value {
                Self::Float {
                    value: PreparedFloat::UnspecifiedNaN(_),
                    origin,
                } => {
                    return Err(MaterializationError {
                        path: path
                            .into_iter()
                            .map(|component| match component {
                                Component::Element(index) => MaterializationComponent::Element(index),
                                Component::Field(owner, index) => MaterializationComponent::Field { owner: owner.clone(), index },
                            })
                            .collect(),
                        origin: origin.clone(),
                    });
                }
                Self::Integer(_) | Self::Bool(_) | Self::Float { .. } => continue,
                Self::Array(fields) | Self::Tuple(fields) => (fields, None),
                Self::Record { owner, fields } => (fields, Some(owner)),
                Self::Sum { case, fields, .. } => (fields, Some(case)),
            };
            let (fields, owner) = fields;
            stack.extend(fields.iter().enumerate().rev().map(|(index, field)| {
                let component = owner.map_or(Component::Element(index), |owner| Component::Field(owner, index));
                (field, Some(component), path.len())
            }));
        }
        Ok(())
    }
}

/// Path from the constant's root to the first component with unspecified NaN bits.
#[derive(Debug, Eq, PartialEq)]
pub struct MaterializationError {
    pub path: Vec<MaterializationComponent>,
    pub origin: Site,
}

#[derive(Debug, Eq, PartialEq)]
pub enum MaterializationComponent {
    Element(usize),
    /// A declaration-order slot in a record or an active sum case.
    Field {
        owner: DeclarationId,
        index: usize,
    },
}

fn static_fields(
    values: &[PreparedValue],
    requires_proof: &impl Fn(&DeclarationId) -> bool,
) -> Option<Arc<[StaticValue]>> {
    values
        .iter()
        .map(|value| value.as_static(requires_proof))
        .collect()
}

impl From<&StaticValue> for PreparedValue {
    fn from(value: &StaticValue) -> Self {
        match value {
            StaticValue::Integer(value) => Self::Integer(value.clone()),
            StaticValue::Bool(value) => Self::Bool(*value),
            StaticValue::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            StaticValue::Record { owner, fields } => Self::Record {
                owner: owner.clone(),
                fields: fields.iter().map(Self::from).collect(),
            },
            StaticValue::Sum { owner, case, fields } => Self::Sum {
                owner: owner.clone(),
                case: case.clone(),
                fields: fields.iter().map(Self::from).collect(),
            },
        }
    }
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
    pub(super) fn canonical_identity(
        &self,
        value: &PreparedValue,
        demand: &StaticValueDemand,
    ) -> Option<StaticValue> {
        let proof = OnceCell::new();
        let canonical = value.as_static(&|owner| {
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

    pub(super) fn owned_field_position(
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

    pub(super) fn construction<'a>(
        &'a self,
        tree: &'a TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        owner: &DeclarationId,
    ) -> Result<(&'a TypedConstruction, &'a [ValueType]), EvaluationError> {
        let construction = tree
            .construction(node)
            .ok_or("the selected nominal value has no supported initializer")?;
        if &construction.owner != owner || &construction.carrier != carrier {
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

    pub(super) fn check_construction_proof(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        fields: &[PreparedValue],
    ) -> Result<(), EvaluationError> {
        let construction = tree.construction(node).ok_or(UNSUPPORTED_BODY)?;
        if let Some(obligation) = construction_obligation(tree, node, &construction.owner)? {
            let proof = self
                .nominal
                .establishments
                .iter()
                .find(|proof| {
                    proof.covers(self.trees, obligation, &construction.carrier)
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

    pub(super) fn construction_positions(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        construction: &TypedConstruction,
        fields: &[ValueType],
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
                if fields.get(position) != Some(ty) || std::mem::replace(&mut seen[position], true) {
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

    pub(super) fn check_owned_type(
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

    pub(super) fn constant_reference(
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
