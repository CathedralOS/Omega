//! Authenticate logical storage snapshots against invocation entry or an exact
//! call's normal return. Names alone establish no value or type guarantee.

use terminal_check_tables::values::ValueOrigin;

use super::*;

impl Storage<'_> {
    /// The callee establishes only its result and the exact subtrees borrowed
    /// exclusively. A caller's enclosing record is outside that guarantee.
    pub(super) fn call_content_boundary(
        &self,
        call: OperationId,
        leaf: &Lineage,
        concerns: Construct,
    ) -> Checked<Lineage> {
        let operation = self.operation(call, concerns)?;
        let callee = operation.kind.declared_invocation().ok_or_else(|| {
            self.foreign(concerns, "a storage snapshot is not after a declared call")
        })?;
        if let Some(result) = operation.result.as_structural()
            && result.place == leaf.root
            && matches!(self.root_kind(leaf.root, concerns)?, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == call && structural_type == result.structural_type)
        {
            return Ok(Lineage {
                root: leaf.root,
                segments: Vec::new(),
            });
        }
        let formals = self
            .module
            .callable_structural_parameters(callee.callee)
            .ok_or_else(|| {
                self.foreign(
                    concerns,
                    "a storage snapshot's callee has no structural telescope",
                )
            })?;
        let arguments = operation.kind.structural_arguments();
        if formals.len() != arguments.len() {
            return Err(self.foreign(
                concerns,
                "a storage snapshot's call has another structural arity",
            ));
        }
        let mut found = None;
        for (formal, actual) in formals.iter().zip(arguments) {
            if !matches!(
                actual.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                continue;
            }
            let captured = self.argument(actual, concerns)?;
            if captured.root != leaf.root || !leaf.segments.starts_with(&captured.segments) {
                continue;
            }
            if actual.access != formal.access {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has another formal access",
                ));
            }
            let matches_type = match self.shape(&captured, concerns)? {
                Shape::Typed(actual) => actual == formal.structural_type,
                Shape::Scalar(actual) => {
                    matches!(self.declaration(formal.structural_type, concerns)?.shape,
                    StructuralTypeShape::PrimitiveScalar(declared) if declared == actual)
                }
                Shape::View | Shape::Payload(_) => false,
            };
            if !matches_type {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has another formal type",
                ));
            }
            if found.replace(captured).is_some() {
                return Err(self.foreign(
                    concerns,
                    "a returned storage snapshot has overlapping call owners",
                ));
            }
        }
        found.ok_or_else(|| {
            self.foreign(
                concerns,
                "a returned storage snapshot is outside the call's established storage",
            )
        })
    }

    pub(super) fn initial_contents_are_declared(&self, tables: &MachineTables) -> Checked<()> {
        for (value, row) in tables.values.iter() {
            let ValueOrigin::StorageContent { place, after } = row.origin else {
                continue;
            };
            let concerns = Construct::Value(value);
            let leaf = lineage(self.machine, &tables.places, place)?;
            let supported = match after {
                None => matches!(
                    self.root_kind(leaf.root, concerns)?,
                    StructuralPlaceKind::Parameter { .. }
                ),
                Some(call) => {
                    self.call_content_boundary(call, &leaf, concerns)?;
                    true
                }
            };
            if !supported
                || !matches!(self.shape(&leaf, concerns)?, Shape::Scalar(declared) if declared == row.scalar_type)
            {
                return Err(self.foreign(concerns,
                    "an initial content value of storage that is no scalar leaf of a formal or declared call result"));
            }
        }
        Ok(())
    }

    /// Type validity is a call guarantee even without an authored `ensures`.
    /// Substitute only types inside each authenticated returned subtree, and
    /// use its post-call versions, never the caller's earlier observations.
    pub(crate) fn returned_type_premises(
        &self,
        tables: &MachineTables,
        operation: &Operation,
    ) -> Checked<Vec<Proposition>> {
        let Some(values) = self.returned_contents.get(index(operation.id.get())) else {
            return Ok(Vec::new());
        };
        let concerns = Construct::Operation(operation.id);
        let mut owners = Vec::new();
        for (place, _) in values {
            let leaf = lineage(self.machine, &tables.places, *place)?;
            let captured = self.call_content_boundary(operation.id, &leaf, concerns)?;
            for length in captured.segments.len()..leaf.segments.len() {
                owners.push(Lineage {
                    root: leaf.root,
                    segments: leaf.segments[..length].to_vec(),
                });
            }
        }
        owners
            .sort_by(|left, right| (left.root, &left.segments).cmp(&(right.root, &right.segments)));
        owners.dedup();
        let (block, up_to) = self
            .graph
            .locate(ProgramPoint::AfterOperation(operation.id))?;
        let mut premises = Vec::new();
        for owner in owners {
            let Shape::Typed(data) = self.shape(&owner, concerns)? else {
                continue;
            };
            for clause in self.clauses_of(data, concerns)? {
                let mut held = Held {
                    storage: self,
                    tables,
                    place: &owner,
                    block,
                    up_to,
                    concerns,
                };
                match crate::substitute(clause, &mut held) {
                    Ok(Proposition::Truth | Proposition::Falsehood) => {}
                    Ok(premise) => premises.push(premise),
                    Err(crate::Unbound::Unproved(_)) => {}
                    Err(unbound) => return Err(unbound.refusal(self.machine, concerns)),
                }
            }
        }
        Ok(premises)
    }
}
