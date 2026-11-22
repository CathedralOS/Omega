//! Region parameters have declaration identity, not a runtime carrier.
use crate::{AggregateRegionSlot, LoweringError, MachineRegion};
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{GenericParameters, TypedSignature, TypedTree, TypedTrees, ValueType};

/// The region belongs to this declaration, not merely a binder at the same
/// position. Only direct place references have a slot mapping here.
pub(crate) fn direct<'a>(
    ty: &'a ValueType,
    machine: &DeclarationId,
) -> Option<(usize, &'a ValueType)> {
    let ValueType::AtLifetime(reference, region) = ty else { return None };
    let index = binder(region, machine)?;
    crate::lowering::types::place_reference(reference).map(|_| (index, reference.as_ref()))
}

pub(crate) fn slot_carrier<'a>(
    ty: &'a ValueType,
    machine: &DeclarationId,
) -> &'a ValueType {
    direct(ty, machine).map_or(ty, |(_, reference)| reference)
}

/// The binder of `owner` a lifetime argument names, when it is one of them.
fn binder(
    argument: &ValueType,
    owner: &DeclarationId,
) -> Option<usize> {
    match argument {
        ValueType::Parameter { owner: declared, index } if declared.as_ref() == owner => Some(*index),
        _ => None,
    }
}

fn names_binder(
    ty: &ValueType,
    owner: &DeclarationId,
) -> bool {
    ty.components().any(|part| binder(part, owner).is_some())
}

/// The shared view of `ty` when `ty` is that view at a region binder of
/// `machine`. In `forward<'t>(schema: Schema<'t>)` the read `schema.storage`
/// has type `@'t [u8]`, and this is its `@[u8]`.
///
/// Such a value has no region slot of its own and needs none. It is a copy of
/// the descriptor a record stores, and the copy's defining operation names the
/// field it came from. That record's slot already says which region the field
/// is in. Only a view qualifies: reading a stored place reference reborrows
/// its referent instead (`machines::loans::descriptor`).
pub(crate) fn stored_view<'a>(
    ty: &'a ValueType,
    machine: &DeclarationId,
) -> Option<&'a ValueType> {
    let ValueType::AtLifetime(view, region) = ty else { return None };
    binder(region, machine)?;
    (crate::lowering::types::is_shared_byte_view(view) || crate::lowering::types::is_shared_element_view(view)).then_some(view.as_ref())
}

/// Whether some parameter of `sig` names its region binder `index`. A call's
/// operand at such a parameter carries the region the call applies there.
pub(crate) fn parameter_carries(
    sig: &TypedSignature,
    index: usize,
) -> bool {
    sig.parameters.iter().any(|(_, formal)| {
        formal
            .components()
            .any(|part| binder(part, &sig.machine) == Some(index))
    })
}

/// The records and sums of one package that keep a type row while taking
/// lifetime arguments, with each one's binder count. All binders of such a
/// declaration are lifetimes, and every field that names one stores it where
/// Psi records it (`stored`): a reference at that binder, or another of these
/// declarations applied at this one's binders and held by value or behind
/// such a reference when it is shared, as in `@'t Bytes<'t>`.
///
/// A record whose field uses a binder in an unsupported position, such as
/// `@Bytes<'t>` with no lifetime on the reference, or `@'t mutable Bytes<'t>`,
/// remains a data template. Imported declarations are judged in
/// their defining package before a caller uses their region slots.
pub(crate) struct RegionData<'a>(Vec<(&'a DeclarationId, usize)>);

impl<'a> RegionData<'a> {
    /// Judge every data declaration of `trees`, whose typed facts are in
    /// `forest`. A field is stored only when the record it holds or refers to
    /// is one of these rows, so start from every lifetime-only declaration
    /// and drop the ones with a field that is not stored, until none is left
    /// to drop. Records that refer to each other through references stay
    /// together or drop together.
    pub(crate) fn of_package(
        forest: &TypedTrees<'_>,
        trees: &[TypedTree<'a>],
    ) -> Self {
        let mut rows: Vec<(&'a DeclarationId, usize)> = trees
            .iter()
            .flat_map(|tree| tree.resolved().declaration_nodes())
            .filter(|(_, declaration)| declaration.kind == NodeKind::Data)
            .filter(|(_, declaration)| {
                !forest.type_alias_targets.contains_key(declaration)
                    && forest
                        .enclosing_data_parameters
                        .get(declaration)
                        .is_none_or(Vec::is_empty)
            })
            .filter_map(|(_, declaration)| {
                let telescope = forest
                    .generic_parameters
                    .get(declaration)
                    .filter(|t| t.witness_count == 0 && t.equations.is_empty())?;
                let lifetimes = telescope.parameters.len();
                (lifetimes != 0
                    && telescope
                        .parameters
                        .iter()
                        .all(|p| p.kind == NodeKind::LifetimeParam))
                .then_some((declaration, lifetimes))
            })
            .collect();
        rows.sort_unstable();
        rows.dedup();
        loop {
            let judged = Self(rows.clone());
            rows.retain(|&(declaration, lifetimes)| {
                let payloads = forest
                    .case_owners
                    .iter()
                    .filter(|(_, owner)| *owner == declaration)
                    .map(|(case, _)| case);
                std::iter::once(declaration)
                    .chain(payloads)
                    .filter_map(|shape| forest.record_fields.get(shape))
                    .flat_map(|fields| fields.iter())
                    .all(|field| stored(&field.ty, declaration, lifetimes, &judged).is_some())
            });
            if rows.len() == judged.0.len() {
                return Self(rows);
            }
        }
    }

    /// These rows with `other`'s. A machine can hold a record of its own
    /// package and one of a library, such as the toolchain's `TypeSchema<'t>`.
    /// Each package still judges only its own declarations, so the two lists
    /// name different declarations and nothing is judged again.
    pub(crate) fn with(
        &self,
        other: &Self,
    ) -> Self {
        let mut rows = self.0.clone();
        rows.extend(&other.0);
        rows.sort_unstable();
        rows.dedup();
        Self(rows)
    }

    pub(crate) fn contains(
        &self,
        declaration: &DeclarationId,
    ) -> bool {
        self.binders(declaration).is_some()
    }

    fn binders(
        &self,
        declaration: &DeclarationId,
    ) -> Option<usize> {
        let row = self
            .0
            .binary_search_by(|(listed, _)| (*listed).cmp(declaration))
            .ok()?;
        Some(self.0[row].1)
    }

    /// The lifetime arguments of `ty` when it is one of these declarations
    /// applied to exactly its lifetime binders.
    pub(crate) fn lifetime_arguments<'t>(
        &self,
        ty: &'t ValueType,
    ) -> Option<&'t [ValueType]> {
        let ValueType::Applied(owner, arguments) = ty else { return None };
        (self.binders(owner)? == arguments.len()).then_some(arguments.as_slice())
    }
}

/// How one field stores the lifetime binders of its data declaration.
pub(crate) enum Stored<'t> {
    /// The field names no binder.
    Plain,
    /// `storage` is a reference held at this binder. When it is a shared
    /// reference to a `RegionData` declaration, `referent` lists the binder
    /// of the enclosing declaration that fills each of the referent's own, in
    /// the order the field writes them. Otherwise the referent names no
    /// binder and the list is empty.
    Reference {
        storage: &'t ValueType,
        binder: usize,
        referent: Vec<usize>,
    },
    /// The field holds a `RegionData` declaration, directly or as the element
    /// of fixed arrays or of an owned sequence. Each of that declaration's binders is filled by the
    /// listed binder of the enclosing one, in the order the field writes them.
    Record(Vec<usize>),
}

/// Where the field type `ty` keeps each binder of `owner` it names, or `None`
/// when it names one where a data row records nothing. `owner` declares
/// `count` lifetime binders.
pub(crate) fn stored<'t>(
    ty: &'t ValueType,
    owner: &DeclarationId,
    count: usize,
    data: &RegionData<'_>,
) -> Option<Stored<'t>> {
    if !names_binder(ty, owner) {
        return Some(Stored::Plain);
    }
    // Each argument of a stored record, or of the record a stored reference
    // refers to, is a binder of the enclosing declaration: neither has a
    // region of its own to choose.
    let fills = |record: &ValueType| -> Option<Vec<usize>> {
        data.lifetime_arguments(record)?
            .iter()
            .map(|argument| binder(argument, owner).filter(|index| *index < count))
            .collect()
    };
    if let ValueType::AtLifetime(storage, region) = ty {
        let binder = binder(region, owner).filter(|index| *index < count)?;
        let ValueType::Reference(access, referent) = storage.as_ref() else {
            return None;
        };
        // The row keeps the reference's own binder and, beside it, the
        // binders that fill the record it refers to. Only one shared
        // reference to a record with a row has that form. The owner stores
        // the descriptor and forwards it whole; nothing here says a body may
        // read the borrows the referent holds.
        let referent = if !names_binder(referent, owner) {
            Vec::new()
        } else if *access == omega_typed::Access::Shared {
            fills(referent)?
        } else {
            return None;
        };
        return Some(Stored::Reference { storage, binder, referent });
    }
    // The field must declare its reference lifetime. Typing can still pass an
    // unannotated reference here, so there is no declared binder to publish.
    // The elements of a fixed array or of an owned sequence `[T]` are stored
    // by value, so they hold their record at the same binders the field names.
    let mut leaf = ty;
    while let ValueType::Array(element, _) | ValueType::Slice(element) = leaf {
        leaf = element;
    }
    fills(leaf).map(Stored::Record)
}

/// Whether one executable body serves every application of a lifetime-only
/// machine. That holds only when `signature` can publish each binder use: as
/// the region of a direct place reference, or as a lifetime argument of a
/// parameter or result whose record this package lowers (`RegionData`). The
/// result is those slots.
///
/// A signature with an unsupported slot, such as `schema: @mutable Schema<'t>`, stays an
/// open template, and the template planner judges it with its generic
/// signature intact. It must not become a checked body whose carriers name an
/// unapplied binder.
pub(crate) fn single_body(
    sig: &TypedSignature,
    telescope: Option<&GenericParameters>,
    data: &RegionData<'_>,
) -> Option<Vec<MachineRegion>> {
    telescope
        .filter(|t| !t.parameters.is_empty() && t.parameters.iter().all(|p| p.kind == NodeKind::LifetimeParam))
        .and_then(|_| signature(sig, telescope, data).ok())
}

/// Whether a value of type `ty` keeps every region binder of `machine` it
/// names where Psi records one (`Slot`).
pub(crate) fn keeps_binders(
    ty: &ValueType,
    machine: &DeclarationId,
    data: &RegionData<'_>,
) -> bool {
    // A local fixed array or owned sequence stores the element by value.
    // Its element type identifies the nominal row, just as it does on a
    // record field. The local adds no formal or result region of its own.
    let mut held = ty;
    while let ValueType::Array(element, _) | ValueType::Slice(element) = held {
        held = element;
    }
    if data.lifetime_arguments(held).is_some() {
        return true;
    }
    let slot = Slot::of(ty, machine, data);
    slot.record.is_some() || !names_binder(slot.held, machine)
}

/// Where one parameter, result or body value names regions of its machine.
struct Slot<'t> {
    /// The machine binder a direct place reference is held at.
    reference: Option<usize>,
    /// The value without that binder: the reference itself, or `ty`.
    held: &'t ValueType,
    /// The lifetime arguments, by binder, of the `RegionData` record the
    /// value holds or refers to through one shared reference.
    record: Option<&'t [ValueType]>,
}

impl<'t> Slot<'t> {
    fn of(
        ty: &'t ValueType,
        machine: &DeclarationId,
        data: &RegionData<'_>,
    ) -> Self {
        let (reference, held) = match direct(ty, machine) {
            Some((index, reference)) => (Some(index), reference),
            None => (None, ty),
        };
        let record = match held {
            ValueType::Reference(omega_typed::Access::Shared, referent) => data.lifetime_arguments(referent),
            owned => data.lifetime_arguments(owned),
        };
        Self { reference, held, record }
    }
}

/// The machine binder of a direct reference, and each `(data binder, machine
/// binder)` pair of a record, in `ty`, one parameter or the result of `sig`.
/// `count` is the number of region binders the machine declares. An argument
/// that is not a binder of this machine, such as the region typing infers for
/// an omitted lifetime, promises nothing a slot can say.
fn slot(
    ty: &ValueType,
    sig: &TypedSignature,
    count: usize,
    data: &RegionData<'_>,
) -> Result<(Option<usize>, Vec<(usize, usize)>), LoweringError> {
    let unbound = || LoweringError::new(&sig.site, "reference requires its machine region binder");
    let slot = Slot::of(ty, &sig.machine, data);
    if slot.reference.is_some_and(|index| index >= count) {
        return Err(unbound());
    }
    let Some(arguments) = slot.record else {
        if count != 0 && names_binder(slot.held, &sig.machine) {
            return Err(LoweringError::new(
                &sig.site,
                "machine region needs a direct reference or a record this package lowers",
            ));
        }
        return Ok((slot.reference, Vec::new()));
    };
    let mut filled = Vec::new();
    for (position, argument) in arguments.iter().enumerate() {
        if let Some(index) = binder(argument, &sig.machine) {
            if index >= count {
                return Err(unbound());
            }
            filled.push((position, index));
        }
    }
    Ok((slot.reference, filled))
}

/// Each region binder of `sig` with every parameter and result slot that
/// names it. Two binders never share a slot, and a slot is recorded under the
/// binder the source wrote there: nothing here equates two regions.
pub(crate) fn signature(
    sig: &TypedSignature,
    telescope: Option<&GenericParameters>,
    data: &RegionData<'_>,
) -> Result<Vec<MachineRegion>, LoweringError> {
    let mut regions = Vec::new();
    if let Some(params) = telescope.filter(|t| t.parameters.iter().any(|p| p.kind == NodeKind::LifetimeParam)) {
        if params.witness_count != 0
            || !params.equations.is_empty()
            || !params
                .parameters
                .iter()
                .all(|p| p.kind == NodeKind::LifetimeParam && p.carrier.is_none() && p.multiplicity.is_none())
        {
            return Err(LoweringError::new(&sig.site, "machine needs a region-only telescope"));
        }
        regions.resize_with(params.parameters.len(), MachineRegion::default);
    }
    let count = regions.len();
    let (reference, record) = slot(&sig.result, sig, count, data)?;
    if let Some(index) = reference {
        regions[index].reference_result = true;
    }
    for (binder, index) in record {
        regions[index].aggregate_result.push(binder);
    }
    for (parameter, (_, ty)) in sig.parameters.iter().enumerate() {
        let (reference, record) = slot(ty, sig, count, data)?;
        if let Some(index) = reference {
            regions[index].reference_parameters.push(parameter);
        }
        for (binder, index) in record {
            regions[index]
                .aggregate_parameters
                .push(AggregateRegionSlot { parameter, binder });
        }
    }
    Ok(regions)
}
