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
    /// The domain selects an access policy, so who may add it to a value is
    /// undecided here. A cast that adds it must be one the package reports.
    pub selects_policy: bool,
}

/// Data rows share storage when only their lifetime arguments differ. The
/// checked declaration determines which slots are lifetimes. In particular,
/// an open type or const parameter must never be mistaken for a lifetime.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AppliedData {
    by_owner: std::collections::HashMap<DeclarationId, AppliedDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AppliedDeclaration {
    lifetime_count: usize,
    arity: usize,
    rows: std::collections::HashMap<Vec<ValueType>, DataId>,
}

impl AppliedData {
    fn intern(applications: &[super::applied_data::Application<'_>], first: usize) -> Self {
        let mut result = Self::default();
        for (index, application) in applications.iter().enumerate() {
            let ValueType::Applied(owner, arguments) = application.carrier else { continue };
            let owner = result.by_owner.entry((**owner).clone()).or_insert_with(|| AppliedDeclaration {
                lifetime_count: application.lifetime_count,
                arity: arguments.len(),
                rows: Default::default(),
            });
            owner.rows.insert(arguments[owner.lifetime_count..].to_vec(), DataId::from_index(first + index));
        }
        result
    }

    fn storage_arguments<'a>(&self, carrier: &'a ValueType) -> Option<(&AppliedDeclaration, &'a DeclarationId, &'a [ValueType])> {
        let ValueType::Applied(id, arguments) = carrier else { return None };
        let owner = self.by_owner.get(id.as_ref())?;
        if arguments.len() != owner.arity {
            return None;
        }
        Some((owner, id, &arguments[owner.lifetime_count..]))
    }

    fn id(&self, carrier: &ValueType) -> Option<DataId> {
        let (owner, _, arguments) = self.storage_arguments(carrier)?;
        owner.rows.get(arguments).copied()
    }

    fn shared_indexed_target(&self, carrier: &ValueType) -> Option<IndexedReferent> {
        let ValueType::Reference(omega_typed::Access::Shared, inner) = carrier else { return None };
        let (_, owner, arguments) = self.storage_arguments(inner)?;
        let values = arguments.iter().map(|argument| match argument {
            ValueType::StaticInteger { carrier, value } => Some((scalar_carrier(carrier).ok()?, value.clone())),
            _ => None,
        }).collect::<Option<_>>()?;
        Some((crate::identity::portable(owner)?, values))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoweringTypes {
    rows: Vec<TypeShape>,
    data_declarations: Vec<DeclarationId>,
    /// How many lifetime binders each of `data_declarations` takes, when all
    /// of its binders are lifetimes. Empty when no declaration registered any.
    data_lifetimes: Vec<usize>,
    applied_data: AppliedData,
    domain_sets: Vec<Vec<AtomKey>>,
    domain_cleanups: std::collections::BTreeMap<DeclarationId, crate::MachineId>,
    indexed_referents: Vec<IndexedReferent>,
    /// What adding each local domain declaration must establish at a cast.
    domain_establishments: std::collections::BTreeMap<DeclarationId, DomainEstablishment>,
    /// Each cast the package reports as adding a domain whose access policy is
    /// unanswered: the domain, the machine that owns the cast, and the cast.
    policy_introductions: Vec<(DeclarationId, DeclarationId, omega_typed::Site)>,
    /// Runtime callable contracts in declaration-site order; see `crate::declarations::callables`.
    callables: Vec<omega_typed::CallableType>,
}

impl LoweringTypes {
    /// `data_lifetimes` pairs with `data_declarations`: the lifetime binder
    /// count of each concrete declaration whose binders are all lifetimes,
    /// and zero for any other. Such a declaration has one row, and each of its
    /// applications at lifetimes shares it (`array_dimensions`).
    pub(super) fn from_declarations<'a>(
        carriers: impl Iterator<Item = &'a ValueType> + Clone,
        data_declarations: Vec<DeclarationId>,
        data_lifetimes: Vec<usize>,
        applications: &[super::applied_data::Application<'_>],
        domains: &[DomainSchema<'_>],
    ) -> Result<Self, (usize, TypeError)> {
        let mut sets = std::collections::BTreeSet::new();
        for (index, carrier) in carriers.clone().enumerate() {
            let subject = match held_reference(carrier) {
                ValueType::Reference(omega_typed::Access::Shared, inner) => inner.as_ref(),
                _ => carrier,
            };
            if matches!(array_leaf(subject), ValueType::Qualified(..)) {
                let members = local_domain_members(subject, domains).ok_or_else(|| (index, TypeError::UnsupportedCarrier(array_leaf(carrier).to_string())))?;
                sets.insert(atom_keys(members).ok_or_else(|| (index, TypeError::UnsupportedCarrier(carrier.to_string())))?);
            }
        }
        let domain_sets = sets.into_iter().collect();
        let callables = crate::declarations::callables::collect(carriers.clone())?;
        let applied_data = AppliedData::intern(applications, data_declarations.len());
        let indexed_referents = carriers.clone().map(held_reference)
            .filter_map(|carrier| applied_data.shared_indexed_target(carrier))
            .collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        Self {
            applied_data,
            indexed_referents,
            domain_sets,
            domain_cleanups: Default::default(),
            domain_establishments: domains
                .iter()
                // An indexed family's predicate binds its index; it still owes proof at addition.
                .map(|d| (d.declaration.clone(), DomainEstablishment {
                    predicate: d.has_predicates(),
                    selects_policy: d.policy.is_some(),
                }))
                .collect(),
            policy_introductions: Vec::new(),
            rows: Vec::new(),
            data_declarations,
            data_lifetimes,
            callables,
        }
        .intern_carriers(carriers)
    }

    fn intern_carriers<'a>(
        mut self,
        carriers: impl IntoIterator<Item = &'a ValueType>,
    ) -> Result<Self, (usize, TypeError)> {
        let carriers: Vec<&ValueType> = carriers.into_iter().collect();
        let mut leaves = Vec::new();
        // One frontier per depth: the layer each carrier has at that distance
        // from its leaf. An array and a sequence at the same depth share one.
        let mut frontiers: Vec<Vec<(usize, Layer)>> = Vec::new();
        let mut layers = Vec::new();
        for (index, carrier) in carriers.iter().enumerate() {
            let leaf = self
                .layers(carrier, &mut layers)
                .map_err(|error| (index, error))?;
            leaves.push((leaf, index));
            frontiers.resize_with(frontiers.len().max(layers.len()), Vec::new);
            for (frontier, layer) in frontiers.iter_mut().zip(layers.drain(..).rev()) {
                frontier.push((index, layer));
            }
        }

        let mut resolved = vec![None; leaves.len()];
        self.intern_frontier(&mut leaves, &mut resolved)?;
        for frontier in frontiers {
            leaves.clear();
            for (index, layer) in frontier {
                let element = resolved[index].expect("the element of a layer has already been interned");
                let row = self
                    .layer_row(layer, element, carriers[index])
                    .map_err(|error| (index, error))?;
                leaves.push((row, index));
            }
            self.intern_frontier(&mut leaves, &mut resolved)?;
        }
        Ok(self)
    }

    pub fn id_for(
        &self,
        carrier: &ValueType,
    ) -> Result<TypeId, TypeError> {
        let mut layers = Vec::new();
        let leaf = self.layers(carrier, &mut layers)?;
        let mut id = self.id_for_shape(leaf)?;
        for layer in layers.into_iter().rev() {
            id = self.id_for_shape(self.layer_row(layer, id, carrier)?)?;
        }
        Ok(id)
    }

    /// The leaf row of `carrier` and the layers around it, outermost first.
    /// A layer is a fixed array or a by-value sequence `[T]`; the two nest in
    /// any order. `array_dimensions` judges the leaf together with the arrays
    /// directly above it, as it does for a carrier with no sequence.
    fn layers(
        &self,
        mut carrier: &ValueType,
        layers: &mut Vec<Layer>,
    ) -> Result<TypeShape, TypeError> {
        loop {
            let mut below_arrays = carrier;
            while let ValueType::Array(element, _) = below_arrays {
                below_arrays = element;
            }
            let ValueType::Slice(element) = below_arrays else {
                let mut lengths = Vec::new();
                let leaf = self.array_dimensions(carrier, &mut lengths)?;
                layers.extend(lengths.into_iter().map(Layer::Array));
                return Ok(leaf);
            };
            while let ValueType::Array(inner, length) = carrier {
                layers.push(Layer::Array(*length));
                carrier = inner;
            }
            layers.push(Layer::Sequence);
            carrier = element;
        }
    }

    /// The row of one layer over the interned `element`. A sequence takes the
    /// exact row of its element. The element may be anything whose completed
    /// value the constructor can take as one place and that the sequence can
    /// share between its copies: a scalar (in its own home), a record or sum,
    /// a shared reference or view, a fixed array or another sequence. A
    /// domain-qualified value, a bare callable and an exclusive loan carrier
    /// have no such form here, so a sequence of one has no row.
    fn layer_row(
        &self,
        layer: Layer,
        element: TypeId,
        carrier: &ValueType,
    ) -> Result<TypeShape, TypeError> {
        match layer {
            Layer::Array(length) => Ok(TypeShape::FixedArray { element, length }),
            Layer::Sequence => match self.get(element) {
                Some(
                    TypeShape::PrimitiveScalar(_)
                    | TypeShape::Nominal(_)
                    | TypeShape::SharedByteView
                    | TypeShape::SharedElementView { .. }
                    | TypeShape::ScalarReference {
                        access: crate::ReferenceAccess::Shared,
                        ..
                    }
                    | TypeShape::SharedQualifiedScalarReference { .. }
                    | TypeShape::NominalReference {
                        access: crate::ReferenceAccess::Shared,
                        ..
                    }
                    | TypeShape::IndexedSharedReference(_)
                    | TypeShape::ElementSequence { .. }
                    | TypeShape::FixedArray { .. },
                ) => Ok(TypeShape::ElementSequence { element }),
                _ => Err(TypeError::UnsupportedCarrier(carrier.to_string())),
            },
        }
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
        declarations: &std::collections::BTreeMap<&DeclarationId, DomainDeclaration<'_>>,
    ) -> Result<omega_psi::TypeTable, crate::LoweringError> {
        let mut sets = Vec::with_capacity(self.domain_sets.len());
        for set in self.domain_sets {
            let mut atoms = Vec::with_capacity(set.len());
            for (id, arguments) in set {
                // local_domain_members has already checked every identity against this catalog.
                let domain = match &declarations[&id] {
                    DomainDeclaration::Named { row, .. } => row,
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
        // Every deeper layer refers to an element with a greater ID than any
        // shallower layer uses. Sorting each depth by `row_order` therefore
        // keeps all rows sorted for `id_for_shape`.
        frontier.sort_unstable_by(|(left, at), (right, other)| row_order(left, right).then(at.cmp(other)));
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
        let leaf = array_leaf(carrier);
        // Copying a by-value sequence copies its descriptor; the immutable
        // elements stay in the backing its construction established.
        supports_value_copy(carrier)
            || (matches!(leaf, ValueType::Qualified(base, _) if stored_scalar(base).is_ok())
                || matches!(leaf, ValueType::Callable(_))
                || is_element_sequence(carrier))
                && self.id_for(carrier).is_ok()
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

    pub(crate) fn register_domain_cleanup(&mut self, domain: &DeclarationId, machine: crate::MachineId) {
        self.domain_cleanups.insert(domain.clone(), machine);
    }

    pub(crate) fn domain_cleanup(&self, domain: &DeclarationId) -> Option<crate::MachineId> {
        self.domain_cleanups.get(domain).copied()
    }

    /// Typing checks that all applicable selections agree, including the default
    /// selection on the data. A bare occurrence receives only the default.
    pub(crate) fn cleanup_target(&self, subject: TypeId, data: &[crate::LoweredData]) -> Option<crate::MachineId> {
        match self.get(subject)? {
            TypeShape::Nominal(id) => data[id.get() - 1].cleanup_target,
            TypeShape::QualifiedNominal { data: id, domains } => self.domain_sets[domains.0.get() - 1].iter()
                .find_map(|(decl, _)| self.domain_cleanup(decl)).or(data[id.get() - 1].cleanup_target),
            _ => None,
        }
    }

    pub(crate) fn domain_establishment(&self, declaration: &DeclarationId) -> Option<DomainEstablishment> {
        self.domain_establishments.get(declaration).copied()
    }

    /// Record the casts the package will hand to its driver as introductions
    /// of pending domains. The cast lowerer admits no other such cast.
    pub(crate) fn report_policy_introductions(&mut self, introductions: Vec<(DeclarationId, DeclarationId, omega_typed::Site)>) {
        self.policy_introductions = introductions;
    }

    /// Whether the package reports the cast at `site` in `machine` as adding `domain`.
    pub(crate) fn reports_policy_introduction(&self, domain: &DeclarationId, machine: &DeclarationId, site: &omega_typed::Site) -> bool {
        self.policy_introductions
            .iter()
            .any(|(reported, owner, cast)| reported == domain && owner == machine && cast == site)
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

    fn domain_set_id(&self, carrier: &ValueType, members: &[DomainApplication]) -> Result<DomainSetId, TypeError> {
        let key = atom_keys(members).ok_or_else(|| TypeError::UnsupportedCarrier(carrier.to_string()))?;
        let index = self.domain_sets.binary_search(&key).map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
        Ok(DomainSetId(std::num::NonZeroUsize::new(index + 1).expect("domain set belongs to this table")))
    }

    fn id_for_shape(
        &self,
        shape: TypeShape,
    ) -> Result<TypeId, TypeError> {
        let index = self
            .rows
            .binary_search_by(|row| row_order(row, &shape))
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
        // A lifetime adds no layout, so `@'r T` is the row of `@T`, as every
        // application of a lifetime-only record below is that record's one
        // row. The row forgets the region. A stored field keeps it in
        // `LoweredField::lifetime`, a signature in its `MachineRegion` slots,
        // and a value typing bound to a declared region in
        // `MachineBody::lifetime_bindings`.
        if lengths.is_empty() {
            carrier = held_reference(carrier);
        }
        if lengths.is_empty() && let Some(key) = self.applied_data.shared_indexed_target(carrier) && let Ok(i) = self.indexed_referents.binary_search(&key) {
            return Ok(TypeShape::IndexedSharedReference(omega_psi::IndexedReferentId(i)));
        }
        if lengths.is_empty()
            && let Some((access, referent)) = nominal_reference(carrier)
        {
            let index = self
                .data_declarations
                .binary_search(referent)
                .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
            // A referent applied at lifetimes is its declaration's one row, as
            // an owned value of it is below. The row forgets the arguments.
            if let Some((_, ValueType::Applied(_, arguments))) = place_reference(carrier)
                && self.data_lifetimes.get(index) != Some(&arguments.len())
            {
                return Err(TypeError::UnsupportedCarrier(carrier.to_string()));
            }
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
        if lengths.is_empty()
            && let ValueType::Reference(omega_typed::Access::Shared, inner) = carrier
            && let ValueType::Qualified(base, members) = inner.as_ref()
        {
            return Ok(TypeShape::SharedQualifiedScalarReference { referent: stored_scalar(base)?, domains: self.domain_set_id(inner, members)? });
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
        if lengths.is_empty() {
            let view = if let Some((access, scalar)) = scalar_element_view(carrier) {
                Some((access, crate::ViewElement::Scalar(scalar)))
            } else if let Some((access, declaration)) = nominal_element_view(carrier) {
                let index = self.data_declarations.binary_search(declaration)
                    .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
                Some((access, crate::ViewElement::Nominal(DataId::from_index(index))))
            } else {
                None
            };
            if let Some((access, element)) = view {
                return Ok(if *access == omega_typed::Access::Mut {
                    TypeShape::MutableElementView { element }
                } else {
                    TypeShape::SharedElementView { element }
                });
            }
        }
        if let ValueType::Qualified(base, members) = carrier {
            let domains = self.domain_set_id(carrier, members)?;
            if let ValueType::Nominal(declaration) = base.as_ref() {
                let index = self.data_declarations.binary_search(declaration)
                    .map_err(|_| TypeError::UnsupportedCarrier(carrier.to_string()))?;
                return Ok(TypeShape::QualifiedNominal { data: DataId::from_index(index), domains });
            }
            return Ok(TypeShape::QualifiedScalar { carrier: stored_scalar(base)?, domains });
        }
        if let Some(id) = self.applied_data.id(carrier) {
            return Ok(TypeShape::Nominal(id));
        }
        // A lifetime argument adds no layout, so every application of a
        // lifetime-only record is that record's one row. The row forgets which
        // region was applied. A machine signature keeps it in its
        // `MachineRegion` slots, and a stored reference in the operation that
        // supplied its field.
        if let ValueType::Applied(declaration, arguments) = carrier
            && let Ok(index) = self.data_declarations.binary_search(declaration)
            && self.data_lifetimes.get(index).is_some_and(|&lifetimes| lifetimes != 0 && lifetimes == arguments.len())
        {
            return Ok(TypeShape::Nominal(DataId::from_index(index)));
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

/// One layer around a stored leaf: a fixed array of this length, or a
/// by-value sequence.
#[derive(Clone, Copy)]
enum Layer {
    Array(u64),
    Sequence,
}

/// The order `LoweringTypes::rows` is kept in. Leaves come first in their
/// derived order. A sequence or array row comes after the row of its element,
/// so those order by element first: a deeper layer always sorts after a
/// shallower one, whichever kind each is.
fn row_order(
    left: &TypeShape,
    right: &TypeShape,
) -> std::cmp::Ordering {
    let element = |shape: &TypeShape| match shape {
        TypeShape::ElementSequence { element } | TypeShape::FixedArray { element, .. } => Some(*element),
        _ => None,
    };
    element(left).cmp(&element(right)).then_with(|| left.cmp(right))
}

pub(crate) fn local_domain_members<'a>(
    ty: &'a ValueType,
    domains: &[DomainSchema<'_>],
) -> Option<&'a [omega_typed::DomainApplication]> {
    let ValueType::Qualified(base, members) = array_leaf(ty) else { return None };
    ((stored_scalar(base).is_ok() || matches!(base.as_ref(), ValueType::Nominal(_)))
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

/// The const-indexed application a shared reference refers to, by storage
/// identity: its declaration and closed integer arguments.
///
/// The lifetime arguments an application writes first select no storage, so
/// `@Link<'a, 4>` and `@Link<'b, 4>` name one referent and the key leaves
/// them out. This reads only the type, like `lifetime_application`: a leading
/// run of binders and inferred regions before at least one closed integer.
/// The type table then decides. A key becomes a row only for a declaration
/// with a closed application (`from_declarations`), and every type the table
/// interns is closed apart from regions, so nothing else can lead such a run.
/// The region positions are kept beside the row: in the field that stores the
/// reference and in the `MachineRegion` slots of a formal or result.
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
    array_leaf(carrier).carrier().nominal_parts().is_some()
}

fn array_leaf(mut carrier: &ValueType) -> &ValueType {
    while let ValueType::Array(element, _) = carrier {
        carrier = element;
    }
    carrier
}

pub(crate) fn nominal_reference(carrier: &ValueType) -> Option<(crate::ReferenceAccess, &DeclarationId)> {
    let (access, inner) = place_reference(carrier)?;
    match inner {
        ValueType::Nominal(id) => Some((access, id)),
        other => lifetime_application(other).map(|id| (access, id)),
    }
}

/// A record applied at lifetimes only, as far as the type shows: a region
/// binder or a region typing inferred. The type table decides whether the
/// declaration takes exactly that many lifetimes.
fn lifetime_application(ty: &ValueType) -> Option<&DeclarationId> {
    let ValueType::Applied(declaration, arguments) = ty else { return None };
    let lifetimes = !arguments.is_empty()
        && arguments
            .iter()
            .all(|argument| matches!(argument, ValueType::Parameter { .. } | ValueType::InferredLifetime { .. }));
    lifetimes.then_some(declaration.as_ref())
}

/// The reference of a place reference held at a lifetime: `@T` for `@'r T`,
/// with its access. Any other type is returned as it is. A view at a lifetime,
/// `@'r [u8]`, is not a place reference and keeps its own handling
/// (`machines::loans::view_at_region`).
pub(crate) fn held_reference(carrier: &ValueType) -> &ValueType {
    match carrier {
        ValueType::AtLifetime(reference, _) if matches!(reference.as_ref(), ValueType::Reference(..)) && place_reference(reference).is_some() => reference.as_ref(),
        other => other,
    }
}

/// The access and referent of a reference to one place: a scalar, a record or
/// sum, or a record applied at lifetimes only. The lifetime such a reference
/// is held at, as in `@'r mutable Leaf`, changes neither, so this reads
/// through it. Callers that publish the region read it from the type itself
/// (`declarations::regions`).
pub(crate) fn place_reference(carrier: &ValueType) -> Option<(crate::ReferenceAccess, &ValueType)> {
    let carrier = match carrier {
        ValueType::AtLifetime(reference, _) => reference.as_ref(),
        other => other,
    };
    let ValueType::Reference(access, inner) = carrier else { return None };
    // A shared reference to a closed const-indexed application names its
    // referent's identity without that referent's layout. It forms, forwards
    // and reborrows like any shared place reference; only a step that needs
    // the referent's schema refuses it.
    let shared_qualified = *access == omega_typed::Access::Shared && matches!(inner.as_ref(), ValueType::Qualified(base, _) if stored_scalar(base).is_ok());
    // A reference to a record applied at lifetimes refers to that record's
    // one row, so it reads, forwards and lends like a reference to a bare
    // record, whatever its access. The row forgets the lifetime arguments.
    let record_at_lifetimes = lifetime_application(inner).is_some();
    if !matches!(inner.as_ref(), ValueType::Nominal(_)) && stored_scalar(inner).is_err() && !shared_qualified && !record_at_lifetimes && !matches!((access, inner.as_ref()),
        (omega_typed::Access::Shared, ValueType::Applied(_, arguments))
            if matches!(arguments.last(), Some(ValueType::StaticInteger { .. }))) {
        return None;
    }
    let access = match access {
        omega_typed::Access::Shared => crate::ReferenceAccess::Shared,
        omega_typed::Access::Mut => crate::ReferenceAccess::Mutable,
        omega_typed::Access::Write => crate::ReferenceAccess::WriteOnly,
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

pub(crate) fn element_view_access(carrier: &ValueType) -> Option<&omega_typed::Access> {
    scalar_element_view(carrier).map(|(access, _)| access)
        .or_else(|| nominal_element_view(carrier).map(|(access, _)| access))
}

pub(crate) fn is_shared_element_view(carrier: &ValueType) -> bool {
    element_view_access(carrier) == Some(&omega_typed::Access::Shared)
}

pub(crate) fn nominal_element_view(carrier: &ValueType) -> Option<(&omega_typed::Access, &DeclarationId)> {
    let ValueType::Reference(access @ (omega_typed::Access::Shared | omega_typed::Access::Mut), slice) = carrier else { return None };
    let ValueType::Slice(element) = slice.as_ref() else { return None };
    let ValueType::Nominal(id) = element.as_ref() else { return None };
    Some((access, id))
}

/// An exclusive place reference: `@mutable T` or `@write T`. A write-only loan is
/// exclusive too (language/ownership.md#borrows-and-aliases), and exclusive
/// descriptors never gain copy permission
/// (terminal-psi/structural_access.md#write-only-authority).
pub(crate) fn is_exclusive_reference(carrier: &ValueType) -> bool {
    matches!(
        place_reference(carrier),
        Some((crate::ReferenceAccess::Mutable | crate::ReferenceAccess::WriteOnly, _))
    )
}

/// An exclusive loan carrier: a `@mutable T` or `@write T` reference or a
/// `@mutable [T]` element view. It is affine, so a state edge transfers it rather
/// than copying it (terminal-psi/byte_views.md#calls-and-control-transfers).
pub(crate) fn is_exclusive_descriptor(carrier: &ValueType) -> bool {
    is_exclusive_reference(carrier) || is_exclusive_element_view(carrier)
}

/// A `@mutable [T]` element view of scalar or nominal elements. Like every other
/// descriptor it lives in a structural place, but it is never copied.
pub(crate) fn is_exclusive_element_view(carrier: &ValueType) -> bool {
    element_view_access(carrier) == Some(&omega_typed::Access::Mut)
}

pub(crate) fn is_reference_descriptor(carrier: &ValueType) -> bool {
    is_shared_byte_view(carrier) || is_shared_element_view(carrier) || place_reference(carrier).is_some()
}

pub(crate) fn uses_structural_place(carrier: &ValueType) -> bool {
    matches!(carrier.carrier(), ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..))
        || is_reference_descriptor(carrier)
        || is_exclusive_element_view(carrier)
        || is_element_sequence(carrier)
}

/// `[T]` held by value: an immutable sequence, not a borrowed `@[T]` view and
/// not a fixed `[T; N]`. Whether its element has a type-table row is decided
/// by `LoweringTypes::id_for`.
pub(crate) fn is_element_sequence(carrier: &ValueType) -> bool {
    matches!(carrier, ValueType::Slice(_))
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
/// IEEE floats keep their exact bits when carrying or erasing a qualification.
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
