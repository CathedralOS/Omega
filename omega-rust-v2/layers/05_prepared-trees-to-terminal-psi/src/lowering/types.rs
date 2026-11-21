use crate::declarations::domains::{DomainDeclaration, DomainSchema};
use omega_resolved::DeclarationId;
use omega_typed::{DomainApplication, ValueType};

use crate::DataId;
use std::fmt;

use crate::{DomainSetId, ScalarType, TypeId, TypeShape};

type AtomKey = (DeclarationId, Vec<ScalarDomainArgument>);
type IndexedReferent = (omega_psi::DeclarationId, Vec<(ScalarType, num_bigint::BigInt)>);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum ScalarDomainArgument {
    Integer { carrier: ScalarType, value: num_bigint::BigInt },
    Boolean(bool),
}

impl From<ScalarDomainArgument> for omega_psi::DeclarationTerm {
    fn from(value: ScalarDomainArgument) -> Self {
        match value {
            ScalarDomainArgument::Integer { carrier, value } => Self::StaticInteger { carrier, value },
            ScalarDomainArgument::Boolean(b) => Self::StaticBoolean(b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    MissingIntegerWidth { signed: bool },
    ZeroIntegerWidth { signed: bool },
    OpenIntegerWidth,
    OpenArrayExtent,
    NoResult,
    UnsupportedCarrier(String),
    MissingCarrier(TypeShape),
    TooManyTypes,
}

impl fmt::Display for TypeError {
    fn fmt(
        &self,
        output: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::MissingIntegerWidth { signed } => {
                write!(output, "integer carrier {} needs a fixed width", if *signed { "i" } else { "u" })
            }
            Self::ZeroIntegerWidth { signed } => {
                write!(output, "integer carrier {}0 needs a positive width", if *signed { "i" } else { "u" })
            }
            Self::OpenIntegerWidth => output.write_str("integer width is still open after preparation"),
            Self::OpenArrayExtent => output.write_str("array extent is still open after preparation"),
            Self::NoResult => output.write_str("an absent result has no runtime carrier"),
            Self::UnsupportedCarrier(carrier) => write!(output, "no Psi lowering for carrier {carrier}"),
            Self::MissingCarrier(shape) => write!(output, "carrier {shape:?} is absent from the Psi type table"),
            Self::TooManyTypes => output.write_str("too many Psi type declarations"),
        }
    }
}

impl std::error::Error for TypeError {}

/// The parts of a domain declaration that decide its origin when a cast adds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DomainEstablishment {
    /// The declaration has `requires` clauses, including `requires true`.
    pub predicate: bool,
    /// The domain's subject carries a parent domain, so membership needs that parent.
    pub requires_parent: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoweringTypes {
    rows: Vec<TypeShape>,
    data_declarations: Vec<DeclarationId>,
    applied_data: std::collections::HashMap<ValueType, DataId>,
    domain_sets: Vec<Vec<AtomKey>>,
    indexed_referents: Vec<IndexedReferent>,
    /// What adding each local domain declaration must establish at a cast.
    domain_establishments: std::collections::BTreeMap<DeclarationId, DomainEstablishment>,
    /// Runtime callable contracts in declaration-site order; see `crate::declarations::callables`.
    callables: Vec<omega_typed::CallableType>,
}

impl LoweringTypes {
    pub(crate) fn from_declarations<'a>(
        carriers: impl Iterator<Item = &'a ValueType> + Clone,
        data_declarations: Vec<DeclarationId>,
        applications: Vec<ValueType>,
        domains: &[DomainSchema<'_>],
    ) -> Result<Self, (usize, TypeError)> {
        let mut sets = std::collections::BTreeSet::new();
        for (index, carrier) in carriers.clone().enumerate() {
            if matches!(array_leaf(carrier), ValueType::Qualified(..)) {
                let members = local_scalar_members(carrier, domains).ok_or_else(|| (index, TypeError::UnsupportedCarrier(array_leaf(carrier).to_string())))?;
                sets.insert(atom_keys(members).ok_or_else(|| (index, TypeError::UnsupportedCarrier(carrier.to_string())))?);
            }
        }
        let domain_sets = sets.into_iter().collect();
        let callables = crate::declarations::callables::collect(carriers.clone())?;
        let indexed_owners: std::collections::HashSet<_> = applications.iter().filter_map(ValueType::nominal_parts).filter_map(|(id, _)| crate::identity::portable(id)).collect();
        let indexed_referents = carriers.clone().filter_map(shared_indexed_target).filter(|(id, _)| indexed_owners.contains(id))
            .collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let applied_data = applications
            .into_iter()
            .enumerate()
            .map(|(index, carrier)| (carrier, DataId::from_index(data_declarations.len() + index)))
            .collect();
        Self {
            applied_data,
            indexed_referents,
            domain_sets,
            domain_establishments: domains
                .iter()
                // An indexed family's predicate binds its index; it still owes proof at addition.
                .map(|d| (d.declaration.clone(), DomainEstablishment {
                    predicate: d.has_predicates(),
                    requires_parent: !d.carrier.domains().is_empty(),
                }))
                .collect(),
            rows: Vec::new(),
            data_declarations,
            callables,
        }
        .intern_carriers(carriers)
    }

    fn intern_carriers<'a>(
        mut self,
        carriers: impl IntoIterator<Item = &'a ValueType>,
    ) -> Result<Self, (usize, TypeError)> {
        let mut leaves = Vec::new();
        let mut frontiers: Vec<Vec<(usize, u64)>> = Vec::new();
        let mut lengths = Vec::new();
        for (index, carrier) in carriers.into_iter().enumerate() {
            let leaf = self
                .array_dimensions(carrier, &mut lengths)
                .map_err(|error| (index, error))?;
            leaves.push((leaf, index));
            frontiers.resize_with(frontiers.len().max(lengths.len()), Vec::new);
            for (frontier, length) in frontiers.iter_mut().zip(lengths.drain(..).rev()) {
                frontier.push((index, length));
            }
        }

        let mut resolved = vec![None; leaves.len()];
        self.intern_frontier(&mut leaves, &mut resolved)?;
        for frontier in frontiers {
            leaves.clear();
            leaves.extend(frontier.into_iter().map(|(index, length)| {
                let element = resolved[index].expect("array element has already been interned");
                (TypeShape::FixedArray { element, length }, index)
            }));
            self.intern_frontier(&mut leaves, &mut resolved)?;
        }
        Ok(self)
    }

    pub fn id_for(
        &self,
        carrier: &ValueType,
    ) -> Result<TypeId, TypeError> {
        let mut lengths = Vec::new();
        let leaf = self.array_dimensions(carrier, &mut lengths)?;
        let mut id = self.id_for_shape(leaf)?;
        for length in lengths.into_iter().rev() {
            id = self.id_for_shape(TypeShape::FixedArray { element: id, length })?;
        }
        Ok(id)
    }

    pub(crate) fn parameter_type(
        &self,
        carrier: &ValueType,
    ) -> Result<crate::ParameterType, TypeError> {
        let id = self.id_for(carrier)?;
        match self.get(id) {
            Some(TypeShape::PrimitiveScalar(_) | TypeShape::QualifiedScalar { .. } | TypeShape::Callable(_)) => Ok(crate::ParameterType::Scalar(id)),
            Some(_) => Ok(crate::ParameterType::Structural(id)),
            None => Err(TypeError::UnsupportedCarrier("missing interned formal type".into())),
        }
    }

    pub(crate) fn get(
        &self,
        id: TypeId,
    ) -> Option<&TypeShape> {
        self.rows.get(id.get() as usize - 1)
    }
    pub(crate) fn finish(
        self,
        declarations: &std::collections::BTreeMap<&DeclarationId, DomainDeclaration>,
    ) -> Result<omega_psi::TypeTable, crate::LoweringError> {
        let mut sets = Vec::with_capacity(self.domain_sets.len());
        for set in self.domain_sets {
            let mut atoms = Vec::with_capacity(set.len());
            for (id, arguments) in set {
                // local_scalar_members has already checked every identity against this catalog.
                let domain = match &declarations[&id] {
                    DomainDeclaration::Named(d) => d,
                    DomainDeclaration::Alias(a) => return Err(crate::LoweringError::new(&a.site, "a scalar qualification requires expanded named domains")),
                };
                // Each atom supplies one closed argument per head index. Entry and result
                // contracts bind the specialized predicate (`append_domain_predicates`), so
                // the catalog's `requires` and `index_binders` describe each application.
                if !arguments.is_empty() && arguments.len() != domain.index_binders.len() {
                    return Err(crate::LoweringError::new(
                        &domain.site,
                        "applied scalar qualification requires a closed argument for each domain index",
                    ));
                }
                atoms.push(omega_psi::DomainAtom {
                    declaration: domain.declaration.clone(),
                    arguments: arguments.into_iter().map(Into::into).collect(),
                });
            }
            sets.push(atoms);
        }
        let targets = self.indexed_referents.into_iter().map(|(id, args)| {
            (id, args.into_iter().map(|(carrier, value)| omega_psi::DataArgument::Integer { carrier, value }).collect())
        }).collect();
        Ok(omega_psi::TypeTable::with_domain_atoms(self.rows, sets).with_indexed_referents(targets))
    }
    fn intern_frontier(
        &mut self,
        frontier: &mut [(TypeShape, usize)],
        resolved: &mut [Option<TypeId>],
    ) -> Result<(), (usize, TypeError)> {
        // Every deeper array refers to an element with a greater ID than any
        // shallower array uses. Sorting each depth therefore keeps all rows sorted.
        frontier.sort_unstable();
        for &(shape, input_index) in frontier.iter() {
            let already_interned = self.rows.last() == Some(&shape);
            let row_index = if already_interned { self.rows.len() - 1 } else { self.rows.len() };
            let id = TypeId::from_index(row_index)
                .ok_or(TypeError::TooManyTypes)
                .map_err(|error| (input_index, error))?;
            if !already_interned {
                self.rows.push(shape);
            }
            resolved[input_index] = Some(id);
        }
        Ok(())
    }

    pub(crate) fn can_forward_value(
        &self,
        carrier: &ValueType,
    ) -> bool {
        // Copying preserves every membership of the existing value. Copying a
        // descriptor preserves its selected application and contract.
        supports_value_copy(carrier) || matches!(array_leaf(carrier), ValueType::Qualified(..) | ValueType::Callable(_)) && self.id_for(carrier).is_ok()
    }

    /// The interned contract of a runtime callable carrier.
    pub(crate) fn callable_contract(
        &self,
        callable: &omega_typed::CallableType,
    ) -> Result<crate::CallableContractId, TypeError> {
        crate::declarations::callables::find(&self.callables, callable)
    }

    pub(crate) fn callable_contracts(&self) -> &[omega_typed::CallableType] {
        &self.callables
    }

    pub(crate) fn domain_establishment(&self, declaration: &DeclarationId) -> Option<DomainEstablishment> {
        self.domain_establishments.get(declaration).copied()
    }

    /// The Psi atom of one scalar domain application, with the same closed arguments
    /// the interned qualification set stores for it (`finish` checks their count).
    pub(crate) fn domain_atom(
        application: &DomainApplication,
        site: &omega_typed::Site,
    ) -> Result<omega_psi::DomainAtom, crate::LoweringError> {
        let (_, arguments) = atom_keys(std::slice::from_ref(application))
            .and_then(|mut keys| keys.pop())
            .ok_or_else(|| crate::LoweringError::new(site, "scalar domain application requires closed integer or Boolean arguments"))?;
        Ok(omega_psi::DomainAtom {
            declaration: crate::identity::declaration(&application.declaration, site)?,
            arguments: arguments.into_iter().map(Into::into).collect(),
        })
    }

    fn id_for_shape(
        &self,
        shape: TypeShape,
    ) -> Result<TypeId, TypeError> {
        let index = self
            .rows
            .binary_search(&shape)
            .map_err(|_| TypeError::MissingCarrier(shape))?;
        TypeId::from_index(index).ok_or(TypeError::TooManyTypes)
    }
    fn array_dimensions(
        &self,
        mut carrier: &ValueType,
        lengths: &mut Vec<u64>,
    ) -> Result<TypeShape, TypeError> {
        while let ValueType::Array(element, length) = carrier {
            lengths.push(*length);
            carrier = element;
        }
        if lengths.is_empty() && let Some(key) = shared_indexed_target(carrier) && let Ok(i) = self.indexed_referents.binary_search(&key) {
            return Ok(TypeShape::IndexedSharedReference(omega_psi::IndexedReferentId(i)));
        }
        if lengths.is_empty()
            && let Some((access, referent)) = nominal_reference(carrier)
        {
            let index = self
                .data_declarations
                .binary_search(referent)
                .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            return Ok(TypeShape::NominalReference {
                access,
                referent: DataId::from_index(index),
            });
        }
        if lengths.is_empty()
            && let Some((access, inner)) = place_reference(carrier)
            && let Ok(referent) = stored_scalar(inner)
        {
            return Ok(TypeShape::ScalarReference { access, referent });
        }
        if let ValueType::Callable(callable) = carrier {
            if !lengths.is_empty() {
                return Err(TypeError::UnsupportedCarrier(carrier.to_string()));
            }
            return self.callable_contract(callable).map(TypeShape::Callable);
        }
        if lengths.is_empty() && is_shared_byte_view(carrier) {
            return Ok(TypeShape::SharedByteView);
        }
        if lengths.is_empty()
            && let Some((access, element)) = scalar_element_view(carrier)
        {
            return Ok(if *access == omega_typed::Access::Mut {
                TypeShape::MutableScalarElementView { element }
            } else {
                TypeShape::SharedElementView { element }
            });
        }
        if lengths.is_empty()
            && let Some(element) = mutable_element_view(carrier)
        {
            let index = self
                .data_declarations
                .binary_search(element)
                .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            return Ok(TypeShape::MutableElementView {
                element: DataId::from_index(index),
            });
        }
        if let ValueType::Qualified(base, members) = carrier {
            let key = atom_keys(members).ok_or_else(|| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            let index = self
                .domain_sets
                .binary_search(&key)
                .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            return Ok(TypeShape::QualifiedScalar {
                carrier: scalar_carrier(base)?,
                domains: DomainSetId(std::num::NonZeroUsize::new(index + 1).expect("domain set belongs to this table")),
            });
        }
        if let Some(&id) = self.applied_data.get(carrier) {
            return Ok(TypeShape::Nominal(id));
        }
        if let ValueType::Nominal(declaration) = carrier {
            let index = self
                .data_declarations
                .binary_search(declaration)
                .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            Ok(TypeShape::Nominal(DataId::from_index(index)))
        } else {
            stored_scalar(carrier).map(TypeShape::PrimitiveScalar)
        }
    }
}

pub(crate) fn local_scalar_members<'a>(
    ty: &'a ValueType,
    domains: &[DomainSchema<'_>],
) -> Option<&'a [omega_typed::DomainApplication]> {
    let ValueType::Qualified(base, members) = array_leaf(ty) else { return None };
    (scalar_carrier(base).is_ok()
        && !members.is_empty()
        && members.iter().all(|m| {
            let Ok(index) = domains.binary_search_by(|d| d.declaration.cmp(&m.declaration)) else {
                return false;
            };
            let domain = &domains[index];
            if matches!(ty, ValueType::Array(..)) {
                // The element TypeId refers to the named scalar's complete theory.
                // Selecting an existing element is transport, not a domain cast.
                return m.arguments.is_empty() && domain.carrier == base.as_ref() && domain.predicates().is_some();
            }
            domain.carrier.carrier() == base.as_ref()
                && domain
                    .carrier
                    .domains()
                    .iter()
                    .all(|parent| members.contains(parent))
                && (domain.predicates().is_some() || !m.arguments.is_empty() && m.arguments.iter().all(|arg| scalar_argument(arg).is_some()))
        }))
    .then_some(members)
}

fn scalar_argument(arg: &ValueType) -> Option<ScalarDomainArgument> {
    match arg {
        ValueType::StaticInteger { carrier, value } => Some(ScalarDomainArgument::Integer {
            carrier: scalar_carrier(carrier)
                .ok()
                .filter(|c| matches!(c, ScalarType::Integer { .. }))?,
            value: value.clone(),
        }),
        ValueType::StaticValue {
            carrier,
            value: omega_typed::StaticValue::Bool(b),
        } if **carrier == ValueType::Bool => Some(ScalarDomainArgument::Boolean(*b)),
        _ => None,
    }
}

fn shared_indexed_target(carrier: &ValueType) -> Option<IndexedReferent> {
    let ValueType::Reference(omega_typed::Access::Shared, inner) = carrier else { return None };
    let ValueType::Applied(owner, args) = inner.as_ref() else { return None };
    let arguments = args.iter().map(|arg| match arg {
        ValueType::StaticInteger { carrier, value } => Some((scalar_carrier(carrier).ok()?, value.clone())),
        _ => None,
    }).collect::<Option<_>>()?;
    Some((crate::identity::portable(owner)?, arguments))
}

fn atom_keys(members: &[DomainApplication]) -> Option<Vec<AtomKey>> {
    members
        .iter()
        .map(|m| Some((m.declaration.clone(), m.arguments.iter().map(scalar_argument).collect::<Option<_>>()?)))
        .collect()
}

/// A value made entirely of scalar bits cannot carry a loan across a return.
pub(crate) fn has_only_scalar_payload(carrier: &ValueType) -> bool {
    scalar_carrier(array_leaf(carrier).carrier()).is_ok()
}

pub(crate) fn contains_nominal_value(carrier: &ValueType) -> bool {
    array_leaf(carrier).nominal_parts().is_some()
}

fn array_leaf(mut carrier: &ValueType) -> &ValueType {
    while let ValueType::Array(element, _) = carrier {
        carrier = element;
    }
    carrier
}

pub(crate) fn nominal_reference(carrier: &ValueType) -> Option<(crate::ReferenceAccess, &DeclarationId)> {
    let (access, inner) = place_reference(carrier)?;
    let ValueType::Nominal(id) = inner else { return None };
    Some((access, id))
}

pub(crate) fn place_reference(carrier: &ValueType) -> Option<(crate::ReferenceAccess, &ValueType)> {
    let ValueType::Reference(access, inner) = carrier else { return None };
    // A shared reference to a closed const-indexed application names its
    // referent's identity without that referent's layout. It forms, forwards
    // and reborrows like any shared place reference; only a step that needs
    // the referent's schema refuses it.
    if !matches!(inner.as_ref(), ValueType::Nominal(_)) && stored_scalar(inner).is_err() && shared_indexed_target(carrier).is_none() {
        return None;
    }
    let access = match access {
        omega_typed::Access::Shared => crate::ReferenceAccess::Shared,
        omega_typed::Access::Mut => crate::ReferenceAccess::Mutable,
        omega_typed::Access::Write => return None,
    };
    Some((access, inner))
}

pub(crate) fn is_shared_byte_view(carrier: &ValueType) -> bool {
    byte_view_access(carrier) == Some(&omega_typed::Access::Shared)
}

pub(crate) fn byte_view_access(carrier: &ValueType) -> Option<&omega_typed::Access> {
    match carrier {
        ValueType::Reference(access @ (omega_typed::Access::Shared | omega_typed::Access::Mut), slice) if matches!(slice.as_ref(),ValueType::Slice(element) if matches!(element.as_ref(),ValueType::Integer{signed:false,width:Some(8)})) => {
            Some(access)
        }
        _ => None,
    }
}

/// Readable unqualified scalar elements, indexed in elements rather than bytes.
/// The descriptor retains its declared access even for a shared read.
pub(crate) fn scalar_element_view(carrier: &ValueType) -> Option<(&omega_typed::Access, ScalarType)> {
    let ValueType::Reference(access @ (omega_typed::Access::Shared | omega_typed::Access::Mut), slice) = carrier else {
        return None;
    };
    let ValueType::Slice(element) = slice.as_ref() else { return None };
    if matches!(element.as_ref(), ValueType::Integer { signed: false, width: Some(8) }) {
        return None;
    }
    Some((access, scalar_carrier(element).ok()?))
}

/// A shared (read-only) scalar element view; `@mut [T]` element views are excluded.
pub(crate) fn is_shared_element_view(carrier: &ValueType) -> bool {
    matches!(scalar_element_view(carrier), Some((omega_typed::Access::Shared, _)))
}

/// The nominal element of an exclusive element view `@mut [T]`. It is not a
/// copyable or forwardable descriptor: only its loan's element places lower.
pub(crate) fn mutable_element_view(carrier: &ValueType) -> Option<&DeclarationId> {
    let ValueType::Reference(omega_typed::Access::Mut, slice) = carrier else {
        return None;
    };
    let ValueType::Slice(element) = slice.as_ref() else { return None };
    let ValueType::Nominal(id) = element.as_ref() else { return None };
    Some(&**id)
}

/// An exclusive loan carrier: a `@mut T` reference or a `@mut [T]` element
/// view. It is affine, so a state edge transfers it rather than copying it
/// (terminal-psi/byte_views.md#calls-and-control-transfers).
pub(crate) fn is_exclusive_descriptor(carrier: &ValueType) -> bool {
    matches!(place_reference(carrier), Some((crate::ReferenceAccess::Mutable, _))) || is_exclusive_element_view(carrier)
}

/// A `@mut [T]` element view of scalar or nominal elements. Like every other
/// descriptor it lives in a structural place, but it is never copied.
pub(crate) fn is_exclusive_element_view(carrier: &ValueType) -> bool {
    matches!(scalar_element_view(carrier), Some((omega_typed::Access::Mut, _))) || mutable_element_view(carrier).is_some()
}

pub(crate) fn is_reference_descriptor(carrier: &ValueType) -> bool {
    is_shared_byte_view(carrier) || is_shared_element_view(carrier) || place_reference(carrier).is_some()
}

pub(crate) fn uses_structural_place(carrier: &ValueType) -> bool {
    matches!(carrier, ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..))
        || is_reference_descriptor(carrier)
        || is_exclusive_element_view(carrier)
}

pub(crate) fn supports_value_copy(carrier: &ValueType) -> bool {
    is_shared_byte_view(carrier)
        || is_shared_element_view(carrier)
        || matches!(
            array_leaf(carrier),
            ValueType::Bool | ValueType::Integer { width: Some(1..), .. } | ValueType::Float32 | ValueType::Float64
        )
}

/// A scalar that Psi can store, copy, borrow or view without computing on it.
/// IEEE floats keep their exact bits here. Operations that compute on a
/// scalar, such as arithmetic, domains, predicates, constants and registers,
/// use `scalar_carrier`, which still refuses floats.
pub(crate) fn stored_scalar(carrier: &ValueType) -> Result<ScalarType, TypeError> {
    match carrier {
        ValueType::Float32 => Ok(ScalarType::IeeeFloat(omega_psi::IeeeFloatFormat::Binary32)),
        ValueType::Float64 => Ok(ScalarType::IeeeFloat(omega_psi::IeeeFloatFormat::Binary64)),
        other => scalar_carrier(other),
    }
}

pub(crate) fn scalar_carrier(carrier: &ValueType) -> Result<ScalarType, TypeError> {
    match carrier {
        ValueType::Bool => Ok(ScalarType::Boolean),
        ValueType::Integer { signed, width: Some(0) } => Err(TypeError::ZeroIntegerWidth { signed: *signed }),
        ValueType::Integer { signed, width: Some(width) } => Ok(ScalarType::Integer {
            signed: *signed,
            width: *width,
        }),
        ValueType::Integer { signed, width: None } => Err(TypeError::MissingIntegerWidth { signed: *signed }),
        ValueType::IndexedInteger { .. } => Err(TypeError::OpenIntegerWidth),
        ValueType::IndexedArray(..) => Err(TypeError::OpenArrayExtent),
        ValueType::NoResult => Err(TypeError::NoResult),
        other => Err(TypeError::UnsupportedCarrier(other.to_string())),
    }
}
