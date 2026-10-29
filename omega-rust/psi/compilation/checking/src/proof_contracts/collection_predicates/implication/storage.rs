//! Current storage belongs to one execution path; values remain immutable.
//! An owned binding receives a fresh home even when its initial contents are
//! shared. A reference binds the actual place, with selectors captured before
//! subsequent operands. Neither kind of binding rewrites predicate snapshots.

use super::*;

#[derive(Clone)]
pub(super) struct Home {
    identity: usize,
    value: Value,
}

#[derive(Clone)]
pub(super) struct Binding {
    pub(super) symbol: SymbolHandle,
    pub(super) place: Place,
}

#[derive(Clone)]
pub(super) struct Place {
    home: usize,
    projection: Vec<Projection>,
    reference: TypeReferenceHandle,
}

#[derive(Clone)]
enum Projection {
    Field(SymbolHandle),
    Element(Integer),
}

#[derive(Clone)]
pub(super) struct Argument {
    value: Value,
    reference: TypeReferenceHandle,
    backing: Option<Place>,
}

impl Implication<'_> {
    pub(super) fn seed_body_storage(
        &mut self,
        frame: &mut Frame,
        path: &mut Path,
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<()> {
        for parameter in parameters {
            self.spend()?;
            let value = frame
                .bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)?
                .1
                .clone();
            // Entry formals already identify their admitted incoming value or
            // referent. Later reference bindings must retain an actual place.
            let place = self.storage_home(path, value, parameter.type_reference);
            frame.storage.push(Binding {
                symbol: parameter.symbol,
                place,
            });
        }
        Some(())
    }

    fn storage_home(
        &mut self,
        path: &mut Path,
        value: Value,
        reference: TypeReferenceHandle,
    ) -> Place {
        let identity = self.next_identity;
        self.next_identity += 1;
        path.storage.push(Home { identity, value });
        Place {
            home: identity,
            projection: Vec::new(),
            reference,
        }
    }

    pub(super) fn bind_body_storage(
        &mut self,
        frame: &mut Frame,
        path: &mut Path,
        symbol: SymbolHandle,
        argument: Argument,
    ) -> Option<()> {
        self.spend()?;
        let place = match argument.backing {
            Some(mut place) => {
                place.reference = argument.reference;
                place
            }
            None => self.storage_home(path, argument.value, argument.reference),
        };
        frame.storage.push(Binding { symbol, place });
        Some(())
    }

    pub(super) fn body_local(
        &mut self,
        frame: &Frame,
        path: Path,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<Vec<(Frame, Path)>> {
        self.body_argument(frame, path, local.initial_value, local.type_reference)?
            .into_iter()
            .map(|(mut path, argument)| {
                let mut frame = frame.clone();
                self.bind_body_storage(&mut frame, &mut path, local.symbol, argument)?;
                Some((frame, path))
            })
            .collect()
    }

    pub(super) fn body_arguments(
        &mut self,
        frame: &Frame,
        path: Path,
        expressions: &[ExpressionHandle],
        parameters: &[typed_trees::signature::StateParameter],
    ) -> Option<Vec<(Path, Vec<Argument>)>> {
        let mut rows = vec![(path, Vec::new())];
        for (expression, parameter) in expressions.iter().zip(parameters) {
            let mut next = Vec::new();
            for (path, arguments) in rows {
                for (path, argument) in
                    self.body_argument(frame, path, *expression, parameter.type_reference)?
                {
                    let mut arguments = arguments.clone();
                    arguments.push(argument);
                    next.push((path, arguments));
                }
            }
            rows = next;
        }
        Some(rows)
    }

    fn body_argument(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> Option<Vec<(Path, Argument)>> {
        if crate::reference_shell(self.program, reference).is_some() {
            let mut result = Vec::new();
            for (path, place) in self.storage_place(frame, path, expression, 0)? {
                for (path, value) in self.storage_value(path, &place)? {
                    result.push((
                        path,
                        Argument {
                            value: declared_value(self.program, value, reference)?,
                            reference,
                            backing: Some(place.clone()),
                        },
                    ));
                }
            }
            Some(result)
        } else {
            self.expression(frame, path, expression, 0)?
                .into_iter()
                .map(|(path, value)| {
                    Some((
                        path,
                        Argument {
                            value: declared_value(self.program, value, reference)?,
                            reference,
                            backing: None,
                        },
                    ))
                })
                .collect()
        }
    }

    fn storage_place(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<Vec<(Path, Place)>> {
        self.spend()?;
        if depth >= 64 {
            return None;
        }
        let machine = &self.program.machines()[frame.machine];
        let state = &self.program.machine_states(machine)[frame.state];
        if !crate::value_custody::places::place_has_builtin_coordinates(
            self.program,
            machine,
            Some(state),
            expression,
        ) {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(name)
                if name.symbol == name.head_symbol
                    && self
                        .program
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                let binding = frame
                    .storage
                    .iter()
                    .rev()
                    .find(|binding| binding.symbol == name.symbol)?;
                Some(vec![(path, binding.place.clone())])
            }
            ExpressionNode::Borrow(borrow) if borrow.access.is_readable() => {
                self.storage_place(frame, path, borrow.target, depth + 1)
            }
            ExpressionNode::Member(member) => {
                let symbol = crate::value_custody::places::effective_member_symbol(
                    self.program,
                    member.receiver,
                    member,
                );
                let mut result = Vec::new();
                for (path, place) in self.storage_place(frame, path, member.receiver, depth + 1)? {
                    for (path, receiver) in self.storage_value(path, &place)? {
                        let Value::Record { reference, .. } = receiver else {
                            return None;
                        };
                        // Reading authenticates the exact common declaration
                        // field and rejects unrepresented stored references.
                        let (path, _) = self.record_member(path, receiver, symbol)?;
                        let reference = crate::project_type_reference_from_segments(
                            self.program,
                            reference,
                            &[facts::PlaceSegment::Field { symbol }],
                        )?;
                        let mut place = place.clone();
                        place.projection.push(Projection::Field(symbol));
                        place.reference = reference;
                        result.push((path, place));
                    }
                }
                Some(result)
            }
            ExpressionNode::Indexed(indexed) => {
                let mut result = Vec::new();
                for (path, place) in
                    self.storage_place(frame, path, indexed.collection, depth + 1)?
                {
                    for (path, collection) in self.storage_value(path, &place)? {
                        let Value::Collection {
                            length, element, ..
                        } = collection
                        else {
                            return None;
                        };
                        for (path, selector) in
                            self.expression(frame, path, indexed.index, depth + 1)?
                        {
                            let Value::Integer {
                                value: selector, ..
                            } = selector
                            else {
                                return None;
                            };
                            self.storage_index(&path, &selector, &length)?;
                            let mut place = place.clone();
                            place.projection.push(Projection::Element(selector));
                            place.reference = element;
                            result.push((path, place));
                        }
                    }
                }
                Some(result)
            }
            _ => None,
        }
    }

    fn storage_index(&self, path: &Path, selector: &Integer, length: &Integer) -> Option<()> {
        (self.implies(
            path,
            &compare(BinaryOperator::GreaterOrEqual, selector.clone(), number(0)),
        ) && self.implies(
            path,
            &compare(BinaryOperator::Less, selector.clone(), length.clone()),
        ))
        .then_some(())
    }

    pub(super) fn storage_value(
        &mut self,
        path: Path,
        place: &Place,
    ) -> Option<Vec<(Path, Value)>> {
        let value = path
            .storage
            .iter()
            .find(|home| home.identity == place.home)?
            .value
            .clone();
        let mut rows = vec![(path, value)];
        for projection in &place.projection {
            let mut next = Vec::new();
            for (path, value) in rows {
                match projection {
                    Projection::Field(symbol) => {
                        next.push(self.record_member(path, value, *symbol)?)
                    }
                    Projection::Element(selector) => {
                        let Value::Collection {
                            identity,
                            length,
                            element,
                        } = value
                        else {
                            return None;
                        };
                        self.storage_index(&path, selector, &length)?;
                        next.extend(self.read(path, identity, selector.clone(), element, 0)?);
                    }
                }
            }
            rows = next;
        }
        rows.into_iter()
            .map(|(path, value)| {
                Some((path, declared_value(self.program, value, place.reference)?))
            })
            .collect()
    }

    pub(super) fn assignment(
        &mut self,
        frame: &Frame,
        path: Path,
        target: ExpressionHandle,
        source: ExpressionHandle,
    ) -> Option<Vec<(Frame, Path)>> {
        // This judgment interprets element stores. Replacing a reference
        // carrier is a different storage operation; it cannot be interpreted
        // as replacing that reference's current referent contents.
        if !matches!(
            self.program.expression_table.expression(target),
            ExpressionNode::Indexed(_)
        ) {
            return None;
        }
        let mut result = Vec::new();
        for (path, place) in self.storage_place(frame, path, target, 0)? {
            for (path, value) in self.expression(frame, path, source, 0)? {
                let replacement = declared_value(self.program, value, place.reference)?;
                let current = path
                    .storage
                    .iter()
                    .find(|home| home.identity == place.home)?
                    .value
                    .clone();
                for (mut path, value) in
                    self.replace_storage(path, current, &place.projection, &replacement)?
                {
                    path.storage
                        .iter_mut()
                        .find(|home| home.identity == place.home)?
                        .value = value;
                    result.push((frame.clone(), path));
                }
            }
        }
        Some(result)
    }

    fn replace_storage(
        &mut self,
        path: Path,
        current: Value,
        projection: &[Projection],
        replacement: &Value,
    ) -> Option<Vec<(Path, Value)>> {
        self.spend()?;
        let Some((first, rest)) = projection.split_first() else {
            return Some(vec![(path, replacement.clone())]);
        };
        let mut result = Vec::new();
        match first {
            Projection::Field(symbol) => {
                let (path, child) = self.record_member(path, current.clone(), *symbol)?;
                for (path, value) in self.replace_storage(path, child, rest, replacement)? {
                    result.push((path, self.record_store(current.clone(), *symbol, value)?));
                }
            }
            Projection::Element(selector) => {
                let Value::Collection {
                    identity: previous,
                    length,
                    element,
                } = current
                else {
                    return None;
                };
                self.storage_index(&path, selector, &length)?;
                // A nested element update needs its exact prior contents. A
                // leaf store does not read the overwritten element.
                let rows = if rest.is_empty() {
                    vec![(path, replacement.clone())]
                } else {
                    let mut rows = Vec::new();
                    for (path, child) in self.read(path, previous, selector.clone(), element, 0)? {
                        rows.extend(self.replace_storage(path, child, rest, replacement)?);
                    }
                    rows
                };
                for (path, value) in rows {
                    let identity = self.next_identity;
                    self.next_identity += 1;
                    self.updates.push(CollectionUpdate {
                        identity,
                        previous,
                        selector: selector.clone(),
                        value,
                    });
                    result.push((
                        path,
                        Value::Collection {
                            identity,
                            length: length.clone(),
                            element,
                        },
                    ));
                }
            }
        }
        Some(result)
    }
}
