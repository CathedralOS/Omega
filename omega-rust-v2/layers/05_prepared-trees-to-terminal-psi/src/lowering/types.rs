use crate::declarations::domains::DomainSchema;
use omega_resolved::DeclarationId;
use omega_typed::ValueType;

use crate::DataId;
use std::fmt;

use crate::{DomainSetId, ScalarType, TypeId, TypeShape};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoweringTypes {
    rows: Vec<TypeShape>,
    data_declarations: Vec<DeclarationId>,
    applied_data: std::collections::HashMap<ValueType, DataId>,
    domain_sets: Vec<Vec<DeclarationId>>,
    predicate_domains: Vec<DeclarationId>,
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
                sets.insert(members.iter().map(|m| &m.declaration).collect::<Vec<_>>());
            }
        }
        let domain_sets = sets
            .into_iter()
            .map(|members| members.into_iter().cloned().collect())
            .collect();
        let callables = crate::declarations::callables::collect(carriers.clone())?;
        let applied_data = applications
            .into_iter()
            .enumerate()
            .map(|(index, carrier)| (carrier, DataId::from_index(data_declarations.len() + index)))
            .collect();
        Self {
            applied_data,
            domain_sets,
            predicate_domains: domains
                .iter()
                .filter(|d| d.predicates().is_some_and(|p| !p.is_empty()))
                .map(|d| d.declaration.clone())
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
        identities: &std::collections::HashMap<&DeclarationId, omega_psi::DeclarationId>,
    ) -> omega_psi::TypeTable {
        let domain_sets = self
            .domain_sets
            .iter()
            .map(|set| set.iter().map(|id| identities[id].clone()).collect())
            .collect();
        omega_psi::TypeTable::new(self.rows, domain_sets)
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

    pub(crate) fn has_domain_predicate(
        &self,
        declaration: &DeclarationId,
    ) -> bool {
        self.predicate_domains.binary_search(declaration).is_ok()
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
            && let Ok(referent) = scalar_carrier(inner)
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
            && let Some(element) = shared_element_view(carrier)
        {
            return Ok(TypeShape::SharedElementView { element });
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
            if members.iter().any(|m| !m.arguments.is_empty()) {
                return Err(TypeError::UnsupportedCarrier(carrier.to_string()));
            }
            let index = self
                .domain_sets
                .binary_search_by(|set| set.iter().cmp(members.iter().map(|m| &m.declaration)))
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
            scalar_carrier(carrier).map(TypeShape::PrimitiveScalar)
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
            // Array contracts cannot yet express a predicate over every element.
            m.arguments.is_empty()
                && domain.carrier == base.as_ref()
                && domain
                    .predicates()
                    .is_some_and(|p| p.is_empty() || !matches!(ty, ValueType::Array(..)))
        }))
    .then_some(members)
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
    if !matches!(inner.as_ref(), ValueType::Nominal(_)) && scalar_carrier(inner).is_err() {
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

/// The scalar element of a shared non-byte slice view. `@[u8]` remains a byte
/// view; mutable, qualified, bounded and structured elements are not admitted.
pub(crate) fn shared_element_view(carrier: &ValueType) -> Option<ScalarType> {
    let ValueType::Reference(omega_typed::Access::Shared, slice) = carrier else {
        return None;
    };
    let ValueType::Slice(element) = slice.as_ref() else { return None };
    if matches!(element.as_ref(), ValueType::Integer { signed: false, width: Some(8) }) {
        return None;
    }
    scalar_carrier(element).ok()
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

pub(crate) fn is_reference_descriptor(carrier: &ValueType) -> bool {
    is_shared_byte_view(carrier) || shared_element_view(carrier).is_some() || place_reference(carrier).is_some()
}

pub(crate) fn uses_structural_place(carrier: &ValueType) -> bool {
    matches!(carrier, ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..)) || is_reference_descriptor(carrier)
}

pub(crate) fn supports_value_copy(carrier: &ValueType) -> bool {
    is_shared_byte_view(carrier)
        || shared_element_view(carrier).is_some()
        || matches!(array_leaf(carrier), ValueType::Bool | ValueType::Integer { width: Some(1..), .. })
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
