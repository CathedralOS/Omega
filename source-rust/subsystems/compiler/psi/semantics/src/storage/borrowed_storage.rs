use crate::scalars::scalar_ordering::ScalarOrdering;
use arena::IdTable;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralPathSegment;

pub type Position = (BlockId, u32);
pub type AliasSources = BTreeMap<PlaceId, Vec<(PlaceId, Projection)>>;

/// Prepare each body's frame once from the same canonical storage flow
/// used for captured aliases and descriptor custody. A missing analysis
/// leaves the ownership ceiling intact; it never means no writes.
pub fn inferred_write_frames(
    module: &terminal_psi::TerminalModule,
) -> Result<IdTable<MachineId, terminal_psi::PublishedWriteFrame>, String> {
    use crate::descriptor_results::{
        DescriptorAnalysis, DescriptorGraphError, DescriptorSlotKind,
    };
    use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;

    let mut frames = IdTable::default();
    for machine in &module.machines {
        if frames.push(None) != machine.id {
            return Err("write-frame machines are not at their declared positions".to_owned());
        }
    }
    let mut graph = Vec::new();
    for row in crate::descriptor_results::terminal_graph_rows(
        module,
        module.machines.iter().map(|machine| machine.id),
        DescriptorSlotKind::BorrowedStorage,
        &[],
    ) {
        match row {
            Ok(body) => graph.push(body),
            Err(DescriptorGraphError::IncompleteMachine { .. }) => {}
            Err(DescriptorGraphError::MissingOrAmbiguousMachine(machine)) => {
                return Err(format!("write frame has no unique machine {machine:?}"));
            }
        }
    }
    // Later local summaries may strengthen scalar entry and result facts.
    // They must not silently narrow a published storage-effect contract.
    // Retain the actual alias/custody flow and infer a conservative frame
    // without those scalar premises on either producer or receiver.
    for machine in &mut graph {
        machine.payload_entry_predicates = None;
        machine.payload_scalar_postconditions.clear();
        machine.scalar_payload_writes.clear();
    }
    let mut analysis =
        DescriptorAnalysis::with_borrowed_storage_slots(&module.structural_types, &graph);
    for body in &graph {
        // The frame roster above checked dense module positions before
        // graph construction, so this is the same original body owner.
        let position = usize::try_from(body.id.get() - 1)
            .map_err(|_| "write-frame machine position cannot be represented")?;
        let machine = module
            .machines
            .get(position)
            .ok_or("write-frame source machine disappeared")?;
        let Some(written) = analysis
            .write_frame(machine.id)
            .map_err(|detail| format!("write frame for {:?}: {detail}", machine.id))?
        else {
            continue;
        };
        let mut paths = Vec::with_capacity(written.len());
        for region in written {
            let mut parameters = machine
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.place == region.root);
            let (position, _) = parameters
                .next()
                .ok_or("write frame names storage outside its structural formals")?;
            if parameters.next().is_some() {
                return Err("write frame has an ambiguous structural formal".to_owned());
            }
            let position = u32::try_from(position)
                .map_err(|_| "write-frame formal ordinal cannot be represented")?;
            let mut fields = Vec::with_capacity(region.path.len());
            for segment in &region.path {
                let DescriptorPathSegment::Field(field) = segment else {
                    return Err("published write frame contains a non-field step".to_owned());
                };
                fields.push(*field);
            }
            paths.push((position, fields));
        }
        paths.sort_unstable();
        paths.dedup();
        *frames
            .get_mut(machine.id)
            .ok_or("write-frame owner disappeared")? = Some(paths);
    }
    Ok(frames)
}

/// A supplied frame is usable only when the canonical body derives it.
/// Omitting a summary keeps the ceiling and grants no precision. A
/// bodyless header cannot invent an empty or narrowed write promise.
pub fn validate_write_frames(module: &terminal_psi::TerminalModule) -> Result<(), String> {
    if module
        .declaration_contracts
        .iter()
        .any(|contract| contract.published_writes.is_some())
    {
        return Err("a bodyless contract claims an inferred write frame".to_owned());
    }
    if module
        .machines
        .iter()
        .all(|machine| machine.contract.published_writes.is_none())
    {
        return Ok(());
    }
    let inferred = inferred_write_frames(module)?;
    for machine in &module.machines {
        let Some(published) = &machine.contract.published_writes else {
            continue;
        };
        if inferred.get(machine.id).and_then(Option::as_ref) != Some(published) {
            return Err(format!(
                "published write frame for {:?} differs from its body",
                machine.id
            ));
        }
    }
    Ok(())
}

/// The referents a callee can store into through the `&mut` references
/// carried by a value it receives with `access`: one path per reference,
/// relative to the value and ending in `Referent`, outermost first.
/// Lowering writes each of them at a call that has no narrower frame,
/// and the checker derives the same rows, because a caller's fact
/// survives a call only when its storage is disjoint from the callee's
/// frame, "aliases included" (dependent_values.md, "Mutation frames").
/// A shared or write-only borrow of the value grants no store through
/// what it carries, and a shared reference grants none beneath it
/// (loans.md, "Reborrow lineage and access"). `None` when the referents
/// are not a finite list of exact paths: a recursive type, an array of
/// open extent, or an exclusive view of values that carry references.
pub fn carried_exclusive_referents(
    types: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
    access: terminal_psi::StructuralAccess,
) -> Option<Vec<Vec<terminal_psi::ResolvedStructuralPathSegment>>> {
    use terminal_psi::{
        ResolvedStructuralPathSegment as Segment, StructuralAccess, StructuralFieldType,
        StructuralTypeShape,
    };
    type Paths = Vec<Vec<Segment>>;

    fn walk(
        types: &[terminal_psi::StructuralTypeDeclaration],
        structural_type: semantic_vocabulary::StructuralTypeId,
        active: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    ) -> Option<Paths> {
        if active.contains(&structural_type) || active.len() > 64 {
            return None;
        }
        let declaration = crate::exactly_one::exactly_one(types.iter(), |declaration| {
            declaration.id == structural_type
        })?;
        active.push(structural_type);
        let mut found = Paths::new();
        let beneath = |prefix: &[Segment], paths: Paths, found: &mut Paths| {
            found.extend(
                paths
                    .into_iter()
                    .map(|path| prefix.iter().copied().chain(path).collect()),
            );
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { referent, access } => match access {
                StructuralAccess::MutableBorrow => {
                    found.push(vec![Segment::Referent]);
                    let paths = walk(types, *referent, active)?;
                    beneath(&[Segment::Referent], paths, &mut found);
                }
                StructuralAccess::WriteOnlyBorrow => found.push(vec![Segment::Referent]),
                StructuralAccess::SharedBorrow | StructuralAccess::Owned => {}
            },
            StructuralTypeShape::FixedArray { element, length } => {
                let element = walk(types, *element, active)?;
                if !element.is_empty() {
                    let length = length.literal().filter(|length| *length <= 4096)?;
                    for index in 0..length {
                        for path in &element {
                            found.push(
                                [Segment::FixedIndex(index)]
                                    .into_iter()
                                    .chain(path.iter().copied())
                                    .collect(),
                            );
                        }
                    }
                }
            }
            StructuralTypeShape::ElementView {
                element,
                access: Some(StructuralAccess::MutableBorrow),
            } if !walk(types, *element, active)?.is_empty() => return None,
            _ => {}
        }
        for (case, field) in declaration.shape.fields_with_case() {
            let StructuralFieldType::Structural(child) = field.field_type else {
                continue;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let prefix: &[Segment] = match case {
                Some(case) => &[Segment::Case(case.id), Segment::Field(field.id)],
                None => &[Segment::Field(field.id)],
            };
            let paths = walk(types, child, active)?;
            beneath(prefix, paths, &mut found);
        }
        active.pop();
        Some(found)
    }

    match access {
        StructuralAccess::MutableBorrow | StructuralAccess::Owned => {
            walk(types, structural_type, &mut Vec::new())
        }
        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow => Some(Vec::new()),
    }
}

/// Substitute a published frame through the call's already evaluated
/// operands. These are invalidation regions, not new borrows or move
/// permissions. An unserved reference origin retains the call ceiling.
pub fn call_write_regions(
    module: &terminal_psi::TerminalModule,
    caller: &terminal_psi::TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<Option<Vec<Region>>, String> {
    use semantic_vocabulary::CanonicalStructuralPathSegment;

    let Some(call) = terminal_psi::direct_call_parts(&operation.kind) else {
        return Ok(None);
    };
    let mut callees = module
        .machines
        .iter()
        .filter(|callee| callee.id == call.callee);
    let callee = callees.next().ok_or("write-frame call has no callee")?;
    if callees.next().is_some() {
        return Err("write-frame call has an ambiguous callee".to_owned());
    }
    let Some(frame) = &callee.contract.published_writes else {
        return Ok(None);
    };
    if call.structural_arguments.len() != callee.structural_parameters.len() {
        return Err("write-frame actuals do not match the structural telescope".to_owned());
    }
    let mut regions = Vec::with_capacity(frame.len());
    for (parameter, fields) in frame {
        let parameter = usize::try_from(*parameter)
            .map_err(|_| "write-frame formal ordinal cannot be represented")?;
        let formal = callee
            .structural_parameters
            .get(parameter)
            .ok_or("write frame names a missing structural formal")?;
        let actual = call
            .structural_arguments
            .get(parameter)
            .ok_or("write frame has no evaluated actual")?;
        let root = caller
            .structural_place_type(actual.place)
            .ok_or("write-frame actual has no declared root")?;
        let selected = crate::runtime_structural_path_tip(
            module.structural_types.iter(),
            root,
            &actual.path,
        )
        .map(|declaration| declaration.id)
        .or_else(|| {
            crate::borrowed_primitive_projection_declaration(
                module.structural_types.iter(),
                root,
                &actual.path,
                Some(formal.structural_type),
            )
        });
        if selected != Some(formal.structural_type) {
            return Err("write-frame actual does not have its formal's exact type".to_owned());
        }
        let mut path = actual.path.clone();
        if let Some((last, prefix)) = fields.split_last() {
            let prefix: Vec<_> = prefix
                .iter()
                .copied()
                .map(CanonicalStructuralPathSegment::Field)
                .collect();
            let relative = crate::canonical_field_storage_region(
                module.structural_types.iter(),
                formal.structural_type,
                &prefix,
                *last,
            );
            if relative.len() != fields.len() {
                return Err(
                    "write frame does not follow its formal's declared fields".to_owned()
                );
            }
            path.extend(relative);
        }
        let field_path: Vec<_> = fields
            .iter()
            .copied()
            .map(CanonicalStructuralPathSegment::Field)
            .collect();
        if crate::shared_mutation(
            module.structural_types.iter(),
            formal.structural_type,
            &field_path,
        ) == crate::SharedMutation::Unbounded
            || crate::canonical_structural_path_tip(
                module.structural_types.iter(),
                formal.structural_type,
                &field_path,
            )
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference { .. }
                        | terminal_psi::StructuralTypeShape::ElementView { .. }
                        | terminal_psi::StructuralTypeShape::ByteSequence(_)
                )
            })
        {
            return Ok(None);
        }
        if path.iter().any(|step| {
            matches!(
                step,
                StructuralPathSegment::Referent
                    | StructuralPathSegment::RepresentationView { .. }
            )
        }) {
            return Ok(None);
        }
        let region =
            crate::projected_storage_region(module.structural_types.iter(), root, &path, None);
        if region.indirect {
            return Ok(None);
        }
        regions.push(Region {
            root: Some(actual.place),
            projection: Projection::new(&region.path),
        });
    }
    Ok(Some(regions))
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Projection {
    address: Option<Vec<AddressStep>>,
    descriptor: DescriptorPrefix,
    pub path: Vec<StructuralPathSegment>,
    pub exact: bool,
    pub indirect: bool,
    pub window: Option<(ValueId, ValueId)>,
    pub captured_at: Option<Position>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DescriptorPrefix {
    Open(Vec<StructuralPathSegment>),
    Closed(Vec<StructuralPathSegment>),
    Unknown,
}

/// One step of a projection's captured address. Stripping runtime
/// coordinates for publication erases each runtime selector to a
/// coordinate-free step rather than forgetting the whole address, so a
/// returned view of one runtime element still names the storage after
/// that element (`cells[*].x`) instead of the whole array it lies in.
/// A forgotten public region has no address at all, which keeps the
/// two kinds of inexact bound apart.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AddressStep {
    Path(Vec<StructuralPathSegment>, Option<Position>),
    Window {
        start: ValueId,
        end: ValueId,
        at: Position,
    },
    /// A runtime element selector whose index was stripped.
    ErasedElement,
    /// A runtime window whose bounds were stripped.
    ErasedWindow,
}

impl Projection {
    pub fn new(path: &[StructuralPathSegment]) -> Self {
        let length = path
            .iter()
            .position(|segment| {
                !matches!(
                    segment,
                    StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RepresentationView { .. }
                )
            })
            .unwrap_or(path.len());
        let bounded_length = length
            + usize::from(matches!(
                path.get(length),
                Some(
                    StructuralPathSegment::FixedRange { .. }
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            ));
        Self {
            descriptor: if length == path.len() {
                DescriptorPrefix::Open(path.to_vec())
            } else {
                DescriptorPrefix::Unknown
            },
            address: Some(if path.is_empty() {
                Vec::new()
            } else {
                vec![AddressStep::Path(path.to_vec(), None)]
            }),
            path: path[..bounded_length].to_vec(),
            exact: length == path.len(),
            indirect: crate::crosses_unaliased_reference(path),
            window: None,
            captured_at: None,
        }
    }

    pub fn at(mut self, position: Position) -> Self {
        if let Some(steps) = &mut self.address {
            for step in steps {
                if let AddressStep::Path(path, captured @ None) = step
                    && path.iter().any(|segment| segment.runtime_index().is_some())
                {
                    *captured = Some(position);
                }
            }
        }
        if self.window.is_some()
            || self
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
        {
            self.captured_at = Some(position);
        }
        self
    }

    /// Recover the complete captured address, including fields after a
    /// dynamic selector. A widened region or a window has no single such
    /// path; a dynamic coordinate must retain its formation point.
    pub fn captured_storage_path(&self) -> Option<Vec<StructuralPathSegment>> {
        if self.indirect || self.window.is_some() {
            return None;
        }
        let mut result = Vec::new();
        for step in self.address.as_ref()? {
            let AddressStep::Path(path, at) = step else {
                return None;
            };
            if path.iter().any(|segment| segment.runtime_index().is_some()) && at.is_none() {
                return None;
            }
            result.extend_from_slice(path);
        }
        Some(result)
    }

    /// Validate the complete retained address without erasing dynamic
    /// capture coordinates. A window is complete but not a single path;
    /// erased or forgotten addresses never form a complete storage family.
    pub fn validate_captured_storage_address(
        &self,
        mut position_exists: impl FnMut(Position) -> bool,
    ) -> Result<Option<Vec<StructuralPathSegment>>, &'static str> {
        let steps = self
            .address
            .as_ref()
            .ok_or("captured storage address was forgotten")?;
        let mut path = Vec::new();
        let mut window = false;
        for step in steps {
            match step {
                AddressStep::Path(segments, at) => {
                    if segments.iter().any(|step| step.runtime_index().is_some())
                        && at.is_none()
                    {
                        return Err("captured storage selector has no formation position");
                    }
                    if at.is_some_and(|at| !position_exists(at)) {
                        return Err("captured storage selector formation position is stale");
                    }
                    path.extend_from_slice(segments);
                }
                AddressStep::Window { at, .. } => {
                    if !position_exists(*at) {
                        return Err("captured storage window formation position is stale");
                    }
                    window = true;
                }
                AddressStep::ErasedElement | AddressStep::ErasedWindow => {
                    return Err("captured storage address has erased runtime coordinates");
                }
            }
        }
        Ok((!window).then_some(path))
    }

    /// Reference completeness must inspect every captured crossing. The
    /// overlap-oriented `indirect` flag intentionally skips a leading
    /// Referent and cannot authenticate that descriptor's entry identity.
    pub(crate) fn has_captured_reference_crossing(&self) -> Result<bool, &'static str> {
        self.validate_captured_storage_address(|_| true)?;
        Ok(self.address.as_ref().ok_or("captured storage address was forgotten")?.iter().any(|step| {
            matches!(step, AddressStep::Path(path, _) if path.contains(&StructuralPathSegment::Referent))
        }))
    }

    /// The complete finite captured path, including Referent crossings.
    /// Unlike captured_storage_path, this grants no inline-storage or loan
    /// permission. Windows retain an address account but are not one path.
    pub fn captured_address_path(&self) -> Result<Vec<StructuralPathSegment>, &'static str> {
        self.validate_captured_storage_address(|_| true)?
            .ok_or("captured storage window is not a finite structural path")
    }

    /// Compare complete addresses while keeping each runtime selector's
    /// capture position. This grants no source, type or loan authority.
    pub fn captured_address_is_prefix_of(&self, other: &Self) -> bool {
        let (Some(parent), Some(child)) = (&self.address, &other.address) else {
            return false;
        };
        let complete = |steps: &[AddressStep]| {
            steps.iter().all(|step| match step {
                AddressStep::Path(path, at) => path.iter().all(|segment| match segment {
                    StructuralPathSegment::RuntimeIndex { .. } => at.is_some(),
                    StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Case(_)
                    | StructuralPathSegment::RepresentationView { .. }
                    | StructuralPathSegment::Referent => true,
                    StructuralPathSegment::AllElements
                    | StructuralPathSegment::FixedRange { .. } => false,
                }),
                AddressStep::Window { .. }
                | AddressStep::ErasedElement
                | AddressStep::ErasedWindow => false,
            })
        };
        if !complete(parent) || !complete(child) {
            return false;
        }
        // Step boundaries can differ after composition. Compare the retained
        // segments directly; both lists were checked as complete paths above.
        fn segments(
            steps: &[AddressStep],
        ) -> impl Iterator<Item = (&StructuralPathSegment, Option<Position>)> {
            steps
                .iter()
                .filter_map(|step| match step {
                    AddressStep::Path(path, at) => Some((path.as_slice(), *at)),
                    _ => None,
                })
                .flat_map(|(path, at)| path.iter().map(move |segment| (segment, at)))
        }
        let mut child = segments(child);
        segments(parent).all(|(segment, at)| {
            child.next().is_some_and(|(selected, captured)| {
                segment == selected && (segment.runtime_index().is_none() || at == captured)
            })
        })
    }

    pub fn window(start: ValueId, end: ValueId, position: Position) -> Self {
        Self::new(&[]).with_window(start, end, position)
    }

    /// A captured contiguous window of the whole original descriptor.
    /// A forgotten address, erased selector, field projection or indirect
    /// region cannot establish this correspondence.
    pub fn is_captured_whole_descriptor_window(&self) -> bool {
        !self.indirect
            && self.original_descriptor_path() == Some(&[][..])
            && self.address.as_ref().is_some_and(|steps| {
                !steps.is_empty()
                    && steps
                        .iter()
                        .all(|step| matches!(step, AddressStep::Window { .. }))
            })
    }

    pub fn with_window(mut self, start: ValueId, end: ValueId, position: Position) -> Self {
        if self.original_descriptor_path().is_none() {
            self.descriptor = DescriptorPrefix::Unknown;
        }
        if let DescriptorPrefix::Open(path) = &self.descriptor {
            self.descriptor = DescriptorPrefix::Closed(path.clone());
        }
        if let Some(steps) = &mut self.address {
            steps.push(AddressStep::Window {
                start,
                end,
                at: position,
            });
        }
        self.exact = false;
        self.window = Some((start, end));
        self.captured_at = Some(position);
        self
    }

    pub fn without_runtime_coordinates(mut self) -> Self {
        if let Some(steps) = self.address.take() {
            let mut erased = Vec::with_capacity(steps.len());
            for step in steps {
                match step {
                    AddressStep::Window { .. } => erased.push(AddressStep::ErasedWindow),
                    AddressStep::Path(path, _) => {
                        let mut run = Vec::new();
                        for segment in path {
                            if segment.runtime_index().is_some() {
                                if !run.is_empty() {
                                    erased.push(AddressStep::Path(
                                        std::mem::take(&mut run),
                                        None,
                                    ));
                                }
                                erased.push(AddressStep::ErasedElement);
                            } else {
                                run.push(segment);
                            }
                        }
                        if !run.is_empty() {
                            erased.push(AddressStep::Path(run, None));
                        }
                    }
                    step @ (AddressStep::ErasedElement | AddressStep::ErasedWindow) => {
                        erased.push(step)
                    }
                }
            }
            self.address = Some(erased);
        }
        if let Some(position) = self
            .path
            .iter()
            .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        {
            self.path.truncate(position);
            self.exact = false;
        }
        self.window = None;
        self.captured_at = None;
        self
    }

    pub fn enclosing_storage(&self) -> Option<Self> {
        if self.indirect {
            return None;
        }
        let length = self
            .path
            .iter()
            .take_while(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            })
            .count();
        Some(Self::new(&self.path[..length]))
    }

    pub fn without_captured_address(mut self) -> Self {
        self.address = None;
        self
    }

    pub fn forget_address(&mut self) {
        if self.original_descriptor_path().is_none() {
            self.descriptor = DescriptorPrefix::Unknown;
        }
        if let DescriptorPrefix::Open(path) = &self.descriptor {
            self.descriptor = DescriptorPrefix::Closed(path.clone());
        }
        self.exact = false;
        self.address = None;
    }

    /// Keep the input field that supplied a view after its address is
    /// forgotten. Selecting another field after slicing loses that
    /// identity, even though `path` still describes overlapping storage.
    pub fn original_descriptor_path(&self) -> Option<&[StructuralPathSegment]> {
        match &self.descriptor {
            DescriptorPrefix::Open(path)
                if self.exact && *path == self.path && !self.indirect =>
            {
                Some(path)
            }
            DescriptorPrefix::Closed(path)
                if !self.exact && *path == self.path && !self.indirect =>
            {
                Some(path)
            }
            _ => None,
        }
    }

    pub fn reference_origin_path(
        &self,
        ordering: &ScalarOrdering,
    ) -> Option<Vec<terminal_psi::ReferenceOriginProjection>> {
        self.origin_path_with(|at, value| ordering.known_unsigned(at, value))
    }

    /// The coordinate-free origin of a projection whose runtime
    /// coordinates were stripped: each erased element selector becomes
    /// `Element`, and the storage named after it stays exact. `None` for
    /// a projection with no erased coordinate, an unaddressed (forgotten)
    /// region, or a stripped window not narrowed back to one element,
    /// whose start is unknown.
    pub fn stripped_origin_path(&self) -> Option<Vec<terminal_psi::ReferenceOriginProjection>> {
        if !self
            .address
            .as_ref()?
            .iter()
            .any(|step| matches!(step, AddressStep::ErasedElement | AddressStep::ErasedWindow))
        {
            return None;
        }
        self.origin_path_with(|_, _| None)
    }

    fn origin_path_with(
        &self,
        known_unsigned: impl Fn(Position, ValueId) -> Option<u64>,
    ) -> Option<Vec<terminal_psi::ReferenceOriginProjection>> {
        use terminal_psi::ReferenceOriginProjection::{Element, Exact};
        if self.indirect {
            return None;
        }
        let mut result = Vec::new();
        let mut offset: Option<Option<u64>> = None;
        let append_offset = |offset: &mut Option<Option<u64>>, start: Option<u64>| {
            *offset = Some(match (offset.unwrap_or(Some(0)), start) {
                (Some(left), Some(right)) => Some(left.checked_add(right)?),
                _ => None,
            });
            Some(())
        };
        for step in self.address.as_ref()? {
            match step {
                AddressStep::Window { start, at, .. } => {
                    append_offset(&mut offset, known_unsigned(*at, *start))?
                }
                AddressStep::ErasedWindow => append_offset(&mut offset, None)?,
                AddressStep::ErasedElement => {
                    offset = None;
                    result.push(Element);
                }
                AddressStep::Path(path, at) => {
                    for segment in path {
                        match segment {
                            StructuralPathSegment::FixedRange { start, .. } => {
                                append_offset(&mut offset, Some(*start))?
                            }
                            StructuralPathSegment::FixedIndex(_)
                            | StructuralPathSegment::RuntimeIndex { .. } => {
                                let index = match segment {
                                    StructuralPathSegment::FixedIndex(index) => Some(*index),
                                    StructuralPathSegment::RuntimeIndex { index, .. } => {
                                        known_unsigned((*at)?, *index)
                                    }
                                    _ => unreachable!(),
                                };
                                result.push(match (offset.take().unwrap_or(Some(0)), index) {
                                    (Some(start), Some(index)) => {
                                        Exact(StructuralPathSegment::FixedIndex(
                                            start.checked_add(index)?,
                                        ))
                                    }
                                    _ => Element,
                                });
                            }
                            StructuralPathSegment::Field(_)
                            | StructuralPathSegment::Case(_)
                            | StructuralPathSegment::RepresentationView { .. }
                                if offset.is_none() =>
                            {
                                result.push(Exact(segment.clone()))
                            }
                            _ => return None,
                        }
                    }
                }
            }
        }
        offset.is_none().then_some(result)
    }

    pub fn then(&self, suffix: &Self) -> Self {
        let mut composed = self.clone();
        composed.descriptor = match (
            self.original_descriptor_path(),
            suffix.original_descriptor_path(),
        ) {
            (Some(prefix), Some(tail)) => match (&self.descriptor, &suffix.descriptor) {
                (DescriptorPrefix::Open(_), DescriptorPrefix::Open(_)) => {
                    DescriptorPrefix::Open(prefix.iter().chain(tail).cloned().collect())
                }
                (DescriptorPrefix::Open(_), DescriptorPrefix::Closed(_)) => {
                    DescriptorPrefix::Closed(prefix.iter().chain(tail).cloned().collect())
                }
                (DescriptorPrefix::Closed(_), _) if tail.is_empty() => {
                    DescriptorPrefix::Closed(prefix.to_vec())
                }
                _ => DescriptorPrefix::Unknown,
            },
            _ => DescriptorPrefix::Unknown,
        };
        composed.address =
            self.address
                .as_ref()
                .zip(suffix.address.as_ref())
                .and_then(|(prefix, suffix)| {
                    let work = prefix.iter().chain(suffix).try_fold(0usize, |used, step| {
                        used.checked_add(match step {
                            AddressStep::Path(path, _) => path.len(),
                            AddressStep::Window { .. }
                            | AddressStep::ErasedElement
                            | AddressStep::ErasedWindow => 1,
                        })
                    })?;
                    if work > 4096
                        || suffix.iter().any(|step| {
                            prefix.iter().any(|prior| match (prior, step) {
                                (
                                    AddressStep::Window { at: before, .. },
                                    AddressStep::Window { at, .. },
                                ) => before == at,
                                (
                                    AddressStep::Path(previous, Some(before)),
                                    AddressStep::Path(path, Some(at)),
                                ) if before == at => path
                                    .iter()
                                    .filter_map(StructuralPathSegment::runtime_index)
                                    .any(|selector| {
                                        previous.iter().any(|segment| {
                                            segment.runtime_index() == Some(selector)
                                        })
                                    }),
                                _ => false,
                            })
                        })
                    {
                        return None;
                    }
                    let mut steps: Vec<AddressStep> = Vec::new();
                    for step in prefix.iter().chain(suffix) {
                        if let AddressStep::Path(path, at) = step
                            && let Some(AddressStep::Path(previous, captured)) =
                                steps.last_mut()
                            && (captured == at
                                || captured.is_none()
                                    && !previous
                                        .iter()
                                        .any(|segment| segment.runtime_index().is_some())
                                || at.is_none()
                                    && !path
                                        .iter()
                                        .any(|segment| segment.runtime_index().is_some()))
                        {
                            previous.extend_from_slice(path);
                            *captured = (*captured).or(*at);
                        } else {
                            steps.push(step.clone());
                        }
                    }
                    Some(steps)
                });
        composed.indirect |= suffix.indirect;
        if composed.exact {
            composed.path.extend_from_slice(&suffix.path);
            composed.exact = suffix.exact;
            composed.window = suffix.window;
            composed.captured_at = suffix.captured_at;
        }
        composed
    }
}

#[cfg(test)]
#[test]
fn exact_storage_projection_keeps_the_view_between_its_prefix_and_suffix() {
    let prefix = StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(31));
    let view = StructuralPathSegment::RepresentationView {
        structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
        stored_scalar: None,
    };
    let suffix = StructuralPathSegment::FixedIndex(3);
    let projection = Projection::new(std::slice::from_ref(&prefix))
        .then(&Projection::new(std::slice::from_ref(&view)))
        .then(&Projection::new(std::slice::from_ref(&suffix)));
    assert!(projection.exact);
    assert!(!projection.indirect);
    assert_eq!(projection.path, [prefix, view, suffix]);
}

#[derive(Clone)]
pub struct Region {
    pub root: Option<PlaceId>,
    pub projection: Projection,
}

impl Region {
    /// A logical write names its captured storage when retained. Alias
    /// bounds may widen a case payload to its containing sum; that bound
    /// remains the safe write footprint when the address was forgotten.
    pub fn written_storage_path(&self) -> Vec<StructuralPathSegment> {
        self.projection
            .captured_storage_path()
            .unwrap_or_else(|| self.projection.path.clone())
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.projection.indirect
            || other.projection.indirect
            || self.root.is_none()
            || other.root.is_none()
            || (self.root == other.root
                && crate::structural_paths_may_overlap(
                    &self.projection.path,
                    &other.projection.path,
                ))
    }
}

pub fn regions_overlap_at(
    left: &Region,
    right: &Region,
    position: Position,
    relations: &ScalarOrdering,
) -> bool {
    if !left.overlaps(right) {
        return false;
    }
    if left.projection.indirect
        || right.projection.indirect
        || left.root.is_none()
        || left.root != right.root
    {
        return true;
    }
    !window_excludes_index(&left.projection, &right.projection, relations, position)
        && !window_excludes_index(&right.projection, &left.projection, relations, position)
}

fn window_excludes_index(
    window: &Projection,
    selected: &Projection,
    relations: &ScalarOrdering,
    position: Position,
) -> bool {
    let (Some((start, end)), Some(birth)) = (window.window, window.captured_at) else {
        return false;
    };
    use crate::scalars::scalar_ordering::Term;
    let index = match selected.path.get(window.path.len()) {
        Some(StructuralPathSegment::FixedIndex(index)) => Term::Unsigned(*index),
        Some(StructuralPathSegment::RuntimeIndex { index, .. })
            if selected.captured_at.is_some_and(|captured| {
                relations.values_unchanged_since(captured, position, &[*index])
            }) =>
        {
            Term::Value(*index)
        }
        _ => return false,
    };
    if selected.path[..window.path.len()] != window.path
        || !relations.values_unchanged_since(birth, position, &[start, end])
    {
        return false;
    }
    relations.proves(position, index, Term::Value(start), true)
        || relations.proves(position, Term::Value(end), index, false)
}

pub fn resolve(aliases: &AliasSources, root: PlaceId, projection: Projection) -> Vec<Region> {
    let mut result = Vec::new();
    let mut pending = vec![(root, projection, BTreeSet::new())];
    let mut expanded = BTreeSet::new();
    while let Some((place, projection, mut visited)) = pending.pop() {
        if !visited.insert(place) {
            continue;
        }
        let key = (place, projection.clone());
        if !expanded.insert(key) {
            continue;
        }
        match aliases.get(&place) {
            Some(sources) => {
                for (source, prefix) in sources {
                    pending.push((*source, prefix.then(&projection), visited.clone()));
                }
            }
            None => result.push(Region {
                root: Some(place),
                projection,
            }),
        }
    }
    if result.is_empty() {
        result.push(Region {
            root: None,
            projection: Projection::new(&[]),
        });
    }
    result
}
#[cfg(test)]
mod tests;
