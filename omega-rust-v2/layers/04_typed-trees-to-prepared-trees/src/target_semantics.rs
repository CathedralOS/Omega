//! Preparation consumes properties of targets and plans already selected and validated by the compiler.
//! All observables come from a selected target, including when its geometry matches the host.

use std::collections::HashMap;
use std::sync::Arc;

use omega_parser::NodeKind;
use omega_typer::{LayoutQuery, ValueType};

use crate::floating_point::{FloatFormat, FloatOperation, FloatRealization};
use crate::invocation::admission::SelectedRealization;

mod properties;
pub use properties::{
    AddressSpaceIdentity, Endianness, EntryProfileIdentity, EntryStackGuarantee, SelectedTargetProperty, TargetProperty, TargetPropertyQuery,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetIdentity {
    pub profile: String,
    pub semantics_version: u32,
    pub realization: Arc<[u8]>,
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
pub struct SelectedLayout {
    pub target: TargetIdentity,
    pub subject: ValueType,
    pub schema: Arc<[u8]>,
    pub plan: Arc<[u8]>,
    pub geometry: LayoutGeometry,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TargetObservation {
    Layout {
        target: TargetIdentity,
        projection: LayoutQuery,
        layout: Arc<SelectedLayout>,
    },
    FloatRealization {
        target: TargetIdentity,
        realization: FloatRealization,
    },
    SelectedRealization {
        target: TargetIdentity,
        realization: SelectedRealization,
    },
    Property {
        target: TargetIdentity,
        property: SelectedTargetProperty,
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

/// Compiler-owned target inputs for one preparation. This API accepts selected
/// plans from a trusted compiler caller. The plan owner validates layout
/// coverage, representation and policy before projecting geometry here.
pub struct TargetSemantics {
    identity: TargetIdentity,
    layouts: HashMap<ValueType, Arc<SelectedLayout>>,
    floats: HashMap<(FloatFormat, FloatOperation), FloatRealization>,
    realizations: Vec<SelectedRealization>,
    properties: Vec<SelectedTargetProperty>,
}

impl TargetSemantics {
    pub fn new(
        identity: TargetIdentity,
        layouts: impl IntoIterator<Item = SelectedLayout>,
        float_realizations: impl IntoIterator<Item = FloatRealization>,
    ) -> Result<Self, TargetSemanticsError> {
        if identity.profile.is_empty() || identity.realization.is_empty() {
            return Err(TargetSemanticsError::MissingTargetIdentity);
        }
        let mut selected_layouts = HashMap::new();
        for layout in layouts {
            if layout.target != identity {
                return Err(TargetSemanticsError::MismatchedLayoutTarget);
            }
            if layout.schema.is_empty()
                || layout.plan.is_empty()
                || !is_data_layout_subject(&layout.subject)
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
        selections: impl IntoIterator<Item = SelectedRealization>,
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
        properties: impl IntoIterator<Item = SelectedTargetProperty>,
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
        query: TargetPropertyQuery<'_>,
    ) -> Option<TargetObservation> {
        let property = self.selected_property(query)?;
        Some(TargetObservation::Property {
            target: self.identity.clone(),
            property: property.clone(),
        })
    }

    fn selected_property(
        &self,
        query: TargetPropertyQuery<'_>,
    ) -> Option<&SelectedTargetProperty> {
        self.properties
            .iter()
            .find(|property| property.value.query() == query)
    }

    pub fn identity(&self) -> &TargetIdentity {
        &self.identity
    }

    pub fn layout_for(
        &self,
        subject: &ValueType,
    ) -> Option<&SelectedLayout> {
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
        observation: &TargetObservation,
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

    pub(super) fn observe_layout(
        &self,
        projection: LayoutQuery,
        subject: &ValueType,
    ) -> Result<(u64, TargetObservation), &'static str> {
        if !is_data_layout_subject(subject) {
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

fn is_data_layout_subject(subject: &ValueType) -> bool {
    match subject {
        ValueType::Nominal(id) | ValueType::Applied(id, _) => id.kind == NodeKind::Data,
        ValueType::Qualified(ty, _) | ValueType::AtLifetime(ty, _) => is_data_layout_subject(ty),
        ValueType::Bool
        | ValueType::Address
        | ValueType::Integer { width: Some(_), .. }
        | ValueType::Float32
        | ValueType::Float64
        | ValueType::Tuple(_)
        | ValueType::Reference(..)
        | ValueType::Array(..)
        | ValueType::Slice(_) => true,
        _ => false,
    }
}
