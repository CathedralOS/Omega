//! Preserve the public signature's lifetime membership before region erasure.
//! These groups describe all possible inputs, not the implementation's chosen
//! return. They establish neither backing nor a loan. Collection membership
//! uses the existing AllElements family path, never an unchecked source index.

use super::{Program, Region};
use crate::body::{BorrowedReferent, Signature, StorageShape, ValueType};
use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryLifetimeDependency, DeclaredResultLifetime, StructuralAccess, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

type Checked<T> = Result<T, &'static str>;

type Member = (Region, Vec<StructuralPathSegment>, StructuralAccess);
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
        let mut inputs = Vec::new();
        let mut lane = 0_u32;
        for (position, parameter) in signature.parameters.iter().enumerate() {
            if !signature.is_place(position) {
                continue;
            }
            if let Some(borrowed) = signature.borrowed(position)
                && borrowed.referent == BorrowedReferent::Value
            {
                let [region] = self.region_tuple(borrowed.regions) else {
                    return Err("borrowed formal has no single declared outer region");
                };
                let Some(region) = reader.region(*region, &[])? else {
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
            if !reader.value(*parameter, &[], &mut Vec::new(), &mut stored)? {
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
