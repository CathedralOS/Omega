//! Preserve the public signature's lifetime membership before region erasure.
//! These groups describe all possible inputs, not the implementation's chosen
//! return. They establish neither backing nor a loan. Collection membership
//! uses the existing AllElements family path, never an unchecked source index.

use super::{Program, Region};
use crate::body::{BorrowedReferent, Signature, StorageShape, ValueType};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
    DeclaredResultLifetime, StructuralAccess, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

type Checked<T> = Result<T, &'static str>;

type Member = (Region, Vec<StructuralPathSegment>, StructuralAccess);
type Input = (Region, BoundaryLifetimeDependency);
type Substitution = [(
    typed_trees::type_identity::LifetimeParameterIdentity,
    Region,
)];

impl Program<'_> {
    /// `None` preserves an unresolved frontier. In particular it is never a
    /// complete empty result contract for an opaque or rigid result type.
    pub(crate) fn declared_result_lifetimes(
        &self,
        machine: MachineId,
        signature: &Signature,
        types: &[StructuralTypeDeclaration],
    ) -> Checked<Option<Vec<DeclaredResultLifetime>>> {
        let Some(result) = signature.result else {
            return Ok(Some(Vec::new()));
        };
        let mut reader = Frontier {
            program: self,
            machine,
            types,
            active: Vec::new(),
        };
        let mut results = Vec::new();
        if !reader.value(result, &[], &mut Vec::new(), &mut results)? {
            return Ok(None);
        }
        if results.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let Some(inputs) = reader.inputs(signature)? else {
            return Ok(None);
        };
        let mut groups: Vec<(Region, DeclaredResultLifetime)> = Vec::new();
        results.sort_unstable_by_key(|(region, _, _)| region_key(*region));
        for (region, path, _) in results {
            if groups.last().is_none_or(|(known, _)| *known != region) {
                let key = region_key(region);
                let start = inputs.partition_point(|(known, _)| region_key(*known) < key);
                let end = inputs.partition_point(|(known, _)| region_key(*known) <= key);
                let mut sources: Vec<_> = inputs[start..end]
                    .iter()
                    .map(|(_, source)| source.clone())
                    .collect();
                sources.sort_unstable();
                sources.dedup();
                groups.push((
                    region,
                    DeclaredResultLifetime {
                        inputs: sources,
                        results: Vec::new(),
                    },
                ));
            }
            groups
                .last_mut()
                .expect("result group was just installed")
                .1
                .results
                .push(path);
        }
        let mut groups: Vec<_> = groups
            .into_iter()
            .map(|(_, mut group)| {
                group.results.sort_unstable();
                group.results.dedup();
                group
            })
            .collect();
        groups.sort_unstable();
        Ok(Some(groups))
    }

    pub(crate) fn declared_reference_outputs(
        &self,
        machine: MachineId,
        signature: &Signature,
        types: &[StructuralTypeDeclaration],
        parameters: &[StructuralParameterDeclaration],
    ) -> Checked<Vec<BoundaryReferenceOutput>> {
        use terminal_semantics::descriptor_lifetimes::descriptor_path;
        use terminal_semantics::descriptor_results::DescriptorSlotKind;

        let mut outputs = Vec::new();
        for parameter in parameters {
            for kind in [DescriptorSlotKind::References, DescriptorSlotKind::Views] {
                let slots = kind
                    .writable_slots(
                        types,
                        parameter.place,
                        parameter.structural_type,
                        parameter.access,
                    )
                    .ok_or("boundary parameter has an unresolved writable descriptor frontier")?;
                for slot in slots {
                    let path = descriptor_path(types, parameter.structural_type, &slot.path)
                        .filter(|path| {
                            path.iter().all(|segment| {
                                matches!(
                                    segment,
                                    StructuralPathSegment::Field(_)
                                        | StructuralPathSegment::Case(_)
                                        | StructuralPathSegment::FixedIndex(_)
                                )
                            })
                        })
                        .ok_or("writable descriptor output has no exact storage path")?;
                    outputs.push((parameter.position, path));
                }
            }
        }
        if outputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut reader = Frontier {
            program: self,
            machine,
            types,
            active: Vec::new(),
        };
        let inputs = reader
            .inputs(signature)?
            .ok_or("boundary output has an unresolved declared lifetime frontier")?;
        let mut stored = inputs
            .iter()
            .filter_map(|(region, input)| match input {
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    ..
                } => Some(((*argument_index, path.as_slice()), *region)),
                _ => None,
            })
            .collect::<Vec<_>>();
        stored.sort_unstable_by_key(|(destination, _)| *destination);
        let mut declared = Vec::with_capacity(outputs.len());
        for (position, path) in outputs {
            let family = declared_storage_path(&path);
            let destination = (position, family.as_slice());
            let first = stored.partition_point(|(key, _)| *key < destination);
            let last = stored.partition_point(|(key, _)| *key <= destination);
            let [(_, region)] = &stored[first..last] else {
                return Err("writable descriptor has no unique declared lifetime");
            };
            let formal = parameters
                .get(position as usize)
                .filter(|formal| formal.position == position)
                .ok_or("writable descriptor has no exact formal")?;
            let access =
                terminal_semantics::references::boundary_descriptor_access(types, formal, &path)
                    .ok_or("writable descriptor has no declared access")?;
            let key = region_key(*region);
            let start = inputs.partition_point(|(other, _)| region_key(*other) < key);
            let end = inputs.partition_point(|(other, _)| region_key(*other) <= key);
            let dependencies =
                boundary_output_dependencies(types, parameters, &inputs[start..end])?;
            if dependencies.is_empty() {
                return Err("writable descriptor has no readable lifetime anchor");
            }
            declared.push(BoundaryReferenceOutput {
                destination: BoundaryReferenceDestination::Parameter(position),
                path,
                access,
                dependencies,
            });
        }
        declared.sort_unstable();
        Ok(declared)
    }
}

fn declared_storage_path(path: &[StructuralPathSegment]) -> Vec<StructuralPathSegment> {
    path.iter()
        .map(|segment| match segment {
            StructuralPathSegment::FixedIndex(_) => StructuralPathSegment::AllElements,
            segment => segment.clone(),
        })
        .collect()
}

fn boundary_output_dependencies(
    types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    inputs: &[Input],
) -> Checked<Vec<BoundaryLifetimeDependency>> {
    use terminal_semantics::descriptor_lifetimes::descriptor_path;
    use terminal_semantics::descriptor_results::DescriptorSlotKind;

    let mut dependencies = Vec::new();
    for (_, input) in inputs {
        let BoundaryLifetimeDependency::Stored {
            argument_index,
            path,
            ..
        } = input
        else {
            dependencies.push(input.clone());
            continue;
        };
        let parameter = parameters
            .get(*argument_index as usize)
            .filter(|parameter| parameter.position == *argument_index)
            .ok_or("boundary output lifetime anchor has no exact formal")?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow {
            continue;
        }
        let mut add_dependency = |path: Vec<StructuralPathSegment>| -> Checked<()> {
            let access =
                terminal_semantics::references::boundary_descriptor_access(types, parameter, &path)
                    .ok_or("boundary output lifetime anchor has no readable descriptor")?;
            dependencies.push(BoundaryLifetimeDependency::Stored {
                argument_index: *argument_index,
                path,
                access,
            });
            Ok(())
        };
        if !path.contains(&StructuralPathSegment::AllElements) {
            add_dependency(path.clone())?;
            continue;
        }
        // Expand a declared family only through the already checked finite
        // slot inventory. A view with unresolved positions supplies no paths.
        let mut found = false;
        for kind in [DescriptorSlotKind::References, DescriptorSlotKind::Views] {
            let slots = kind
                .slots(types, parameter.place, parameter.structural_type)
                .ok_or("boundary output lifetime anchor has no finite slot inventory")?;
            for slot in slots {
                let Some(leaf) = descriptor_path(types, parameter.structural_type, &slot.path)
                else {
                    continue;
                };
                if declared_storage_path(&leaf) == *path {
                    add_dependency(leaf)?;
                    found = true;
                }
            }
        }
        if !found {
            return Err("boundary output lifetime family has no exact stored descriptor");
        }
    }
    dependencies.sort_unstable();
    dependencies.dedup();
    Ok(dependencies)
}

fn region_key(region: Region) -> (u8, u64, u32, u32, usize) {
    match region {
        Region::Parameter(parameter) => (
            0,
            0,
            parameter.declaration.arena_index(),
            parameter.declaration.generation(),
            parameter.ordinal,
        ),
        Region::Fixed(index) => (1, 0, index, 0, 0),
        Region::Variable(index) => (2, 0, index, 0, 0),
        Region::Storage { machine, root } => {
            (3, machine.get(), root.arena_index(), root.generation(), 0)
        }
    }
}

struct Frontier<'program, 'typed> {
    program: &'program Program<'typed>,
    machine: MachineId,
    types: &'program [StructuralTypeDeclaration],
    active: Vec<StructuralTypeId>,
}

impl Frontier<'_, '_> {
    fn inputs(&mut self, signature: &Signature) -> Checked<Option<Vec<Input>>> {
        let mut inputs = Vec::new();
        let mut lane = 0_u32;
        for (position, parameter) in signature.parameters.iter().enumerate() {
            if !signature.is_place(position) {
                continue;
            }
            if let Some(borrowed) = signature.borrowed(position)
                && borrowed.referent == BorrowedReferent::Value
            {
                let [region] = self.program.region_tuple(borrowed.regions) else {
                    return Err("borrowed formal has no single declared outer region");
                };
                let Some(region) = self.region(*region, &[])? else {
                    return Ok(None);
                };
                let access = match borrowed.access {
                    language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
                    language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
                    language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
                };
                inputs.push((
                    region,
                    BoundaryLifetimeDependency::Referent {
                        argument_index: lane,
                        access,
                    },
                ));
            }
            let mut stored = Vec::new();
            if !self.value(*parameter, &[], &mut Vec::new(), &mut stored)? {
                return Ok(None);
            }
            inputs.extend(stored.into_iter().map(|(region, path, access)| {
                (
                    region,
                    BoundaryLifetimeDependency::Stored {
                        argument_index: lane,
                        path,
                        access,
                    },
                )
            }));
            lane = lane
                .checked_add(1)
                .ok_or("reference input ordinal overflows")?;
        }
        // Sort once by source identity, then merge each result with its full
        // input set. There is no input × output expansion in the product.
        inputs.sort_unstable_by_key(|(region, _)| region_key(*region));
        Ok(Some(inputs))
    }

    fn region(&self, mut region: Region, substitution: &Substitution) -> Checked<Option<Region>> {
        for _ in 0..=substitution.len() {
            region = self.program.resolved(region);
            let Region::Parameter(parameter) = region else {
                return Ok((!matches!(region, Region::Variable(_))).then_some(region));
            };
            let next = substitution
                .iter()
                .rev()
                .find(|(known, _)| *known == parameter)
                .map(|(_, value)| *value);
            match next {
                Some(next) if next != region => region = next,
                _ => return Ok(Some(region)),
            }
        }
        Err("declared reference regions contain a substitution cycle")
    }

    fn value(
        &mut self,
        value: ValueType,
        substitution: &Substitution,
        path: &mut Vec<StructuralPathSegment>,
        members: &mut Vec<Member>,
    ) -> Checked<bool> {
        match value {
            ValueType::Scalar(..) => Ok(true),
            ValueType::Rigid(_) => Ok(false),
            ValueType::Record(record, _, regions) => {
                let tuple = self
                    .program
                    .region_tuple(regions)
                    .iter()
                    .map(|region| self.region(*region, substitution))
                    .collect::<Checked<Option<Vec<_>>>>()?;
                let Some(tuple) = tuple else {
                    return Ok(false);
                };
                self.record(record, &tuple, substitution, path, members)
            }
        }
    }

    fn record(
        &mut self,
        record: StructuralTypeId,
        regions: &[Region],
        substitution: &Substitution,
        path: &mut Vec<StructuralPathSegment>,
        members: &mut Vec<Member>,
    ) -> Checked<bool> {
        if self.active.contains(&record) {
            return Ok(false);
        }
        let source = self
            .program
            .data_type(self.machine, record)
            .map_err(|_| "declared reference frontier has no prepared source type")?;
        let canonical = self
            .types
            .iter()
            .find(|row| row.id == record)
            .ok_or("declared reference frontier lost its canonical type")?;
        match &source.shape {
            StorageShape::Reference { access, referent } => {
                let Some(region) = regions.first() else {
                    return Err("reference result has no declared outer region");
                };
                members.push((*region, path.clone(), *access));
                // An outer reference does not erase lifetimes carried inside
                // its referent. Retain that frontier too; a consumer unable to
                // account for these crossings must refuse the complete result.
                if regions.len() > 1 {
                    path.push(StructuralPathSegment::Referent);
                    let known =
                        self.record(*referent, &regions[1..], substitution, path, members)?;
                    path.pop();
                    return Ok(known);
                }
                return Ok(true);
            }
            StorageShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
            ) => {
                let (Some(region), Some(access)) = (regions.first(), access) else {
                    return Ok(false);
                };
                members.push((*region, path.clone(), *access));
                return Ok(true);
            }
            StorageShape::Opaque => return Ok(regions.is_empty()),
            StorageShape::PrimitiveScalar { .. } | StorageShape::ByteSequence(_) => {
                return Ok(true);
            }
            _ => {}
        }
        self.active.push(record);
        let mut bound = substitution.to_vec();
        let declaration = self
            .program
            .source_declaration(self.machine, record)
            .map_err(|_| "declared reference frontier has no source declaration")?;
        if declaration.is_valid() {
            let own = self.program.own_regions(declaration);
            if own.len() != regions.len() {
                return Err("declared reference frontier has another region arity");
            }
            bound.extend(
                own.into_iter()
                    .zip(regions)
                    .filter_map(|(parameter, region)| match parameter {
                        Region::Parameter(parameter) if Region::Parameter(parameter) != *region => {
                            Some((parameter, *region))
                        }
                        _ => None,
                    }),
            );
        }
        let known = match &source.shape {
            StorageShape::FixedArray {
                element, length, ..
            } => {
                if length.literal() == Some(0) {
                    true
                } else {
                    path.push(StructuralPathSegment::AllElements);
                    let known = self.record(*element, regions, &bound, path, members)?;
                    path.pop();
                    known
                }
            }
            StorageShape::ElementView {
                element, access, ..
            } => {
                let mut elements = regions;
                if let Some(access) = access {
                    let Some((region, nested)) = regions.split_first() else {
                        return Err("borrowed sequence has no declared outer region");
                    };
                    members.push((*region, path.clone(), *access));
                    elements = nested;
                }
                path.push(StructuralPathSegment::AllElements);
                let known = self.record(*element, elements, &bound, path, members)?;
                path.pop();
                known
            }
            StorageShape::Record(fields) => {
                let StructuralTypeShape::Record {
                    fields: declared, ..
                } = &canonical.shape
                else {
                    return Err("reference frontier changed its record shape");
                };
                self.fields(fields, declared, &bound, path, members)?
            }
            StorageShape::Sum(cases) => {
                let StructuralTypeShape::Sum { cases: declared } = &canonical.shape else {
                    return Err("reference frontier changed its sum shape");
                };
                if cases.len() != declared.len() {
                    return Err("reference frontier changed its case roster");
                }
                let mut known = true;
                for (source, canonical) in cases.iter().zip(declared) {
                    if source.id != canonical.id {
                        return Err("reference frontier changed a case identity");
                    }
                    path.push(StructuralPathSegment::Case(canonical.id));
                    known &=
                        self.fields(&source.fields, &canonical.fields, &bound, path, members)?;
                    path.pop();
                }
                known
            }
            _ => unreachable!("leaf shapes returned above"),
        };
        self.active.pop();
        Ok(known)
    }

    fn fields(
        &mut self,
        source: &[crate::body::RecordField],
        canonical: &[terminal_psi::StructuralFieldDeclaration],
        substitution: &Substitution,
        path: &mut Vec<StructuralPathSegment>,
        members: &mut Vec<Member>,
    ) -> Checked<bool> {
        if source.len() != canonical.len() {
            return Err("reference frontier changed its field roster");
        }
        let mut known = true;
        for (source, canonical) in source.iter().zip(canonical) {
            if canonical.relevance.is_erased() {
                continue;
            }
            path.push(StructuralPathSegment::Field(canonical.identity));
            known &= self.value(source.value_type, substitution, path, members)?;
            path.pop();
        }
        Ok(known)
    }
}
