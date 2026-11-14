/*
The loans of one machine: which `Loan` rows the lowered form owes, and
whether each loan may be formed where it is.

A reference can keep a loan live after the operation that formed it. The
existing descriptor flow supplies its transported homes and lifetime; all
reads, writes and later loans resolve the same captured backing. Missing
immediate-parent or returned-call correspondence remains unavailable rather
than being treated as a call-duration loan. A checked call result retains its
actual input loan across every transported result descriptor.

A call-duration loan is a call's structural argument with a borrowed access: the call lends
the place the argument names, for the call's duration, and the place is the
caller's again when the call returns (`wiki/spec/terminal-psi/loans.md`:
"Loan resource | Exact occurrence, owner lineage, captured place, access,
parent lifetime, and restoration obligation."). The occurrence is the call,
the captured place is the argument's place, the access is the argument's,
and the lineage is the root's: a borrowed formal's root reborrows the loan
the invocation passed in (`Lender::Incoming`), any other root lends owned
storage (`Lender::Owner`). A duration-only loan ends when the call returns,
so it is live on no edge and no edge disposes of it. A loan formed
for a call with no normal return is carried into that terminal transfer
(`wiki/spec/language/state_contracts.md`: "A caller-local loan carried into
a terminal machine call closes at the last use of that loan and of every
loan derived from it, even when that use is inside the callee's terminal
chain"): the transfer's edge consumes it, and nothing of this machine runs
after to use the lent place again.

The rows are recomputed from the lowered form, as every other row is
(`minted`): one `Loan` per borrowed argument of each call, created at the
point before the call, whose captured place is the invocation row's actual
at that position and is the place the argument's storage path names. A row
the lowered form does not owe, or owes elsewhere or of another kind, is a
defect of the route (`ForeignIdentity`).

Whether a loan may be formed is a resource judgment, not a proposition
(loans.md: "Loan descent, access attenuation, temporal containment and
restoration are resource judgments. Logical facts cannot create, amplify,
transfer, extend, return, consume or duplicate authority."), so it has no
certificate, as the initialization judgment has none. Two refusals, each
`RefusalReason::Conflict`, a compile error of the program:

- Two loans of one call whose places may share storage, when either is
  exclusive. The places' overlap is the one relation of `places::overlap`:
  two different fields of one record are disjoint, anything else beneath one
  root may share storage. ownership.md, "Borrows and aliases": "exclusive
  loans must not overlap incompatibly"; loans.md: "A live exclusive loan does
  not prove projections disjoint."
- An exclusive reborrow of a place beneath a borrowed formal declared
  shared. loans.md, "Reborrow lineage and access": a `Read` parent rejects a
  mutable child.
*/

use language_core::ReferenceAccess;
use semantic_vocabulary::StructuralPlaceKind;
use semantic_vocabulary::{InvocationSite, ObligationId};
use terminal_check_tables::edges::{Actual, Fate};
use terminal_check_tables::obligations::{Lender, ObligationKind};
use terminal_check_tables::{
    Checked, Construct, InvocationId, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::StructuralAccess;

use super::{Storage, index, lineage, overlap};

/// Why two loans of one call may not both be formed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanConflict {
    /// Both loans are exclusive.
    TwiceExclusive,
    /// One is exclusive and the other shared.
    ExclusiveAndShared,
    /// An exclusive loan of storage this machine holds only shared.
    ExclusiveOfShared,
    /// Observation bypasses a live exclusive descriptor.
    ObserveWhileExclusive,
    /// A write or move conflicts with a retained loan.
    ChangeWhileBorrowed,
    /// A parent is disposed before its children close.
    ReleaseBeforeChildren,
    /// The actual projected source does not grant the requested access.
    AccessExceedsParent,
}

/// A loan that may not be formed: its row, optional call invocation and
/// argument position. A persistent reference has its own formation operation
/// and no fabricated invocation. `with` names an earlier conflicting position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictingLoan {
    pub loan: ObligationId,
    pub invocation: Option<InvocationId>,
    pub position: usize,
    pub with: Option<usize>,
    pub conflict: LoanConflict,
    pub at: Construct,
}

/// One loan the lowered form forms, as recomputed from its actual occurrence.
struct Formed {
    obligation: ObligationId,
    invocation: Option<InvocationId>,
    position: usize,
    access: ReferenceAccess,
    lineage: super::Lineage,
    /// For a reborrow, the access of the formal it reborrows.
    parent: Option<ReferenceAccess>,
    lender: Lender,
    from: Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation>,
    /// Derived at this formation from the canonical source path, never a
    /// supplied acceptance marker or a property of a descriptor's shape alone.
    permitted: bool,
    /// The exact formation occurrence and its transported descriptor lifetime.
    /// A call-duration argument has no retained descriptor after its call.
    at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    lifetime: Option<terminal_semantics::descriptor_lifetimes::DescriptorLifetime>,
}

fn reference_access(access: StructuralAccess) -> Option<ReferenceAccess> {
    match access {
        StructuralAccess::SharedBorrow => Some(ReferenceAccess::Shared),
        StructuralAccess::MutableBorrow => Some(ReferenceAccess::Mutable),
        StructuralAccess::WriteOnlyBorrow => Some(ReferenceAccess::WriteOnly),
        StructuralAccess::Owned => None,
    }
}

/// One canonical descriptor closure per machine judgment. If the module has
/// no reference-bearing storage there is no reference work; otherwise a missing closure
/// is an explicit refusal, never an empty approximation.
pub(crate) fn reference_graph(
    module: &terminal_psi::TerminalModule,
    machine: semantic_vocabulary::MachineId,
) -> Checked<Vec<terminal_semantics::descriptor_results::DescriptorMachine>> {
    let lowered = module.machine(machine).ok_or_else(|| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Machine,
            "reference closure has no exact machine",
        )
    })?;
    let contains = |root| {
        terminal_psi::structural_type_contains_reference_by(|id| module.structural_type(id), root)
    };
    let demanded = lowered
        .result
        .structural()
        .is_some_and(|result| contains(result.structural_type))
        || lowered
            .structural_parameters
            .iter()
            .any(|parameter| contains(parameter.structural_type))
        || lowered.blocks.iter().any(|block| {
            block
                .structural_parameters
                .iter()
                .any(|parameter| contains(parameter.structural_type))
                || block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        terminal_psi::OperationKind::EstablishReference { .. }
                            | terminal_psi::OperationKind::ReleaseReference { .. }
                    ) || operation
                        .result
                        .as_structural()
                        .is_some_and(|result| contains(result.structural_type))
                })
        });
    if !demanded {
        return Ok(Vec::new());
    }
    terminal_semantics::descriptor_results::terminal_borrowed_storage_graph_from(
        module,
        [machine],
        &[],
    )
    .map_err(|reason| {
        crate::refusal(
            RefusalReason::Unsupported,
            machine,
            Construct::Machine,
            reason,
        )
    })
}

impl Storage<'_> {
    /// The access a root's storage is held with here, when it is a borrowed
    /// formal: the declared access of its structural parameter.
    fn incoming(&self, root: semantic_vocabulary::PlaceId) -> Option<StructuralAccess> {
        self.lowered
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == root)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
            .map(|parameter| parameter.access)
    }

    /// A permission crossing retains the descriptor's immediate resource;
    /// owning that descriptor never changes the backing into owned storage.
    fn source_lender(
        &self,
        raw: &super::Lineage,
        access: ReferenceAccess,
        concerns: Construct,
    ) -> Checked<(
        Option<Lender>,
        Option<ReferenceAccess>,
        Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation>,
        bool,
    )> {
        use terminal_check_tables::bindings::Segment;
        if let Some(crossing) = raw
            .segments
            .iter()
            .rposition(|step| *step == Segment::Referent)
        {
            let descriptor = super::Lineage {
                root: raw.root,
                segments: raw.segments[..crossing].to_vec(),
            };
            let super::Shape::Typed(row) = self.shape(&descriptor, concerns)? else {
                return Err(self.foreign(concerns, "a reborrow has no parent reference descriptor"));
            };
            let terminal_psi::StructuralTypeShape::Reference { access: parent, .. } =
                self.declaration(row, concerns)?.shape
            else {
                return Err(self.foreign(concerns, "a reborrow crosses another value constructor"));
            };
            let descriptor = descriptor.descriptor_location().ok_or_else(|| {
                self.unsupported(
                    concerns,
                    "reference parent has no exact static descriptor path",
                )
            })?;
            let path = raw
                .descriptor_location()
                .and_then(|location| {
                    terminal_semantics::descriptor_lifetimes::descriptor_path(
                        self.types,
                        self.lowered.structural_place_type(raw.root)?,
                        &location.path,
                    )
                })
                .ok_or_else(|| {
                    self.unsupported(concerns, "reborrow has no complete typed source path")
                })?;
            let (multiplicity, authority) =
                if let Some(parameter) = self.lowered.structural_parameter(raw.root) {
                    (parameter.multiplicity, parameter.access)
                } else if let StructuralPlaceKind::OperationResult { producer, .. } =
                    self.root_kind(raw.root, concerns)?
                {
                    let result = self
                        .operation(producer, concerns)?
                        .result
                        .as_structural()
                        .filter(|result| result.place == raw.root)
                        .ok_or_else(|| {
                            self.foreign(
                                concerns,
                                "reference parent lost its actual producing result",
                            )
                        })?;
                    (result.multiplicity, StructuralAccess::Owned)
                } else {
                    return Err(self.unsupported(
                        concerns,
                        "reference parent has no supported storage authority",
                    ));
                };
            let requested = match access {
                ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
            };
            let permitted = terminal_semantics::reference_source_projection_accessible(
                self.types.iter(),
                self.root_type(raw.root, concerns)?,
                multiplicity,
                authority,
                &path,
                requested,
            );
            return Ok((None, reference_access(parent), Some(descriptor), permitted));
        }
        if let Some(parent) = self.incoming(raw.root) {
            return Ok((Some(Lender::Incoming), reference_access(parent), None, true));
        }
        match self.root_kind(raw.root, concerns)? {
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::OperationResult { .. } => {
                Ok((Some(Lender::Owner), None, None, true))
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } if access == ReferenceAccess::Shared
                && self
                    .declaration(structural_type, concerns)?
                    .shape
                    .is_shared_readable_byte_view() =>
            {
                Ok((Some(Lender::Owner), None, None, true))
            }
            _ => Err(self.unsupported(concerns, "loan source has no supported owning resource")),
        }
    }

    /// The loans one call forms: one per borrowed structural argument, in
    /// argument order, each claiming its `Loan` row created at `before`.
    /// `call` is the invocation site and callee when the construct is a
    /// call; anything else may not lend.
    #[allow(clippy::too_many_arguments)]
    fn lent(
        &self,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
        (concerns, before): (Construct, ProgramPoint),
        arguments: &[terminal_psi::StructuralArgument],
        call: Option<(InvocationSite, terminal_psi::TargetMachineCallable)>,
        retained: Vec<(
            usize,
            terminal_semantics::descriptor_lifetimes::DescriptorLifetime,
        )>,
        claimed: &mut [bool],
        formed: &mut Vec<Formed>,
    ) -> Checked<()> {
        if arguments
            .iter()
            .all(|argument| argument.access == StructuralAccess::Owned)
        {
            if !retained.is_empty() {
                return Err(self.foreign(concerns, "retained result has no actual input loan"));
            }
            return Ok(());
        }
        let Some((site, callee)) = call else {
            return Err(self.unsupported(concerns, "a loan to something not a call"));
        };
        let formals = module
            .callable_structural_parameters(callee)
            .ok_or_else(|| self.foreign(concerns, "call has no canonical structural formals"))?;
        let (invocation, row) = tables
            .invocations
            .iter()
            .find(|(_, row)| row.site == site)
            .ok_or_else(|| self.foreign(concerns, "a call has no invocation row"))?;
        // The structural lane's places, in argument order.
        let places: Vec<_> = row
            .actuals
            .iter()
            .enumerate()
            .filter_map(|(position, actual)| match actual {
                Actual::Place(place) => Some((position, *place)),
                Actual::Value(_) => None,
            })
            .collect();
        if places.len() != arguments.len() || formals.len() != arguments.len() {
            return Err(self.foreign(
                concerns,
                "an invocation row's places are not the call's structural arguments",
            ));
        }
        let mut retained = retained.into_iter().peekable();
        for (structural_position, ((argument, (position, place)), parameter)) in
            arguments.iter().zip(places).zip(formals).enumerate()
        {
            let Some(access) = reference_access(argument.access) else {
                continue;
            };
            if access == ReferenceAccess::WriteOnly {
                return Err(self.unsupported(concerns, "a write-only loan"));
            }
            // Terminal validation passes a borrowed argument only to a
            // formal declared with the same access.
            if parameter.access != argument.access {
                return Err(self.foreign(
                    concerns,
                    "a loan's access is not its formal's declared access",
                ));
            }
            let lent = self.argument(argument, concerns)?;
            if lineage(self.machine, &tables.places, place)? != lent {
                return Err(self.foreign(
                    concerns,
                    "an invocation row's place is not the place the call lends",
                ));
            }
            let (expected_lender, parent, from, permitted) =
                self.source_lender(&lent, access, concerns)?;
            let (obligation, row) = tables
                .obligations
                .iter()
                .find(|(id, row)| {
                    matches!(row.kind, ObligationKind::Loan { captured, access: borrowed, .. }
                    if captured == place && borrowed == access)
                        && row.created_at == before
                        && !claimed[index(id.get())]
                })
                .ok_or_else(|| {
                    self.foreign(
                        concerns,
                        "a loan the call forms has no exact occurrence row",
                    )
                })?;
            let ObligationKind::Loan { lender, .. } = row.kind else {
                unreachable!()
            };
            if expected_lender.is_some_and(|expected| expected != lender) {
                return Err(self.foreign(
                    concerns,
                    "loan lender does not match its actual source resource",
                ));
            }
            let row = terminal_check_tables::row(
                &tables.obligations,
                obligation,
                self.machine,
                Construct::Obligation(obligation),
            )?;
            if row.created_at != before {
                return Err(self.foreign(
                    Construct::Obligation(obligation),
                    "a loan is not created at the point before its call",
                ));
            }
            claimed[index(obligation.get())] = true;
            let lifetime = if retained
                .peek()
                .is_some_and(|(input, _)| *input == structural_position)
            {
                Some(
                    retained
                        .next()
                        .ok_or_else(|| {
                            self.foreign(concerns, "retained result loan lifetime disappeared")
                        })?
                        .1,
                )
            } else {
                None
            };
            formed.push(Formed {
                obligation,
                invocation: Some(invocation),
                position,
                access,
                lineage: {
                    let (block, before) = self.graph.locate(before)?;
                    self.backing(&lent, block, before, concerns)?
                },
                parent,
                lender,
                from,
                permitted,
                at: self.loan_position(before, concerns)?,
                lifetime,
            });
        }
        if retained.next().is_some() {
            return Err(self.foreign(
                concerns,
                "returned reference lifetime has no borrowed argument occurrence",
            ));
        }
        Ok(())
    }

    fn loan_position(
        &self,
        point: ProgramPoint,
        concerns: Construct,
    ) -> Checked<terminal_semantics::descriptor_lifetimes::DescriptorPosition> {
        let (block, position) = self.graph.locate(point)?;
        let length = self
            .lowered
            .blocks
            .iter()
            .find(|row| row.id == block)
            .ok_or_else(|| self.foreign(concerns, "loan has no block"))?
            .operations
            .len();
        let position = u32::try_from(position.min(length))
            .map_err(|_| self.foreign(concerns, "loan occurrence exceeds its descriptor graph"))?;
        Ok((block, position))
    }

    #[allow(clippy::too_many_arguments)]
    fn reference_loan(
        &self,
        tables: &MachineTables,
        operation: &terminal_psi::Operation,
        before: ProgramPoint,
        flow: Option<
            &std::collections::BTreeMap<
                terminal_semantics::descriptor_lifetimes::DescriptorPosition,
                terminal_semantics::descriptor_lifetimes::DescriptorFlowStep,
            >,
        >,
        claimed: &mut [bool],
        formed: &mut Vec<Formed>,
    ) -> Checked<()> {
        use terminal_psi::{OperationKind, StructuralTypeShape};
        use terminal_semantics::descriptor_lifetimes::{
            DescriptorLocation, descriptor_location_lifetime,
        };
        let OperationKind::EstablishReference { source, storage } = &operation.kind else {
            return Ok(());
        };
        let concerns = Construct::Operation(operation.id);
        if storage.is_some() {
            return Err(self.unsupported(
                concerns,
                "a reference representation view without its checked storage correspondence",
            ));
        }
        let result = operation.result.as_structural().ok_or_else(|| {
            self.foreign(concerns, "reference formation has no descriptor result")
        })?;
        let StructuralTypeShape::Reference {
            referent,
            access: declared,
        } = self.declaration(result.structural_type, concerns)?.shape
        else {
            return Err(self.foreign(concerns, "reference formation produces another constructor"));
        };
        if source.access != declared {
            return Err(self.foreign(concerns, "reference formation changes its source access"));
        }
        let access = reference_access(source.access)
            .ok_or_else(|| self.foreign(concerns, "reference formation has no borrowed access"))?;
        if access == ReferenceAccess::WriteOnly {
            return Err(self.unsupported(concerns, "a write-only persistent loan"));
        }
        let raw = self.argument(source, concerns)?;
        let expected = self.declaration(referent, concerns)?;
        let compatible = match self.shape(&raw, concerns)? {
            super::Shape::Scalar(scalar) => {
                expected.shape == StructuralTypeShape::PrimitiveScalar(scalar)
            }
            super::Shape::Typed(actual) => actual == referent,
            super::Shape::View | super::Shape::Payload(_) => false,
        };
        if !compatible {
            return Err(self.foreign(
                concerns,
                "reference formation changes its captured referent type",
            ));
        }
        let captured = self.interned(tables, &raw, concerns)?.ok_or_else(|| {
            self.foreign(concerns, "reference formation has no captured place row")
        })?;
        let (expected_lender, parent, from, permitted) =
            self.source_lender(&raw, access, concerns)?;
        let (obligation, row) = tables
            .obligations
            .iter()
            .find(|(id, row)| {
                matches!(row.kind, ObligationKind::Loan { captured: actual, access: borrowed, .. }
                if actual == captured && borrowed == access)
                    && row.created_at == before
                    && !claimed[index(id.get())]
            })
            .ok_or_else(|| {
                self.foreign(
                    concerns,
                    "reference formation has no exact Loan occurrence row",
                )
            })?;
        let ObligationKind::Loan { lender, .. } = row.kind else {
            unreachable!()
        };
        if expected_lender.is_some_and(|expected| expected != lender) {
            return Err(self.foreign(
                concerns,
                "reference lender does not match its actual source resource",
            ));
        }
        if row.created_at != before {
            return Err(self.foreign(
                concerns,
                "reference loan is not created at its actual formation",
            ));
        }
        let at = self.loan_position(before, concerns)?;
        let flow = flow.ok_or_else(|| {
            self.unsupported(concerns, "persistent loan has no complete descriptor flow")
        })?;
        let lifetime =
            descriptor_location_lifetime(flow, at, &DescriptorLocation::whole(result.place));
        if lifetime.live.contains(&at) {
            return Err(self.unsupported(
                concerns,
                "persistent loan crosses its own formation without a fresh occurrence",
            ));
        }
        claimed[index(obligation.get())] = true;
        formed.push(Formed {
            obligation,
            invocation: None,
            position: 0,
            access,
            lineage: self.backing(&raw, at.0, at.1 as usize, concerns)?,
            parent,
            lender,
            from,
            permitted,
            at,
            lifetime: Some(lifetime),
        });
        Ok(())
    }

    pub(super) fn released_reference(
        &self,
        source: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        concerns: Construct,
    ) -> Checked<super::Lineage> {
        let place = self.stored_at(source, path, concerns)?;
        if self.incoming(source).is_some()
            || place
                .segments
                .contains(&terminal_check_tables::bindings::Segment::Referent)
        {
            return Err(self.unsupported(concerns,
                "reference release through borrowed storage needs its exact restoration and child closure"));
        }
        let super::Shape::Typed(row) = self.shape(&place, concerns)? else {
            return Err(self.foreign(concerns, "reference release names no descriptor"));
        };
        if !matches!(
            self.declaration(row, concerns)?.shape,
            terminal_psi::StructuralTypeShape::Reference { .. }
        ) {
            return Err(self.foreign(
                concerns,
                "reference release names a different value constructor",
            ));
        }
        match self.root_kind(source, concerns)? {
            StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::OperationResult { .. } => {
                Ok(place)
            }
            _ => Err(self.unsupported(
                concerns,
                "reference release without an owned descriptor home",
            )),
        }
    }

    /// A retained loan constrains every use of its backing, not just later
    /// loan formation. Only a live transported descriptor may use an exclusive
    /// loan; a shared loan permits observation but no write through any alias.
    fn uses_retained_loan(
        &self,
        loan: &Formed,
        formed: &[Formed],
    ) -> Checked<Option<(Construct, LoanConflict)>> {
        let Some(lifetime) = &loan.lifetime else {
            return Ok(None);
        };
        for block in &self.lowered.blocks {
            for (position, operation) in block.operations.iter().enumerate() {
                let position = u32::try_from(position).map_err(|_| {
                    self.foreign(
                        Construct::Operation(operation.id),
                        "reference observation position exceeds u32",
                    )
                })?;
                let at = (block.id, position);
                let concerns = Construct::Operation(operation.id);
                if let terminal_psi::OperationKind::ReleaseReference { source, path } =
                    &operation.kind
                {
                    let descriptor = self
                        .released_reference(*source, path, concerns)?
                        .descriptor_location()
                        .ok_or_else(|| {
                            self.unsupported(
                                concerns,
                                "reference release has no exact descriptor identity",
                            )
                        })?;
                    if lifetime
                        .authorized
                        .get(&at)
                        .is_some_and(|names| names.contains(&descriptor))
                    {
                        for child in formed {
                            if child
                                .lifetime
                                .as_ref()
                                .is_some_and(|life| life.live.contains(&at))
                                && self.descends(child, loan.obligation, formed)?
                            {
                                return Ok(Some((concerns, LoanConflict::ReleaseBeforeChildren)));
                            }
                        }
                    }
                }
                if !lifetime.live.contains(&at) {
                    continue;
                }
                for place in self.operation_uses(operation)? {
                    let backing = self.backing(&place, block.id, position as usize, concerns)?;
                    if loan.access.is_exclusive()
                        && overlap(&loan.lineage, &backing)
                        && !self.authorized_reference(loan, &place, at, formed)?
                    {
                        return Ok(Some((concerns, LoanConflict::ObserveWhileExclusive)));
                    }
                }
                for place in self.operation_changes(operation)? {
                    let backing = self.backing(&place, block.id, position as usize, concerns)?;
                    if overlap(&loan.lineage, &backing)
                        && (!loan.access.is_exclusive()
                            || !self.authorized_reference(loan, &place, at, formed)?)
                    {
                        return Ok(Some((concerns, LoanConflict::ChangeWhileBorrowed)));
                    }
                }
            }
        }
        Ok(None)
    }

    fn authorized_reference(
        &self,
        loan: &Formed,
        place: &super::Lineage,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        formed: &[Formed],
    ) -> Checked<bool> {
        let Some(crossing) = place
            .segments
            .iter()
            .rposition(|step| *step == terminal_check_tables::bindings::Segment::Referent)
        else {
            return Ok(false);
        };
        let descriptor = super::Lineage {
            root: place.root,
            segments: place.segments[..crossing].to_vec(),
        }
        .descriptor_location()
        .ok_or_else(|| {
            self.unsupported(
                Construct::Obligation(loan.obligation),
                "reference authorization has no exact descriptor path",
            )
        })?;
        for candidate in formed {
            if candidate.obligation != loan.obligation
                && !self.descends(candidate, loan.obligation, formed)?
            {
                continue;
            }
            if candidate.lifetime.as_ref().is_some_and(|life| {
                life.live.contains(&at)
                    && life
                        .authorized
                        .get(&at)
                        .is_some_and(|names| names.contains(&descriptor))
            }) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn parents(&self, module: &terminal_psi::TerminalModule, formed: &[Formed]) -> Checked<()> {
        for loan in formed {
            let Some(descriptor) = &loan.from else {
                continue;
            };
            let parent = terminal_semantics::reference_loan_parent(
                module,
                self.lowered,
                &mut self.references.borrow_mut(),
                loan.at,
                descriptor,
                formed
                    .iter()
                    .filter(|other| other.obligation != loan.obligation)
                    .filter_map(|other| {
                        other.lifetime.as_ref().map(|life| (other.obligation, life))
                    }),
            )
            .map_err(|reason| self.unsupported(Construct::Obligation(loan.obligation), reason))?;
            if loan.lender != parent.map_or(Lender::Incoming, Lender::Loan) {
                return Err(self.foreign(
                    Construct::Obligation(loan.obligation),
                    "reborrow names a different immediate parent occurrence",
                ));
            }
            if let Some(parent) = parent {
                let owner = formed
                    .iter()
                    .find(|other| other.obligation == parent)
                    .ok_or_else(|| {
                        self.foreign(
                            Construct::Obligation(loan.obligation),
                            "reborrow parent has no actual formation",
                        )
                    })?;
                if owner.lineage.root != loan.lineage.root
                    || !loan.lineage.segments.starts_with(&owner.lineage.segments)
                {
                    return Err(self.foreign(
                        Construct::Obligation(loan.obligation),
                        "reborrow changes its parent's original captured referent",
                    ));
                }
            }
        }
        Ok(())
    }

    fn descends(&self, child: &Formed, parent: ObligationId, formed: &[Formed]) -> Checked<bool> {
        let mut at = child;
        for _ in 0..formed.len() {
            let Lender::Loan(identity) = at.lender else {
                return Ok(false);
            };
            if identity == parent {
                return Ok(true);
            }
            at = formed
                .iter()
                .find(|row| row.obligation == identity)
                .ok_or_else(|| {
                    self.foreign(
                        Construct::Obligation(child.obligation),
                        "loan ancestry names no actual parent occurrence",
                    )
                })?;
        }
        Err(self.foreign(
            Construct::Obligation(child.obligation),
            "loan ancestry does not reach an original lender",
        ))
    }

    /// The loans a call with no normal return carries into its transfer are
    /// exactly the obligations its edge consumes, each once, and the edge
    /// disposes of nothing else. ownership.md: "every incoming owned
    /// obligation occurs exactly once in the edge's transfer map, explicit
    /// terminal consumption, eligible automatic cleanup, or validated
    /// no-code affine discard."
    fn carried(
        &self,
        tables: &MachineTables,
        edge: semantic_vocabulary::EdgeId,
        loans: &[Formed],
    ) -> Checked<()> {
        let concerns = Construct::Edge(edge);
        let row = terminal_check_tables::row(&tables.edges, edge, self.machine, concerns)?;
        let consumed = |loan: &Formed| {
            row.dispositions.iter().any(|disposition| {
                disposition.obligation == loan.obligation && disposition.fate == Fate::Consumed
            })
        };
        if row.dispositions.len() != loans.len() || !loans.iter().all(consumed) {
            return Err(self.foreign(
                concerns,
                "a no-normal call's edge does not consume exactly the loans it carries",
            ));
        }
        Ok(())
    }

    /// Recomputes every loan from the lowered form and refuses `tables`
    /// unless its `Loan` rows are exactly those, each once. Returns them in
    /// block and operation order, each call's loans in argument order.
    fn formed(
        &self,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
    ) -> Checked<Vec<Formed>> {
        let mut claimed = vec![false; tables.obligations.len()];
        let mut formed = Vec::new();
        let flow = if self
            .lowered
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishReference { .. }
                )
            }) {
            Some(
                self.references
                    .borrow_mut()
                    .flow(self.machine)
                    .ok_or_else(|| {
                        self.unsupported(
                            Construct::Machine,
                            "persistent loans have no complete descriptor flow",
                        )
                    })?,
            )
        } else {
            None
        };
        for block in &self.lowered.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                self.reference_loan(
                    tables,
                    operation,
                    before,
                    flow.as_ref(),
                    &mut claimed,
                    &mut formed,
                )?;
                let arguments = operation.kind.structural_arguments();
                let call = operation
                    .kind
                    .declared_invocation()
                    .map(|inputs| (InvocationSite::Operation(operation.id), inputs.callee));
                let retained = terminal_semantics::reference_call_lifetimes(
                    module,
                    self.lowered,
                    operation,
                    &mut self.references.borrow_mut(),
                )
                .map_err(|reason| self.unsupported(concerns, reason))?;
                self.lent(
                    module,
                    tables,
                    (concerns, before),
                    arguments,
                    call,
                    retained,
                    &mut claimed,
                    &mut formed,
                )?;
                before = ProgramPoint::AfterOperation(operation.id);
            }
            // A terminator that is a call lends as an operation does; the
            // loans of one that never returns are disposed of on its edge.
            if let Some((site, inputs)) = block.terminator.declared_invocation() {
                let first = formed.len();
                self.lent(
                    module,
                    tables,
                    (Construct::Block(block.id), before),
                    inputs.structural_arguments,
                    Some((site, inputs.callee)),
                    Vec::new(),
                    &mut claimed,
                    &mut formed,
                )?;
                self.carried(tables, block.terminator.edge(), &formed[first..])?;
            }
        }
        for (obligation, row) in tables.obligations.iter() {
            if matches!(row.kind, ObligationKind::Loan { .. }) && !claimed[index(obligation.get())]
            {
                return Err(self.foreign(
                    Construct::Obligation(obligation),
                    "a loan row that no call or reference formation owns",
                ));
            }
        }
        Ok(formed)
    }

    /// The advertised return paths must be the paths the body actually
    /// returns. A header cannot turn a local borrow into caller-owned storage.
    fn reference_results(&self, module: &terminal_psi::TerminalModule) -> Checked<()> {
        let Some(result) = self.lowered.result.structural().filter(|result| {
            terminal_psi::structural_type_contains_reference_by(
                |id| module.structural_type(id),
                result.structural_type,
            )
        }) else {
            return Ok(());
        };
        let concerns = Construct::Machine;
        let slots = self
            .references
            .borrow_mut()
            .result_slots(self.machine)
            .ok_or_else(|| {
                self.unsupported(
                    concerns,
                    "reference result has no complete actual return-source correspondence",
                )
            })?;
        let actual = terminal_semantics::reference_result_sources(
            self.types,
            &self.lowered.structural_parameters,
            result,
            &slots,
        )
        .map_err(|reason| self.unsupported(concerns, reason))?;
        if actual != result.reference_sources {
            return Err(self.foreign(
                concerns,
                "reference result origins do not match its actual returned backing",
            ));
        }
        Ok(())
    }

    /// Returning a child keeps its incoming backing, but cannot leave an
    /// intermediate local parent or another child undisposed. Check each
    /// actual return against the existing occurrence/cohort judgment; names
    /// without a proved disposition remain unavailable.
    fn reference_completions(
        &self,
        module: &terminal_psi::TerminalModule,
        formed: &[Formed],
    ) -> Checked<()> {
        use terminal_semantics::ReferenceCompletionOccurrence;
        use terminal_semantics::descriptor_lifetimes::{descriptor_location, descriptor_path};
        let result = self.lowered.result.structural().filter(|result| {
            terminal_psi::structural_type_contains_reference_by(
                |id| module.structural_type(id),
                result.structural_type,
            )
        });
        if result.is_none() && formed.iter().all(|loan| loan.lifetime.is_none()) {
            return Ok(());
        }
        for block in &self.lowered.blocks {
            let (source, cleanup_actions) = match &block.terminator {
                terminal_psi::Terminator::ReturnStructural {
                    source,
                    cleanup_actions,
                    ..
                } => (Some(*source), cleanup_actions),
                terminal_psi::Terminator::Return {
                    cleanup_actions, ..
                }
                | terminal_psi::Terminator::ReturnUnit {
                    cleanup_actions, ..
                } => (None, cleanup_actions),
                _ => continue,
            };
            let concerns = Construct::Block(block.id);
            let at = (
                block.id,
                u32::try_from(block.operations.len()).map_err(|_| {
                    self.foreign(concerns, "reference completion position overflows")
                })?,
            );
            for loan in formed {
                if loan
                    .lifetime
                    .as_ref()
                    .and_then(|lifetime| lifetime.authorized.get(&at))
                    .is_some_and(|names| {
                        names.iter().any(|name| self.incoming(name.root).is_some())
                    })
                {
                    return Err(self.unsupported(concerns,
                        "a reference retained in caller storage needs its checked output-loan correspondence"));
                }
            }
            let mut returned = Vec::new();
            if let Some((source, result)) = source.zip(result) {
                for mapping in &result.reference_sources {
                    let descriptor = descriptor_location(
                        self.types,
                        source,
                        result.structural_type,
                        &mapping.path,
                        None,
                    )
                    .ok_or_else(|| {
                        self.foreign(
                            concerns,
                            "returned reference has no actual result descriptor",
                        )
                    })?;
                    if let Some(identity) = terminal_semantics::reference_loan_parent(
                        module,
                        self.lowered,
                        &mut self.references.borrow_mut(),
                        at,
                        &descriptor,
                        formed.iter().filter_map(|loan| {
                            loan.lifetime.as_ref().map(|life| (loan.obligation, life))
                        }),
                    )
                    .map_err(|reason| self.unsupported(concerns, reason))?
                    {
                        returned.push(identity);
                    }
                }
            }
            if returned.is_empty() {
                // Incoming occurrences were authenticated above. With no
                // local result loan or supported caller-storage output,
                // ordinary loan and affine cleanup own the local remainder.
                continue;
            }
            let occurrences = formed
                .iter()
                .map(|loan| {
                    let is_returned = returned.contains(&loan.obligation);
                    let disposed = if is_returned {
                        false
                    } else if let Some(lifetime) = &loan.lifetime {
                        lifetime
                            .authorized
                            .get(&at)
                            .filter(|names| !names.is_empty())
                            .is_some_and(|names| {
                                names.iter().all(|name| {
                                    self.lowered
                                        .structural_place_type(name.root)
                                        .and_then(|root| {
                                            descriptor_path(self.types, root, &name.path)
                                        })
                                        .is_some_and(|path| {
                                            terminal_semantics::cleanup_disposes_reference(
                                                cleanup_actions,
                                                name.root,
                                                &path,
                                            )
                                        })
                                })
                            })
                    } else {
                        // Its normal call completion closes a duration-only loan.
                        loan.invocation.is_some()
                    };
                    ReferenceCompletionOccurrence {
                        identity: &loan.obligation,
                        parents: match &loan.lender {
                            Lender::Loan(parent) => vec![parent],
                            Lender::Owner | Lender::Incoming => Vec::new(),
                        },
                        returned: is_returned,
                        in_caller_storage: false,
                        disposed,
                    }
                })
                .collect::<Vec<_>>();
            terminal_semantics::validate_reference_result_completion(&occurrences)
                .map_err(|reason| self.unsupported(concerns, reason.description()))?;
        }
        Ok(())
    }

    /// The verdict on every `Loan` row, and each conflict found. A loan
    /// that may be formed is `Ok`; one that may not is a `Conflict`
    /// refusal, listed once in the conflicts with what it conflicts with.
    pub(crate) fn loans(
        &self,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
    ) -> Checked<(Vec<(ObligationId, Checked<()>)>, Vec<ConflictingLoan>)> {
        self.reference_results(module)?;
        let formed = self.formed(module, tables)?;
        self.parents(module, &formed)?;
        self.reference_completions(module, &formed)?;
        let mut verdicts = Vec::with_capacity(formed.len());
        let mut conflicts = Vec::new();
        for (at, loan) in formed.iter().enumerate() {
            let used = self.uses_retained_loan(loan, &formed)?;
            let site = used.map_or(Construct::Obligation(loan.obligation), |(at, _)| at);
            let exclusive = loan.access.is_exclusive();
            let mut found = used.map(|(_, conflict)| (None, conflict));
            if found.is_none() && !loan.permitted {
                found = Some((None, LoanConflict::AccessExceedsParent));
            }
            if found.is_none() && exclusive && loan.parent == Some(ReferenceAccess::Shared) {
                found = Some((None, LoanConflict::ExclusiveOfShared));
            }
            if found.is_none() {
                // The later listed loan reports the conflict. Retained
                // lifetimes determine overlap across calls and block order;
                // arena ordering is not lifetime ordering.
                for earlier in formed[..at].iter().filter(|earlier| {
                    (earlier.invocation.is_some() && earlier.invocation == loan.invocation)
                        || earlier
                            .lifetime
                            .as_ref()
                            .is_some_and(|life| life.live.contains(&loan.at))
                        || loan
                            .lifetime
                            .as_ref()
                            .is_some_and(|life| life.live.contains(&earlier.at))
                }) {
                    if self.descends(loan, earlier.obligation, &formed)?
                        || self.descends(earlier, loan.obligation, &formed)?
                    {
                        continue;
                    }
                    let earlier_exclusive = earlier.access.is_exclusive();
                    if (exclusive || earlier_exclusive) && overlap(&earlier.lineage, &loan.lineage)
                    {
                        let conflict = if exclusive && earlier_exclusive {
                            LoanConflict::TwiceExclusive
                        } else {
                            LoanConflict::ExclusiveAndShared
                        };
                        found = Some((Some(earlier.position), conflict));
                        break;
                    }
                }
            }
            let verdict = match found {
                None => Ok(()),
                Some((with, conflict)) => {
                    conflicts.push(ConflictingLoan {
                        loan: loan.obligation,
                        invocation: loan.invocation,
                        position: loan.position,
                        with,
                        conflict,
                        at: site,
                    });
                    Err(Refusal {
                        reason: RefusalReason::Conflict,
                        machine: self.machine,
                        concerns: Construct::Obligation(loan.obligation),
                        detail: String::from(match conflict {
                            LoanConflict::TwiceExclusive => {
                                "an exclusive loan of storage an earlier argument of the same call \
                                 lends exclusively"
                            }
                            LoanConflict::ExclusiveAndShared => {
                                "a loan of storage an earlier argument of the same call lends, \
                                 one of the two exclusively"
                            }
                            LoanConflict::ExclusiveOfShared => {
                                "an exclusive reborrow of storage this machine holds shared"
                            }
                            LoanConflict::ObserveWhileExclusive => {
                                "storage is observed outside its live exclusive reference"
                            }
                            LoanConflict::ChangeWhileBorrowed => {
                                "storage is changed while a conflicting reference remains live"
                            }
                            LoanConflict::ReleaseBeforeChildren => {
                                "reference release precedes its complete child-loan closure"
                            }
                            LoanConflict::AccessExceedsParent => {
                                "reborrow exceeds the access of its actual source path"
                            }
                        }),
                    })
                }
            };
            verdicts.push((loan.obligation, verdict));
        }
        Ok((verdicts, conflicts))
    }
}
