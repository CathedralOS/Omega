/*
The loans of one machine: which `Loan` rows the lowered form owes, and
whether each loan may be formed where it is.

A reference can keep a loan live after the operation that formed it. The
existing descriptor flow supplies its transported homes and lifetime; all
reads, writes and later loans resolve the same captured backing. Missing
immediate-parent or returned-call correspondence remains unavailable rather
than being treated as a call-duration loan. A checked call result retains the
input storage its published origin names across every transported result
descriptor: the argument's own loan when the origin is the whole argument,
otherwise a second `Loan` row of the same call capturing the narrower source
path, while the argument's own loan ends when the call returns.

A call-duration loan is a call's structural argument with a borrowed access: the call lends
the place the argument names, for the call's duration, and the place is the
caller's again when the call returns (`wiki/spec/terminal-psi/loans.md`:
"Loan resource | Exact occurrence, owner lineage, captured place, access,
parent lifetime, and restoration obligation."). The occurrence is the call,
the captured place is the argument's place, the access is the argument's,
and the lineage is the root's: a borrowed formal's root reborrows the loan
the invocation passed in (`Lender::Incoming`). A provider-established view
keeps its provider permission (`Lender::Provider`); owned storage lends from
its owner (`Lender::Owner`). A duration-only loan ends when the call returns,
so it is live on no edge and no edge disposes of it. A loan formed
for a call with no normal return is carried into that terminal transfer
(`wiki/spec/language/state_contracts.md`: "A caller-local loan carried into
a terminal machine call closes at the last use of that loan and of every
loan derived from it, even when that use is inside the callee's terminal
chain"): the transfer's edge consumes it, and nothing of this machine runs
after to use the lent place again.

The rows are recomputed from the lowered form, as every other row is
(`minted`): one `Loan` per borrowed argument of each call, plus one per
narrower region a returned reference retains, created at the point before the
call, whose captured place is the invocation row's actual
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
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowStep, DescriptorLocation, descriptor_path,
};

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
    lineage: LoanRegion,
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
    /// For the narrower region a returned reference retains, the call that
    /// formed it. That call's own argument loans cover it only for the call.
    returned_by: Option<InvocationId>,
}

/// Action prestates are projections of the existing lifetime authority flow.
/// Cache each prefix once; no cleanup action creates a new loan occurrence.
#[derive(Default)]
struct CleanupCohort {
    surviving: std::collections::BTreeSet<DescriptorLocation>,
    authorized: std::collections::BTreeSet<DescriptorLocation>,
}
type CleanupCohorts =
    std::collections::BTreeMap<(ObligationId, semantic_vocabulary::EdgeId, u32), CleanupCohort>;

#[derive(Clone, Copy)]
pub(super) enum BorrowedResource {
    Storage,
    ViewPayload,
}

#[derive(Debug)]
enum LoanRegion {
    Storage(super::Lineage),
    /// An admitted provider establishes a stable shared permission. Its input
    /// loans are retained separately; its address remains unavailable.
    Provider,
}

impl LoanRegion {
    fn respects_parent(&self, parent: &Self) -> bool {
        match (self, parent) {
            (Self::Storage(child), Self::Storage(parent)) => {
                child.root == parent.root && child.segments.starts_with(&parent.segments)
            }
            // Exact parent identity was already checked through descriptor
            // flow. This adds no assertion about where its bytes reside.
            (Self::Provider, Self::Provider) => true,
            _ => false,
        }
    }
}

fn reference_access(access: StructuralAccess) -> Option<ReferenceAccess> {
    match access {
        StructuralAccess::SharedBorrow => Some(ReferenceAccess::Shared),
        StructuralAccess::MutableBorrow => Some(ReferenceAccess::Mutable),
        StructuralAccess::WriteOnlyBorrow => Some(ReferenceAccess::WriteOnly),
        StructuralAccess::Owned => None,
    }
}

fn loan_lender(source: terminal_semantics::BorrowedLoanSource) -> Lender {
    match source {
        terminal_semantics::BorrowedLoanSource::Owner => Lender::Owner,
        terminal_semantics::BorrowedLoanSource::Incoming => Lender::Incoming,
        terminal_semantics::BorrowedLoanSource::Loan(parent) => Lender::Loan(parent),
        terminal_semantics::BorrowedLoanSource::Provider => Lender::Provider,
    }
}

fn competing_borrows(
    left: ReferenceAccess,
    left_place: &LoanRegion,
    right: ReferenceAccess,
    right_place: &LoanRegion,
) -> Option<LoanConflict> {
    // The boundary contract establishes permission and stability, not a
    // physical region covering the caller. Conflicts with its inputs are
    // checked through the original loans kept alive by the complete result
    // dependencies. No address or disjointness fact follows from this case.
    if matches!(left_place, LoanRegion::Provider) && left.is_exclusive()
        || matches!(right_place, LoanRegion::Provider) && right.is_exclusive()
    {
        return Some(LoanConflict::AccessExceedsParent);
    }
    let (LoanRegion::Storage(left_place), LoanRegion::Storage(right_place)) =
        (left_place, right_place)
    else {
        return None;
    };
    if !overlap(left_place, right_place) || !left.is_exclusive() && !right.is_exclusive() {
        None
    } else if left.is_exclusive() && right.is_exclusive() {
        Some(LoanConflict::TwiceExclusive)
    } else {
        Some(LoanConflict::ExclusiveAndShared)
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
        terminal_psi::structural_type_has_borrowed_storage_by(|id| module.structural_type(id), root)
    };
    let demanded = lowered
        .result
        .structural()
        .is_some_and(|result| contains(result.structural_type))
        || lowered
            .structural_parameters
            .iter()
            .any(|parameter| contains(parameter.structural_type))
        // Byte literals have declared storage even without a producing operation.
        || lowered
            .structural_places
            .iter()
            .filter_map(|place| place.kind.produced_structural_type())
            .any(contains)
        || lowered.blocks.iter().any(|block| {
            block.structural_parameters.iter().any(|parameter| {
                parameter.access != StructuralAccess::Owned || contains(parameter.structural_type)
            }) || block.operations.iter().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishReference { .. }
                        | terminal_psi::OperationKind::EstablishElementView { .. }
                        | terminal_psi::OperationKind::EstablishMutableElementView { .. }
                        | terminal_psi::OperationKind::ReleaseReference { .. }
                )
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
    .map_err(|error| graph_refusal(module, error))
}

fn graph_refusal(
    module: &terminal_psi::TerminalModule,
    error: terminal_semantics::descriptor_results::DescriptorGraphError,
) -> terminal_check_tables::Refusal {
    crate::refusal(
        RefusalReason::Unsupported,
        error.machine(),
        error
            .operation(module)
            .map(Construct::Operation)
            .unwrap_or(Construct::Machine),
        &error.in_module(module),
    )
}

impl<'a> Storage<'a> {
    pub(super) fn borrowed_resource(
        &self,
        parameter: &terminal_psi::StructuralParameterDeclaration,
    ) -> BorrowedResource {
        if terminal_semantics::borrowed_view_payload_parameter(parameter, self.types.iter()) {
            BorrowedResource::ViewPayload
        } else {
            BorrowedResource::Storage
        }
    }
    fn borrow_backing(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        raw: &super::Lineage,
        resource: BorrowedResource,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        concerns: Construct,
    ) -> Checked<LoanRegion> {
        use terminal_semantics::descriptor_results::{
            DescriptorCallOrigin, DescriptorSourceOrigin,
        };
        // A descriptor home and its captured payload can be borrowed at the
        // same time. Follow the backing only when the operation lends it.
        let shape = self.shape(raw, concerns)?;
        let is_view = matches!(shape, super::Shape::View)
            || matches!(shape, super::Shape::Typed(ty)
                if terminal_semantics::descriptor_results::is_descriptor(self.types.iter(), ty));
        if matches!(resource, BorrowedResource::Storage) || !is_view {
            return self
                .backing(references, raw, at.0, at.1 as usize, concerns)
                .map(LoanRegion::Storage);
        }
        let location = raw.descriptor_location().ok_or_else(|| {
            self.unsupported(concerns, "view loan has no exact descriptor coordinate")
        })?;
        let origins = references
            .try_symbolic_source_origins(self.machine, at, &location)
            .map_err(|detail| self.unsupported(concerns, &detail))?;
        if origins.is_empty() {
            return Err(
                self.unsupported(concerns, "view loan has no original payload correspondence")
            );
        }
        let mut captured = None;
        for origin in origins {
            let next = match &origin {
                DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                    source,
                    projection,
                }) => {
                    let enclosing = projection.enclosing_storage().ok_or_else(|| {
                        self.unsupported(concerns, "view loan has unresolved payload projection")
                    })?;
                    self.backing(
                        references,
                        &self.stored_at(*source, &enclosing.path, concerns)?,
                        at.0,
                        at.1 as usize,
                        concerns,
                    )?
                }
                DescriptorSourceOrigin::IncomingSlot(_)
                | DescriptorSourceOrigin::IncomingContents { .. } => {
                    // This payload came from a caller borrow. Keep its input
                    // field as the loan identity, rather than flattening the
                    // payload into an address of this activation's storage.
                    let original = terminal_semantics::incoming_view_occurrence(
                        self.module,
                        self.lowered,
                        std::slice::from_ref(&origin),
                    )
                    .map_err(|detail| self.unsupported(concerns, detail))?;
                    let path = terminal_semantics::descriptor_lifetimes::descriptor_path(
                        self.types,
                        self.root_type(original.root, concerns)?,
                        &original.path,
                    )
                    .ok_or_else(|| {
                        self.foreign(concerns, "incoming view lost its exact input field")
                    })?;
                    self.stored_at(original.root, &path, concerns)?
                }
                DescriptorSourceOrigin::Backing(
                    DescriptorCallOrigin::Literal { .. } | DescriptorCallOrigin::Empty,
                ) => raw.clone(),
                DescriptorSourceOrigin::Provider(_) => {
                    if !matches!(
                        terminal_semantics::borrowed_loan_source(
                            self.module,
                            self.lowered,
                            references,
                            at,
                            &location,
                            std::iter::empty(),
                        )
                        .map_err(|detail| self.unsupported(concerns, detail))?,
                        terminal_semantics::BorrowedLoanSource::Provider
                    ) {
                        return Err(
                            self.unsupported(concerns, "provider payload has no exact permission")
                        );
                    }
                    return Ok(LoanRegion::Provider);
                }
                _ => {
                    return Err(self.unsupported(
                        concerns,
                        "view loan has unserved original backing correspondence",
                    ));
                }
            };
            if captured.as_ref().is_some_and(|previous| *previous != next) {
                return Err(
                    self.unsupported(concerns, "view loan has multiple possible backing regions")
                );
            }
            captured = Some(next);
        }
        captured
            .map(LoanRegion::Storage)
            .ok_or_else(|| self.foreign(concerns, "view loan has no captured payload"))
    }

    /// The access a root's storage is held with here, when it is a borrowed
    /// formal: the declared access of its structural parameter.
    pub(super) fn incoming(&self, root: semantic_vocabulary::PlaceId) -> Option<StructuralAccess> {
        terminal_semantics::structural_argument_source(self.lowered, root, 0)
            .filter(|parameter| parameter.access != StructuralAccess::Owned)
            .map(|parameter| parameter.access)
    }

    /// A permission crossing retains the descriptor's immediate resource;
    /// owning that descriptor never changes the backing into owned storage.
    fn source_lender(
        &self,
        raw: &super::Lineage,
        path: &[terminal_psi::StructuralPathSegment],
        access: ReferenceAccess,
        resource: BorrowedResource,
        concerns: Construct,
    ) -> Checked<(
        Option<Lender>,
        Option<ReferenceAccess>,
        Option<terminal_semantics::descriptor_lifetimes::DescriptorLocation>,
        bool,
    )> {
        use terminal_check_tables::bindings::Segment;
        let permitted = if let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = self.root_kind(raw.root, concerns)?
        {
            access == ReferenceAccess::Shared
                && self
                    .declaration(structural_type, concerns)?
                    .shape
                    .is_shared_readable_byte_view()
        } else {
            let actual = terminal_semantics::structural_argument_source(self.lowered, raw.root, 0)
                .ok_or_else(|| {
                    self.unsupported(concerns, "loan source has no actual storage authority")
                })?;
            let requested = match access {
                ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
            };
            terminal_semantics::reference_source_projection_accessible(
                self.types.iter(),
                actual.structural_type,
                actual.multiplicity,
                actual.access,
                path,
                requested,
            )
        };
        if matches!(resource, BorrowedResource::ViewPayload) {
            let parent = if matches!(
                self.root_kind(raw.root, concerns)?,
                StructuralPlaceKind::ByteSequenceLiteral { .. }
            ) {
                StructuralAccess::SharedBorrow
            } else {
                terminal_semantics::structural_projection_authority(
                    self.lowered,
                    self.types,
                    &terminal_psi::StructuralArgument {
                        place: raw.root,
                        path: path.to_vec(),
                        access: StructuralAccess::SharedBorrow,
                    },
                )
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "view payload loan has no retained source permission",
                    )
                })?
            };
            return Ok((
                None,
                reference_access(parent),
                Some(raw.descriptor_location().ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "view payload loan has no exact authorizing descriptor",
                    )
                })?),
                permitted,
            ));
        }
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
            return Ok((None, reference_access(parent), Some(descriptor), permitted));
        }
        if let Some(parent) = self.incoming(raw.root) {
            if matches!(
                self.root_kind(raw.root, concerns)?,
                StructuralPlaceKind::BlockParameter { .. }
            ) {
                return Ok((
                    None,
                    reference_access(parent),
                    Some(
                        terminal_semantics::descriptor_lifetimes::DescriptorLocation::whole(
                            raw.root,
                        ),
                    ),
                    permitted,
                ));
            }
            return Ok((
                Some(Lender::Incoming),
                reference_access(parent),
                None,
                permitted,
            ));
        }
        match self.root_kind(raw.root, concerns)? {
            StructuralPlaceKind::Parameter { .. }
            | StructuralPlaceKind::BlockParameter { .. }
            | StructuralPlaceKind::OperationResult { .. } => {
                Ok((Some(Lender::Owner), None, None, permitted))
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } if access == ReferenceAccess::Shared
                && self
                    .declaration(structural_type, concerns)?
                    .shape
                    .is_shared_readable_byte_view() =>
            {
                Ok((Some(Lender::Owner), None, None, permitted))
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
        references: &mut super::DescriptorAnalysis<'a>,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
        (concerns, before): (Construct, ProgramPoint),
        arguments: &[terminal_psi::StructuralArgument],
        call: Option<InvocationSite>,
        retained: Vec<terminal_semantics::RetainedCallLoan>,
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
        let Some(site) = call else {
            return Err(self.unsupported(concerns, "a loan to something not a call"));
        };
        let mut invocations = tables
            .invocations
            .iter()
            .filter(|(_, row)| row.site == site);
        let (Some((invocation, row)), None) = (invocations.next(), invocations.next()) else {
            return Err(self.foreign(concerns, "a call does not have exactly one invocation row"));
        };
        let actual =
            crate::call_contracts::invocation(module, self.lowered, tables, invocation, row)?;
        if actual.inputs.structural_arguments != arguments {
            return Err(self.foreign(concerns, "loan arguments differ from their canonical call"));
        }
        let formals = actual.contract.structural_parameters;
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
            self.byte_array_presentation(argument, parameter, concerns)?;
            let resource = self.borrowed_resource(parameter);
            let (expected_lender, parent, from, permitted) =
                self.source_lender(&lent, &argument.path, access, resource, concerns)?;
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
            // The argument's own loan is retained only when the result keeps
            // its whole place; a narrower retained region gets its own row.
            let lifetime = retained
                .next_if(|loan| loan.position == structural_position && loan.path == argument.path)
                .map(|loan| loan.lifetime);
            let cut = {
                let (block, before) = self.graph.locate(before)?;
                (
                    block,
                    u32::try_from(before).map_err(|_| {
                        self.foreign(concerns, "loan cut exceeds descriptor coordinates")
                    })?,
                )
            };
            formed.push(Formed {
                obligation,
                invocation: Some(invocation),
                position,
                access,
                lineage: self.borrow_backing(references, &lent, resource, cut, concerns)?,
                parent,
                lender,
                from,
                permitted,
                at: self.loan_position(before, concerns)?,
                lifetime,
                returned_by: None,
            });
            while let Some(narrower) = retained.next_if(|loan| loan.position == structural_position)
            {
                if !narrower.path.starts_with(&argument.path) {
                    return Err(
                        self.foreign(concerns, "a retained call region lies outside its argument")
                    );
                }
                let region = terminal_psi::StructuralArgument {
                    place: argument.place,
                    path: narrower.path,
                    access: argument.access,
                };
                let raw = self.argument(&region, concerns)?;
                let captured = self.interned(tables, &raw, concerns)?.ok_or_else(|| {
                    self.foreign(concerns, "a retained call region has no captured place row")
                })?;
                let (expected_lender, parent, from, permitted) =
                    self.source_lender(&raw, &region.path, access, resource, concerns)?;
                let (obligation, row) = tables
                    .obligations
                    .iter()
                    .find(|(id, row)| {
                        matches!(row.kind, ObligationKind::Loan { captured: place, access: borrowed, .. }
                            if place == captured && borrowed == access)
                            && row.created_at == before
                            && !claimed[index(id.get())]
                    })
                    .ok_or_else(|| {
                        self.foreign(
                            concerns,
                            "a retained call region has no exact occurrence row",
                        )
                    })?;
                let ObligationKind::Loan { lender, .. } = row.kind else {
                    unreachable!()
                };
                if expected_lender.is_some_and(|expected| expected != lender) {
                    return Err(self.foreign(
                        concerns,
                        "retained call region lender does not match its actual source resource",
                    ));
                }
                claimed[index(obligation.get())] = true;
                formed.push(Formed {
                    obligation,
                    invocation: None,
                    position,
                    access,
                    lineage: self.borrow_backing(references, &raw, resource, cut, concerns)?,
                    parent,
                    lender,
                    from,
                    permitted,
                    at: self.loan_position(before, concerns)?,
                    lifetime: Some(narrower.lifetime),
                    returned_by: Some(invocation),
                });
            }
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
        references: &mut super::DescriptorAnalysis<'a>,
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
        let concerns = Construct::Operation(operation.id);
        let result = operation.result.as_structural();
        let (source, declared, reference_referent) = match &operation.kind {
            OperationKind::EstablishReference { source, storage } => {
                let result = result.ok_or_else(|| {
                    self.foreign(concerns, "reference formation has no descriptor result")
                })?;
                let StructuralTypeShape::Reference { referent, access } =
                    self.declaration(result.structural_type, concerns)?.shape
                else {
                    return Err(
                        self.foreign(concerns, "reference formation produces another constructor")
                    );
                };
                (source, access, Some((referent, storage.as_ref())))
            }
            OperationKind::EstablishElementView { source, .. }
            | OperationKind::EstablishMutableElementView { source, .. } => {
                self.array_view_formation(operation)?;
                (source, source.access, None)
            }
            _ => return Ok(()),
        };
        let result = result
            .ok_or_else(|| self.foreign(concerns, "loan formation has no descriptor result"))?;
        if source.access != declared {
            return Err(self.foreign(concerns, "reference formation changes its source access"));
        }
        let access = reference_access(source.access)
            .ok_or_else(|| self.foreign(concerns, "reference formation has no borrowed access"))?;
        if access == ReferenceAccess::WriteOnly {
            return Err(self.unsupported(concerns, "a write-only persistent loan"));
        }
        let raw = self.argument(source, concerns)?;
        let resource = if reference_referent.is_none()
            && match self.shape(&raw, concerns)? {
                super::Shape::View => true,
                super::Shape::Typed(ty) => {
                    terminal_semantics::descriptor_results::is_descriptor(self.types.iter(), ty)
                }
                _ => false,
            } {
            BorrowedResource::ViewPayload
        } else {
            BorrowedResource::Storage
        };
        if let Some((referent, storage)) = reference_referent {
            let expected = self.declaration(referent, concerns)?;
            let compatible = if let Some(storage) = storage {
                if access != ReferenceAccess::Shared {
                    return Err(self.unsupported(
                        concerns,
                        "exclusive representation view without establishment and handback evidence",
                    ));
                }
                terminal_semantics::validate_reference_view_source(
                    self.module,
                    self.lowered,
                    source.place,
                )
                .map_err(|reason| self.unsupported(concerns, reason))?;
                let offset = tables.values.get(storage.byte_offset).ok_or_else(|| {
                    self.foreign(concerns, "reference view lost its offset value")
                })?;
                let terminal_check_tables::values::ValueOrigin::Operation(producer) = offset.origin
                else {
                    return Err(self.unsupported(
                        concerns,
                        "reference view offset without captured geometry evidence",
                    ));
                };
                let (block, after) = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
                let producer = after
                    .checked_sub(1)
                    .and_then(|position| {
                        self.lowered
                            .blocks
                            .get(index(block.get()))?
                            .operations
                            .get(position)
                    })
                    .ok_or_else(|| {
                        self.foreign(concerns, "reference view offset has no producer")
                    })?;
                if offset.scalar_type
                    != producer
                        .result
                        .as_scalar()
                        .map(|row| row.scalar_type)
                        .ok_or_else(|| {
                            self.foreign(concerns, "reference view offset has no scalar producer")
                        })?
                    || !terminal_semantics::reference_view_offset_is_zero(
                        producer,
                        storage.byte_offset,
                    )
                {
                    return Err(self.unsupported(
                        concerns,
                        "reference view offset without captured geometry evidence",
                    ));
                }
                terminal_semantics::reference_view_compatible(
                    self.module,
                    self.root_type(source.place, concerns)?,
                    &source.path,
                    result.structural_type,
                    storage.stored_scalar,
                )
                .map_err(|reason| self.unsupported(concerns, reason))?
            } else {
                match self.shape(&raw, concerns)? {
                    super::Shape::Scalar(scalar) => {
                        expected.shape == StructuralTypeShape::PrimitiveScalar(scalar)
                    }
                    super::Shape::Atomic(cell) => {
                        expected.shape == StructuralTypeShape::AtomicCell(cell)
                    }
                    super::Shape::Typed(actual) => {
                        actual == referent
                            || terminal_semantics::shared_direct_byte_descriptor_source(
                                self.module,
                                self.lowered,
                                source,
                                result.structural_type,
                            ) == Some(true)
                    }
                    super::Shape::View | super::Shape::Payload(_) => false,
                }
            };
            if !compatible {
                return Err(self.foreign(
                    concerns,
                    "reference formation changes its captured referent type",
                ));
            }
        }
        let captured = self.interned(tables, &raw, concerns)?.ok_or_else(|| {
            self.foreign(concerns, "reference formation has no captured place row")
        })?;
        let (expected_lender, parent, from, permitted) =
            self.source_lender(&raw, &source.path, access, resource, concerns)?;
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
            lineage: self.borrow_backing(references, &raw, resource, at, concerns)?,
            parent,
            lender,
            from,
            permitted,
            at,
            lifetime: Some(lifetime),
            returned_by: None,
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
        let root_type = self.root_type(source, concerns)?;
        if terminal_semantics::inline_descriptor_type(
            self.module.structural_types.iter(),
            root_type,
            path,
        )
        .is_none()
        {
            return Err(self.foreign(concerns, "reference release names no descriptor"));
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

    /// A retained loan constrains every use of its backing until its actual
    /// carriers are disposed, even after their last observation. Future-use
    /// liveness can propose a release, but cannot itself return custody.
    /// Only an authenticated transported descriptor may use an exclusive
    /// loan; a shared loan permits observation but no write through any alias.
    fn uses_retained_loan(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        loan: &Formed,
        formed: &[Formed],
        cleanup_cohorts: &CleanupCohorts,
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
                            if child.lifetime.as_ref().is_some_and(|life| holds(life, &at))
                                && self.descends(child, loan.obligation, formed)?
                            {
                                return Ok(Some((concerns, LoanConflict::ReleaseBeforeChildren)));
                            }
                        }
                    }
                }
                if !holds(lifetime, &at) {
                    continue;
                }
                let atomic_change = matches!(
                    operation.kind,
                    terminal_psi::OperationKind::AtomicAccess { .. }
                );
                let effects = self
                    .operation_read_effects(operation)?
                    .into_iter()
                    .map(|effect| (effect, Touch::Read))
                    .chain(
                        self.operation_changes(operation)?
                            .into_iter()
                            .map(|effect| {
                                (
                                    effect,
                                    if atomic_change {
                                        Touch::AtomicChange
                                    } else {
                                        Touch::Change
                                    },
                                )
                            }),
                    );
                for (effect, touch) in effects {
                    if let Some(conflict) = self
                        .retained_conflict(references, loan, formed, effect, touch, at, concerns)?
                    {
                        return Ok(Some((concerns, conflict)));
                    }
                }
            }
            // The block's terminator observes and relinquishes storage just
            // as an operation does: a dispatch reads its subject, an owned
            // edge or return argument leaves its home (ownership.md: an
            // exclusive child suspends its parent branch).
            let at = (
                block.id,
                crate::position(block.operations.len(), self.machine)?,
            );
            if holds(lifetime, &at) {
                // A dispatch has one edge per case, so it is named by its
                // block, as the place model names it.
                let concerns = match &block.terminator {
                    terminal_psi::Terminator::StructuralCase { .. } => Construct::Block(block.id),
                    terminator => terminator
                        .edges()
                        .next()
                        .map_or(Construct::Block(block.id), Construct::Edge),
                };
                let (reads, changes) = self.terminator_effects(block)?;
                let effects = reads
                    .into_iter()
                    .map(|place| (place, Touch::Read))
                    .chain(changes.into_iter().map(|place| (place, Touch::Change)));
                for (place, touch) in effects {
                    if let Some(conflict) = self.retained_conflict(
                        references,
                        loan,
                        formed,
                        super::StorageEffect::Storage(place),
                        touch,
                        at,
                        concerns,
                    )? {
                        return Ok(Some((concerns, conflict)));
                    }
                }
            }
            for (edge, actions) in block.terminator.cleanup_rosters() {
                let concerns = Construct::Edge(edge);
                for (ordinal, action) in actions.iter().enumerate() {
                    let ordinal = crate::position(ordinal, self.machine)?;
                    if !cleanup_cohorts
                        .get(&(loan.obligation, edge, ordinal))
                        .is_some_and(|cohort| !cohort.surviving.is_empty())
                    {
                        continue;
                    }
                    let cleaned = self.stored_at(action.place(), action.path(), concerns)?;
                    let backing = LoanRegion::Storage(self.backing(
                        references,
                        &cleaned,
                        block.id,
                        block.operations.len(),
                        concerns,
                    )?);
                    if competing_borrows(
                        loan.access,
                        &loan.lineage,
                        ReferenceAccess::Mutable,
                        &backing,
                    )
                    .is_some()
                    {
                        return Ok(Some((concerns, LoanConflict::ChangeWhileBorrowed)));
                    }
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) = action
                    {
                        for (raw, backing, access) in self.cleanup_reference_accesses(
                            references, block.id, edge, ordinal, cleanup,
                        )? {
                            if competing_borrows(
                                loan.access,
                                &loan.lineage,
                                access,
                                &LoanRegion::Storage(backing),
                            )
                            .is_some()
                                && ((access.is_exclusive() && !loan.access.is_exclusive())
                                    || !self.authorized_cleanup_reference(
                                        loan,
                                        &raw,
                                        edge,
                                        ordinal,
                                        formed,
                                        cleanup_cohorts,
                                    )?)
                            {
                                return Ok(Some((
                                    concerns,
                                    if access.is_exclusive() {
                                        LoanConflict::ChangeWhileBorrowed
                                    } else {
                                        LoanConflict::ObserveWhileExclusive
                                    },
                                )));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    /// Whether one storage effect at `at` conflicts with a retained loan:
    /// a read of exclusively lent storage, or a change of lent storage, not
    /// made through a descriptor the loan authorizes.
    #[allow(clippy::too_many_arguments)]
    fn retained_conflict(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        loan: &Formed,
        formed: &[Formed],
        effect: super::StorageEffect,
        touch: Touch,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        concerns: Construct,
    ) -> Checked<Option<LoanConflict>> {
        let (place, backing) = match effect {
            super::StorageEffect::Payload(place) => {
                let backing = self.borrow_backing(
                    references,
                    &place,
                    BorrowedResource::ViewPayload,
                    at,
                    concerns,
                )?;
                (place, backing)
            }
            super::StorageEffect::Storage(place) => {
                let backing = LoanRegion::Storage(self.backing(
                    references,
                    &place,
                    at.0,
                    at.1 as usize,
                    concerns,
                )?);
                (place, backing)
            }
        };
        let access = match touch {
            Touch::Read | Touch::AtomicChange => ReferenceAccess::Shared,
            Touch::Change => ReferenceAccess::Mutable,
        };
        if competing_borrows(loan.access, &loan.lineage, access, &backing).is_none() {
            return Ok(None);
        }
        let conflict = match touch {
            Touch::Read => LoanConflict::ObserveWhileExclusive,
            Touch::Change | Touch::AtomicChange => LoanConflict::ChangeWhileBorrowed,
        };
        // A shared loan permits no ordinary write through any alias.
        if touch == Touch::Change && !loan.access.is_exclusive() {
            return Ok(Some(conflict));
        }
        Ok((!self.authorized_reference(loan, &place, at, formed)?).then_some(conflict))
    }

    fn cleanup_cohorts(
        &self,
        formed: &[Formed],
        flow: &std::collections::BTreeMap<(semantic_vocabulary::EdgeId, u32), DescriptorFlowStep>,
    ) -> Checked<CleanupCohorts> {
        let mut rosters = Vec::new();
        for block in &self.lowered.blocks {
            let at = (
                block.id,
                crate::position(block.operations.len(), self.machine)?,
            );
            for (edge, actions) in block.terminator.cleanup_rosters() {
                let mut killed = Vec::with_capacity(actions.len());
                for action in actions {
                    killed.push(
                        self.stored_at(action.place(), action.path(), Construct::Edge(edge))?
                            .descriptor_location()
                            .ok_or_else(|| {
                                self.unsupported(
                                    Construct::Edge(edge),
                                    "cleanup consumption has no exact carrier spelling",
                                )
                            })?,
                    );
                }
                rosters.push((edge, at, killed));
            }
        }
        let mut result = CleanupCohorts::new();
        for loan in formed {
            let Some(life) = &loan.lifetime else {
                continue;
            };
            for (edge, at, killed) in &rosters {
                let mut surviving = life.surviving.get(at).cloned().unwrap_or_default();
                let mut authorized = life.authorized.get(at).cloned().unwrap_or_default();
                for (ordinal, consumed) in killed.iter().enumerate() {
                    if surviving.is_empty() && authorized.is_empty() {
                        break;
                    }
                    let ordinal = crate::position(ordinal, self.machine)?;
                    result.insert(
                        (loan.obligation, *edge, ordinal),
                        CleanupCohort {
                            surviving: surviving.clone(),
                            authorized: authorized.clone(),
                        },
                    );
                    if let Some(step) = flow.get(&(*edge, ordinal)) {
                        // A payload formation is not a carrier transfer. Both
                        // rosters follow authority derivations, with MAY union
                        // and MUST requiring every inverse source spelling.
                        surviving = step.transport_authority(&surviving, true);
                        authorized = step.transport_authority(&authorized, false);
                    }
                    surviving.retain(|name| !consumed.contains(name));
                    authorized.retain(|name| !consumed.contains(name));
                }
            }
        }
        Ok(result)
    }

    fn authorized_cleanup_reference(
        &self,
        loan: &Formed,
        raw: &super::Lineage,
        edge: semantic_vocabulary::EdgeId,
        ordinal: u32,
        formed: &[Formed],
        cohorts: &CleanupCohorts,
    ) -> Checked<bool> {
        let Some(crossing) = raw
            .segments
            .iter()
            .rposition(|segment| *segment == terminal_check_tables::bindings::Segment::Referent)
        else {
            return Ok(false);
        };
        let descriptor = DescriptorLocation::captured(raw.root, &raw.segments[..crossing])
            .ok_or_else(|| {
                self.unsupported(
                    Construct::Edge(edge),
                    "cleanup access has no retained descriptor identity",
                )
            })?;
        for candidate in formed {
            if candidate.obligation != loan.obligation
                && !self.descends(candidate, loan.obligation, formed)?
            {
                continue;
            }
            if cohorts
                .get(&(candidate.obligation, edge, ordinal))
                .is_some_and(|cohort| {
                    !cohort.surviving.is_empty() && cohort.authorized.contains(&descriptor)
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn cleanup_reference_accesses(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        block: semantic_vocabulary::BlockId,
        edge: semantic_vocabulary::EdgeId,
        ordinal: u32,
        cleanup: &terminal_psi::OwnedCleanupInvocation,
    ) -> Checked<Vec<(super::Lineage, super::Lineage, ReferenceAccess)>> {
        let concerns = Construct::Edge(edge);
        let mut accesses = self
            .cleanup_writes
            .iter()
            .filter(|write| {
                write.edge == edge
                    && write.action_ordinal == ordinal
                    && write.content == terminal_check_tables::bindings::Content::Overwritten
            })
            .map(|write| {
                (
                    super::Lineage {
                        root: write.captured.root,
                        segments: write.captured.path.clone(),
                    },
                    super::Lineage {
                        root: write.backing.root,
                        segments: write.backing.path.clone(),
                    },
                    ReferenceAccess::Mutable,
                )
            })
            .collect::<Vec<_>>();
        let root = self
            .lowered
            .structural_place_type(cleanup.subject.place)
            .ok_or_else(|| self.foreign(concerns, "cleanup subject has no type"))?;
        let inventory = terminal_semantics::descriptor_results::DescriptorSlotKind::References
            .inventory(self.types, cleanup.subject.place, cleanup.structural_type)
            .ok_or_else(|| {
                self.unsupported(concerns, "cleanup has no finite reference access inventory")
            })?;
        for leaf in inventory.slots {
            let path = descriptor_path(self.types, cleanup.structural_type, &leaf.path)
                .ok_or_else(|| self.unsupported(concerns, "cleanup reference access is untyped"))?;
            let mut actual = cleanup.subject.path.clone();
            actual.extend(path);
            actual.push(terminal_psi::StructuralPathSegment::Referent);
            if !terminal_semantics::reference_source_projection_accessible(
                self.types.iter(),
                root,
                terminal_psi::StructuralMultiplicity::Affine,
                StructuralAccess::Owned,
                &actual,
                StructuralAccess::SharedBorrow,
            ) {
                continue;
            }
            let raw = self.stored_at(cleanup.subject.place, &actual, concerns)?;
            let descriptor = raw.descriptor_location().ok_or_else(|| {
                self.unsupported(concerns, "cleanup read has no exact captured path")
            })?;
            let at = (
                block,
                crate::position(self.block(block)?.operations.len(), self.machine)?,
            );
            let backing = references
                .location_at(
                    self.machine,
                    at,
                    terminal_psi::InvocationOwner::Cleanup {
                        edge,
                        action_ordinal: ordinal,
                    },
                    &descriptor,
                )
                .ok_or_else(|| {
                    self.unsupported(
                        concerns,
                        "cleanup read has no actual owner-prestate backing",
                    )
                })?;
            accesses.push((
                raw,
                super::Lineage {
                    root: backing.root,
                    segments: backing.path,
                },
                ReferenceAccess::Shared,
            ));
        }
        Ok(accesses)
    }

    fn authorized_reference(
        &self,
        loan: &Formed,
        place: &super::Lineage,
        at: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
        formed: &[Formed],
    ) -> Checked<bool> {
        let crossing = place
            .segments
            .iter()
            .rposition(|step| *step == terminal_check_tables::bindings::Segment::Referent);
        let descriptor = if let Some(crossing) = crossing {
            super::Lineage {
                root: place.root,
                segments: place.segments[..crossing].to_vec(),
            }
        } else if matches!(self.shape(place, Construct::Machine)?, super::Shape::View)
            || matches!(self.shape(place, Construct::Machine)?, super::Shape::Typed(ty)
                if terminal_semantics::descriptor_results::is_descriptor(self.types.iter(), ty))
        {
            place.clone()
        } else if self.incoming(place.root).is_some()
            && matches!(
                self.root_kind(place.root, Construct::Machine)?,
                StructuralPlaceKind::BlockParameter { .. }
            )
        {
            super::Lineage {
                root: place.root,
                segments: Vec::new(),
            }
        } else {
            return Ok(false);
        };
        let descriptor = descriptor.descriptor_location().ok_or_else(|| {
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
                life.surviving
                    .get(&at)
                    .is_some_and(|names| !names.is_empty())
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

    /// An edge transports the original borrow, not a fresh incoming resource.
    /// Capture all actuals before binding names or cleaning the old frontier.
    fn successor_borrows(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        formed: &[Formed],
    ) -> Checked<()> {
        for block in &self.lowered.blocks {
            let successors = block.terminator.successor_edges();
            let arguments = block.terminator.successor_block_arguments();
            for ((edge, target), arguments) in successors.into_iter().zip(arguments) {
                let concerns = Construct::Edge(edge);
                let target = self.block(target)?;
                if arguments.len() != target.structural_parameters.len() {
                    return Err(self.foreign(concerns, "successor borrow telescope is incomplete"));
                }
                let at = self.loan_position(ProgramPoint::Edge(edge), concerns)?;
                let mut captured = Vec::new();
                for (argument, parameter) in arguments.iter().zip(&target.structural_parameters) {
                    let Some(access) = reference_access(parameter.access) else {
                        continue;
                    };
                    let raw = self.argument(argument, concerns)?;
                    let resource = self.borrowed_resource(parameter);
                    let (_, parent, from, permitted) =
                        self.source_lender(&raw, &argument.path, access, resource, concerns)?;
                    if !permitted
                        || access.is_exclusive() && parent == Some(ReferenceAccess::Shared)
                    {
                        return Err(crate::refusal(
                            RefusalReason::Conflict,
                            self.machine,
                            concerns,
                            "successor borrow exceeds its original parent access",
                        ));
                    }
                    let backing = self.borrow_backing(references, &raw, resource, at, concerns)?;
                    if let Some(descriptor) = from {
                        terminal_semantics::borrowed_loan_source(
                            self.module,
                            self.lowered,
                            references,
                            at,
                            &descriptor,
                            formed.iter().filter_map(|loan| {
                                loan.lifetime.as_ref().map(|life| (loan.obligation, life))
                            }),
                        )
                        .map_err(|reason| self.unsupported(concerns, reason))?;
                    }
                    for (other_access, other) in &captured {
                        if let Some(conflict) =
                            competing_borrows(access, &backing, *other_access, other)
                        {
                            let detail = match conflict {
                                LoanConflict::TwiceExclusive => {
                                    "successor arguments duplicate an exclusive borrow"
                                }
                                _ => "successor arguments overlap shared and exclusive borrows",
                            };
                            return Err(crate::refusal(
                                RefusalReason::Conflict,
                                self.machine,
                                concerns,
                                detail,
                            ));
                        }
                    }
                    for loan in formed {
                        if loan
                            .lifetime
                            .as_ref()
                            .is_some_and(|life| life.live.contains(&at))
                            && competing_borrows(loan.access, &loan.lineage, access, &backing)
                                .is_some()
                            && !self.authorized_reference(loan, &raw, at, formed)?
                        {
                            return Err(crate::refusal(
                                RefusalReason::Conflict,
                                self.machine,
                                concerns,
                                "successor borrow conflicts with an original live loan",
                            ));
                        }
                    }
                    captured.push((access, backing));
                }
            }
        }
        Ok(())
    }

    fn parents(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        module: &terminal_psi::TerminalModule,
        formed: &[Formed],
    ) -> Checked<()> {
        for loan in formed {
            let Some(descriptor) = &loan.from else {
                continue;
            };
            let source = terminal_semantics::borrowed_loan_source(
                module,
                self.lowered,
                references,
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
            if loan.lender != loan_lender(source) {
                return Err(self.foreign(
                    Construct::Obligation(loan.obligation),
                    "reborrow names a different immediate parent occurrence",
                ));
            }
            if let terminal_semantics::BorrowedLoanSource::Loan(parent) = source {
                let owner = formed
                    .iter()
                    .find(|other| other.obligation == parent)
                    .ok_or_else(|| {
                        self.foreign(
                            Construct::Obligation(loan.obligation),
                            "reborrow parent has no actual formation",
                        )
                    })?;
                if !loan.lineage.respects_parent(&owner.lineage) {
                    return Err(self.foreign(
                        Construct::Obligation(loan.obligation),
                        &format!(
                            "reborrow changes its parent's original captured referent: parent {:?}, child {:?} at {:?}, descriptor {:?}",
                            owner.lineage, loan.lineage, loan.at, loan.from,
                        ),
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

    /// A descriptor's last direct use does not restore its loan while a child
    /// still uses that authority. Close liveness over the authenticated forest,
    /// without making child descriptors names for their parent's permission.
    fn retain_ancestors(&self, formed: &mut [Formed]) -> Checked<()> {
        let mut positions = formed
            .iter()
            .enumerate()
            .map(|(position, loan)| (loan.obligation, position))
            .collect::<Vec<_>>();
        positions.sort_unstable_by_key(|(identity, _)| *identity);
        let mut parents = Vec::with_capacity(formed.len());
        let mut children = vec![0usize; formed.len()];
        for loan in formed.iter() {
            let parent = match loan.lender {
                Lender::Loan(identity) => {
                    let position = positions
                        .binary_search_by_key(&identity, |(identity, _)| *identity)
                        .map_err(|_| {
                            self.foreign(
                                Construct::Obligation(loan.obligation),
                                "loan ancestry names no actual parent occurrence",
                            )
                        })?;
                    let parent = positions[position].1;
                    children[parent] += 1;
                    Some(parent)
                }
                Lender::Owner | Lender::Incoming | Lender::Provider => None,
            };
            parents.push(parent);
        }
        let mut pending = children
            .iter()
            .enumerate()
            .filter_map(|(position, count)| (*count == 0).then_some(position))
            .collect::<Vec<_>>();
        let mut closed = 0;
        while let Some(child) = pending.pop() {
            closed += 1;
            let Some(parent) = parents[child] else {
                continue;
            };
            let (owner, descendant) = if parent < child {
                let (before, after) = formed.split_at_mut(child);
                (&mut before[parent], &after[0])
            } else {
                let (before, after) = formed.split_at_mut(parent);
                (&mut after[0], &before[child])
            };
            if let Some(lifetime) = &descendant.lifetime {
                let retained = owner.lifetime.as_mut().ok_or_else(|| {
                    self.foreign(
                        Construct::Obligation(owner.obligation),
                        "a retained child has no retained parent occurrence",
                    )
                })?;
                retained.live.extend(lifetime.live.iter().copied());
            }
            children[parent] -= 1;
            if children[parent] == 0 {
                pending.push(parent);
            }
        }
        if closed != formed.len() {
            return Err(self.foreign(
                Construct::Machine,
                "loan ancestry does not reach an original lender",
            ));
        }
        Ok(())
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
        references: &mut super::DescriptorAnalysis<'a>,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
    ) -> Checked<Vec<Formed>> {
        let mut claimed = vec![false; tables.obligations.len()];
        let mut formed = Vec::new();
        let flow = if self.lowered.blocks.iter().any(|block| {
            block
                .structural_parameters
                .iter()
                .any(|parameter| parameter.access != StructuralAccess::Owned)
        }) || self
            .lowered
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::EstablishReference { .. }
                        | terminal_psi::OperationKind::EstablishElementView { .. }
                        | terminal_psi::OperationKind::EstablishMutableElementView { .. }
                )
            }) {
            Some(references.flow(self.machine).map_err(|error| {
                self.unsupported(
                    error
                        .operation(module)
                        .map(Construct::Operation)
                        .unwrap_or(Construct::Machine),
                    error
                        .detail()
                        .unwrap_or("persistent loans have no complete descriptor flow"),
                )
            })?)
        } else {
            None
        };
        for block in &self.lowered.blocks {
            let mut before = ProgramPoint::BlockEntry(block.id);
            for operation in &block.operations {
                let concerns = Construct::Operation(operation.id);
                self.reference_loan(
                    references,
                    tables,
                    operation,
                    before,
                    flow.as_ref(),
                    &mut claimed,
                    &mut formed,
                )?;
                let arguments = operation.kind.structural_arguments();
                let site = InvocationSite::Operation(operation.id);
                let call = crate::call_contracts::inputs_at(self.lowered, site)?.map(|_| site);
                let retained = terminal_semantics::reference_call_lifetimes(
                    module,
                    self.lowered,
                    operation,
                    references,
                )
                .map_err(|error| graph_refusal(module, error))?;
                self.lent(
                    references,
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
            if let Some(site) = crate::call_contracts::terminator_site(&block.terminator) {
                let inputs =
                    crate::call_contracts::inputs_at(self.lowered, site)?.ok_or_else(|| {
                        self.foreign(
                            Construct::Block(block.id),
                            "terminal call lost its canonical inputs",
                        )
                    })?;
                let first = formed.len();
                self.lent(
                    references,
                    module,
                    tables,
                    (Construct::Block(block.id), before),
                    inputs.structural_arguments,
                    Some(site),
                    Vec::new(),
                    &mut claimed,
                    &mut formed,
                )?;
                self.carried(tables, block.terminator.edge(), &formed[first..])?;
            }
            let successors = block.terminator.successor_edges();
            for ((edge, target), arguments) in successors
                .into_iter()
                .zip(block.terminator.successor_block_arguments())
            {
                let concerns = Construct::Edge(edge);
                let before = ProgramPoint::Edge(edge);
                let at = self.loan_position(before, concerns)?;
                for (position, (argument, parameter)) in arguments
                    .iter()
                    .zip(&self.block(target)?.structural_parameters)
                    .enumerate()
                {
                    let Some(access) = reference_access(parameter.access) else {
                        continue;
                    };
                    let authority = terminal_semantics::structural_projection_authority(
                        self.lowered,
                        self.types,
                        argument,
                    )
                    .ok_or_else(|| {
                        self.foreign(concerns, "successor borrow has no actual parent permission")
                    })?;
                    let root = self
                        .lowered
                        .structural_place_type(argument.place)
                        .ok_or_else(|| {
                            self.foreign(
                                concerns,
                                "successor borrow has no original storage declaration",
                            )
                        })?;
                    if authority == parameter.access
                        && terminal_semantics::projection_stays_in_inline_storage(
                            self.types.iter(),
                            root,
                            &argument.path,
                        )
                    {
                        continue;
                    }
                    let raw = self.argument(argument, concerns)?;
                    let captured = self.interned(tables, &raw, concerns)?.ok_or_else(|| {
                        self.foreign(concerns, "successor borrow has no captured place row")
                    })?;
                    let resource = self.borrowed_resource(parameter);
                    let (expected_lender, parent, from, permitted) =
                        self.source_lender(&raw, &argument.path, access, resource, concerns)?;
                    let (obligation, row) = tables.obligations.iter().find(|(id, row)| {
                        !claimed[index(id.get())] && row.created_at == before
                            && matches!(row.kind, ObligationKind::Loan { captured: place, access: actual, .. } if place == captured && actual == access)
                    }).ok_or_else(|| self.foreign(concerns, "new successor borrow has no exact Loan occurrence row"))?;
                    let ObligationKind::Loan { lender, .. } = row.kind else {
                        unreachable!()
                    };
                    if expected_lender.is_some_and(|expected| expected != lender) {
                        return Err(self.foreign(
                            concerns,
                            "successor lender differs from its original resource",
                        ));
                    }
                    let lifetime =
                        terminal_semantics::descriptor_lifetimes::descriptor_successor_lifetime(
                            flow.as_ref().ok_or_else(|| {
                                self.unsupported(
                                    concerns,
                                    "successor loan has no complete descriptor flow",
                                )
                            })?,
                            at,
                            (target, 0),
                            &terminal_semantics::descriptor_lifetimes::DescriptorLocation::whole(
                                parameter.place,
                            ),
                        )
                        .ok_or_else(|| {
                            self.foreign(concerns, "successor loan has no exact selected arrival")
                        })?;
                    if lifetime.live.contains(&at) {
                        return Err(self.unsupported(
                            concerns,
                            "successor loan crosses its own formation without a fresh occurrence",
                        ));
                    }
                    claimed[index(obligation.get())] = true;
                    formed.push(Formed {
                        obligation,
                        invocation: None,
                        position,
                        access,
                        lineage: self.borrow_backing(references, &raw, resource, at, concerns)?,
                        parent,
                        lender,
                        from,
                        permitted,
                        at,
                        lifetime: Some(lifetime),
                        returned_by: None,
                    });
                }
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
        let mut disposed = vec![false; tables.obligations.len()];
        for (edge, row) in tables.edges.iter() {
            for disposition in &row.dispositions {
                let Some(loan) = formed.iter().find(|loan| {
                    loan.obligation == disposition.obligation
                        && loan.invocation.is_some()
                        && loan.lifetime.is_none()
                }) else {
                    continue;
                };
                let invocation = tables
                    .invocations
                    .get(loan.invocation.unwrap())
                    .ok_or_else(|| {
                        self.foreign(Construct::Edge(edge), "disposed loan has no invocation")
                    })?;
                let expected = match invocation.site {
                    InvocationSite::InvokeNoNormal(site)
                    | InvocationSite::ExternalCompletion(site) => site,
                    // Cleanup owns an exact edge/action invocation but returns
                    // normally. It cannot consume a duration loan as a terminal
                    // transfer; its contained references follow cleanup cohorts.
                    InvocationSite::Operation(_) | InvocationSite::Cleanup { .. } => {
                        return Err(self.foreign(
                            Construct::Edge(edge),
                            "normal call loan has an edge disposition",
                        ));
                    }
                };
                if expected != edge
                    || disposition.fate != Fate::Consumed
                    || std::mem::replace(&mut disposed[index(loan.obligation.get())], true)
                {
                    return Err(self.foreign(
                        Construct::Edge(edge),
                        "loan disposition repeats or differs from its terminal transfer",
                    ));
                }
            }
        }
        Ok(formed)
    }

    /// The advertised return paths must be the paths the body actually
    /// returns. A header cannot turn a local borrow into caller-owned storage.
    fn reference_results(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        module: &terminal_psi::TerminalModule,
    ) -> Checked<()> {
        let Some(result) = self.lowered.result.structural().filter(|result| {
            terminal_psi::structural_type_contains_reference_by(
                |id| module.structural_type(id),
                result.structural_type,
            )
        }) else {
            return Ok(());
        };
        let concerns = Construct::Machine;
        let slots = references.result_slots(self.machine).ok_or_else(|| {
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

    /// The arrival's address loan stays in its activation-local carrier.
    /// A different loan stored through that address is reference contents in
    /// the original storage and still needs caller output correspondence.
    fn retained_in_caller_storage(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
        loan: &Formed,
        name: &DescriptorLocation,
        at: (semantic_vocabulary::BlockId, u32),
        concerns: Construct,
    ) -> Checked<bool> {
        let borrowed_arrival =
            matches!(
                self.root_kind(name.root, concerns)?,
                StructuralPlaceKind::BlockParameter { .. }
            ) && terminal_semantics::structural_argument_source(self.lowered, name.root, 0)
                .is_some_and(|parameter| {
                    parameter.access != StructuralAccess::Owned
                        && !terminal_semantics::descriptor_results::is_descriptor(
                            self.types.iter(),
                            parameter.structural_type,
                        )
                });
        if !borrowed_arrival {
            return Ok(self.incoming(name.root).is_some());
        }
        let selected = super::Lineage {
            root: name.root,
            segments: name.path.clone(),
        };
        let backing = self.backing(references, &selected, at.0, at.1 as usize, concerns)?;
        if name.path.is_empty()
            && matches!(&loan.lineage, LoanRegion::Storage(region) if *region == backing)
        {
            return Ok(false);
        }
        if backing == selected {
            return Err(self.unsupported(
                concerns,
                "retained reference has no original arrival backing",
            ));
        }
        Ok(self.incoming(backing.root).is_some())
    }

    /// Returning a child keeps its incoming backing, but cannot leave an
    /// intermediate local parent or another child undisposed. Check each
    /// actual return against the existing occurrence/cohort judgment; names
    /// without a proved disposition remain unavailable.
    fn reference_completions(
        &self,
        references: &mut super::DescriptorAnalysis<'a>,
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
                if let Some(names) = loan
                    .lifetime
                    .as_ref()
                    .and_then(|lifetime| lifetime.authorized.get(&at))
                {
                    for name in names {
                        if self.retained_in_caller_storage(references, loan, name, at, concerns)? {
                            return Err(self.unsupported(concerns,
                                "a reference retained in caller storage needs its checked output-loan correspondence"));
                        }
                    }
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
                        references,
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
                            Lender::Owner | Lender::Incoming | Lender::Provider => Vec::new(),
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
        references: &mut super::DescriptorAnalysis<'a>,
        module: &terminal_psi::TerminalModule,
        tables: &MachineTables,
    ) -> Checked<(Vec<(ObligationId, Checked<()>)>, Vec<ConflictingLoan>)> {
        self.reference_results(references, module)?;
        let mut formed = self.formed(references, module, tables)?;
        self.parents(references, module, &formed)?;
        self.successor_borrows(references, &formed)?;
        self.reference_completions(references, module, &formed)?;
        self.retain_ancestors(&mut formed)?;
        let mut cleanup_flow = std::collections::BTreeMap::new();
        if formed.iter().any(|loan| loan.lifetime.is_some()) {
            for block in &self.lowered.blocks {
                let position = (
                    block.id,
                    crate::position(block.operations.len(), self.machine)?,
                );
                for (edge, actions) in block.terminator.cleanup_rosters() {
                    for (ordinal, action) in actions.iter().enumerate() {
                        if matches!(
                            action,
                            terminal_psi::TerminalAffineCleanupAction::InvokeOwned(_)
                        ) {
                            let ordinal = crate::position(ordinal, self.machine)?;
                            let effect = references
                                .invocation_flow_at(
                                    self.machine,
                                    position,
                                    terminal_psi::InvocationOwner::Cleanup {
                                        edge,
                                        action_ordinal: ordinal,
                                    },
                                )
                                .map_err(|_| {
                                    self.unsupported(
                                        Construct::Edge(edge),
                                        "cleanup has no exact call-flow effects",
                                    )
                                })?;
                            cleanup_flow.insert((edge, ordinal), effect);
                        }
                    }
                }
            }
        }
        let cleanup_cohorts = self.cleanup_cohorts(&formed, &cleanup_flow)?;
        let mut verdicts = Vec::with_capacity(formed.len());
        let mut conflicts = Vec::new();
        for (at, loan) in formed.iter().enumerate() {
            let used = self.uses_retained_loan(references, loan, &formed, &cleanup_cohorts)?;
            let site = used.map_or(Construct::Obligation(loan.obligation), |(at, _)| at);
            let exclusive = loan.access.is_exclusive();
            let mut found = used.map(|(_, conflict)| (None, conflict));
            if found.is_none() && exclusive && loan.parent == Some(ReferenceAccess::Shared) {
                found = Some((None, LoanConflict::ExclusiveOfShared));
            }
            if found.is_none() && !loan.permitted {
                found = Some((None, LoanConflict::AccessExceedsParent));
            }
            if found.is_none() {
                // The later listed loan reports the conflict. Retained
                // lifetimes determine overlap across calls and block order;
                // arena ordering is not lifetime ordering.
                for earlier in formed[..at].iter().filter(|earlier| {
                    // A retained region and the loans of the call that
                    // returned it: those loans end as the region begins.
                    let returned = |region: &Formed, call: &Formed| {
                        region.returned_by.is_some() && region.returned_by == call.invocation
                    };
                    if returned(loan, earlier) || returned(earlier, loan) {
                        return false;
                    }
                    (earlier.invocation.is_some() && earlier.invocation == loan.invocation)
                        || earlier
                            .lifetime
                            .as_ref()
                            .is_some_and(|life| holds(life, &loan.at))
                        || loan
                            .lifetime
                            .as_ref()
                            .is_some_and(|life| holds(life, &earlier.at))
                }) {
                    if self.descends(loan, earlier.obligation, &formed)?
                        || self.descends(earlier, loan.obligation, &formed)?
                    {
                        continue;
                    }
                    if let Some(conflict) = competing_borrows(
                        loan.access,
                        &loan.lineage,
                        earlier.access,
                        &earlier.lineage,
                    ) {
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
                        source_span: None,
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

/// Whether a loan with `lifetime` is still held at `at`: while a carrier
/// still holds it (until its release, even after its last observation), and
/// wherever a later use still depends on it. A view whose origin is unknown,
/// such as one an opaque boundary returned from its input, has no carrier
/// that names the input's loan, yet the input stays borrowed until the view's
/// last use (lifetimes.md, "Returned views": the source must "remain live for
/// the result's use").
/// How one effect touches storage a retained loan may cover.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Touch {
    Read,
    Change,
    /// An atomic access changes storage that a shared loan may also observe.
    AtomicChange,
}

fn holds(
    lifetime: &terminal_semantics::descriptor_lifetimes::DescriptorLifetime,
    at: &terminal_semantics::descriptor_lifetimes::DescriptorPosition,
) -> bool {
    lifetime.live.contains(at)
        || lifetime
            .surviving
            .get(at)
            .is_some_and(|names| !names.is_empty())
}
