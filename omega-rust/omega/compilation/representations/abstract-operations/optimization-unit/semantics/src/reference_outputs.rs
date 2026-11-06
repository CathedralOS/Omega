//! A helper can return a new loan through storage supplied by its caller.
//! Keep the actual loan and its parent from every completed body path. Equal
//! pointer origins cannot say whether that loan was moved, copied or borrowed.
//!
//! These records are disposable results of current-body ownership replay.
//! Their private constructors do not admit a unit, choose a body, prove value
//! predicates or grant permission to a particular caller. Each call still binds
//! its own captured input occurrences and checks every output alternative.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralCaseId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

pub use crate::unit_validation::references::{
    LiveReference, ReferenceFormation, ReferenceIdentity, ReferenceInterpretation,
    ReferenceOccurrenceIdentity, ReferenceOrigin, ReferenceOriginIdentity, ReferenceParent,
};

/// One typed caller-storage slot. This is a storage coordinate, independently
/// of the identity of the permission currently installed there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceOutputSlot {
    index: usize,
    parameter: usize,
    path: Vec<StructuralPathSegment>,
    reference_type: StructuralTypeId,
    access: StructuralAccess,
}

impl ReferenceOutputSlot {
    pub fn parameter(&self) -> usize {
        self.parameter
    }

    pub fn path(&self) -> &[StructuralPathSegment] {
        &self.path
    }

    pub fn reference_type(&self) -> StructuralTypeId {
        self.reference_type
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
}

/// Keep exits separate: a loan from one path cannot justify a missing parent
/// or an extra exclusive output on another path.
#[derive(Clone, Debug)]
pub struct ReferenceOutputExit {
    block: BlockId,
    returned: Vec<LiveReference>,
    stored: Vec<LiveReference>,
    graph: Vec<ReferenceOutputNode>,
    cleared_slots: Option<Vec<usize>>,
}

impl ReferenceOutputExit {
    pub fn block(&self) -> BlockId {
        self.block
    }

    pub fn returned(&self) -> &[LiveReference] {
        &self.returned
    }

    pub fn stored(&self) -> &[LiveReference] {
        &self.stored
    }

    /// Parents precede their children. Each node is a permission occurrence,
    /// even when another node has exactly the same pointer origin.
    pub fn cleared_slots(&self) -> Option<&[usize]> {
        self.cleared_slots.as_deref()
    }

    pub fn graph(&self) -> &[ReferenceOutputNode] {
        &self.graph
    }
}

#[derive(Clone, Debug)]
pub enum ReferenceOutputResidence {
    Entry(usize),
    Parameter { slot: usize },
    Returned { path: Vec<StructuralPathSegment> },
}

#[derive(Clone, Debug)]
pub enum ReferenceOutputOrigin {
    Entry {
        entry: usize,
        suffix: Vec<terminal_psi::ReferenceOriginProjection>,
        extent: semantic_vocabulary::ReferenceOriginExtent,
    },
    Storage {
        parameter: usize,
        path: Vec<terminal_psi::ReferenceOriginProjection>,
        extent: semantic_vocabulary::ReferenceOriginExtent,
    },
}

#[derive(Clone, Debug)]
pub enum ReferenceOutputParent {
    Entry(usize),
    Node(usize),
    Root(Vec<ReferenceOutputOrigin>),
}

#[derive(Clone, Debug)]
pub enum ReferenceOutputSupply {
    Entry(usize),
    Constructed {
        origins: Vec<ReferenceOutputOrigin>,
        parent: ReferenceOutputParent,
    },
}

#[derive(Clone, Debug)]
pub enum ReferenceOutputInterpretation {
    Unplaced,
    Entry(usize),
    Source {
        parameter: usize,
        path: Vec<StructuralPathSegment>,
        access: StructuralAccess,
    },
    Captured(abstract_operations::AccessReferenceBacking),
}

#[derive(Clone, Debug)]
pub struct ReferenceOutputNode {
    residence: ReferenceOutputResidence,
    reference_type: StructuralTypeId,
    supply: ReferenceOutputSupply,
    interpretation: ReferenceOutputInterpretation,
}

impl ReferenceOutputNode {
    pub fn residence(&self) -> &ReferenceOutputResidence {
        &self.residence
    }
    pub fn reference_type(&self) -> StructuralTypeId {
        self.reference_type
    }
    pub fn interpretation(&self) -> &ReferenceOutputInterpretation {
        &self.interpretation
    }

    pub fn supply(&self) -> &ReferenceOutputSupply {
        &self.supply
    }
}

/// All normal exits of one prerequisite-checked immutable body view.
#[derive(Debug)]
pub struct ReferenceBodyOutputs {
    machine: MachineId,
    entry: Vec<LiveReference>,
    entry_parameters: Vec<usize>,
    entry_writable: Vec<bool>,
    slots: Vec<ReferenceOutputSlot>,
    exits: Vec<ReferenceOutputExit>,
    required_entries: Vec<usize>,
    compatibility_requirements:
        Vec<terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement>,
}

impl ReferenceBodyOutputs {
    pub fn machine(&self) -> MachineId {
        self.machine
    }

    pub fn entry(&self) -> &[LiveReference] {
        &self.entry
    }

    pub fn entry_parameter(&self, entry: usize) -> Option<usize> {
        self.entry_parameters.get(entry).copied()
    }

    pub fn entry_is_writable(&self, entry: usize) -> bool {
        self.entry_writable.get(entry).copied().unwrap_or(false)
    }

    pub fn slots(&self) -> &[ReferenceOutputSlot] {
        &self.slots
    }

    pub fn exits(&self) -> &[ReferenceOutputExit] {
        &self.exits
    }

    pub fn required_entries(&self) -> &[usize] {
        &self.required_entries
    }

    pub fn compatibility_requirements(
        &self,
    ) -> &[terminal_semantics::descriptor_results::DescriptorCompatibilityRequirement] {
        &self.compatibility_requirements
    }
}

/// A catalog belongs to one immutable operation view. In particular, a native
/// view with admitted selected bodies is not interchangeable with the original
/// portable view just because both contain the same machine identifiers.
#[derive(Clone, Debug)]
pub struct ReferenceOutputCatalog {
    rows: Vec<(MachineId, ReferenceOutputState)>,
    normal_completion: Vec<bool>,
}

#[derive(Clone, Debug)]
enum ReferenceOutputState {
    Pending,
    Active,
    Complete(Arc<ReferenceBodyOutputs>),
    Conditional(Arc<ReferenceBodyOutputs>),
    Unavailable(String),
}

/// Successful ordinary replay is independent of whether its optional stored
/// graph can be represented. Conditional completion cannot donate interpretation.
pub(crate) struct ReferenceBodyCompletion {
    pub(crate) outputs: Result<ReferenceBodyOutputs, String>,
    pub(crate) unconditional: bool,
}

pub(crate) type ReferenceOutputPreparation = Rc<RefCell<ReferenceOutputCatalog>>;

mod ancestry;
mod catalog;
mod completion;
mod graph;

pub use ancestry::ReferenceAncestryIndex;
pub use catalog::reconstruct_reference_output_catalog;
pub(crate) use completion::ReferenceOutputBuilder;
use graph::{external_root, is_entry_storage, output_graph};
