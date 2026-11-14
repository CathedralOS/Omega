//! Follow the code value that was evaluated, keeping its original storage cut.
//!
//! Construction retains code. Only invocation creates a call edge. This query
//! describes possible constructed values and formal inputs; it never searches
//! for functions with an eligible signature or calling plan.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, CallableContractId, EdgeId, InvocationSite, MachineDescriptorId,
    MachineId, OperationId, PlaceId, ScalarTerm, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{StructuralTypeDeclaration, TerminalCallableCatalog};

use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPathSegment, DescriptorPosition};
use crate::descriptor_results::{DescriptorAnalysis, DescriptorDefinition, DescriptorMachine};

mod reachable;
mod terminal;
pub use reachable::{
    CallableInvocationRoots, ReachableCallableInvocation, ReachableCallableInvocations,
    ReachedCallableTarget, reachable_callable_invocations,
    reachable_callable_invocations_with_roots,
};
pub use terminal::{
    project_callable_invocation, project_nominal_cleanup_invocation, stored_callable_definition,
    terminal_callable_values,
};

pub type CallableValueResult<T> = Result<T, &'static str>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallableValueQuery {
    Scalar {
        machine: MachineId,
        value: ValueId,
        contract: CallableContractId,
    },
    Storage {
        machine: MachineId,
        position: DescriptorPosition,
        location: DescriptorLocation,
        contract: CallableContractId,
    },
}

impl CallableValueQuery {
    pub fn contract(&self) -> CallableContractId {
        match self {
            Self::Scalar { contract, .. } | Self::Storage { contract, .. } => *contract,
        }
    }

    pub fn machine(&self) -> MachineId {
        match self {
            Self::Scalar { machine, .. } | Self::Storage { machine, .. } => *machine,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CallableParameter {
    Scalar(usize),
    Structural {
        position: usize,
        path: Vec<DescriptorPathSegment>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallableFormalInput {
    pub machine: MachineId,
    pub parameter: CallableParameter,
    pub contract: CallableContractId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallableValuePossibilities {
    pub descriptors: BTreeSet<MachineDescriptorId>,
    pub inputs: BTreeSet<CallableFormalInput>,
    pub unresolved_ceilings: BTreeSet<CallableContractId>,
}

impl CallableValuePossibilities {
    pub fn is_resolved(&self) -> bool {
        !self.descriptors.is_empty()
            && self.inputs.is_empty()
            && self.unresolved_ceilings.is_empty()
    }

    fn is_bottom(&self) -> bool {
        self.descriptors.is_empty() && self.inputs.is_empty() && self.unresolved_ceilings.is_empty()
    }

    fn join(&mut self, other: Self) -> bool {
        let before = (
            self.descriptors.len(),
            self.inputs.len(),
            self.unresolved_ceilings.len(),
        );
        self.descriptors.extend(other.descriptors);
        self.inputs.extend(other.inputs);
        self.unresolved_ceilings.extend(other.unresolved_ceilings);
        before
            != (
                self.descriptors.len(),
                self.inputs.len(),
                self.unresolved_ceilings.len(),
            )
    }

    fn ceiling(contract: CallableContractId) -> Self {
        Self {
            unresolved_ceilings: BTreeSet::from([contract]),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
pub enum CallableDefinition {
    Constructed(MachineDescriptorId),
    Formal(usize),
    Incoming(BlockId),
    Stored {
        position: DescriptorPosition,
        location: DescriptorLocation,
    },
    Invocation(DescriptorPosition),
    Opaque,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallableInvocationTarget {
    Machine(MachineId),
    Value(ValueId, CallableContractId),
    Opaque,
}

pub use semantic_vocabulary::CallableInvocationSite;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallableInvocation {
    pub site: CallableInvocationSite,
    pub operation: Option<OperationId>,
    pub boundary: Option<BoundaryMachineId>,
    pub position: DescriptorPosition,
    pub successor: Option<usize>,
    pub target: CallableInvocationTarget,
    pub scalar_arguments: Vec<ValueId>,
    pub structural_arguments: Vec<DescriptorLocation>,
    pub structural_actual_types: Vec<(StructuralTypeId, terminal_psi::StructuralAccess)>,
}

#[derive(Default)]
pub struct CallableValueBody {
    pub attachment: Option<StructuralTypeId>,
    pub nominal_cleanup_subject: Option<StructuralTypeId>,
    pub unit_result: bool,
    pub structural_formals: Vec<terminal_psi::StructuralParameterDeclaration>,
    pub scalar_parameters: Vec<(ValueId, ScalarType)>,
    pub structural_parameters: Vec<(StructuralTypeId, terminal_psi::StructuralAccess)>,
    pub scalar_result: Option<ScalarType>,
    pub structural_result: Option<StructuralTypeId>,
    pub scalar_types: BTreeMap<ValueId, ScalarType>,
    pub definitions: BTreeMap<ValueId, CallableDefinition>,
    pub invocations: BTreeMap<CallableInvocationSite, CallableInvocation>,
    pub invalid_invocations: BTreeMap<CallableInvocationSite, (DescriptorPosition, &'static str)>,
    pub cleanup_actions: BTreeMap<EdgeId, Vec<terminal_psi::TerminalAffineCleanupAction>>,
    pub structural_results: BTreeMap<DescriptorPosition, PlaceId>,
    pub scalar_returns: Vec<ValueId>,
    pub structural_returns: Vec<(DescriptorPosition, PlaceId)>,
}

impl CallableValueBody {
    pub fn retain_invocation(
        &mut self,
        site: CallableInvocationSite,
        position: DescriptorPosition,
        invocation: CallableValueResult<CallableInvocation>,
    ) -> CallableValueResult<()> {
        if self.invocations.contains_key(&site) || self.invalid_invocations.contains_key(&site) {
            return Err("callable invocation has an ambiguous source occurrence identity");
        }
        match invocation {
            Ok(invocation) => {
                if invocation.site != site || invocation.position != position {
                    return Err("callable invocation changed its source occurrence identity");
                }
                self.invocations.insert(site, invocation);
            }
            Err(reason) => {
                self.invalid_invocations.insert(site, (position, reason));
            }
        }
        Ok(())
    }

    fn invocation(&self, site: CallableInvocationSite) -> CallableValueResult<&CallableInvocation> {
        if let Some((_, reason)) = self.invalid_invocations.get(&site) {
            return Err(reason);
        }
        self.invocations
            .get(&site)
            .ok_or("callable result lost its exact invocation")
    }

    pub fn has_primary_invocation(&self, position: DescriptorPosition) -> bool {
        self.invocations.values().any(|call| {
            call.position == position && matches!(call.site, CallableInvocationSite::Invocation(_))
        }) || self.invalid_invocations.iter().any(|(site, (cut, _))| {
            *cut == position && matches!(site, CallableInvocationSite::Invocation(_))
        })
    }

    fn primary_invocation(
        &self,
        position: DescriptorPosition,
    ) -> CallableValueResult<&CallableInvocation> {
        if let Some((_, (_, reason))) = self.invalid_invocations.iter().find(|(site, (cut, _))| {
            *cut == position && matches!(site, CallableInvocationSite::Invocation(_))
        }) {
            return Err(reason);
        }
        let mut rows = self.invocations.values().filter(|call| {
            call.position == position && matches!(call.site, CallableInvocationSite::Invocation(_))
        });
        let row = rows
            .next()
            .ok_or("callable result lost its exact invocation")?;
        if rows.next().is_some() {
            return Err("callable result has ambiguous invocation occurrences");
        }
        Ok(row)
    }

    /// Entry block parameters may name the same formal as the machine entry.
    pub fn insert_value(
        &mut self,
        value: ValueId,
        scalar_type: ScalarType,
        definition: CallableDefinition,
        entry_parameter: bool,
    ) -> CallableValueResult<()> {
        if let Some(previous) = self.scalar_types.get(&value) {
            if entry_parameter
                && *previous == scalar_type
                && matches!(
                    self.definitions.get(&value),
                    Some(CallableDefinition::Formal(_))
                )
            {
                return Ok(());
            }
            return Err("callable provenance has a missing or ambiguous SSA definition");
        }
        self.scalar_types.insert(value, scalar_type);
        self.definitions.insert(value, definition);
        Ok(())
    }
}

/// Immutable source projections; every resolution has its own temporary solver.
pub struct CallableValueProgram<'a> {
    pub catalog: &'a TerminalCallableCatalog,
    pub structural_types: &'a [StructuralTypeDeclaration],
    pub graph: Vec<DescriptorMachine>,
    pub bodies: BTreeMap<MachineId, CallableValueBody>,
}

#[derive(Clone)]
enum Source {
    Value(CallableValueQuery),
    Constructed(MachineDescriptorId),
    Input(CallableFormalInput),
    Invocation(CallableInvocation, ResultProjection),
    Ceiling,
}

#[derive(Clone)]
enum ResultProjection {
    Scalar,
    Structural(Vec<DescriptorPathSegment>),
}

enum CallableActualValue {
    Value(CallableValueQuery),
    Ceiling(CallableContractId),
}

impl CallableValueProgram<'_> {
    pub(crate) fn validate_occurrence(
        &self,
        caller: MachineId,
        invocation: &CallableInvocation,
    ) -> CallableValueResult<()> {
        let body = self.body(caller)?;
        if body.invocation(invocation.site)? != invocation
            || invocation.operation != invocation.site.operation()
            || invocation.structural_actual_types.len() != invocation.structural_arguments.len()
            || invocation
                .scalar_arguments
                .iter()
                .any(|actual| !body.scalar_types.contains_key(actual))
        {
            return Err("callable invocation no longer matches its exact source occurrence");
        }
        if let CallableInvocationTarget::Value(value, contract) = invocation.target {
            self.scalar_query(caller, value, contract)?;
            let signature = self
                .catalog
                .contract(contract)
                .ok_or("callable invocation has no exact authored contract")?;
            if signature.scalar_parameters.len() != invocation.scalar_arguments.len()
                || signature.structural_parameters.len() != invocation.structural_arguments.len()
                || !signature
                    .scalar_parameters
                    .iter()
                    .zip(&invocation.scalar_arguments)
                    .all(|(formal, actual)| {
                        body.scalar_types.get(actual) == Some(&formal.scalar_type)
                    })
                || !signature
                    .structural_parameters
                    .iter()
                    .zip(&invocation.structural_actual_types)
                    .all(|(formal, actual)| (formal.structural_type, formal.access) == *actual)
            {
                return Err("callable invocation actuals disagree with its authored contract");
            }
        }
        Ok(())
    }

    fn input_actual(
        &self,
        input: &CallableFormalInput,
        caller: MachineId,
        invocation: &CallableInvocation,
    ) -> CallableValueResult<CallableActualValue> {
        let query = match &input.parameter {
            CallableParameter::Scalar(position) => self.scalar_query(
                caller,
                *invocation
                    .scalar_arguments
                    .get(*position)
                    .ok_or("callable substitution lost its scalar actual")?,
                input.contract,
            )?,
            CallableParameter::Structural { position, path } => {
                let mut location = invocation
                    .structural_arguments
                    .get(*position)
                    .ok_or("callable substitution lost its structural actual")?
                    .clone();
                location.path.extend_from_slice(path);
                CallableValueQuery::Storage {
                    machine: caller,
                    position: invocation.position,
                    location,
                    contract: input.contract,
                }
            }
        };
        self.validate_query(&query)?;
        if let (
            CallableInvocationSite::CleanupAction {
                edge,
                action_ordinal,
            },
            CallableValueQuery::Storage { location, .. },
        ) = (invocation.site, &query)
        {
            let actions = self
                .body(caller)?
                .cleanup_actions
                .get(&edge)
                .ok_or("callable cleanup input lost its exact action roster")?;
            let ordinal = usize::try_from(action_ordinal)
                .map_err(|_| "callable cleanup action ordinal cannot be indexed")?;
            if ordinal >= actions.len() {
                return Err("callable cleanup action ordinal is stale");
            }
            let flow = self.flow(caller)?;
            for action in &actions[..ordinal] {
                let prior = crate::descriptor_lifetimes::descriptor_location(
                    self.structural_types,
                    action.place(),
                    *flow
                        .place_types
                        .get(&action.place())
                        .ok_or("earlier cleanup has no exact storage declaration")?,
                    action.path(),
                    None,
                )
                .ok_or("earlier cleanup has no exact storage projection")?;
                if self.cleanup_locations_may_overlap(
                    caller,
                    invocation.position,
                    location,
                    &prior,
                )? {
                    if matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    ) {
                        return Ok(CallableActualValue::Ceiling(input.contract));
                    }
                    return Err("callable cleanup input was retired by an earlier action");
                }
            }
        }
        Ok(CallableActualValue::Value(query))
    }

    fn cleanup_locations_may_overlap(
        &self,
        machine: MachineId,
        position: DescriptorPosition,
        left: &DescriptorLocation,
        right: &DescriptorLocation,
    ) -> CallableValueResult<bool> {
        let flow = self.flow(machine)?;
        let direct = |location: &DescriptorLocation| {
            !location.path.contains(&DescriptorPathSegment::Referent)
                && flow.places.iter().any(|place| {
                    place.place == location.root
                        && matches!(
                            place.definition,
                            DescriptorDefinition::Local
                                | DescriptorDefinition::Literal
                                | DescriptorDefinition::Parameter {
                                    access: terminal_psi::StructuralAccess::Owned,
                                    ..
                                }
                        )
                })
        };
        let mut analysis =
            DescriptorAnalysis::with_reference_slots(self.structural_types, &self.graph);
        let left = analysis
            .location(machine, position, left)
            .or_else(|| direct(left).then(|| left.clone()));
        let right = analysis
            .location(machine, position, right)
            .or_else(|| direct(right).then(|| right.clone()));
        Ok(match (left, right) {
            (Some(left), Some(right)) => left.contains(&right) || right.contains(&left),
            _ => true,
        })
    }

    pub(crate) fn validate_actuals(
        &self,
        caller: MachineId,
        callee: MachineId,
        invocation: &CallableInvocation,
    ) -> CallableValueResult<()> {
        let body = self.body(callee)?;
        if invocation.scalar_arguments.len() != body.scalar_parameters.len()
            || invocation.structural_arguments.len() != body.structural_parameters.len()
            || invocation.structural_actual_types != body.structural_parameters
        {
            return Err("callable invocation actuals do not match the selected formal telescope");
        }
        for ((_, expected), actual) in body
            .scalar_parameters
            .iter()
            .zip(&invocation.scalar_arguments)
        {
            if self.body(caller)?.scalar_types.get(actual) != Some(expected) {
                return Err("callable invocation scalar actual has the wrong selected formal type");
            }
        }
        Ok(())
    }

    fn validate_query(&self, query: &CallableValueQuery) -> CallableValueResult<()> {
        self.catalog
            .contract(query.contract())
            .ok_or("callable provenance contract is absent or stale")?;
        match query {
            CallableValueQuery::Scalar {
                machine,
                value,
                contract,
            } => {
                self.scalar_query(*machine, *value, *contract)?;
            }
            CallableValueQuery::Storage {
                machine,
                position,
                location,
                contract,
            } => {
                let flow = self.flow(*machine)?;
                if !flow.flow.contains_key(position) {
                    return Err("callable storage query has a stale observation cut");
                }
                let root = *flow
                    .place_types
                    .get(&location.root)
                    .ok_or("callable storage query has no exact declared root type")?;
                let path = crate::descriptor_lifetimes::descriptor_path(
                    self.structural_types,
                    root,
                    &location.path,
                )
                .ok_or("callable storage query has an invalid projection")?;
                let scalar =
                    crate::runtime_structural_field(self.structural_types.iter(), root, &path)
                        .and_then(|field| match field.field_type {
                            terminal_psi::StructuralFieldType::Scalar(scalar) => Some(scalar),
                            _ => None,
                        })
                        .or_else(|| {
                            let declaration = crate::runtime_structural_path_tip(
                                self.structural_types.iter(),
                                root,
                                &path,
                            )?;
                            match declaration.shape {
                                terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar) => {
                                    Some(scalar)
                                }
                                _ => None,
                            }
                        });
                if scalar != Some(ScalarType::Callable(*contract)) {
                    return Err("callable storage projection disagrees with its exact contract");
                }
            }
        }
        Ok(())
    }

    fn body(&self, machine: MachineId) -> CallableValueResult<&CallableValueBody> {
        self.bodies
            .get(&machine)
            .ok_or("callable provenance has no exact machine body")
    }

    fn flow(&self, machine: MachineId) -> CallableValueResult<&DescriptorMachine> {
        let mut rows = self.graph.iter().filter(|row| row.id == machine);
        let row = rows.next().ok_or("callable provenance flow is absent")?;
        if rows.next().is_some() {
            return Err("callable provenance flow is ambiguous");
        }
        Ok(row)
    }

    fn scalar_query(
        &self,
        machine: MachineId,
        value: ValueId,
        contract: CallableContractId,
    ) -> CallableValueResult<CallableValueQuery> {
        if self.body(machine)?.scalar_types.get(&value) != Some(&ScalarType::Callable(contract)) {
            return Err("callable provenance SSA type disagrees with its exact contract");
        }
        Ok(CallableValueQuery::Scalar {
            machine,
            value,
            contract,
        })
    }

    fn sources(&self, query: &CallableValueQuery) -> CallableValueResult<Vec<Source>> {
        self.catalog
            .contract(query.contract())
            .ok_or("callable provenance contract is absent or stale")?;
        match query {
            CallableValueQuery::Scalar {
                machine,
                value,
                contract,
            } => {
                self.scalar_query(*machine, *value, *contract)?;
                let body = self.body(*machine)?;
                let source = match body
                    .definitions
                    .get(value)
                    .ok_or("callable provenance SSA definition is absent")?
                {
                    CallableDefinition::Constructed(descriptor) => Source::Constructed(*descriptor),
                    CallableDefinition::Formal(position) => Source::Input(CallableFormalInput {
                        machine: *machine,
                        parameter: CallableParameter::Scalar(*position),
                        contract: *contract,
                    }),
                    CallableDefinition::Incoming(block) => {
                        return self.incoming(*machine, *block, *value, *contract);
                    }
                    CallableDefinition::Stored { position, location } => {
                        Source::Value(CallableValueQuery::Storage {
                            machine: *machine,
                            position: *position,
                            location: location.clone(),
                            contract: *contract,
                        })
                    }
                    CallableDefinition::Invocation(position) => Source::Invocation(
                        body.primary_invocation(*position)?.clone(),
                        ResultProjection::Scalar,
                    ),
                    CallableDefinition::Opaque => Source::Ceiling,
                    CallableDefinition::Unsupported => {
                        return Err("callable provenance uses an unsupported scalar definition");
                    }
                };
                Ok(vec![source])
            }
            CallableValueQuery::Storage {
                machine,
                position,
                location,
                contract,
            } => self.stored_sources(*machine, *position, location, *contract),
        }
    }

    fn incoming(
        &self,
        machine: MachineId,
        block: BlockId,
        value: ValueId,
        contract: CallableContractId,
    ) -> CallableValueResult<Vec<Source>> {
        let flow = self.flow(machine)?;
        let reach = crate::descriptors::descriptor_results::payload_reachability::predecessors(
            flow,
            (block, 0),
        )
        .ok_or("callable SSA join has unresolved incoming flow")?;
        let mut result = Vec::new();
        for (position, step) in &flow.flow {
            for (ordinal, edge) in step.successors.iter().enumerate() {
                if edge.target != (block, 0) || !reach.includes_edge(*position, ordinal) {
                    continue;
                }
                let bindings = flow
                    .payload_scalar_bindings
                    .get(position)
                    .and_then(|rows| rows.get(ordinal))
                    .ok_or("callable SSA join lost an incoming actual binding")?;
                let mut bindings = bindings.iter().filter(|(formal, _)| *formal == value);
                let (_, actual) = bindings
                    .next()
                    .ok_or("callable SSA join has no actual for its formal")?;
                if bindings.next().is_some() {
                    return Err("callable SSA join has ambiguous actual bindings");
                }
                let ScalarTerm::Value { id, scalar_type } = actual else {
                    return Err("callable SSA join has a non-value actual");
                };
                if *scalar_type != ScalarType::Callable(contract) {
                    return Err("callable SSA join changes its contract");
                }
                result.push(Source::Value(self.scalar_query(machine, *id, contract)?));
            }
        }
        if result.is_empty() {
            return Err("callable SSA join has no feasible incoming value");
        }
        Ok(result)
    }

    fn stored_sources(
        &self,
        machine: MachineId,
        position: DescriptorPosition,
        observed: &DescriptorLocation,
        contract: CallableContractId,
    ) -> CallableValueResult<Vec<Source>> {
        let flow = self.flow(machine)?;
        if !flow.flow.contains_key(&position) {
            return Err("callable storage query has a stale observation cut");
        }
        let mut analysis =
            DescriptorAnalysis::with_reference_slots(self.structural_types, &self.graph);
        let incoming_borrow = flow.places.iter().any(|place| {
            place.place == observed.root
                && matches!(place.definition,
                    DescriptorDefinition::Parameter { access, .. }
                        | DescriptorDefinition::Arrival { access }
                    if access != terminal_psi::StructuralAccess::Owned)
        });
        let (location, unresolved_address) = match analysis.location(machine, position, observed) {
            Some(location) => (location, false),
            None => (
                observed.clone(),
                incoming_borrow || observed.path.contains(&DescriptorPathSegment::Referent),
            ),
        };
        if position == flow.entry {
            let mut places = flow
                .places
                .iter()
                .filter(|place| place.place == location.root);
            let place = places.next().ok_or("callable storage root is absent")?;
            if places.next().is_some() {
                return Err("callable storage root is ambiguous");
            }
            let DescriptorDefinition::Parameter { position, .. } = place.definition else {
                return Err("callable storage has no initialized incoming value");
            };
            return Ok(vec![Source::Input(CallableFormalInput {
                machine,
                parameter: CallableParameter::Structural {
                    position,
                    path: location.path,
                },
                contract,
            })]);
        }
        // The existing version proof is a fast path for a single definition
        // that reaches every incoming path. Branch-dependent definitions fall
        // through to the general reaching-definition equations below.
        if !unresolved_address
            && let Some(sources) = crate::descriptor_results::unchanged_stored_projection_sources_in(
                self.structural_types,
                &self.graph,
                &mut analysis,
                machine,
                position,
                &location,
                |producer| {
                    flow.scalar_payload_writes
                        .get(&producer)
                        .into_iter()
                        .flatten()
                        .filter_map(|(written, term)| match term {
                            ScalarTerm::Value { id, scalar_type }
                                if *scalar_type == ScalarType::Callable(contract) =>
                            {
                                Some((written.clone(), *id))
                            }
                            _ => None,
                        })
                        .collect()
                },
            )
            && let [value] = sources.as_slice()
        {
            return Ok(vec![Source::Value(
                self.scalar_query(machine, *value, contract)?,
            )]);
        }
        let reach = crate::descriptors::descriptor_results::payload_reachability::predecessors(
            flow, position,
        )
        .ok_or("callable storage query has unresolved incoming flow")?;
        let mut result = Vec::new();
        for (producer, step) in &flow.flow {
            for (ordinal, edge) in step.successors.iter().enumerate() {
                if edge.target != position || !reach.includes_edge(*producer, ordinal) {
                    continue;
                }
                let mut arrival = location.clone();
                let mut replaced = false;
                for binding in &edge.bindings {
                    if binding.parameter.contains(&arrival) {
                        let Some((actual, formal)) = binding.locations() else {
                            result.push(Source::Ceiling);
                            replaced = true;
                            break;
                        };
                        arrival = arrival
                            .transported(formal, actual)
                            .ok_or("callable storage arrival lost its exact projection")?;
                        break;
                    }
                }
                if replaced {
                    continue;
                }
                if edge
                    .parameters
                    .iter()
                    .any(|formal| formal.contains(&location))
                    && arrival == location
                    && !edge
                        .bindings
                        .iter()
                        .any(|binding| binding.parameter.contains(&location))
                {
                    return Err("callable storage arrival has no exact actual binding");
                }
                let overlaps = |other: &DescriptorLocation| {
                    other.contains(&arrival) || arrival.contains(other)
                };
                let writes = flow
                    .scalar_payload_writes
                    .get(producer)
                    .into_iter()
                    .flatten()
                    .filter(|(written, _)| overlaps(written))
                    .collect::<Vec<_>>();
                if !writes.is_empty() {
                    if writes.len() != 1 || writes[0].0 != arrival {
                        result.push(Source::Ceiling);
                        continue;
                    }
                    let ScalarTerm::Value { id, scalar_type } = &writes[0].1 else {
                        return Err("callable storage write is not a code value");
                    };
                    if *scalar_type != ScalarType::Callable(contract) {
                        return Err("callable storage write changes its exact contract");
                    }
                    result.push(Source::Value(self.scalar_query(machine, *id, contract)?));
                    continue;
                }
                if unresolved_address
                    && flow
                        .scalar_payload_writes
                        .get(producer)
                        .is_some_and(|writes| !writes.is_empty())
                {
                    result.push(Source::Ceiling);
                    continue;
                }
                let body = self.body(machine)?;
                if body.structural_results.get(producer) == Some(&arrival.root)
                    && body.has_primary_invocation(*producer)
                {
                    result.push(Source::Invocation(
                        body.primary_invocation(*producer)?.clone(),
                        ResultProjection::Structural(arrival.path),
                    ));
                    continue;
                }
                let copies = step
                    .derivations
                    .iter()
                    .filter(|(_, destination)| destination.contains(&arrival))
                    .collect::<Vec<_>>();
                if !copies.is_empty() {
                    for (source, destination) in copies {
                        result.push(Source::Value(CallableValueQuery::Storage {
                            machine,
                            position: *producer,
                            location: arrival
                                .transported(destination, source)
                                .ok_or("callable storage copy lost its exact projection")?,
                            contract,
                        }));
                    }
                    continue;
                }
                let invalidated = flow.unresolved_contents_writes.contains(producer)
                    || step.definitions.iter().any(overlaps)
                    || flow.payload_invalidations.get(producer).into_iter().flatten().any(|query| {
                        query.as_ref().and_then(|query| analysis.payload_targets(machine, *producer, query))
                            .is_none_or(|targets| targets.iter().any(|target| match target {
                                crate::descriptor_results::DescriptorPayloadTarget::Storage(location) => overlaps(location),
                                crate::descriptor_results::DescriptorPayloadTarget::Incoming(_) => true,
                            }))
                    });
                result.push(if invalidated {
                    Source::Ceiling
                } else {
                    Source::Value(CallableValueQuery::Storage {
                        machine,
                        position: *producer,
                        location: arrival,
                        contract,
                    })
                });
            }
        }
        if result.is_empty() {
            return Err("callable storage read has no feasible initialized source");
        }
        Ok(result)
    }

    fn result_queries(
        &self,
        machine: MachineId,
        projection: &ResultProjection,
        contract: CallableContractId,
    ) -> CallableValueResult<Vec<CallableValueQuery>> {
        let body = self.body(machine)?;
        match projection {
            ResultProjection::Scalar
                if body.scalar_result != Some(ScalarType::Callable(contract)) =>
            {
                return Err("callable result query changes the selected result contract");
            }
            ResultProjection::Structural(_) if body.structural_result.is_none() => {
                return Err("callable result projection has no selected structural result");
            }
            _ => {}
        }
        let result = match projection {
            ResultProjection::Scalar => body
                .scalar_returns
                .iter()
                .map(|value| self.scalar_query(machine, *value, contract))
                .collect::<CallableValueResult<Vec<_>>>()?,
            ResultProjection::Structural(path) => body
                .structural_returns
                .iter()
                .map(|(position, root)| {
                    if self.flow(machine)?.place_types.get(root).copied() != body.structural_result
                    {
                        return Err("callable structural return changes its selected result type");
                    }
                    Ok(CallableValueQuery::Storage {
                        machine,
                        position: *position,
                        location: DescriptorLocation {
                            root: *root,
                            path: path.clone(),
                        },
                        contract,
                    })
                })
                .collect::<CallableValueResult<Vec<_>>>()?,
        };
        if result.is_empty() {
            return Err("callable result has no normal return source");
        }
        Ok(result)
    }
}

struct Solver<'a, 'program> {
    program: &'a CallableValueProgram<'program>,
    queries: Vec<CallableValueQuery>,
    values: BTreeMap<CallableValueQuery, CallableValuePossibilities>,
    sources: BTreeMap<CallableValueQuery, Vec<Source>>,
    reached_inputs: Option<&'a BTreeMap<CallableFormalInput, CallableValuePossibilities>>,
    requested_inputs: BTreeSet<CallableFormalInput>,
}

impl Solver<'_, '_> {
    fn read(
        &mut self,
        query: CallableValueQuery,
    ) -> CallableValueResult<CallableValuePossibilities> {
        if let Some(value) = self.values.get(&query) {
            return Ok(value.clone());
        }
        self.program.validate_query(&query)?;
        // Recursive projections can grow without producing a finite stored
        // coordinate. Keep the authored ceiling rather than inventing a leaf.
        if let CallableValueQuery::Storage {
            machine,
            position,
            location,
            contract,
        } = &query
            && self.queries.iter().any(|previous| {
                matches!(previous,
                CallableValueQuery::Storage { machine: m, position: p, location: l, contract: c }
                if m == machine && p == position && c == contract && l.root == location.root
                    && l.path.len() < location.path.len() && location.path.starts_with(&l.path))
            })
        {
            return Ok(CallableValuePossibilities::ceiling(*contract));
        }
        self.queries.push(query.clone());
        self.values
            .insert(query, CallableValuePossibilities::default());
        Ok(CallableValuePossibilities::default())
    }

    fn invocation(
        &mut self,
        caller: MachineId,
        invocation: &CallableInvocation,
        projection: &ResultProjection,
        result_contract: CallableContractId,
    ) -> CallableValueResult<CallableValuePossibilities> {
        self.program.validate_occurrence(caller, invocation)?;
        let mut result = CallableValuePossibilities::default();
        let mut machines = BTreeSet::new();
        match invocation.target {
            CallableInvocationTarget::Machine(machine) => {
                machines.insert(machine);
            }
            CallableInvocationTarget::Value(value, contract) => {
                let query = self.program.scalar_query(caller, value, contract)?;
                let selected = self.read(query)?;
                if !selected.inputs.is_empty() || !selected.unresolved_ceilings.is_empty() {
                    result.join(CallableValuePossibilities::ceiling(result_contract));
                }
                for descriptor in selected.descriptors {
                    let descriptor = self
                        .program
                        .catalog
                        .descriptor(descriptor)
                        .ok_or("callable selection descriptor is absent or stale")?;
                    if descriptor.contract != contract {
                        return Err("callable selection descriptor changes its contract");
                    }
                    machines.insert(descriptor.machine);
                }
            }
            CallableInvocationTarget::Opaque => {
                return Ok(CallableValuePossibilities::ceiling(result_contract));
            }
        }
        for machine in machines {
            self.program.validate_actuals(caller, machine, invocation)?;
            for returned in self
                .program
                .result_queries(machine, projection, result_contract)?
            {
                let values = self.read(returned)?;
                if self.reached_inputs.is_some() {
                    result.join(values);
                    continue;
                }
                result.descriptors.extend(values.descriptors);
                result
                    .unresolved_ceilings
                    .extend(values.unresolved_ceilings);
                for input in values.inputs {
                    if input.machine != machine {
                        return Err("callable result summary retained another machine's formal");
                    }
                    let actual = self.program.input_actual(&input, caller, invocation)?;
                    result.join(match actual {
                        CallableActualValue::Value(query) => self.read(query)?,
                        CallableActualValue::Ceiling(contract) => {
                            CallableValuePossibilities::ceiling(contract)
                        }
                    });
                }
            }
        }
        Ok(result)
    }

    fn evaluate(
        &mut self,
        query: &CallableValueQuery,
    ) -> CallableValueResult<CallableValuePossibilities> {
        let mut result = CallableValuePossibilities::default();
        let sources = match self.sources.get(query) {
            Some(sources) => sources.clone(),
            None => {
                let sources = self.program.sources(query)?;
                self.sources.insert(query.clone(), sources.clone());
                sources
            }
        };
        for source in sources {
            match source {
                Source::Value(query) => {
                    result.join(self.read(query)?);
                }
                Source::Constructed(id) => {
                    let descriptor = self
                        .program
                        .catalog
                        .descriptor(id)
                        .ok_or("constructed callable descriptor is absent or stale")?;
                    if descriptor.contract != query.contract() {
                        return Err("constructed callable descriptor has the wrong exact contract");
                    }
                    self.program.body(descriptor.machine)?;
                    result.descriptors.insert(id);
                }
                Source::Input(input) => match self.reached_inputs {
                    Some(inputs) => {
                        self.requested_inputs.insert(input.clone());
                        if let Some(values) = inputs.get(&input) {
                            result.join(values.clone());
                        }
                    }
                    None => {
                        result.inputs.insert(input);
                    }
                },
                Source::Invocation(invocation, projection) => {
                    result.join(self.invocation(
                        query.machine(),
                        &invocation,
                        &projection,
                        query.contract(),
                    )?);
                }
                Source::Ceiling => {
                    result.join(CallableValuePossibilities::ceiling(query.contract()));
                }
            }
        }
        Ok(result)
    }
}

pub fn resolve_callable_value(
    program: &CallableValueProgram<'_>,
    query: CallableValueQuery,
) -> CallableValueResult<CallableValuePossibilities> {
    let mut solver = Solver {
        program,
        queries: Vec::new(),
        values: BTreeMap::new(),
        sources: BTreeMap::new(),
        reached_inputs: None,
        requested_inputs: BTreeSet::new(),
    };
    solver.read(query.clone())?;
    loop {
        let count = solver.queries.len();
        let mut changed = false;
        for index in 0..count {
            let key = solver.queries[index].clone();
            let result = solver.evaluate(&key)?;
            changed |= solver
                .values
                .get_mut(&key)
                .ok_or("callable query lost its fixed-point row")?
                .join(result);
        }
        if changed || solver.queries.len() != count {
            continue;
        }
        let bottom = solver
            .queries
            .iter()
            .filter(|key| {
                solver
                    .values
                    .get(*key)
                    .is_some_and(CallableValuePossibilities::is_bottom)
            })
            .cloned()
            .collect::<Vec<_>>();
        if bottom.is_empty() {
            break;
        }
        for key in bottom {
            solver
                .values
                .get_mut(&key)
                .ok_or("callable cycle lost its fixed-point row")?
                .join(CallableValuePossibilities::ceiling(key.contract()));
        }
    }
    solver
        .values
        .remove(&query)
        .ok_or("callable query lost its result")
}

// Reachability supplies only actual inputs from already reached invocations.
// Keep bottom private while that outer fixed point is still collecting them.
fn resolve_reached_value(
    program: &CallableValueProgram<'_>,
    query: CallableValueQuery,
    inputs: &BTreeMap<CallableFormalInput, CallableValuePossibilities>,
) -> CallableValueResult<(CallableValuePossibilities, BTreeSet<CallableFormalInput>)> {
    let mut solver = Solver {
        program,
        queries: Vec::new(),
        values: BTreeMap::new(),
        sources: BTreeMap::new(),
        reached_inputs: Some(inputs),
        requested_inputs: BTreeSet::new(),
    };
    solver.read(query.clone())?;
    loop {
        let count = solver.queries.len();
        let mut changed = false;
        for index in 0..count {
            let key = solver.queries[index].clone();
            let value = solver.evaluate(&key)?;
            changed |= solver
                .values
                .get_mut(&key)
                .ok_or("reached callable query lost its fixed-point row")?
                .join(value);
        }
        if !changed && count == solver.queries.len() {
            break;
        }
    }
    Ok((
        solver
            .values
            .remove(&query)
            .ok_or("reached callable query lost its result")?,
        solver.requested_inputs,
    ))
}

/// Substitute one real invocation's actuals into a value summary of its body.
/// Remaining inputs continue to name their supplying caller; this operation
/// never collects unrelated callers or treats signature eligibility as code.
pub fn resolve_callable_value_at_invocation(
    program: &CallableValueProgram<'_>,
    query: CallableValueQuery,
    caller: MachineId,
    invocation: &CallableInvocation,
) -> CallableValueResult<CallableValuePossibilities> {
    program.validate_occurrence(caller, invocation)?;
    let machine = query.machine();
    let selected = match invocation.target {
        CallableInvocationTarget::Machine(callee) => callee == machine,
        CallableInvocationTarget::Value(value, contract) => {
            let values =
                resolve_callable_value(program, program.scalar_query(caller, value, contract)?)?;
            values.descriptors.iter().any(|id| {
                program.catalog.descriptor(*id).is_some_and(|descriptor| {
                    descriptor.contract == contract && descriptor.machine == machine
                })
            })
        }
        CallableInvocationTarget::Opaque => false,
    };
    if !selected {
        return Err("callable input substitution has no actual code selection for this body");
    }
    program.validate_actuals(caller, machine, invocation)?;
    let mut values = resolve_callable_value(program, query)?;
    let inputs = std::mem::take(&mut values.inputs);
    for input in inputs {
        if input.machine != machine {
            return Err("callable input substitution retained another body's formal");
        }
        let actual = program.input_actual(&input, caller, invocation)?;
        values.join(match actual {
            CallableActualValue::Value(query) => resolve_callable_value(program, query)?,
            CallableActualValue::Ceiling(contract) => CallableValuePossibilities::ceiling(contract),
        });
    }
    Ok(values)
}

#[cfg(test)]
mod tests;
