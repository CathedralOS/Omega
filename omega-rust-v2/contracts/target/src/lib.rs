//! Validated selections for one target. Preparation and receiving products supply their own subject and declaration identities.
#![forbid(unsafe_code)]
use omega_program_contracts::Site;
use std::{collections::HashMap, sync::Arc};
mod instruction;
pub use instruction::*;
mod properties;
pub use properties::*;
mod float;
pub use float::*;
mod projection;
pub use projection::*;

pub trait LayoutSubject: Clone + Eq + std::hash::Hash {
    fn has_data_layout(&self) -> bool;
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RequirementSlot<S, D> {
    pub declaration: D,
    pub trait_arguments: Vec<S>,
}

/// The exact checked target selection used to run a requirement during preparation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SelectedRealization<S, D> {
    pub requirement: RequirementSlot<S, D>,
    pub arguments: Vec<S>,
    pub implementation: D,
    pub conformance: Option<D>,
    pub binding: Site,
    pub checked_contract: Arc<[u8]>,
    pub selected_plan: Arc<[u8]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetIdentity {
    pub profile: String,
    pub semantics_version: u32,
    pub realization: Arc<[u8]>,
}

impl TargetIdentity {
    pub fn validate(&self) -> Result<(), TargetSemanticsError> {
        if self.profile.is_empty() || self.realization.is_empty() {
            return Err(TargetSemanticsError::MissingTargetIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LayoutExtent {
    Fixed(u64),
    Dynamic,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayoutGeometry {
    pub extent: LayoutExtent,
    pub alignment: Option<u64>,
}

/// The exact layout projection supplied by the compiler's layout-plan owner.
/// The identities retain its normalized schema and plan application, including
/// layout-policy selection. An observed byte count alone cannot identify either.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SelectedLayout<S> {
    pub target: TargetIdentity,
    pub subject: S,
    pub schema: Arc<[u8]>,
    pub plan: Arc<[u8]>,
    pub geometry: LayoutGeometry,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TargetObservation<S, D> {
    Layout {
        target: TargetIdentity,
        projection: LayoutQuery,
        layout: Arc<SelectedLayout<S>>,
    },
    FloatRealization {
        target: TargetIdentity,
        realization: FloatRealization,
    },
    SelectedRealization {
        target: TargetIdentity,
        realization: SelectedRealization<S, D>,
    },
    Property {
        target: TargetIdentity,
        property: SelectedTargetProperty<S>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetSemanticsError {
    MissingTargetIdentity,
    MismatchedLayoutTarget,
    InvalidLayoutProjection,
    DuplicateLayoutSelection,
    DuplicateFloatSelection,
    InvalidRealizationSelection,
    DuplicateRealizationSelection,
    InvalidTargetProperty,
    DuplicateTargetProperty,
}

impl std::fmt::Display for TargetSemanticsError {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        let message = match self {
            Self::MissingTargetIdentity => "the target capsule has no exact selected identity",
            Self::MismatchedLayoutTarget => "the selected layout belongs to a different target",
            Self::InvalidLayoutProjection => "the selected layout projection has invalid geometry or identity",
            Self::DuplicateLayoutSelection => "the target capsule selects multiple layouts for one type",
            Self::DuplicateFloatSelection => "the target capsule selects multiple realizations for one floating-point operation",
            Self::InvalidRealizationSelection => "the selected realization has no checked contract or plan identity",
            Self::DuplicateRealizationSelection => "the target capsule selects multiple realizations for one requirement application",
            Self::InvalidTargetProperty => "the selected target property has an invalid subject or meaning",
            Self::DuplicateTargetProperty => "the target capsule selects multiple values for one property and subject",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for TargetSemanticsError {}

/// Compiler-owned inputs for observation and replay on one selected target. This API accepts selected
/// plans from a trusted compiler caller. The plan owner validates layout
/// coverage, representation and policy before projecting geometry here.
pub struct TargetSemantics<S, D> {
    identity: TargetIdentity,
    layouts: HashMap<S, Arc<SelectedLayout<S>>>,
    floats: HashMap<(FloatFormat, FloatOperation), FloatRealization>,
    realizations: Vec<SelectedRealization<S, D>>,
    properties: Vec<SelectedTargetProperty<S>>,
}

impl<S: LayoutSubject, D: Clone + Eq> TargetSemantics<S, D> {
    pub fn new(
        identity: TargetIdentity,
        layouts: impl IntoIterator<Item = SelectedLayout<S>>,
        float_realizations: impl IntoIterator<Item = FloatRealization>,
    ) -> Result<Self, TargetSemanticsError> {
        identity.validate()?;
        let mut selected_layouts = HashMap::new();
        for layout in layouts {
            if layout.target != identity {
                return Err(TargetSemanticsError::MismatchedLayoutTarget);
            }
            if layout.schema.is_empty()
                || layout.plan.is_empty()
                || !layout.subject.has_data_layout()
                || layout
                    .geometry
                    .alignment
                    .is_some_and(|alignment| !alignment.is_power_of_two())
            {
                return Err(TargetSemanticsError::InvalidLayoutProjection);
            }
            if selected_layouts
                .insert(layout.subject.clone(), Arc::new(layout))
                .is_some()
            {
                return Err(TargetSemanticsError::DuplicateLayoutSelection);
            }
        }
        let mut floats = HashMap::new();
        for realization in float_realizations {
            if floats
                .insert((realization.format, realization.operation), realization)
                .is_some()
            {
                return Err(TargetSemanticsError::DuplicateFloatSelection);
            }
        }
        Ok(Self {
            identity,
            layouts: selected_layouts,
            floats,
            realizations: Vec::new(),
            properties: Vec::new(),
        })
    }

    /// Retain selections supplied by the compiler's checked provider plan.
    /// Replaying a selection does not give a runtime slot permission to execute.
    pub fn with_selected_realizations(
        mut self,
        selections: impl IntoIterator<Item = SelectedRealization<S, D>>,
    ) -> Result<Self, TargetSemanticsError> {
        for selection in selections {
            if selection.checked_contract.is_empty() || selection.selected_plan.is_empty() {
                return Err(TargetSemanticsError::InvalidRealizationSelection);
            }
            if self
                .realizations
                .iter()
                .any(|previous| previous.requirement == selection.requirement && previous.arguments == selection.arguments)
            {
                return Err(TargetSemanticsError::DuplicateRealizationSelection);
            }
            self.realizations.push(selection);
        }
        Ok(self)
    }

    /// Retain exact observations supplied by the selected target model. The
    /// compiler owns the meaning and version of each projection.
    pub fn with_target_properties(
        mut self,
        properties: impl IntoIterator<Item = SelectedTargetProperty<S>>,
    ) -> Result<Self, TargetSemanticsError> {
        for property in properties {
            if !property.value.is_valid(&self) {
                return Err(TargetSemanticsError::InvalidTargetProperty);
            }
            if self
                .properties
                .iter()
                .any(|selected| selected.value.query() == property.value.query())
            {
                return Err(TargetSemanticsError::DuplicateTargetProperty);
            }
            self.properties.push(property);
        }
        Ok(self)
    }

    /// Observe an exact property of a selected target for a checker receipt.
    /// An unsettled subject has no observation.
    pub fn observe_target_property(
        &self,
        query: TargetPropertyQuery<'_, S>,
    ) -> Option<TargetObservation<S, D>> {
        let property = self.selected_property(query)?;
        Some(TargetObservation::Property {
            target: self.identity.clone(),
            property: property.clone(),
        })
    }

    fn selected_property(
        &self,
        query: TargetPropertyQuery<'_, S>,
    ) -> Option<&SelectedTargetProperty<S>> {
        self.properties
            .iter()
            .find(|property| property.value.query() == query)
    }

    pub fn identity(&self) -> &TargetIdentity {
        &self.identity
    }

    pub fn layout_for(
        &self,
        subject: &S,
    ) -> Option<&SelectedLayout<S>> {
        self.layouts.get(subject).map(Arc::as_ref)
    }

    pub fn float_realization(
        &self,
        format: FloatFormat,
        operation: FloatOperation,
    ) -> Option<&FloatRealization> {
        self.floats.get(&(format, operation))
    }

    /// Reconstruct a used projection under another selected capsule.
    pub fn matches_observation(
        &self,
        observation: &TargetObservation<S, D>,
    ) -> bool {
        match observation {
            TargetObservation::Layout { projection, layout, .. } => self
                .observe_layout(*projection, &layout.subject)
                .is_ok_and(|(_, reconstructed)| &reconstructed == observation),
            TargetObservation::FloatRealization { target, realization } => {
                &self.identity == target && self.float_realization(realization.format, realization.operation) == Some(realization)
            }
            TargetObservation::SelectedRealization { target, realization } => &self.identity == target && self.realizations.contains(realization),
            TargetObservation::Property { target, property } => &self.identity == target && self.selected_property(property.value.query()) == Some(property),
        }
    }

    pub fn observe_layout(
        &self,
        projection: LayoutQuery,
        subject: &S,
    ) -> Result<(u64, TargetObservation<S, D>), &'static str> {
        if !subject.has_data_layout() {
            return Err("the selected layout query needs a data type with runtime storage");
        }
        let layout = self
            .layouts
            .get(subject)
            .ok_or("the selected target has no validated layout for the type")?;
        let value = match projection {
            LayoutQuery::Size => match layout.geometry.extent {
                LayoutExtent::Fixed(bytes) => bytes,
                LayoutExtent::Dynamic => return Err("the selected layout has no fixed complete extent"),
            },
            LayoutQuery::Alignment => layout
                .geometry
                .alignment
                .ok_or("the selected layout has no known alignment")?,
        };
        Ok((
            value,
            TargetObservation::Layout {
                target: self.identity.clone(),
                projection,
                layout: layout.clone(),
            },
        ))
    }
}
