use crate::{DeclarationId, DeclarationKind, Site, TargetIdentity};
use num_bigint::BigInt;
/// A closed identity in a target observation. Layout subjects need not occur
/// among the types of the executable values, such as in size_of::<u64>()
/// when u64 is used only to compute the length of an array of bytes.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FoldedSubject {
    Bool,
    Integer { signed: bool, width: u64 },
    UnboundedInteger { signed: bool },
    Address,
    Float32,
    Float64,
    Nominal(DeclarationId),
    Applied(DeclarationId, Vec<Self>),
    Tuple(Vec<Self>),
    Array(Box<Self>, u64),
    Slice(Box<Self>),
    Reference(FoldedReferenceAccess, Box<Self>),
    Qualified(Box<Self>, Vec<(DeclarationId, Vec<Self>)>),
    StaticInteger(Box<Self>, BigInt),
    StaticBoolean(Box<Self>, bool),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FoldedReferenceAccess {
    Shared,
    Mutable,
    WriteOnly,
}

impl omega_target_contract::LayoutSubject for FoldedSubject {
    fn has_data_layout(&self) -> bool {
        match self {
            Self::Nominal(id) | Self::Applied(id, _) => id.kind == DeclarationKind::Data,
            Self::Qualified(base, _) => base.has_data_layout(),
            Self::UnboundedInteger { .. } | Self::StaticInteger(..) | Self::StaticBoolean(..) => false,
            _ => true,
        }
    }
}
pub type ReceivingTarget = omega_target_contract::TargetSemantics<FoldedSubject, DeclarationId>;
pub type FoldedLayout = omega_target_contract::SelectedLayout<FoldedSubject>;
pub type FoldedRealization = omega_target_contract::SelectedRealization<FoldedSubject, DeclarationId>;
pub type FoldedProperty = omega_target_contract::TargetProperty<FoldedSubject>;
pub type FoldedTargetObservation = omega_target_contract::TargetObservation<FoldedSubject, DeclarationId>;
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FoldedValueOrigin {
    Constant(DeclarationId),
    StaticIndex {
        carrier: FoldedSubject,
        owner_arguments: Option<Vec<FoldedSubject>>,
    },
}

/// A folded declaration or type argument and the inputs it used.
#[derive(Debug, Eq, PartialEq)]
pub struct FoldedTargetInputs {
    pub origin: FoldedValueOrigin,
    pub site: Site,
    pub constants: Vec<DeclarationId>,
    pub observations: Vec<usize>,
}

/// Target requirements that must be replayed before a receiving target can
/// reuse a folded package.
#[derive(Debug, Eq, PartialEq)]
pub struct FoldedTargetClosure {
    target: TargetIdentity,
    observations: Vec<FoldedTargetObservation>,
    inputs: Vec<FoldedTargetInputs>,
}

impl FoldedTargetClosure {
    pub fn target(&self) -> &TargetIdentity {
        &self.target
    }

    pub fn observations(&self) -> &[FoldedTargetObservation] {
        &self.observations
    }

    pub fn inputs(&self) -> &[FoldedTargetInputs] {
        &self.inputs
    }

    /// Returns the first premise the receiving target cannot reconstruct.
    pub fn replay(
        &self,
        target: &ReceivingTarget,
    ) -> Result<(), &FoldedTargetObservation> {
        match self
            .observations
            .iter()
            .find(|used| target.identity() != &self.target || !target.matches_observation(used))
        {
            Some(changed) => Err(changed),
            None => Ok(()),
        }
    }

    pub fn new(
        target: TargetIdentity,
        observations: Vec<FoldedTargetObservation>,
        inputs: Vec<FoldedTargetInputs>,
    ) -> Self {
        Self { target, observations, inputs }
    }
}
