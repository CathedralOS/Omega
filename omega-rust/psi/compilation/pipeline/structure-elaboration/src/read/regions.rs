//! The regions a record type's views are valid for, and how they move.
//!
//! `wiki/spec/language/lifetimes.md`: "Lifetime arguments are erased regions,
//! separate from runtime generic arity, layout and monomorphization. Erasure
//! does not remove semantic lifetime relations or loan obligations." So a
//! region never selects a structural row: `&'a [u8]` and `&'b [u8]` lower to
//! one byte-view row, and `Message<'a>` and `Message<'b>` to one record row.
//! The region is part of the type the reader compares instead
//! (`body::Regions`), and this module owns what a region is and the two
//! operations that move one:
//!
//! - Substitution. A record row's field types name its declaration's own
//!   lifetime parameters. A value of `Message<'a>` has those parameters at
//!   `'a`, so a field read of it, a literal of it and a store into it see the
//!   field types with the declaration's parameters replaced by the value's
//!   regions (`field_regions`). The same holds for a machine generic over a
//!   lifetime: its signature names its own parameters, and a call sees that
//!   signature with them replaced by the caller's regions.
//! - Instantiation at a call. Nothing in the typed trees records which
//!   region a call passes for each of its callee's lifetime parameters, so
//!   the call infers it: from the type the caller expects of the result, then
//!   from each argument in order (`unify`). One parameter given two regions
//!   is refused, and so is a result whose region no input supplies:
//!   lifetimes.md, "A signature does not authorize returning unrelated
//!   storage merely because its type has the same lifetime spelling".
//!
//! A region is one of two kinds. A named lifetime is the parameter it
//! resolves to (`Region::Parameter`): the declaration that binds it and its
//! position there, which is what substitution replaces. Every other region,
//! an elided reference's or a byte literal's, keeps the text identity the
//! typed trees render for it (`Region::Fixed`): it stands for one region
//! that no substitution names, so a view of it is a value only of a place of
//! that same region, as before regions were split from rows.

use semantic_vocabulary::MachineId;
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::type_identity::LifetimeParameterIdentity;

use super::Program;
use crate::body::{Regions, ValueType};
use crate::{Unserved, refuse};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Region {
    /// A lifetime parameter of a data declaration or a machine.
    Parameter(LifetimeParameterIdentity),
    /// Any other region, by its position in `Program::fixed_regions`.
    Fixed(u32),
    /// A region not yet decided, by its position in
    /// `Program::region_variables` (`Elision::Variables`).
    Variable(u32),
}

/// One substitution: each listed parameter stands for the region beside it.
pub(crate) type Substitution = Vec<(LifetimeParameterIdentity, Region)>;

impl Program<'_> {
    /// The tuple a `Regions` names.
    pub(crate) fn region_tuple(&self, regions: Regions) -> &[Region] {
        self.region_tuples
            .get(regions.0 as usize)
            .map_or(&[], Vec::as_slice)
    }

    /// The `Regions` of `tuple`, interned: equal tuples get equal handles,
    /// so comparing two types compares their regions.
    pub(crate) fn intern_regions(&mut self, tuple: Vec<Region>) -> Regions {
        if tuple.is_empty() {
            return Regions::NONE;
        }
        if let Some(position) = self.region_tuples.iter().position(|known| *known == tuple) {
            return Regions(position as u32);
        }
        self.region_tuples.push(tuple);
        Regions((self.region_tuples.len() - 1) as u32)
    }

    /// The region a text identity stands for, interned.
    pub(crate) fn fixed_region(&mut self, identity: String) -> Region {
        if let Some(position) = self
            .fixed_regions
            .iter()
            .position(|known| *known == identity)
        {
            return Region::Fixed(position as u32);
        }
        self.fixed_regions.push(identity);
        Region::Fixed((self.fixed_regions.len() - 1) as u32)
    }

    /// The lifetime parameters `owner` declares, as regions in order.
    pub(crate) fn own_regions(&self, owner: SymbolHandle) -> Vec<Region> {
        let count = typed_trees::type_identity::declaration_lifetime_parameters(self.typed, owner)
            .map_or(0, <[_]>::len);
        (0..count)
            .map(|ordinal| {
                Region::Parameter(LifetimeParameterIdentity {
                    declaration: owner,
                    ordinal,
                })
            })
            .collect()
    }

    /// `value_type` with every parameter `substitution` lists replaced by
    /// its region. A type that names no region is unchanged.
    pub(crate) fn substitute(
        &mut self,
        value_type: ValueType,
        substitution: &[(LifetimeParameterIdentity, Region)],
    ) -> ValueType {
        let ValueType::Record(record, qualification, regions) = value_type else {
            return value_type;
        };
        if regions == Regions::NONE || substitution.is_empty() {
            return value_type;
        }
        let tuple = self
            .region_tuple(regions)
            .iter()
            .map(|region| match region {
                Region::Parameter(parameter) => substitution
                    .iter()
                    .find(|(bound, _)| bound == parameter)
                    .map_or(*region, |(_, argument)| *argument),
                Region::Fixed(_) | Region::Variable(_) => *region,
            })
            .collect();
        ValueType::Record(record, qualification, self.intern_regions(tuple))
    }

    /// The substitution a value of `record` at `regions` gives its row's
    /// field types: the record's declaration's own parameters, each at the
    /// region the value holds in its place. Empty for a row with no
    /// declaration of its own (a view, an array, a literal) and for a value
    /// at its declaration's own regions.
    pub(crate) fn field_regions(
        &self,
        id: MachineId,
        record: semantic_vocabulary::StructuralTypeId,
        regions: Regions,
    ) -> Checked<Substitution> {
        let declaration = self.source_declaration(id, record)?;
        if !declaration.is_valid() || regions == Regions::NONE {
            return Ok(Vec::new());
        }
        let own = self.own_regions(declaration);
        let applied = self.region_tuple(regions);
        if own.len() != applied.len() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("record applied at another number of regions than it declares"),
            ));
        }
        Ok(own
            .into_iter()
            .zip(applied)
            .filter_map(|(parameter, argument)| match parameter {
                Region::Parameter(parameter) if Region::Parameter(parameter) != *argument => {
                    Some((parameter, *argument))
                }
                _ => None,
            })
            .collect())
    }

    /// Extends `bound` so that `formal`, with `bound` applied, is `actual`,
    /// where `free` are the parameters a call instantiates. Only regions are
    /// compared here: whether the carriers and domains agree is the call's
    /// own argument check, made once every region is bound. A parameter
    /// `bound` already gives another region is refused by name.
    pub(crate) fn unify(
        &mut self,
        id: MachineId,
        free: &[LifetimeParameterIdentity],
        formal: ValueType,
        actual: ValueType,
        bound: &mut Substitution,
    ) -> Checked<()> {
        let (ValueType::Record(formal_row, _, formal), ValueType::Record(actual_row, _, actual)) =
            (formal, actual)
        else {
            return Ok(());
        };
        if formal_row != actual_row {
            return Ok(());
        }
        let formal = self.region_tuple(formal).to_vec();
        let actual = self.region_tuple(actual).to_vec();
        if formal.len() != actual.len() {
            return Ok(());
        }
        for (formal, actual) in formal.iter().zip(&actual) {
            let Region::Parameter(parameter) = formal else {
                continue;
            };
            if !free.contains(parameter) {
                continue;
            }
            match bound
                .iter()
                .find(|(known, _)| known == parameter)
                .map(|(_, region)| *region)
            {
                // Two regions for one parameter agree only if a region
                // variable can be decided as the other (`agrees`).
                Some(region) if !self.same_region(region, *actual) => {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Typing("call that gives one lifetime two regions"),
                    ));
                }
                Some(_) => {}
                None => bound.push((*parameter, *actual)),
            }
        }
        Ok(())
    }

    /// Whether `value_type` still names one of `free`.
    pub(crate) fn names_any(
        &self,
        value_type: ValueType,
        free: &[LifetimeParameterIdentity],
    ) -> bool {
        self.region_tuple(value_type.regions()).iter().any(
            |region| matches!(region, Region::Parameter(parameter) if free.contains(parameter)),
        )
    }
}

/// What an elided lifetime stands for, by where the type is written.
/// lifetimes.md, "Returned views": "For an elided output, one unambiguous
/// reference input supplies the relation. ... Several unnamed carried
/// references do not become one source merely because they occupy one
/// parameter. Ambiguous elision requires an explicit relation, not a guess
/// by name or position."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Elision {
    /// Inside a data declaration: the declaration's own parameters. A field
    /// that elides one is refused before this is asked
    /// (`places::declared_field`).
    Declaration,
    /// In a machine's or requirement's inputs: each elided position is a
    /// fresh lifetime parameter of that declaration, which a call
    /// instantiates like a named one.
    Inputs(SymbolHandle),
    /// In its result: the one lifetime its inputs carry. With several there
    /// is no unambiguous input and the result is refused; with none the
    /// result is a fresh lifetime no input relates, which every call refuses
    /// (`call_of`).
    Result(SymbolHandle),
    /// On a local's type annotation: a region variable, which the first
    /// value that meets it decides (`agrees`).
    Variables,
}

impl Program<'_> {
    /// The region one elided position stands for here, or `None` where an
    /// elided lifetime keeps the region its own declaration names
    /// (`Elision::Declaration`).
    pub(crate) fn elided_region(&mut self, id: MachineId) -> Checked<Option<Region>> {
        match self.elision {
            Elision::Declaration => Ok(None),
            Elision::Inputs(owner) => Ok(Some(self.mint_lifetime(owner))),
            Elision::Result(owner) => match self.machine_lifetimes(owner).as_slice() {
                [only] => Ok(Some(Region::Parameter(*only))),
                [] => Ok(Some(self.mint_lifetime(owner))),
                _ => Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Typing(
                        "result without a lifetime of a signature with several input lifetimes",
                    ),
                )),
            },
            Elision::Variables => {
                self.region_variables.push(None);
                Ok(Some(Region::Variable(
                    (self.region_variables.len() - 1) as u32,
                )))
            }
        }
    }

    /// A fresh lifetime parameter of `owner`, after the ones it declares and
    /// the ones already minted for it.
    fn mint_lifetime(&mut self, owner: SymbolHandle) -> Region {
        let declared =
            typed_trees::type_identity::declaration_lifetime_parameters(self.typed, owner)
                .map_or(0, <[_]>::len);
        let minted = match self
            .elided_lifetimes
            .iter_mut()
            .find(|(machine, _)| *machine == owner)
        {
            Some((_, minted)) => {
                *minted += 1;
                *minted
            }
            None => {
                self.elided_lifetimes.push((owner, 1));
                1
            }
        };
        Region::Parameter(LifetimeParameterIdentity {
            declaration: owner,
            ordinal: declared + minted - 1,
        })
    }

    /// The regions an elided application of `data` stands for here: one
    /// elided position per lifetime the declaration binds.
    pub(crate) fn elided_regions(
        &mut self,
        id: MachineId,
        data: SymbolHandle,
    ) -> Checked<Vec<Region>> {
        let own = self.own_regions(data);
        let mut regions = Vec::with_capacity(own.len());
        for declared in own {
            regions.push(self.elided_region(id)?.unwrap_or(declared));
        }
        Ok(regions)
    }

    /// Every lifetime parameter of `machine`: the ones it declares, then
    /// the ones its signature's elided positions were given.
    pub(crate) fn machine_lifetimes(
        &self,
        machine: SymbolHandle,
    ) -> Vec<LifetimeParameterIdentity> {
        let declared =
            typed_trees::type_identity::declaration_lifetime_parameters(self.typed, machine)
                .map_or(0, <[_]>::len);
        let elided = self
            .elided_lifetimes
            .iter()
            .find(|(owner, _)| *owner == machine)
            .map_or(0, |(_, minted)| *minted);
        (0..declared + elided)
            .map(|ordinal| LifetimeParameterIdentity {
                declaration: machine,
                ordinal,
            })
            .collect()
    }

    /// `region` with every decided variable followed to what it was decided
    /// as.
    fn resolved(&self, mut region: Region) -> Region {
        while let Region::Variable(variable) = region {
            match self
                .region_variables
                .get(variable as usize)
                .copied()
                .flatten()
            {
                Some(decided) => region = decided,
                None => break,
            }
        }
        region
    }

    /// Whether `actual` is a value of `expected`: the same carrier and
    /// domains, and regions that are equal or can be made so by deciding
    /// region variables, which are then decided. Every typing rule that
    /// asks whether a value fits a place asks this, so a local whose type
    /// elides a lifetime takes the region of the first value that meets it.
    pub(crate) fn agrees(&mut self, actual: Option<ValueType>, expected: ValueType) -> bool {
        let Some(actual) = actual else {
            return false;
        };
        if actual.erased() != expected.erased() {
            return false;
        }
        if actual.regions() == expected.regions() {
            return true;
        }
        let left = self.region_tuple(actual.regions()).to_vec();
        let right = self.region_tuple(expected.regions()).to_vec();
        if left.len() != right.len() {
            return false;
        }
        let saved = self.region_variables.clone();
        for (left, right) in left.into_iter().zip(right) {
            let (left, right) = (self.resolved(left), self.resolved(right));
            if left == right {
                continue;
            }
            match (left, right) {
                (Region::Variable(variable), other) | (other, Region::Variable(variable)) => {
                    self.region_variables[variable as usize] = Some(other);
                }
                _ => {
                    self.region_variables = saved;
                    return false;
                }
            }
        }
        true
    }
}

impl Program<'_> {
    /// A local's declared type: an elided lifetime in it is a region
    /// variable, decided by what the local is first given.
    pub(crate) fn local_type(
        &mut self,
        id: MachineId,
        reference: typed_trees::types::TypeReferenceHandle,
    ) -> Checked<ValueType> {
        let elision = std::mem::replace(&mut self.elision, Elision::Variables);
        let read = self.value_type(id, reference);
        self.elision = elision;
        read
    }
}

impl Program<'_> {
    /// Whether two regions are one, deciding a region variable if that
    /// makes them so.
    fn same_region(&mut self, left: Region, right: Region) -> bool {
        let (left, right) = (self.resolved(left), self.resolved(right));
        match (left, right) {
            _ if left == right => true,
            (Region::Variable(variable), other) | (other, Region::Variable(variable)) => {
                self.region_variables[variable as usize] = Some(other);
                true
            }
            _ => false,
        }
    }
}

impl Program<'_> {
    /// Whether the signature of `supplied` (formals, then result) is the
    /// signature of `required` once `supplied`'s own lifetimes are
    /// instantiated with `required`'s regions, as a call of it from
    /// `required`'s body would instantiate them (`call_of`). Two
    /// declarations name their lifetimes under different owners, so equal
    /// signatures are never equal value for value.
    pub(crate) fn same_signature(
        &mut self,
        id: MachineId,
        required: &crate::body::Signature,
        supplied: &crate::body::Signature,
        supplied_machine: SymbolHandle,
    ) -> Checked<bool> {
        if required.parameters.len() != supplied.parameters.len()
            || required.result.is_some() != supplied.result.is_some()
        {
            return Ok(false);
        }
        let free = self.machine_lifetimes(supplied_machine);
        let pairs: Vec<(ValueType, ValueType)> = supplied
            .parameters
            .iter()
            .copied()
            .zip(required.parameters.iter().copied())
            .chain(supplied.result.zip(required.result))
            .collect();
        let mut bound = Substitution::new();
        for (formal, actual) in &pairs {
            self.unify(id, &free, *formal, *actual, &mut bound)?;
        }
        Ok(pairs
            .iter()
            .all(|(formal, actual)| self.substitute(*formal, &bound) == *actual))
    }
}

impl Program<'_> {
    /// The outer region of a borrowed view, resolved from the same retained
    /// source occurrence as its type. Element regions remain separate tuple
    /// positions; a borrowed descriptor cannot erase what its elements carry.
    pub(super) fn borrowed_view_region(
        &mut self,
        machine: MachineId,
        reference: typed_trees::types::TypeReferenceHandle,
        lexical_owner: SymbolHandle,
        identity: &str,
    ) -> Checked<Region> {
        use typed_trees::types::TypeReferenceNode;
        Ok(
            match self.typed.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference {
                    lifetime: Some(tag),
                    ..
                } => {
                    let owner = match self
                        .typed
                        .type_reference_table
                        .operand_lifetime_owner(reference)
                    {
                        retained if retained.is_valid() => retained,
                        _ => lexical_owner,
                    };
                    let parameter = owner
                        .is_valid()
                        .then(|| {
                            typed_trees::type_identity::lifetime_parameter_identity(
                                self.typed,
                                owner,
                                tag.as_str(),
                            )
                        })
                        .flatten()
                        .ok_or_else(|| {
                            refuse(
                                machine,
                                Construct::Machine,
                                Unserved::Type(
                                    "borrowed view has no complete owned region identity",
                                ),
                            )
                        })?;
                    Region::Parameter(parameter)
                }
                // Elided: a fresh lifetime of the signature, a region variable
                // of a local, or the one identity it has anywhere else.
                _ => match self.elided_region(machine)? {
                    Some(region) => region,
                    None => self.fixed_region(identity.to_owned()),
                },
            },
        )
    }

    /// Collection values retain their elements' complete region tuple. A
    /// borrowed element view prefixes its own region; only that one position
    /// is removed when checking an element against the collection's type.
    pub(super) fn collection_element_regions(
        &mut self,
        id: MachineId,
        collection: semantic_vocabulary::StructuralTypeId,
        regions: Regions,
    ) -> Checked<Regions> {
        let borrowed = matches!(
            self.data_type(id, collection)?.shape,
            crate::StorageShape::ElementView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                ..
            }
        );
        if !borrowed {
            return Ok(regions);
        }
        let (_, elements) = self.region_tuple(regions).split_first().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Typing("borrowed sequence without its outer region"),
            )
        })?;
        Ok(self.intern_regions(elements.to_vec()))
    }
}
