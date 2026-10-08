//! Bounded, non-recursive conventional-sum reachability over typed data.
//!
//! Const-materialization and backend layout both need the same judgment —
//! does this record reach a conventional sum through declared fields — and
//! both walk the same non-recursive DFS with identical bounds. The engine
//! lives here once; each caller supplies the domain label it stamps into
//! diagnostics (`ConstMaterializable nested-record` vs `nested-record sum`)
//! and re-stamps [`SumTraversalFault`] into its own error carrier.

use crate::TypedTrees;
use crate::data::{DataDefinition, DataMember, DataShapeKind};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

/// One bounded-traversal failure, already worded with the caller's domain
/// label so it can be re-stamped into any diagnostic carrier verbatim.
pub struct SumTraversalFault(pub String);

/// Whether a record's declared members hold a direct conventional sum, a
/// fixed array containing one, or a deeper record reaching one.
#[derive(Default)]
pub struct RecordSumProfile {
    pub direct: bool,
    pub array: bool,
    pub deeper: bool,
}

/// Classify `data`'s relevant fields against `reachability`'s bounded walk.
pub fn record_sum_profile(
    program: &TypedTrees,
    data: &DataDefinition,
    reachability: &mut SumReachability<'_>,
) -> Result<RecordSumProfile, SumTraversalFault> {
    let mut profile = RecordSumProfile::default();
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.relevance.is_erased() {
            continue;
        }
        match program
            .type_reference_table
            .type_reference(field.type_reference)
        {
            TypeReferenceNode::Named { .. } => {
                let Some(named) =
                    exact_named_data(program, reachability.field_label, field.type_reference)?
                else {
                    continue;
                };
                match DataDefinition::shape_kind_from_members(program.data_members(named)) {
                    // A mixed shape is one case-bearing interior: as a
                    // record's direct member it profiles like a direct sum.
                    DataShapeKind::Enum | DataShapeKind::Mixed => profile.direct = true,
                    DataShapeKind::Record => {
                        if reachability.type_contains_sum(field.type_reference)? {
                            profile.deeper = true;
                        }
                    }
                    DataShapeKind::Empty => {}
                }
            }
            TypeReferenceNode::FixedArray { .. }
                if reachability.type_contains_sum(field.type_reference)? =>
            {
                profile.array = true;
            }
            _ => {}
        }
    }
    Ok(profile)
}

#[derive(Clone, Copy)]
enum ReachabilityState {
    Visiting,
    Done(bool),
}

struct ReachabilityFrame<'a> {
    data: &'a DataDefinition,
    next_member: usize,
    found: bool,
}

/// The shared bounded walk: cycle detection through visited data identities,
/// fixed-array-depth and total-edge caps, and member-shape classification.
/// `label` stamps `"{label} path …"`/`"{label} traversal …"` diagnostics and
/// `field_label` stamps the nominal-identity lookups inside the walk.
pub struct SumReachability<'a> {
    program: &'a TypedTrees,
    label: &'static str,
    field_label: &'static str,
    states: std::collections::HashMap<(u32, u32), ReachabilityState>,
    traversed_edges: usize,
}

impl<'a> SumReachability<'a> {
    const MAX_RECORDS: usize = 4096;
    pub const MAX_EDGES: usize = 16384;

    pub fn new(program: &'a TypedTrees, label: &'static str, field_label: &'static str) -> Self {
        Self {
            program,
            label,
            field_label,
            states: std::collections::HashMap::new(),
            traversed_edges: 0,
        }
    }

    /// Whether `type_reference` — through bounded fixed-array nesting —
    /// denotes a conventional sum or a record that reaches one.
    pub fn type_contains_sum(
        &mut self,
        mut type_reference: TypeReferenceHandle,
    ) -> Result<bool, SumTraversalFault> {
        let mut array_depth = 0usize;
        while let TypeReferenceNode::FixedArray { element_type, .. } = self
            .program
            .type_reference_table
            .type_reference(type_reference)
        {
            array_depth += 1;
            if array_depth > 64 {
                return Err(self.fault("path exceeds bounded fixed-array depth"));
            }
            type_reference = *element_type;
        }
        let Some(data) = exact_named_data(self.program, self.field_label, type_reference)? else {
            return Ok(false);
        };
        match DataDefinition::shape_kind_from_members(self.program.data_members(data)) {
            DataShapeKind::Enum | DataShapeKind::Mixed => Ok(true),
            DataShapeKind::Empty => Ok(false),
            DataShapeKind::Record => self.record_contains_sum(data),
        }
    }

    fn record_contains_sum(&mut self, root: &'a DataDefinition) -> Result<bool, SumTraversalFault> {
        let root_identity = symbol_identity(self.label, root.symbol)?;
        if let Some(state) = self.states.get(&root_identity) {
            return match state {
                ReachabilityState::Done(found) => Ok(*found),
                ReachabilityState::Visiting => Err(SumTraversalFault(format!(
                    "{} path is recursive through `{}`",
                    self.label, root.name
                ))),
            };
        }
        self.insert_state(root_identity, ReachabilityState::Visiting)?;
        let mut stack = Vec::new();
        stack
            .try_reserve(1)
            .map_err(|_| self.fault("traversal stack exceeds compiler resources"))?;
        stack.push(ReachabilityFrame {
            data: root,
            next_member: 0,
            found: false,
        });

        loop {
            let Some(frame) = stack.last_mut() else {
                unreachable!("root reachability frame returns when completed")
            };
            let members = self.program.data_members(frame.data);
            // `found` is the eventual answer, not permission to skip later
            // fields: a later branch can still expose a cycle, malformed
            // nominal identity, or resource-bound failure.
            if frame.next_member == members.len() {
                let completed = stack.pop().expect("active reachability frame");
                let identity = symbol_identity(self.label, completed.data.symbol)?;
                self.states
                    .insert(identity, ReachabilityState::Done(completed.found));
                if let Some(parent) = stack.last_mut() {
                    parent.found |= completed.found;
                    continue;
                }
                return Ok(completed.found);
            }
            let member = &members[frame.next_member];
            frame.next_member += 1;
            let DataMember::Field(field) = member else {
                frame.found = true;
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            self.traversed_edges = self
                .traversed_edges
                .checked_add(1)
                .ok_or_else(|| self.fault("traversal edge count overflows"))?;
            if self.traversed_edges > Self::MAX_EDGES {
                return Err(self.fault("path exceeds bounded schema traversal edges"));
            }
            let mut child_type = field.type_reference;
            let mut array_depth = 0usize;
            while let TypeReferenceNode::FixedArray { element_type, .. } =
                self.program.type_reference_table.type_reference(child_type)
            {
                array_depth += 1;
                if array_depth > 64 {
                    return Err(self.fault("path exceeds bounded fixed-array depth"));
                }
                child_type = *element_type;
            }
            let Some(child) = exact_named_data(self.program, self.field_label, child_type)? else {
                continue;
            };
            match DataDefinition::shape_kind_from_members(self.program.data_members(child)) {
                DataShapeKind::Enum | DataShapeKind::Mixed => frame.found = true,
                DataShapeKind::Empty => {}
                DataShapeKind::Record => {
                    let identity = symbol_identity(self.label, child.symbol)?;
                    match self.states.get(&identity).copied() {
                        Some(ReachabilityState::Done(found)) => frame.found |= found,
                        Some(ReachabilityState::Visiting) => {
                            return Err(SumTraversalFault(format!(
                                "{} path is recursive through `{}`",
                                self.label, child.name
                            )));
                        }
                        None => {
                            self.insert_state(identity, ReachabilityState::Visiting)?;
                            stack.try_reserve(1).map_err(|_| {
                                self.fault("traversal stack exceeds compiler resources")
                            })?;
                            stack.push(ReachabilityFrame {
                                data: child,
                                next_member: 0,
                                found: false,
                            });
                        }
                    }
                }
            }
        }
    }

    fn insert_state(
        &mut self,
        identity: (u32, u32),
        state: ReachabilityState,
    ) -> Result<(), SumTraversalFault> {
        if self.states.len() >= Self::MAX_RECORDS {
            return Err(self.fault("path exceeds bounded schema traversal records"));
        }
        self.states
            .try_reserve(1)
            .map_err(|_| self.fault("visited map exceeds compiler resources"))?;
        self.states.insert(identity, state);
        Ok(())
    }

    fn fault(&self, suffix: &str) -> SumTraversalFault {
        SumTraversalFault(format!("{} {suffix}", self.label))
    }
}

fn symbol_identity(
    label: &'static str,
    symbol: symbols::SymbolHandle,
) -> Result<(u32, u32), SumTraversalFault> {
    if !symbol.is_valid() {
        return Err(SumTraversalFault(format!(
            "{label} path encountered an invalid nominal identity"
        )));
    }
    Ok((symbol.arena_index(), symbol.generation()))
}

/// The `Named` type's unique data definition, rejected when the nominal
/// identity is missing, ambiguous, or mismatched with the authored name.
fn exact_named_data<'a>(
    program: &'a TypedTrees,
    field_label: &'static str,
    type_reference: TypeReferenceHandle,
) -> Result<Option<&'a DataDefinition>, SumTraversalFault> {
    program
        .exact_named_data_definition(field_label, type_reference)
        .map_err(SumTraversalFault)
}
