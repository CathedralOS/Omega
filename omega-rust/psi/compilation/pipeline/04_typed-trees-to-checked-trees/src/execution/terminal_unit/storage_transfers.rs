//! Current custody follows the retained value graph into its receiving leaf.
//! Possible transfers fence disposal of the original slot. Definite transfers
//! additionally prove that every selected alternative moved that slot.

use checked_trees::CheckFacts;
use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
};
use symbols::SymbolHandle;

type Results = [(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)];
use checked_trees::{BorrowLoanOwnerSegment as OwnerSegment, CheckedStructuralValueHandle};
use typed_trees::data::DataMember;

mod computations;

type Path = Vec<CheckedUnitStructuralPathSegment>;

pub(in crate::execution::terminal_unit) struct Transfer {
    pub(in crate::execution::terminal_unit) source: Path,
    pub(in crate::execution::terminal_unit) destination: Destination,
}

pub(in crate::execution::terminal_unit) enum Destination {
    Holder(CheckedUnitStructuralResultBindingPlan, Vec<OwnerSegment>),
    /// An independently registered anonymous home, constructed before a
    /// descriptor lends it. Owned children move into this backing, never into
    /// the reference carrier that observes it.
    Backing(CheckedStructuralValueHandle, Vec<OwnerSegment>),
    /// The call consumes this source. A structural result can carry the
    /// consumed descriptors onward: its returned occurrences retain exact
    /// supplying-loan identities independently of result nesting. A call
    /// without one ends the carrier inside the callee; only a stored
    /// reference effect can still publish a receiving occurrence.
    Call {
        statement_index: u32,
        structural_result: bool,
    },
    Storage {
        /// One authored assignment can expand into several distinct stores;
        /// retain the producer's selection within its captured destination.
        statement_index: u32,
        captured_selection: Path,
    },
    Unknown,
}

/// A completed value can enter a named holder, an anonymous backing, or a
/// consuming call. Traversal must not invent a local binding for the latter two.
#[derive(Clone, Copy)]
enum ValueReceiver<'a> {
    Holder(&'a CheckedUnitStructuralResultBindingPlan),
    Backing(CheckedStructuralValueHandle),
    Call {
        statement_index: u32,
        structural_result: bool,
    },
    Unbound,
}

impl ValueReceiver<'_> {
    fn destination(self, path: &[OwnerSegment]) -> Destination {
        match self {
            Self::Holder(result) => Destination::Holder(result.clone(), path.to_vec()),
            Self::Backing(value) => Destination::Backing(value, path.to_vec()),
            Self::Call {
                statement_index,
                structural_result,
            } => Destination::Call {
                statement_index,
                structural_result,
            },
            Self::Unbound => Destination::Unknown,
        }
    }
}

#[derive(Default)]
pub(in crate::execution::terminal_unit) struct Transfers {
    pub(in crate::execution::terminal_unit) possible: Vec<Transfer>,
    pub(in crate::execution::terminal_unit) definite: Vec<Path>,
    /// Moves after this exact producing occurrence, on every normal path
    /// where it completes. None means this summary contains no such producer.
    /// Unconditional `definite` remains independent for pre-existing storage.
    after_production: Option<Vec<Path>>,
}

impl Transfers {
    fn without_receiver(mut self) -> Self {
        for transfer in &mut self.possible {
            transfer.destination = Destination::Unknown;
        }
        self
    }

    fn append(&mut self, other: Self) {
        // Once produced before a later choice, the source exists on every
        // outgoing alternative. Only that choice's unconditional moves can
        // extend this prefix; an arm-local production cannot erase a sibling.
        let preceding = self.after_production.take().map(|mut paths| {
            paths.extend(other.definite.iter().cloned());
            paths
        });
        self.after_production = match (preceding, other.after_production) {
            (Some(preceding), Some(following)) => Some(common_paths(&[&preceding, &following])),
            (preceding, None) => preceding,
            (None, following) => following,
        };
        self.possible.extend(other.possible);
        self.definite.extend(other.definite);
    }

    fn alternatives(alternatives: Vec<Self>) -> Self {
        let definite = common_paths(
            &alternatives
                .iter()
                .map(|arm| arm.definite.as_slice())
                .collect::<Vec<_>>(),
        );
        let produced = alternatives
            .iter()
            .filter_map(|arm| arm.after_production.as_deref())
            .collect::<Vec<_>>();
        let after_production = (!produced.is_empty()).then(|| common_paths(&produced));
        Self {
            definite,
            after_production,
            possible: alternatives
                .into_iter()
                .flat_map(|arm| arm.possible)
                .collect(),
        }
    }

    /// Paths transferred on every normal path producing this exact occurrence.
    /// A source without a producer in this summary keeps the unconditional
    /// all-alternative proof; a branch-born source cannot donate sibling facts.
    pub(in crate::execution::terminal_unit) fn occurrence_transfers(&self) -> &[Path] {
        self.after_production.as_deref().unwrap_or(&self.definite)
    }
}

fn common_paths(alternatives: &[&[Path]]) -> Vec<Path> {
    alternatives
        .iter()
        .flat_map(|paths| paths.iter())
        .filter(|path| {
            alternatives
                .iter()
                .all(|paths| paths.iter().any(|covered| path.starts_with(covered)))
        })
        .cloned()
        .collect()
}

#[derive(Clone)]
struct Query<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    results: &'a Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    source_is_unrestricted: bool,
    formation: u32,
    operations: &'a [CheckedUnitEffectOperationPlan],
    prepared_calls: &'a super::structural_value_calls::PreparedCalls,
    established_values: Vec<CheckedStructuralValueHandle>,
    completed_computations: Vec<checked_trees::CheckedScalarComputationHandle>,
}

pub(in crate::execution::terminal_unit) fn transfers(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &super::structural_value_calls::PreparedCalls,
    results: &Results,
    binding: u32,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Transfers> {
    let formation = results
        .iter()
        .find(|(result, _)| result.binding_ordinal == binding)
        .map_or(0, |(result, _)| result.statement_index);
    from_storage(
        program,
        facts,
        prepared_calls,
        results,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: binding,
        },
        formation,
        &[],
        operations,
    )
}

pub(in crate::execution::terminal_unit) fn from_storage(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &super::structural_value_calls::PreparedCalls,
    results: &Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    formation: u32,
    selection: &[CheckedUnitStructuralPathSegment],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Transfers> {
    // Owned access selects a by-value operand; the producing declaration still
    // decides whether that operand copies or transfers its storage. All aliases
    // of a registered copyable home retain the original descriptor occurrence.
    // Missing result metadata supplies no copy evidence.
    let mut bindings = results.iter().filter(|(result, root)| match &source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            result.binding_ordinal == *binding_ordinal
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            *root == facts::PlaceRoot::Symbol(*symbol)
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
            *root == facts::PlaceRoot::Expression(*expression)
        }
        _ => false,
    });
    let source_is_unrestricted = bindings.next().is_some_and(|(result, _)| {
        result.multiplicity == language_semantics::Multiplicity::Unrestricted
            && bindings.all(|(other, _)| {
                other.multiplicity == language_semantics::Multiplicity::Unrestricted
            })
    });
    let query = Query {
        program,
        facts,
        results,
        source,
        source_is_unrestricted,
        formation,
        operations,
        prepared_calls,
        established_values: Vec::new(),
        completed_computations: Vec::new(),
    };
    let mut output = Transfers::default();
    for operation in operations {
        if operation
            .source_statement_index()
            .is_some_and(|statement| statement < query.formation)
        {
            continue;
        }
        output.append(query.operation(operation, &mut Vec::new())?);
        // A receiving overwrite starts another occurrence at this slot. Its
        // RHS still sees the old value; later moves belong to the new value.
        // For a leaf query, unrelated sibling replacements do not end it.
        if let CheckedUnitEffectOperationPlan::Store(store) = operation
            && store.statement_index > formation
            && store.installed_value().is_some()
            && query.owner().is_some_and(|symbol| {
                store.destination.root == (checked_trees::CheckedStoreRoot::Local { symbol })
            })
            && query.local_store_owns_destination(store)?
            && selection.starts_with(&store.destination.path)
        {
            break;
        }
    }
    Some(output)
}

/// Retained computations include selected exits not yet appended to the body.
/// A possible owned use fences unconditional retirement, but proves no actual
/// transfer: the selected control path still owns its move or cleanup receipt.
/// This is a conservative fence, not a current-occupant transfer count. Named
/// replacement cleanup separately owns an old occurrence's death; this query
/// cannot authorize retirement or infer that a later root actually executes.
pub(in crate::execution::terminal_unit) fn has_computation_transfer(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    results: &Results,
    result: &CheckedUnitStructuralResultBindingPlan,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<bool> {
    let query = Query {
        program,
        facts,
        results,
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        source_is_unrestricted: result.multiplicity
            == language_semantics::Multiplicity::Unrestricted,
        formation: result.statement_index,
        operations,
        prepared_calls: &super::structural_value_calls::PreparedCalls::EMPTY,
        established_values: Vec::new(),
        completed_computations: Vec::new(),
    };
    for (_, root) in facts.values.scalar_computations.roots.iter() {
        if root.state == state
            && root.statement_ordinal >= result.statement_index
            && !query.computation(root.root)?.possible.is_empty()
        {
            return Some(true);
        }
    }
    Some(false)
}

/// A parameter's vacancies at an ordinary completion: the maximal paths moved
/// on every alternative, and the paths moved on only some alternatives
/// (`maybe`). One completion cannot dispose storage present on only some
/// alternatives, so its caller decides whether a maybe-moved path is admitted.
pub(super) struct ParameterVacancies {
    pub(super) definite: Vec<Path>,
    pub(super) maybe: Vec<Path>,
}

/// Current vacancies differ from historical transfers: a checked installation
/// restores its destination after evaluating its RHS. Both queries consume the
/// same operation/value traversal; no permission count stands for a move.
pub(super) fn parameter_vacancies(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &super::structural_value_calls::PreparedCalls,
    parameter_index: u32,
    parameter_position: u32,
    parameter_symbol: SymbolHandle,
    parameter_multiplicity: language_semantics::Multiplicity,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<ParameterVacancies> {
    let query = Query {
        program,
        facts,
        results: &[],
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        source_is_unrestricted: parameter_multiplicity
            == language_semantics::Multiplicity::Unrestricted,
        formation: 0,
        operations,
        prepared_calls,
        established_values: Vec::new(),
        completed_computations: Vec::new(),
    };
    let mut definite = Vec::<Path>::new();
    let mut possible = Vec::<Path>::new();
    for operation in operations {
        let transfers = query.operation(operation, &mut Vec::new())?;
        definite.extend(transfers.definite);
        possible.extend(
            transfers
                .possible
                .into_iter()
                .map(|transfer| transfer.source),
        );
        if let CheckedUnitEffectOperationPlan::Store(store) = operation
            && store.installed_value().is_some()
            && match store.destination.root {
                checked_trees::CheckedStoreRoot::Parameter { position } => {
                    position == parameter_position
                }
                checked_trees::CheckedStoreRoot::Local { symbol } => symbol == parameter_symbol,
            }
        {
            let written = &store.destination.path;
            // consumed_owners rejects writing through a strict child of a
            // moved place. Do not interpret malformed plans as restoring it.
            if possible
                .iter()
                .any(|path| written.len() > path.len() && written.starts_with(path))
            {
                return None;
            }
            definite.retain(|path| !path.starts_with(written));
            possible.retain(|path| !path.starts_with(written));
        }
    }
    // A single ordinary completion cannot dispose a path present on only
    // some alternatives; report those separately for the caller to judge.
    let mut maybe = possible
        .into_iter()
        .filter(|path| !definite.iter().any(|covered| path.starts_with(covered)))
        .collect::<Vec<_>>();
    maybe.sort();
    maybe.dedup();
    definite.sort();
    definite.dedup();
    let maximal = definite
        .iter()
        .filter(|path| {
            !definite
                .iter()
                .any(|ancestor| ancestor.len() < path.len() && path.starts_with(ancestor))
        })
        .cloned()
        .collect();
    Some(ParameterVacancies {
        definite: maximal,
        maybe,
    })
}

impl Query<'_> {
    fn owner(&self) -> Option<SymbolHandle> {
        match self.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => Some(symbol),
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                self.results.iter().find_map(|(result, root)| {
                    (result.binding_ordinal == binding_ordinal)
                        .then_some(root.as_symbol())
                        .flatten()
                })
            }
            _ => None,
        }
    }
    fn operation(
        &self,
        operation: &CheckedUnitEffectOperationPlan,
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        output.append(self.operation_computations(operation)?);
        // A checked store names its receiving storage independently of the
        // producer. External storage has no retiring holder in this activation.
        if let Some(call) = operation.call() {
            output.append(self.call(call, active)?);
        }
        // A window extraction transfers the selected value to its detached
        // holder just like an explicit operand. Its old aggregate keeps only
        // the complement; ending a loan cannot dispose the moved cells again.
        if let CheckedUnitEffectOperationPlan::MoveStructuralField {
            source,
            result,
            expression,
        } = operation
            && self.belongs(source)
            && extraction_owns_source(self.program, source, result, *expression)?
        {
            output.append(self.leaf(
                source.path.clone(),
                Destination::Holder(result.clone(), Vec::new()),
            ));
        }
        if let Some(value) = operation.installed_value()
            && self.belongs(value)
        {
            let destination = match operation {
                CheckedUnitEffectOperationPlan::Store(store) => Destination::Storage {
                    statement_index: store.statement_index,
                    captured_selection: store.captured_selection.clone(),
                },
                _ => Destination::Unknown,
            };
            output.append(self.leaf(value.path.clone(), destination))
        }
        if let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
        ) = operation
        {
            output.append(self.value(
                *value,
                &[],
                true,
                ValueReceiver::Holder(result),
                &[],
                active,
            )?);
        }
        if operation.structural_result_binding().is_some_and(|result| {
            self.source
                == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                }
        }) {
            // Birth occurs after the operation's inputs and initializer have
            // completed. No earlier observation can spend its partial result.
            output.after_production = Some(Vec::new());
        }
        Some(output)
    }
    fn local_store_owns_destination(
        &self,
        store: &checked_trees::CheckedStorePlan,
    ) -> Option<bool> {
        let checked_trees::CheckedStoreRoot::Local { symbol } = store.destination.root else {
            return None;
        };
        if store
            .destination
            .path
            .contains(&CheckedUnitStructuralPathSegment::Referent)
        {
            return Some(false);
        }
        let state = local_state(self.program, symbol)?;
        let typed_trees::statement::StatementNode::Assignment(assignment) = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(store.statement_index as usize)?
        else {
            return None;
        };
        local_place_is_owned(
            self.program,
            symbol,
            store.statement_index,
            assignment.target,
            store.installed_value()?.type_identity,
        )
    }

    fn belongs(&self, argument: &CheckedUnitStructuralArgumentPlan) -> bool {
        argument.access == CheckedStructuralAccess::Owned
            && (argument.source == self.source
                || match (&self.source, &argument.source) {
                    (
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        },
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    ) => self.results.iter().any(|(result, root)| {
                        result.binding_ordinal == *binding_ordinal
                            && *root == facts::PlaceRoot::Symbol(*symbol)
                    }),
                    _ => false,
                })
    }

    fn leaf(&self, source: Path, destination: Destination) -> Transfers {
        if self.source_is_unrestricted {
            return Transfers::default();
        }
        Transfers {
            definite: vec![source.clone()],
            after_production: None,
            possible: vec![Transfer {
                source,
                destination,
            }],
        }
    }

    fn call(
        &self,
        call: &checked_trees::CheckedUnitCallPlan,
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let mut output = Transfers::default();
        for argument in &call.structural_arguments {
            output.append(self.argument_transfers(
                argument,
                ValueReceiver::Call {
                    statement_index: call.coordinate.statement_index,
                    structural_result: call.structural_result().is_some(),
                },
                active,
            )?);
        }
        Some(output)
    }

    /// Evaluating an anonymous operand constructs its value before the call
    /// consumes or lends it. Moves into that construction retire their original
    /// homes exactly as moves into a named local do.
    fn argument_transfers(
        &self,
        argument: &CheckedUnitStructuralArgumentPlan,
        receiver: ValueReceiver<'_>,
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        if self.belongs(argument) {
            return Some(self.leaf(argument.path.clone(), receiver.destination(&[])));
        }
        let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
            argument.source
        else {
            return Some(Transfers::default());
        };
        let values = &self.facts.values.structural_values;
        let mut roots = values
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.expression == expression);
        let root = roots.next()?;
        if roots.next().is_some() {
            return None;
        }
        self.value(
            root.root,
            &argument.path,
            argument.access == CheckedStructuralAccess::Owned,
            receiver,
            &[],
            active,
        )
    }

    fn value(
        &self,
        value: CheckedStructuralValueHandle,
        selection: &[CheckedUnitStructuralPathSegment],
        consumes_selection: bool,
        receiver: ValueReceiver<'_>,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        if self.established_values.contains(&value) {
            return Some(Transfers::default());
        }
        let values = &self.facts.values.structural_values;
        if !values.nodes.is_valid(value) || active.contains(&value) {
            return None;
        }
        active.push(value);
        let output = self.value_kind(
            values.nodes.get(value),
            selection,
            consumes_selection,
            if consumes_selection {
                receiver
            } else {
                ValueReceiver::Unbound
            },
            destination,
            active,
        );
        active.pop();
        output
    }

    /// Join one retained value occurrence to its actual producer operation.
    /// Aggregate schedules own nested calls; ordinary calls own their result
    /// directly. Both retain the same checked source occurrence, while only
    /// the former supplies operand preparation at this DAG position.
    fn value_call(
        &self,
        value: &checked_trees::CheckedStructuralValue,
        source_call: arena::Handle<checked_trees::FlowCallFact>,
        active: &[CheckedStructuralValueHandle],
    ) -> Option<(
        &CheckedUnitEffectOperationPlan,
        Option<&checked_trees::CheckedStructuralValueCall>,
    )> {
        let control = &self.facts.flow.control;
        if !control.calls.is_valid(source_call) {
            return None;
        }
        let source = control.calls.get(source_call);
        let state = control.state(value.machine, value.state)?;
        if source.authored_expression != value.expression
            || source.statement_index != value.statement_index as usize
            || !control
                .calls
                .span(state.calls)?
                .iter()
                .any(|candidate| std::ptr::eq(candidate, source))
        {
            return None;
        }
        let coordinate = checked_trees::CheckedUnitCallCoordinate {
            statement_index: u32::try_from(source.statement_index).ok()?,
            call_ordinal: u32::try_from(source.call_ordinal).ok()?,
        };
        let mut operations = self
            .operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter(|operation| {
                operation.call().is_some_and(|call| {
                    call.coordinate == coordinate && call.target.state == source.target_symbol
                })
            });
        let ordinary = operations.next();
        if operations.next().is_some() {
            return None;
        }
        // A value-owned call is reached through its actual DAG root's
        // prepared schedule. Private result ordinals belong to that root's
        // preceding namespace; a global coordinate search cannot select it.
        let deferred = if ordinary.is_none() {
            let calls = active.iter().find_map(|root| {
                self.prepared_calls.schedule(*root).or_else(|| {
                    self.facts
                        .values
                        .structural_values
                        .call_schedule(*root)
                        .map(|(_, calls)| calls)
                })
            })?;
            let mut calls = super::structural_value_calls::dependencies(calls).filter(|call| {
                self.facts.values.structural_values.nodes.is_valid(call.value)
                    && matches!(self.facts.values.structural_values.nodes.get(call.value).kind,
                        CheckedStructuralValueKind::Call { source_call: captured } if captured == source_call)
            });
            let call = calls.next()?;
            if calls.next().is_some() {
                return None;
            }
            Some(call)
        } else {
            None
        };
        let operation = ordinary.or_else(|| deferred.map(|call| call.operation()))?;
        if operation.call()?.coordinate != coordinate
            || operation.call()?.target.state != source.target_symbol
            || operation.call()?.structural_result()?.statement_index != value.statement_index
        {
            return None;
        }
        let mut schedules = self
            .operations
            .iter()
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
                ) => Some(calls.as_slice()),
                _ => None,
            })
            .flatten()
            .filter(|scheduled| std::ptr::eq(scheduled.operation(), operation));
        let scheduled = schedules.next().or(deferred);
        if schedules.next().is_some()
            || scheduled.is_some_and(|scheduled| {
                !self
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .is_valid(scheduled.value)
                    || !matches!(
                        self.facts.values.structural_values.nodes.get(scheduled.value).kind,
                        CheckedStructuralValueKind::Call { source_call: captured }
                            if captured == source_call
                    )
            })
        {
            return None;
        }
        Some((operation, scheduled))
    }

    fn value_kind(
        &self,
        value: &checked_trees::CheckedStructuralValue,
        selection: &[CheckedUnitStructuralPathSegment],
        consumes_selection: bool,
        receiver: ValueReceiver<'_>,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let values = &self.facts.values.structural_values;
        let mut output = Transfers::default();
        match &value.kind {
            CheckedStructuralValueKind::Place(argument)
                if consumes_selection && self.belongs(argument) =>
            {
                let mut path = argument.path.clone();
                path.extend_from_slice(selection);
                return Some(self.leaf(path, receiver.destination(destination)));
            }
            CheckedStructuralValueKind::Call { source_call } => {
                let (operation, scheduled) = self.value_call(value, *source_call, active)?;
                // Only a DAG-owned invocation evaluates its preparation here.
                // An ordinary call's operation already summarized its inputs
                // in the ordered prefix; this node names its completed result.
                if let Some(scheduled) = scheduled {
                    // Dispatch intersects these summaries at its own arms;
                    // catalog enumeration never makes a selected call definite.
                    for operand in scheduled.operands().iter() {
                        output.append(self.operation(operand, active)?);
                    }
                    output.append(self.operation(operation, active)?);
                }
                let result = operation.call()?.structural_result()?;
                if consumes_selection
                    && self.source
                        == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        })
                {
                    output.append(self.leaf(selection.to_vec(), receiver.destination(destination)));
                }
                return Some(output);
            }
            CheckedStructuralValueKind::Projection {
                source,
                path,
                projection_evaluations,
                copies_selected_value,
                ..
            } => {
                let mut selected = path.clone();
                selected.extend_from_slice(selection);
                output.append(self.value(
                    *source,
                    &selected,
                    consumes_selection && !copies_selected_value,
                    receiver,
                    destination,
                    active,
                )?);
                for operand in CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                    path,
                    *projection_evaluations,
                    &self.facts.values.scalar_computations.operands,
                ) {
                    output.append(self.computation(operand)?);
                }
                return Some(output);
            }
            checked_trees::record_or_structural_case_pattern!(data_symbol, fields, ..) => {
                let data = self.program.data_definition_by_symbol(*data_symbol)?;
                // Common fields and payload fields have independent path
                // namespaces even when a case establishes both in one roster.
                let mut declared = self
                    .program
                    .data_members(data)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some((field, None)),
                        DataMember::Variant(_) => None,
                    })
                    .collect::<Vec<_>>();
                if let CheckedStructuralValueKind::StructuralCase { case, .. } = &value.kind {
                    let variant =
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.symbol == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            })?;
                    declared.extend(
                        self.program
                            .data_payload_fields(variant)
                            .iter()
                            .map(|field| (field, Some(variant))),
                    );
                }
                for field in values.record_fields.span(*fields)? {
                    let (declaration, case) = declared
                        .iter()
                        .find(|(candidate, _)| candidate.symbol == field.field)?;
                    let mut relative = Vec::new();
                    let mut owner_path = Vec::new();
                    if let Some(case) = case {
                        relative.push(CheckedUnitStructuralPathSegment::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ),
                        ));
                        owner_path.push(OwnerSegment::Case(case.symbol));
                    }
                    relative.push(CheckedUnitStructuralPathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            declaration.identity,
                            declaration.name.clone(),
                        ),
                    ));
                    owner_path.push(OwnerSegment::Field(field.field));
                    let child = match field.value {
                        checked_trees::CheckedStructuralRecordFieldValue::Structural(child) => {
                            child
                        }
                        checked_trees::CheckedStructuralRecordFieldValue::Scalar(operand) => {
                            output.append(self.computation(operand)?);
                            continue;
                        }
                    };
                    if !consumes_selection {
                        // Observing a fresh container does not shorten the
                        // whole child move that established each of its fields.
                        // The enclosing observation removes only the invented
                        // final holder, not genuine input consumption.
                        output.append(self.value(child, &[], true, receiver, &[], active)?);
                    } else if selection.starts_with(&relative) {
                        // The result selects within this field: its container
                        // prefix no longer belongs to the receiving value.
                        output.append(self.value(
                            child,
                            &selection[relative.len()..],
                            true,
                            receiver,
                            destination,
                            active,
                        )?);
                    } else if relative.starts_with(selection) {
                        // Whole construction or a selected case payload keeps
                        // the remaining field path inside the receiving value.
                        let mut path = destination.to_vec();
                        path.extend_from_slice(&owner_path[selection.len()..]);
                        output.append(self.value(child, &[], true, receiver, &path, active)?);
                    } else {
                        // Construction consumed the unselected sibling too;
                        // its custody is absent from the surviving projection.
                        output.append(
                            self.value(child, &[], true, receiver, &[], active)?
                                .without_receiver(),
                        );
                    }
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                if !consumes_selection {
                    for child in elements {
                        output.append(self.value(*child, &[], true, receiver, &[], active)?);
                    }
                    return Some(output);
                }
                if let Some((CheckedUnitStructuralPathSegment::FixedIndex(selected), suffix)) =
                    selection.split_first()
                {
                    let selected = usize::try_from(*selected).ok()?;
                    elements.get(selected)?;
                    for (index, child) in elements.iter().enumerate() {
                        let transferred = if index == selected {
                            self.value(*child, suffix, true, receiver, destination, active)?
                        } else {
                            self.value(*child, &[], true, receiver, &[], active)?
                                .without_receiver()
                        };
                        output.append(transferred);
                    }
                    return Some(output);
                }
                if !selection.is_empty() {
                    return None;
                }
                for (index, child) in elements.iter().enumerate() {
                    let mut path = destination.to_vec();
                    path.push(OwnerSegment::FixedIndex(index));
                    output.append(self.value(*child, &[], true, receiver, &path, active)?);
                }
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                output.append(self.dispatch_subject(subject)?);
                let alternatives = values
                    .dispatch_arms
                    .span(*arms)?
                    .iter()
                    .map(|arm| {
                        self.value(
                            arm.value,
                            selection,
                            consumes_selection,
                            receiver,
                            destination,
                            active,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
                // Keep only source regions transferred on every arm. Taking
                // the union here would end a parent on an unselected move.
                output.append(Transfers::alternatives(alternatives));
            }
            CheckedStructuralValueKind::ScalarArray { elements } => {
                for element in elements {
                    output.append(self.computation(*element)?);
                }
            }
            CheckedStructuralValueKind::ZeroedScalarArray { element, .. } => {
                output.append(self.computation(*element)?)
            }
            CheckedStructuralValueKind::Case(case) => {
                for field in self
                    .facts
                    .values
                    .scalar_computations
                    .case_fields
                    .span(case.fields)?
                {
                    output.append(self.computation(field.value)?);
                }
            }
            CheckedStructuralValueKind::Reference {
                source,
                source_value,
                ..
            } => {
                if source_value.is_valid() {
                    let backing = values.nodes.get(*source_value);
                    if values
                        .root_for_expression(
                            backing.state,
                            backing.statement_index,
                            backing.expression,
                        )?
                        .root
                        != *source_value
                    {
                        return None;
                    }
                    // Reference formation evaluates the entire backing before
                    // selecting the borrowed path. Preserve each child's true
                    // receiving home rather than projecting ownership through
                    // the resulting reference shell.
                    output.append(self.value(
                        *source_value,
                        &[],
                        true,
                        ValueReceiver::Backing(*source_value),
                        &[],
                        active,
                    )?);
                }
                output.append(self.argument_computations(source)?);
            }
            CheckedStructuralValueKind::Place(argument)
            | CheckedStructuralValueKind::CopiedStructuralPlace {
                source: argument, ..
            } => {
                output.append(self.argument_computations(argument)?);
            }
            CheckedStructuralValueKind::BorrowedSliceView { source, region }
            | CheckedStructuralValueKind::MutableSliceView { source, region } => {
                output.append(self.argument_computations(source)?);
                if let checked_trees::CheckedElementViewRegion::RepresentationBytes {
                    start, ..
                } = region
                {
                    output.append(self.computation(*start)?);
                }
            }
            // Captures/copies keep their source custody; call arguments are
            // already represented by the retained operation call schedule.
            _ => {}
        }
        Some(output)
    }
}

// Window operands name the extracted value with Owned access, even when
// the storage belongs to a referent. Only an inline extraction transfers
// custody out of this carrier. Reuse the planner's typed-place judgment:
// its implicit root reference crossing is not present in the checked path.
pub(in crate::execution::terminal_unit) fn extraction_owns_source(
    program: &dyn ::validation::CheckingInput,
    source: &CheckedUnitStructuralArgumentPlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<bool> {
    if source
        .path
        .contains(&CheckedUnitStructuralPathSegment::Referent)
    {
        return Some(false);
    }
    match source.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            local_place_is_owned(
                program,
                symbol,
                result.statement_index,
                expression,
                result.type_identity,
            )
        }
        // Parameter vacancy queries are made only for owned parameters;
        // their nested descriptor crossings are explicit in the path.
        CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => Some(true),
        _ => None,
    }
}

fn local_state(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::state::State> {
    let positions = program.local_data_positions_for_symbol(symbol);
    let [position] = positions.as_slice() else {
        return None;
    };
    let machine = program.machines().get(position.0 as usize)?;
    program.machine_states(machine).get(position.1 as usize)
}

fn local_place_is_owned(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    statement: u32,
    expression: typed_trees::expression::ExpressionHandle,
    selected_type: typed_trees::type_identity::TypeIdentityId,
) -> Option<bool> {
    let state = local_state(program, symbol)?;
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement as usize,
        expression,
    )?;
    Some(
        super::borrowed_windows::owned_assignment_source(
            program,
            state,
            &[],
            statement as usize,
            &place,
            Some(selected_type),
        ) == Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }),
    )
}
