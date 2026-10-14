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
        symbol: SymbolHandle,
        statement_index: u32,
        path: Path,
    },
    Unknown,
}

#[derive(Default)]
pub(in crate::execution::terminal_unit) struct Transfers {
    pub(in crate::execution::terminal_unit) possible: Vec<Transfer>,
    pub(in crate::execution::terminal_unit) definite: Vec<Path>,
}

impl Transfers {
    fn without_receiver(mut self) -> Self {
        for transfer in &mut self.possible {
            transfer.destination = Destination::Unknown;
        }
        self
    }

    fn append(&mut self, other: Self) {
        self.possible.extend(other.possible);
        self.definite.extend(other.definite);
    }

    fn alternatives(alternatives: Vec<Self>) -> Self {
        let definite = alternatives
            .iter()
            .flat_map(|arm| &arm.definite)
            .filter(|path| {
                alternatives
                    .iter()
                    .all(|arm| arm.definite.iter().any(|covered| path.starts_with(covered)))
            })
            .cloned()
            .collect();
        Self {
            definite,
            possible: alternatives
                .into_iter()
                .flat_map(|arm| arm.possible)
                .collect(),
        }
    }
}

struct Query<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    results: &'a Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    formation: u32,
    operations: &'a [CheckedUnitEffectOperationPlan],
}

pub(in crate::execution::terminal_unit) fn transfers(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
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
    results: &Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    formation: u32,
    selection: &[CheckedUnitStructuralPathSegment],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Transfers> {
    let query = Query {
        program,
        facts,
        results,
        source,
        formation,
        operations,
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
    parameter_index: u32,
    parameter_position: u32,
    parameter_symbol: SymbolHandle,
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<ParameterVacancies> {
    let query = Query {
        program,
        facts,
        results: &[],
        source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
        formation: 0,
        operations,
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
            output.append(self.call(call));
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
                CheckedUnitEffectOperationPlan::Store(store) => match store.destination.root {
                    checked_trees::CheckedStoreRoot::Local { symbol } => Destination::Storage {
                        symbol,
                        statement_index: store.statement_index,
                        path: store.destination.path.clone(),
                    },
                    _ => Destination::Unknown,
                },
                _ => Destination::Unknown,
            };
            output.append(self.leaf(value.path.clone(), destination))
        }
        if let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
        ) = operation
        {
            output.append(self.value(*value, &[], result, &[], active)?);
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
        Transfers {
            definite: vec![source.clone()],
            possible: vec![Transfer {
                source,
                destination,
            }],
        }
    }

    fn call(&self, call: &checked_trees::CheckedUnitCallPlan) -> Transfers {
        let mut output = Transfers::default();
        for argument in &call.structural_arguments {
            if self.belongs(argument) {
                output.append(self.leaf(
                    argument.path.clone(),
                    Destination::Call {
                        statement_index: call.coordinate.statement_index,
                        structural_result: call.structural_result().is_some(),
                    },
                ));
            }
        }
        output
    }

    fn value(
        &self,
        value: CheckedStructuralValueHandle,
        selection: &[CheckedUnitStructuralPathSegment],
        result: &CheckedUnitStructuralResultBindingPlan,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let values = &self.facts.values.structural_values;
        if !values.nodes.is_valid(value) || active.contains(&value) {
            return None;
        }
        active.push(value);
        let output = self.value_kind(
            &values.nodes.get(value).kind,
            selection,
            result,
            destination,
            active,
        );
        active.pop();
        output
    }

    fn value_kind(
        &self,
        kind: &CheckedStructuralValueKind,
        selection: &[CheckedUnitStructuralPathSegment],
        result: &CheckedUnitStructuralResultBindingPlan,
        destination: &[OwnerSegment],
        active: &mut Vec<CheckedStructuralValueHandle>,
    ) -> Option<Transfers> {
        let values = &self.facts.values.structural_values;
        let mut output = Transfers::default();
        match kind {
            CheckedStructuralValueKind::Place(argument) if self.belongs(argument) => {
                let mut path = argument.path.clone();
                path.extend_from_slice(selection);
                return Some(self.leaf(
                    path,
                    Destination::Holder(result.clone(), destination.to_vec()),
                ));
            }
            CheckedStructuralValueKind::Call { source_call } => {
                let mut matching = self
                    .operations
                    .iter()
                    .flat_map(|operation| operation.with_value_calls())
                    .filter_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                                calls,
                                ..
                            },
                        ) => Some(calls.as_slice()),
                        _ => None,
                    })
                    .flatten()
                    .filter(|call| {
                        matches!(values.nodes.get(call.value).kind,
                            CheckedStructuralValueKind::Call { source_call: captured }
                                if captured == *source_call)
                    });
                let call = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                // The graph's dispatch intersects definite moves across its
                // arms. Flattening operand calls here instead would assert a
                // conditional owned argument moved on every path.
                for operand in call.operands().iter() {
                    output.append(self.operation(operand, active)?);
                }
                output.append(self.call(call.operation().call()?));
                return Some(output);
            }
            CheckedStructuralValueKind::Projection { source, path, .. } => {
                let mut selected = path.clone();
                selected.extend_from_slice(selection);
                return self.value(*source, &selected, result, destination, active);
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
                if let CheckedStructuralValueKind::StructuralCase { case, .. } = kind {
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
                    if selection.starts_with(&relative) {
                        // The result selects within this field: its container
                        // prefix no longer belongs to the receiving value.
                        output.append(self.value(
                            child,
                            &selection[relative.len()..],
                            result,
                            destination,
                            active,
                        )?);
                    } else if relative.starts_with(selection) {
                        // Whole construction or a selected case payload keeps
                        // the remaining field path inside the receiving value.
                        let mut path = destination.to_vec();
                        path.extend_from_slice(&owner_path[selection.len()..]);
                        output.append(self.value(child, &[], result, &path, active)?);
                    } else {
                        // Construction consumed the unselected sibling too;
                        // its custody is absent from the surviving projection.
                        output.append(
                            self.value(child, &[], result, &[], active)?
                                .without_receiver(),
                        );
                    }
                }
            }
            CheckedStructuralValueKind::FixedArray { elements } => {
                if let Some((CheckedUnitStructuralPathSegment::FixedIndex(selected), suffix)) =
                    selection.split_first()
                {
                    let selected = usize::try_from(*selected).ok()?;
                    elements.get(selected)?;
                    for (index, child) in elements.iter().enumerate() {
                        let transferred = if index == selected {
                            self.value(*child, suffix, result, destination, active)?
                        } else {
                            self.value(*child, &[], result, &[], active)?
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
                    output.append(self.value(*child, &[], result, &path, active)?);
                }
            }
            CheckedStructuralValueKind::Dispatch { subject, arms } => {
                output.append(self.computation(*subject)?);
                let alternatives = values
                    .dispatch_arms
                    .span(*arms)?
                    .iter()
                    .map(|arm| self.value(arm.value, selection, result, destination, active))
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
            CheckedStructuralValueKind::Place(argument)
            | CheckedStructuralValueKind::CopiedStructuralPlace {
                source: argument, ..
            }
            | CheckedStructuralValueKind::Reference {
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
