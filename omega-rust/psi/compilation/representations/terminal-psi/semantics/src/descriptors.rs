/*
Semantic judgments for borrowed byte/element descriptors, observable descriptor lifetimes
(`descriptor_lifetimes`), returned descriptor backing origins and payload histories
(`descriptor_results`), and membership presentations across selections and primitive stores.
*/

pub mod descriptor_lifetimes {

    use semantic_vocabulary::{BlockId, PlaceId};
    use std::collections::{BTreeMap, BTreeSet};

    pub(crate) mod arrivals {
        use super::{DescriptorLocation, DescriptorPathSegment};
        use crate::storage::borrowed_storage::Projection;
        use semantic_vocabulary::{PlaceId, ScalarTerm, StructuralTypeId};
        use terminal_psi::StructuralAccess;

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum DescriptorContentsStep {
            Address(DescriptorPathSegment),
            Element {
                element: StructuralTypeId,
                // The same typed selector term as ScalarReadProjection::Index.
                // Without one the query ranges over all possible elements.
                selector: Option<Box<ScalarTerm>>,
            },
        }

        impl DescriptorContentsStep {
            pub(crate) fn may_match(&self, other: &Self) -> bool {
                self == other
                    || matches!(
                        (self, other),
                        (
                            Self::Element { .. },
                            Self::Address(DescriptorPathSegment::FixedIndex(_))
                        ) | (
                            Self::Address(DescriptorPathSegment::FixedIndex(_)),
                            Self::Element { .. }
                        )
                    )
                    || matches!(
                        (self, other),
                        (Self::Element { element: left, .. }, Self::Element { element: right, .. })
                            if left == right
                    )
            }
            pub fn any_element(element: StructuralTypeId) -> Self {
                Self::Element {
                    element,
                    selector: None,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub struct DescriptorContentsQuery {
            pub root: PlaceId,
            pub steps: Vec<DescriptorContentsStep>,
        }

        #[derive(Clone)]
        pub enum DescriptorFlowSource {
            Location(DescriptorLocation),
            Contents(DescriptorContentsQuery),
            Unavailable(PlaceId),
        }

        impl From<DescriptorLocation> for DescriptorFlowSource {
            fn from(location: DescriptorLocation) -> Self {
                Self::Location(location)
            }
        }

        impl DescriptorFlowSource {
            pub fn from_query(root: PlaceId, query: Option<DescriptorContentsQuery>) -> Self {
                query.map(Self::Contents).unwrap_or(Self::Unavailable(root))
            }
            pub fn exact_location(&self) -> Option<&DescriptorLocation> {
                match self {
                    Self::Location(location) => Some(location),
                    _ => None,
                }
            }
            pub fn observation(&self) -> DescriptorLocation {
                match self {
                    Self::Location(location) => location.clone(),
                    Self::Contents(query) => DescriptorLocation::whole(query.root),
                    Self::Unavailable(root) => DescriptorLocation::whole(*root),
                }
            }
        }

        #[derive(Clone)]
        pub struct DescriptorFlowBinding {
            pub source: DescriptorFlowSource,
            pub parameter: DescriptorLocation,
            pub access: StructuralAccess,
            /// Permission held by the actual projection before this edge.
            /// A weaker header starts a child borrow, not a second name for
            /// its parent's authority.
            pub source_access: StructuralAccess,
            /// The actual descriptor and relative address captured before the
            /// edge installs its parameter. Contents queries may erase selectors;
            /// this projection retains their original formation coordinates.
            pub captured_storage: Option<(DescriptorLocation, Projection)>,
        }

        impl DescriptorFlowBinding {
            pub fn new(
                source: impl Into<DescriptorFlowSource>,
                parameter: DescriptorLocation,
                actual_access: StructuralAccess,
                parameter_access: StructuralAccess,
            ) -> Option<Self> {
                (actual_access == parameter_access).then_some(Self {
                    source: source.into(),
                    parameter,
                    access: parameter_access,
                    source_access: actual_access,
                    captured_storage: None,
                })
            }

            pub fn locations(&self) -> Option<(&DescriptorLocation, &DescriptorLocation)> {
                Some((self.source.exact_location()?, &self.parameter))
            }
        }
    }
    pub use arrivals::{
        DescriptorContentsQuery, DescriptorContentsStep, DescriptorFlowBinding,
        DescriptorFlowSource,
    };
    pub(crate) mod locations {
        use super::{DescriptorLocation, DescriptorPathSegment};
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralTypeId,
        };
        use terminal_psi::{
            ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment,
            StructuralTypeDeclaration, StructuralTypeShape,
        };

        pub fn canonical_descriptor_location(
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            root_type: StructuralTypeId,
            path: &[CanonicalStructuralPathSegment],
        ) -> Option<DescriptorLocation> {
            let path = path
                .iter()
                .map(|segment| match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        DescriptorPathSegment::Field(*field)
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        DescriptorPathSegment::FixedIndex(*index)
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        DescriptorPathSegment::Case(*case)
                    }
                    CanonicalStructuralPathSegment::Referent => DescriptorPathSegment::Referent,
                })
                .collect::<Vec<_>>();
            resolved_path(types, root_type, &path, |_, _, _| None)?;
            Some(DescriptorLocation { root, path })
        }

        pub fn descriptor_location(
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            final_field: Option<StructuralFieldId>,
        ) -> Option<DescriptorLocation> {
            let mut result = DescriptorLocation::whole(root);
            let mut tip = Some(root_type);
            let mut case_fields = None;
            for segment in path {
                let declaration = declaration(types, tip?)?;
                let canonical = match segment {
                    StructuralPathSegment::RepresentationView {
                        structural_type,
                        stored_scalar,
                    } if case_fields.is_none() => {
                        tip = Some(
                            crate::representation_view_declaration(
                                types.iter(),
                                *structural_type,
                                *stored_scalar,
                            )?
                            .id,
                        );
                        DescriptorPathSegment::RepresentationView {
                            structural_type: *structural_type,
                            stored_scalar: *stored_scalar,
                        }
                    }
                    StructuralPathSegment::Referent if case_fields.is_none() => {
                        let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                        else {
                            return None;
                        };
                        tip = Some(referent);
                        DescriptorPathSegment::Referent
                    }
                    StructuralPathSegment::FixedIndex(index) if case_fields.is_none() => {
                        tip = fixed_element(&declaration.shape, *index)?;
                        DescriptorPathSegment::FixedIndex(*index)
                    }
                    StructuralPathSegment::Case(id) if case_fields.is_none() => {
                        let cases = declaration.shape.sum_cases()?;
                        let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                        case_fields = Some(case.fields.as_slice());
                        DescriptorPathSegment::Case(*id)
                    }
                    StructuralPathSegment::Field(name) => {
                        let fields = match case_fields.take() {
                            Some(fields) => fields,
                            None => fields(declaration)?,
                        };
                        let mut matching = fields.iter().filter(|field| {
                            field.identity == *name && !field.relevance.is_erased()
                        });
                        let field = matching.next()?;
                        if matching.next().is_some()
                            || fields.iter().filter(|other| other.id == field.id).count() != 1
                        {
                            return None;
                        }
                        tip = match field.field_type {
                            StructuralFieldType::Structural(id) => Some(id),
                            _ => None,
                        };
                        DescriptorPathSegment::Field(field.id)
                    }
                    _ => return None,
                };
                result.path.push(canonical);
            }
            if let Some(id) = final_field {
                let declaration = declaration(types, tip?)?;
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => fields(declaration)?,
                };
                let mut matching = fields
                    .iter()
                    .filter(|field| field.id == id && !field.relevance.is_erased());
                let field = matching.next()?;
                if matching.next().is_some()
                    || fields
                        .iter()
                        .filter(|other| other.identity == field.identity)
                        .count()
                        != 1
                {
                    return None;
                }
                result.path.push(DescriptorPathSegment::Field(id));
            }
            case_fields.is_none().then_some(result)
        }

        pub fn fixed_runtime_index_alternatives(
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
        ) -> Option<Vec<DescriptorLocation>> {
            let index_at = path.iter().position(|segment| {
                matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
            })?;
            let suffix = &path[index_at + 1..];
            if suffix
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
            {
                return None;
            }
            let prefix = &path[..index_at];
            let prefix_location = descriptor_location(types, root, root_type, prefix, None)?;
            let prefix_type = descriptor_location_type(types, root_type, &prefix_location.path)?;
            let length = indexed_carrier_length(types, prefix_type)?;
            if length == 0 || length > 4096 {
                return None;
            }
            let mut alternatives = Vec::with_capacity(usize::try_from(length).ok()?);
            for index in 0..length {
                let mut structural = prefix.to_vec();
                if !matches!(
                    declaration(types, prefix_type)?.shape,
                    StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::ByteSequence(
                            ByteSequenceCarrier::BoundedView { .. }
                                | ByteSequenceCarrier::BoundedOwned { .. }
                        )
                ) {
                    structural.push(StructuralPathSegment::Referent);
                }
                structural.push(StructuralPathSegment::FixedIndex(index));
                structural.extend_from_slice(suffix);
                alternatives.push(descriptor_location(
                    types,
                    root,
                    root_type,
                    &structural,
                    None,
                )?);
            }
            Some(alternatives)
        }

        fn indexed_carrier_length(
            types: &[StructuralTypeDeclaration],
            structural_type: StructuralTypeId,
        ) -> Option<u64> {
            let shape = &declaration(types, structural_type)?.shape;
            match shape {
                StructuralTypeShape::FixedArray { length, .. } => length.literal(),
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BoundedView { capacity, .. }
                    | ByteSequenceCarrier::BoundedOwned { capacity },
                ) => Some(*capacity),
                StructuralTypeShape::Reference { referent, .. } => {
                    indexed_carrier_length(types, *referent)
                }
                _ => None,
            }
        }

        pub fn descriptor_location_type(
            types: &[StructuralTypeDeclaration],
            root_type: StructuralTypeId,
            path: &[DescriptorPathSegment],
        ) -> Option<StructuralTypeId> {
            resolved_path(types, root_type, path, |_, _, _| None)?.1
        }
        pub fn descriptor_path(
            types: &[StructuralTypeDeclaration],
            root_type: StructuralTypeId,
            path: &[DescriptorPathSegment],
        ) -> Option<Vec<StructuralPathSegment>> {
            Some(resolved_path(types, root_type, path, |_, _, _| None)?.0)
        }
        pub(crate) fn resolved_path(
            types: &[StructuralTypeDeclaration],
            root_type: StructuralTypeId,
            path: &[DescriptorPathSegment],
            mut constructor_element: impl FnMut(
                &[DescriptorPathSegment],
                StructuralTypeId,
                u64,
            ) -> Option<StructuralTypeId>,
        ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
            let mut result = Vec::new();
            let mut tip = Some(root_type);
            let mut case_fields = None;
            for segment in path {
                let declaration = declaration(types, tip?)?;
                result.push(match segment {
                    DescriptorPathSegment::RepresentationView {
                        structural_type,
                        stored_scalar,
                    } if case_fields.is_none() => {
                        tip = Some(
                            crate::representation_view_declaration(
                                types.iter(),
                                *structural_type,
                                *stored_scalar,
                            )?
                            .id,
                        );
                        StructuralPathSegment::RepresentationView {
                            structural_type: *structural_type,
                            stored_scalar: *stored_scalar,
                        }
                    }
                    DescriptorPathSegment::Referent if case_fields.is_none() => {
                        let StructuralTypeShape::Reference { referent, .. } = declaration.shape
                        else {
                            return None;
                        };
                        tip = Some(referent);
                        StructuralPathSegment::Referent
                    }
                    DescriptorPathSegment::FixedIndex(index) if case_fields.is_none() => {
                        tip = match declaration.shape {
                            StructuralTypeShape::ElementView {
                                access: None | Some(terminal_psi::StructuralAccess::SharedBorrow),
                                ..
                            } => {
                                let prefix = &path[..result.len()];
                                Some(constructor_element(prefix, declaration.id, *index)?)
                            }
                            _ => fixed_element(&declaration.shape, *index)?,
                        };
                        StructuralPathSegment::FixedIndex(*index)
                    }
                    DescriptorPathSegment::Case(id) if case_fields.is_none() => {
                        let cases = declaration.shape.sum_cases()?;
                        let case = exactly_one(cases.iter(), |case| case.id == *id)?;
                        case_fields = Some(case.fields.as_slice());
                        StructuralPathSegment::Case(*id)
                    }
                    DescriptorPathSegment::Field(id) => {
                        let fields = match case_fields.take() {
                            Some(fields) => fields,
                            None => fields(declaration)?,
                        };
                        let field = exactly_one(fields.iter(), |field| {
                            field.id == *id && !field.relevance.is_erased()
                        })?;
                        // A leaf field that states its carrier names its
                        // exact declaration. Reading it back from the leaf
                        // shape finds nothing as soon as two declarations
                        // share that shape, as the byte views of two regions
                        // do, and a path ending at such a field would then
                        // have no type at all.
                        tip = match field.field_type {
                            StructuralFieldType::Structural(id) => Some(id),
                            _ => crate::inline_carrier_declaration(types.iter(), field)
                                .or_else(|| {
                                    field
                                        .field_type
                                        .canonical_leaf_type()
                                        .and_then(|leaf| leaf.leaf_declaration(types.iter()))
                                })
                                .map(|declaration| declaration.id),
                        };
                        StructuralPathSegment::Field(field.identity)
                    }
                    _ => return None,
                });
            }
            case_fields.is_none().then_some((result, tip))
        }

        fn declaration(
            types: &[StructuralTypeDeclaration],
            id: StructuralTypeId,
        ) -> Option<&StructuralTypeDeclaration> {
            let mut matching = types.iter().filter(|declaration| declaration.id == id);
            let declaration = matching.next()?;
            matching.next().is_none().then_some(declaration)
        }
        fn fields(
            declaration: &StructuralTypeDeclaration,
        ) -> Option<&[terminal_psi::StructuralFieldDeclaration]> {
            match &declaration.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => Some(fields),
                _ => None,
            }
        }

        fn fixed_element(
            shape: &StructuralTypeShape,
            index: u64,
        ) -> Option<Option<StructuralTypeId>> {
            match shape {
                StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } if index < *length => Some(Some(*element)),
                StructuralTypeShape::ByteSequence(
                    ByteSequenceCarrier::BoundedView { capacity, .. }
                    | ByteSequenceCarrier::BoundedOwned { capacity },
                ) if index < *capacity => Some(None),
                _ => None,
            }
        }
    }
    pub(crate) mod residences {

        use super::{DescriptorFlowStep, DescriptorLocation, DescriptorPosition};
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Default)]
        pub struct DescriptorResidenceRules {
            pub captures: BTreeSet<(DescriptorPosition, DescriptorLocation, DescriptorLocation)>,
        }

        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
        enum ResidenceName {
            Storage(DescriptorLocation),
            AddressContents(DescriptorLocation),
        }

        impl ResidenceName {
            fn location(&self) -> &DescriptorLocation {
                match self {
                    Self::Storage(location) | Self::AddressContents(location) => location,
                }
            }
        }

        pub fn descriptor_residence_live_positions(
            steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            rules: &DescriptorResidenceRules,
            birth: DescriptorPosition,
            residence: &DescriptorLocation,
            at_entry: bool,
            observes: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
        ) -> BTreeSet<DescriptorPosition> {
            type Occurrence = (DescriptorPosition, ResidenceName);
            let mut pending = Vec::new();
            if at_entry {
                pending.push((birth, ResidenceName::Storage(residence.clone())));
            } else if let Some(step) = steps.get(&birth) {
                for edge in &step.successors {
                    pending.push((edge.target, ResidenceName::Storage(residence.clone())));
                }
            }
            let mut visited = BTreeSet::new();
            let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
            let mut observations = Vec::new();
            let mut escaped = Vec::new();
            while let Some(occurrence) = pending.pop() {
                if !visited.insert(occurrence.clone()) {
                    continue;
                }
                let (position, name) = &occurrence;
                let Some(step) = steps.get(position) else {
                    continue;
                };
                let location = name.location();
                if step
                    .observed_inputs()
                    .any(|input| observes(*position, input) && input.overlaps(location))
                {
                    observations.push(occurrence.clone());
                    if step.may_escape {
                        escaped.push(*position);
                    }
                }
                let mut outputs = Vec::new();
                match name {
                    ResidenceName::Storage(_) => {
                        if at_entry || *position != birth || location != residence {
                            outputs.push(name.clone());
                        }
                        for (source, destination) in &step.derivations {
                            if source.overlaps(location)
                                && rules.captures.contains(&(
                                    *position,
                                    source.clone(),
                                    destination.clone(),
                                ))
                            {
                                outputs.push(ResidenceName::AddressContents(destination.clone()));
                            }
                        }
                    }
                    ResidenceName::AddressContents(_) => {
                        if !step
                            .definitions
                            .iter()
                            .any(|definition| definition.contains(location))
                        {
                            outputs.push(name.clone());
                        }
                        for (source, destination) in &step.derivations {
                            if !rules.captures.contains(&(
                                *position,
                                source.clone(),
                                destination.clone(),
                            )) && let Some(target) = location.transported(source, destination)
                            {
                                outputs.push(ResidenceName::AddressContents(target));
                            }
                        }
                    }
                }
                for edge in &step.successors {
                    for output in &outputs {
                        let location = output.location();
                        let mut destinations = Vec::new();
                        if !edge
                            .parameters
                            .iter()
                            .chain(&edge.definitions)
                            .any(|parameter| parameter.contains(location))
                        {
                            destinations.push(output.clone());
                        }
                        for binding in &edge.bindings {
                            let Some((source, destination)) = binding.locations() else {
                                continue;
                            };
                            match output {
                                ResidenceName::Storage(_)
                                    if binding.access != terminal_psi::StructuralAccess::Owned
                                        && source.overlaps(location) =>
                                {
                                    destinations.push(ResidenceName::Storage(destination.clone()));
                                }
                                ResidenceName::AddressContents(_) => {
                                    if let Some(target) = location.transported(source, destination)
                                    {
                                        destinations.push(ResidenceName::AddressContents(target));
                                    }
                                }
                                _ => {}
                            }
                        }
                        for destination in destinations {
                            let next = (edge.target, destination);
                            predecessors
                                .entry(next.clone())
                                .or_default()
                                .insert(occurrence.clone());
                            pending.push(next);
                        }
                    }
                }
            }
            let mut observed = BTreeSet::new();
            let mut live = BTreeSet::new();
            while let Some(occurrence) = observations.pop() {
                if observed.insert(occurrence.clone()) {
                    live.insert(occurrence.0);
                    observations
                        .extend(predecessors.get(&occurrence).into_iter().flatten().cloned());
                }
            }
            let mut escaped_positions = BTreeSet::new();
            while let Some(position) = escaped.pop() {
                if escaped_positions.insert(position) {
                    live.insert(position);
                    if let Some(step) = steps.get(&position) {
                        escaped.extend(step.successors.iter().map(|edge| edge.target));
                    }
                }
            }
            live
        }
    }
    pub use locations::{
        canonical_descriptor_location, descriptor_location, descriptor_location_type,
        descriptor_path, fixed_runtime_index_alternatives,
    };
    pub use residences::{DescriptorResidenceRules, descriptor_residence_live_positions};

    pub type DescriptorPosition = (BlockId, u32);

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DescriptorLocation {
        pub root: PlaceId,
        pub path: Vec<DescriptorPathSegment>,
    }

    pub use terminal_psi::ResolvedStructuralPathSegment as DescriptorPathSegment;

    impl DescriptorLocation {
        /// Retain a captured address without translating its storage steps.
        /// A runtime selector needs its capture point; an extent is metadata.
        pub fn captured(root: PlaceId, path: &[DescriptorPathSegment]) -> Option<Self> {
            if path.iter().any(|step| {
                matches!(
                    step,
                    DescriptorPathSegment::RuntimeIndex { .. } | DescriptorPathSegment::Extent
                )
            }) {
                return None;
            }
            Some(Self {
                root,
                path: path.to_vec(),
            })
        }

        pub fn whole(root: PlaceId) -> Self {
            Self {
                root,
                path: Vec::new(),
            }
        }
        pub fn overlaps(&self, other: &Self) -> bool {
            self.root == other.root
                && crate::storage::place_paths::may_overlap(&self.path, &other.path)
        }
        pub fn contains(&self, other: &Self) -> bool {
            self.root == other.root && other.path.starts_with(&self.path)
        }
        pub fn transported(&self, source: &Self, destination: &Self) -> Option<Self> {
            if !source.contains(self) {
                return None;
            }
            let mut path = destination.path.clone();
            path.extend_from_slice(&self.path[source.path.len()..]);
            Some(Self {
                root: destination.root,
                path,
            })
        }
    }

    /// The operand order also identifies captured constructor elements. A
    /// memory operation belongs to its original typed source, not to aliases
    /// later added solely to keep a loan alive. Its observation is absent when
    /// a replacement does not read the old value. An unavailable source cannot
    /// establish permission even when its whole root is observed for liveness.
    pub type DescriptorFlowInput = (
        Option<DescriptorLocation>,
        Option<(access_plans::AccessOperation, DescriptorFlowSource)>,
    );

    #[derive(Clone, Default)]
    pub struct DescriptorFlowStep {
        pub definitions: Vec<DescriptorLocation>,
        /// The subset of `definitions` that an owned move consumes rather
        /// than overwrites. Loan liveness treats both alike. Descriptor
        /// replay keeps a consumed parameter's entry value readable, so the
        /// machine's summary can still report the caller's storage behind a
        /// reference the parameter held.
        pub consumed: Vec<DescriptorLocation>,
        pub derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
        pub authority_derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
        pub inputs: Vec<DescriptorFlowInput>,
        pub may_escape: bool,
        pub successors: Vec<DescriptorFlowEdge>,
    }

    impl DescriptorFlowStep {
        pub fn transport_authority(
            &self,
            before: &BTreeSet<DescriptorLocation>,
            may: bool,
        ) -> BTreeSet<DescriptorLocation> {
            transport_authority(
                before,
                &self.definitions,
                self.authority_derivations
                    .iter()
                    .map(|(source, destination)| (source, destination)),
                false,
                may,
            )
        }

        pub fn observed_inputs(&self) -> impl Iterator<Item = &DescriptorLocation> {
            self.inputs.iter().filter_map(|(input, _)| input.as_ref())
        }

        /// Dropping a value observation does not erase an actual memory
        /// operation. This matters when cleanup discards an old descriptor.
        pub fn forget_observation(&mut self, index: usize) {
            if self.inputs[index].1.is_some() {
                self.inputs[index].0 = None;
            } else {
                self.inputs.remove(index);
            }
        }

        /// A value-transfer rewrite can refine the loan observations without
        /// replacing the instruction's actual memory operations. Constructor
        /// operands are never rewritten here: their order and duplicates are
        /// the captured element roster.
        pub fn replace_value_inputs(
            &mut self,
            inputs: impl IntoIterator<Item = DescriptorLocation>,
        ) {
            self.inputs.retain(|(_, access)| access.is_some());
            for (observed, _) in &mut self.inputs {
                *observed = None;
            }
            self.inputs
                .extend(inputs.into_iter().map(|input| (Some(input), None)));
        }
    }

    #[derive(Clone)]
    pub struct DescriptorFlowEdge {
        /// Sources consumed on this selected edge. Destination parameters
        /// remain a separate roster, so swaps capture every old value before
        /// any source or destination is retired.
        pub definitions: Vec<DescriptorLocation>,
        /// Present for a genuine Terminal successor, absent on an internal
        /// operation sequence edge. Cleanup calls select this exact edge.
        pub edge: Option<semantic_vocabulary::EdgeId>,
        /// Prepared ordinary call effects, in this edge's exact action order.
        /// Their inputs are observed at the genuine source terminator cut.
        pub effects: Vec<(
            crate::descriptor_results::DescriptorInvocationOwner,
            DescriptorFlowStep,
        )>,
        pub target: DescriptorPosition,
        pub parameters: Vec<DescriptorLocation>,
        pub bindings: Vec<DescriptorFlowBinding>,
    }

    impl DescriptorFlowEdge {
        /// A borrowed header keeps the permission of the actual it names.
        /// When the actual crosses a reference, its immediate descriptor is
        /// the authorizing name; an inline projection keeps its root's loan.
        pub fn borrowed_authority(&self, name: &DescriptorLocation) -> Vec<DescriptorLocation> {
            self.bindings
                .iter()
                .filter(|binding| {
                    binding.access != terminal_psi::StructuralAccess::Owned
                        && binding.source_access == binding.access
                })
                .filter_map(|binding| {
                    let source = binding.source.exact_location()?;
                    let parent = source
                        .path
                        .iter()
                        .rposition(|step| *step == DescriptorPathSegment::Referent);
                    let prefix = parent.map_or(&[][..], |position| &source.path[..position]);
                    (name.root == source.root && name.path == prefix)
                        .then(|| binding.parameter.clone())
                })
                .collect()
        }

        pub fn derivations(
            &self,
        ) -> impl Clone + Iterator<Item = (&DescriptorLocation, &DescriptorLocation)> {
            self.bindings
                .iter()
                .filter_map(DescriptorFlowBinding::locations)
        }

        fn authority_derivations(
            &self,
        ) -> impl Clone + Iterator<Item = (&DescriptorLocation, &DescriptorLocation)> {
            self.bindings
                .iter()
                .filter(|binding| {
                    binding.access == terminal_psi::StructuralAccess::Owned
                        || binding.source_access == binding.access
                })
                .filter_map(DescriptorFlowBinding::locations)
        }

        fn forward_effects(
            &self,
            source: &DescriptorLocation,
            authority: bool,
        ) -> Vec<DescriptorLocation> {
            let mut names = vec![source.clone()];
            for (_, effect) in &self.effects {
                let derivations = if authority {
                    &effect.authority_derivations
                } else {
                    &effect.derivations
                };
                names = names
                    .into_iter()
                    .flat_map(|name| {
                        let retained = (!effect
                            .definitions
                            .iter()
                            .any(|definition| definition.contains(&name)))
                        .then(|| name.clone());
                        retained.into_iter().chain(
                            derivations
                                .iter()
                                .filter_map(move |(from, to)| name.transported(from, to)),
                        )
                    })
                    .collect();
                names.sort();
                names.dedup();
            }
            names
                .into_iter()
                .filter(|name| {
                    !self
                        .parameters
                        .iter()
                        .chain(&self.definitions)
                        .any(|definition| definition.contains(name))
                })
                .collect()
        }

        fn forward_authority(&self, source: &DescriptorLocation) -> Vec<DescriptorLocation> {
            let mut result = self.forward_effects(source, true);
            result.extend(
                self.authority_derivations()
                    .filter_map(|(actual, parameter)| source.transported(actual, parameter)),
            );
            result.extend(self.borrowed_authority(source));
            result
        }

        pub fn forward(&self, source: &DescriptorLocation) -> Vec<DescriptorLocation> {
            let mut result = self.forward_effects(source, false);
            result.extend(
                self.derivations()
                    .filter_map(|(actual, parameter)| source.transported(actual, parameter)),
            );
            result
        }
    }

    pub fn descriptor_location_live_positions(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
    ) -> BTreeSet<DescriptorPosition> {
        location_live_positions(steps, birth, descriptor, false, false, None, |_, _| false).live
    }

    struct DescriptorDemand {
        live: BTreeSet<DescriptorPosition>,
        names: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
    }

    fn location_live_positions(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
        at_entry: bool,
        borrowed_authority: bool,
        successor: Option<DescriptorPosition>,
        needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
    ) -> DescriptorDemand {
        type Occurrence = (DescriptorPosition, DescriptorLocation);
        let mut pending = Vec::new();
        if let Some(target) = successor {
            pending.push((target, descriptor.clone()));
        } else if at_entry {
            pending.push((birth, descriptor.clone()));
        } else if let Some(step) = steps.get(&birth) {
            for edge in &step.successors {
                pending.extend(
                    edge.forward(descriptor)
                        .into_iter()
                        .map(|name| (edge.target, name)),
                );
            }
        }
        let mut visited = BTreeSet::new();
        let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
        let mut observations = Vec::new();
        let mut escaped = Vec::new();
        while let Some(occurrence) = pending.pop() {
            let (position, name) = &occurrence;
            if !visited.insert(occurrence.clone()) {
                continue;
            }
            let Some(step) = steps.get(position) else {
                continue;
            };
            if step
                .observed_inputs()
                .any(|input| input.contains(name) || name.contains(input))
            {
                observations.push(occurrence.clone());
                if step.may_escape {
                    escaped.push(*position);
                }
            } else if needs_contents(*position, name) {
                observations.push(occurrence.clone());
            }
            let mut outputs = Vec::new();
            if !step
                .definitions
                .iter()
                .any(|definition| definition.contains(name))
            {
                outputs.push(name.clone());
            }
            outputs.extend(
                step.derivations
                    .iter()
                    .filter_map(|(source, result)| name.transported(source, result)),
            );
            for edge in &step.successors {
                for output in &outputs {
                    let destinations = if borrowed_authority {
                        edge.forward_authority(output)
                    } else {
                        edge.forward(output)
                    };
                    for destination in destinations {
                        let next = (edge.target, destination);
                        predecessors
                            .entry(next.clone())
                            .or_default()
                            .insert(occurrence.clone());
                        pending.push(next);
                    }
                }
            }
        }
        let mut observed = BTreeSet::new();
        let mut live = BTreeSet::new();
        while let Some(occurrence) = observations.pop() {
            let position = occurrence.0;
            if !observed.insert(occurrence.clone()) {
                continue;
            }
            live.insert(position);
            observations.extend(predecessors.get(&occurrence).into_iter().flatten().cloned());
        }
        let mut escaped_positions = BTreeSet::new();
        while let Some(position) = escaped.pop() {
            if !escaped_positions.insert(position) {
                continue;
            }
            live.insert(position);
            if let Some(step) = steps.get(&position) {
                escaped.extend(step.successors.iter().map(|edge| edge.target));
            }
        }
        let mut names = BTreeMap::<DescriptorPosition, BTreeSet<DescriptorLocation>>::new();
        for (position, name) in observed.into_iter().chain(
            visited
                .into_iter()
                .filter(|(position, _)| escaped_positions.contains(position)),
        ) {
            names.entry(position).or_default().insert(name);
        }
        DescriptorDemand { live, names }
    }

    pub struct DescriptorLifetime {
        pub live: BTreeSet<DescriptorPosition>,
        /// Future demand of each actual carrier spelling. A dead copy can
        /// be released without rewalking the graph from every possible cut.
        pub demanded: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
        pub authorized: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
        /// All carrier names that can still own this resource on any arrival.
        /// Unlike `authorized`, absence of a common name at a join does not
        /// erase a surviving occurrence on one of its predecessors.
        pub surviving: BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>>,
    }

    pub fn descriptor_entry_lifetime(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        entry: DescriptorPosition,
        descriptor: &DescriptorLocation,
    ) -> DescriptorLifetime {
        descriptor_entry_lifetime_with_contents(steps, entry, descriptor, |_, _| false)
    }

    pub(crate) fn descriptor_entry_lifetime_with_contents(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        entry: DescriptorPosition,
        descriptor: &DescriptorLocation,
        needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
    ) -> DescriptorLifetime {
        let demand =
            location_live_positions(steps, entry, descriptor, true, true, None, needs_contents);
        DescriptorLifetime {
            live: demand.live,
            demanded: demand.names,
            authorized: transported_locations(steps, entry, descriptor, true, false, false, None),
            surviving: transported_locations(steps, entry, descriptor, true, false, true, None),
        }
    }

    pub(crate) fn descriptor_continuation_locations(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        at: DescriptorPosition,
        descriptor: &DescriptorLocation,
    ) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
        transported_locations(steps, at, descriptor, true, false, false, None)
    }

    pub fn descriptor_location_lifetime(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
    ) -> DescriptorLifetime {
        descriptor_location_lifetime_with_contents(steps, birth, descriptor, |_, _| false)
    }

    pub(crate) fn descriptor_location_lifetime_with_contents(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
        needs_contents: impl Fn(DescriptorPosition, &DescriptorLocation) -> bool,
    ) -> DescriptorLifetime {
        let demand =
            location_live_positions(steps, birth, descriptor, false, true, None, needs_contents);
        let authorized = authorized_locations(steps, birth, descriptor, &demand.live);
        let surviving = if authorized.is_empty() {
            BTreeMap::new()
        } else {
            transported_locations(steps, birth, descriptor, false, false, true, None)
        };
        DescriptorLifetime {
            live: demand.live,
            demanded: demand.names,
            authorized,
            surviving,
        }
    }

    /// A new edge borrow starts under the selected successor name. Its
    /// lifetime follows the same copies and captures as an explicit reference.
    pub fn descriptor_successor_lifetime(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        target: DescriptorPosition,
        descriptor: &DescriptorLocation,
    ) -> Option<DescriptorLifetime> {
        let mut matching = steps
            .get(&birth)?
            .successors
            .iter()
            .filter(|edge| edge.target == target);
        let selected = matching.next()?;
        if matching.next().is_some()
            || !selected.bindings.iter().any(|binding| {
                binding.parameter == *descriptor
                    && binding.access != terminal_psi::StructuralAccess::Owned
            })
        {
            return None;
        }
        let demand = location_live_positions(
            steps,
            birth,
            descriptor,
            false,
            true,
            Some(target),
            |_, _| false,
        );
        let authorized =
            transported_locations(steps, birth, descriptor, false, false, false, Some(target));
        let surviving =
            transported_locations(steps, birth, descriptor, false, false, true, Some(target));
        Some(DescriptorLifetime {
            live: demand.live,
            demanded: demand.names,
            authorized,
            surviving,
        })
    }

    fn authorized_locations(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
        live: &BTreeSet<DescriptorPosition>,
    ) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
        let Some(formation) = steps.get(&birth) else {
            return BTreeMap::new();
        };
        if !formation
            .definitions
            .iter()
            .any(|definition| definition.contains(descriptor))
        {
            return BTreeMap::new();
        }
        if live.contains(&birth) {
            return BTreeMap::new();
        }
        transported_locations(steps, birth, descriptor, false, false, false, None)
    }

    pub(crate) fn value_locations(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        establishment: DescriptorPosition,
        value: &DescriptorLocation,
        at_entry: bool,
    ) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
        if !at_entry {
            let live = descriptor_location_live_positions(steps, establishment, value);
            if live.contains(&establishment)
                || !steps.get(&establishment).is_some_and(|step| {
                    step.definitions
                        .iter()
                        .any(|definition| definition.contains(value))
                })
            {
                return BTreeMap::new();
            }
        }
        transported_locations(steps, establishment, value, at_entry, true, false, None)
    }

    fn transported_locations(
        steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        birth: DescriptorPosition,
        descriptor: &DescriptorLocation,
        at_entry: bool,
        invalidate_containers: bool,
        may: bool,
        arrival: Option<DescriptorPosition>,
    ) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
        let mut predecessors =
            BTreeMap::<DescriptorPosition, Vec<(DescriptorPosition, &DescriptorFlowEdge)>>::new();
        for (position, step) in steps {
            for edge in &step.successors {
                predecessors
                    .entry(edge.target)
                    .or_default()
                    .push((*position, edge));
            }
        }
        let mut established = steps
            .keys()
            .map(|position| (*position, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        if at_entry {
            established.insert(birth, BTreeSet::from([descriptor.clone()]));
        }
        loop {
            let mut changed = false;
            for position in steps.keys().copied() {
                if position == birth {
                    continue;
                }
                let mut incoming =
                    predecessors
                        .get(&position)
                        .into_iter()
                        .flatten()
                        .map(|(predecessor, edge)| {
                            let step = &steps[predecessor];
                            let before = &established[predecessor];
                            let mut after = transport_authority(
                                before,
                                &step.definitions,
                                step.authority_derivations
                                    .iter()
                                    .map(|(source, destination)| (source, destination)),
                                invalidate_containers,
                                may,
                            );
                            if !at_entry && arrival.is_none() && *predecessor == birth {
                                after.insert(descriptor.clone());
                            }
                            let definitions = edge
                                .parameters
                                .iter()
                                .chain(&edge.definitions)
                                .cloned()
                                .collect::<Vec<_>>();
                            let mut effected = after.clone();
                            for (_, effect) in &edge.effects {
                                let derivations = if invalidate_containers {
                                    &effect.derivations
                                } else {
                                    &effect.authority_derivations
                                };
                                effected = transport_authority(
                                    &effected,
                                    &effect.definitions,
                                    derivations
                                        .iter()
                                        .map(|(source, destination)| (source, destination)),
                                    invalidate_containers,
                                    may,
                                );
                            }
                            let mut transported = transport_authority(
                                &effected,
                                &definitions,
                                std::iter::empty(),
                                invalidate_containers,
                                may,
                            );
                            let captured = if invalidate_containers {
                                transport_authority(
                                    &after,
                                    &definitions,
                                    edge.derivations(),
                                    invalidate_containers,
                                    may,
                                )
                            } else {
                                transport_authority(
                                    &after,
                                    &definitions,
                                    edge.authority_derivations(),
                                    invalidate_containers,
                                    may,
                                )
                            };
                            transported.extend(captured.into_iter().filter(|name| {
                                edge.bindings
                                    .iter()
                                    .any(|binding| binding.parameter.contains(name))
                            }));
                            if !invalidate_containers {
                                for name in &after {
                                    transported.extend(edge.borrowed_authority(name));
                                }
                            }
                            if arrival == Some(position) && *predecessor == birth {
                                transported.insert(descriptor.clone());
                            }
                            transported
                        });
                let mut current = incoming.next().unwrap_or_default();
                for other in incoming {
                    if may {
                        current.extend(other);
                    } else {
                        current.retain(|location| other.contains(location));
                    }
                }
                if established.get(&position) != Some(&current) {
                    established.insert(position, current);
                    changed = true;
                }
            }
            if !changed {
                return established;
            }
        }
    }

    fn transport_authority<'locations>(
        before: &BTreeSet<DescriptorLocation>,
        definitions: &[DescriptorLocation],
        derivations: impl Clone
        + Iterator<
            Item = (
                &'locations DescriptorLocation,
                &'locations DescriptorLocation,
            ),
        >,
        invalidate_containers: bool,
        may: bool,
    ) -> BTreeSet<DescriptorLocation> {
        let mut after = before
            .iter()
            .filter(|location| {
                !definitions.iter().any(|definition| {
                    definition.contains(location)
                        || (invalidate_containers && location.contains(definition))
                })
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let candidates = before
            .iter()
            .flat_map(|location| {
                derivations
                    .clone()
                    .filter_map(move |(source, destination)| {
                        location.transported(source, destination)
                    })
            })
            .collect::<BTreeSet<_>>();
        for candidate in candidates {
            let sources = derivations
                .clone()
                .filter_map(|(source, destination)| candidate.transported(destination, source))
                .collect::<Vec<_>>();
            if !sources.is_empty()
                && if may {
                    sources.iter().any(|source| before.contains(source))
                } else {
                    sources.iter().all(|source| before.contains(source))
                }
            {
                after.insert(candidate);
            }
        }
        after
    }

    #[cfg(test)]
    mod resource_tests {
        use super::*;
        use semantic_vocabulary::{BlockId, PlaceId};

        fn position(ordinal: u32) -> DescriptorPosition {
            (BlockId::new(1).unwrap(), ordinal)
        }

        fn root(number: u64) -> DescriptorLocation {
            DescriptorLocation::whole(PlaceId::new(number).unwrap())
        }

        fn edge(target: u32) -> DescriptorFlowEdge {
            DescriptorFlowEdge {
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                target: position(target),
                parameters: Vec::new(),
                bindings: Vec::new(),
            }
        }

        #[test]
        fn cleanup_effects_compose_in_order_while_edge_bindings_capture_before_cleanup() {
            let mut selected = edge(1);
            selected.edge = Some(semantic_vocabulary::EdgeId::new(1).unwrap());
            selected.definitions = vec![root(1), root(2)];
            selected.parameters = vec![root(4)];
            selected.bindings.push(
                DescriptorFlowBinding::new(
                    DescriptorFlowSource::Location(root(2)),
                    root(4),
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::Owned,
                )
                .unwrap(),
            );
            for (ordinal, source, destination) in [(0, 1, 2), (2, 2, 3)] {
                selected.effects.push((
                    terminal_psi::InvocationOwner::Cleanup {
                        edge: selected.edge.unwrap(),
                        action_ordinal: ordinal,
                    },
                    DescriptorFlowStep {
                        definitions: vec![root(destination)],
                        derivations: vec![(root(source), root(destination))],
                        authority_derivations: vec![(root(source), root(destination))],
                        inputs: vec![(Some(root(source)), None)],
                        ..Default::default()
                    },
                ));
            }
            assert_eq!(selected.forward(&root(1)), [root(3)]);
            assert_eq!(selected.forward_authority(&root(1)), [root(3)]);
            assert_eq!(selected.forward(&root(2)), [root(4)]);
            assert_eq!(selected.forward_authority(&root(2)), [root(4)]);
            selected.effects.reverse();
            assert!(selected.forward(&root(1)).is_empty());
        }

        #[test]
        fn a_carrier_surviving_one_arrival_is_not_a_closed_resource() {
            let mut steps = BTreeMap::new();
            steps.insert(
                position(0),
                DescriptorFlowStep {
                    successors: vec![edge(1), edge(2)],
                    ..Default::default()
                },
            );
            steps.insert(
                position(1),
                DescriptorFlowStep {
                    definitions: vec![root(1)],
                    successors: vec![edge(3)],
                    ..Default::default()
                },
            );
            steps.insert(
                position(2),
                DescriptorFlowStep {
                    successors: vec![edge(3)],
                    ..Default::default()
                },
            );
            steps.insert(position(3), DescriptorFlowStep::default());
            let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
            assert!(lifetime.authorized[&position(3)].is_empty());
            assert_eq!(lifetime.surviving[&position(3)], BTreeSet::from([root(1)]));
        }

        #[test]
        fn a_selected_edge_captures_swapped_carriers_before_consuming_old_names() {
            let mut transfer = edge(1);
            transfer.parameters = vec![root(1), root(2)];
            transfer.definitions = vec![root(1), root(2)];
            transfer.bindings = vec![
                DescriptorFlowBinding::new(
                    DescriptorFlowSource::Location(root(1)),
                    root(2),
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::Owned,
                )
                .unwrap(),
                DescriptorFlowBinding::new(
                    DescriptorFlowSource::Location(root(2)),
                    root(1),
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::Owned,
                )
                .unwrap(),
            ];
            let steps = BTreeMap::from([
                (
                    position(0),
                    DescriptorFlowStep {
                        successors: vec![transfer],
                        ..Default::default()
                    },
                ),
                (position(1), DescriptorFlowStep::default()),
            ]);
            let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
            assert_eq!(lifetime.authorized[&position(1)], BTreeSet::from([root(2)]));
            assert_eq!(lifetime.surviving[&position(1)], BTreeSet::from([root(2)]));
        }

        #[test]
        fn replacing_a_descriptor_keeps_its_write_without_observing_old_contents() {
            let mut replacement = DescriptorFlowStep {
                inputs: vec![(
                    Some(root(1)),
                    Some((
                        access_plans::AccessOperation::Write,
                        DescriptorFlowSource::Unavailable(root(1).root),
                    )),
                )],
                definitions: vec![root(1)],
                successors: vec![edge(1)],
                ..Default::default()
            };
            replacement.replace_value_inputs([root(2)]);
            assert_eq!(
                replacement.observed_inputs().collect::<Vec<_>>(),
                vec![&root(2)]
            );
            assert!(matches!(&replacement.inputs[0],
                (None, Some((access_plans::AccessOperation::Write, DescriptorFlowSource::Unavailable(old))))
                    if *old == root(1).root));
            let steps = BTreeMap::from([
                (position(0), replacement),
                (position(1), DescriptorFlowStep::default()),
            ]);
            let overwritten = descriptor_entry_lifetime(&steps, position(0), &root(1));
            let incoming = descriptor_entry_lifetime(&steps, position(0), &root(2));
            assert!(
                !overwritten.live.contains(&position(0)),
                "replacing the descriptor is not an observation of its old referent"
            );
            assert!(
                incoming.live.contains(&position(0)),
                "the incoming descriptor remains live through the replacement"
            );
        }

        #[test]
        fn an_incoming_resource_observes_the_first_operation_before_transport() {
            let steps = BTreeMap::from([
                (
                    position(0),
                    DescriptorFlowStep {
                        inputs: vec![(Some(root(1)), None)],
                        successors: vec![edge(1)],
                        ..Default::default()
                    },
                ),
                (position(1), DescriptorFlowStep::default()),
            ]);
            let lifetime = descriptor_entry_lifetime(&steps, position(0), &root(1));
            assert!(lifetime.live.contains(&position(0)));
            assert!(!lifetime.live.contains(&position(1)));
        }
    }
}
pub(crate) mod membership_presentations {

    use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        StructuralDomainDeclaration, StructuralPathQualification, StructuralTypeDeclaration,
    };

    pub fn present_structural_memberships<'types, 'domains>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domains StructuralDomainDeclaration> + Clone,
        destination: StructuralTypeId,
        whole: &mut Vec<StructuralDomainId>,
        projected: &mut Vec<StructuralPathQualification>,
    ) -> Result<(), &'static str> {
        present_structural_membership_contract(
            types,
            domains,
            destination,
            whole,
            projected,
            &mut Vec::new(),
        )
    }

    pub fn present_structural_membership_contract<'types, 'domains>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domains StructuralDomainDeclaration> + Clone,
        destination: StructuralTypeId,
        whole: &mut Vec<StructuralDomainId>,
        projected: &mut Vec<StructuralPathQualification>,
        additional_input: &mut Vec<StructuralPathQualification>,
    ) -> Result<(), &'static str> {
        let present = |domain: StructuralDomainId, carrier: StructuralTypeId| {
            let source = domains
                .clone()
                .find(|row| row.id == domain)
                .ok_or("membership presentation has no source domain")?;
            if source.carrier == carrier {
                return Ok(domain);
            }
            if crate::borrowed_referent_presentation(types.clone(), source.carrier, carrier)
                != Some(crate::BorrowedReferentPresentation::Unchanged)
            {
                return Err("membership presentation changes its observed carrier");
            }
            let mut matches = domains.clone().filter(|row| {
                row.carrier == carrier
                    && row.semantic_domain == source.semantic_domain
                    && same_presented_theory(domains.clone(), source, row)
            });
            let destination = matches
                .next()
                .ok_or("membership presentation has no identical destination theory")?;
            if matches.next().is_some() {
                return Err("membership presentation has ambiguous destination theories");
            }
            Ok(destination.id)
        };
        for domain in whole.iter_mut() {
            *domain = present(*domain, destination)?;
        }
        for row in projected.iter_mut().chain(additional_input.iter_mut()) {
            let carrier = crate::qualification_path_carrier(types.clone(), destination, &row.path)
                .ok_or("membership presentation has no destination path carrier")?;
            row.domain = present(row.domain, carrier)?;
        }
        whole.sort();
        whole.dedup();
        projected.sort();
        projected.dedup();
        additional_input.sort();
        additional_input.dedup();
        Ok(())
    }

    fn same_presented_theory<'domains>(
        domains: impl Iterator<Item = &'domains StructuralDomainDeclaration> + Clone,
        source: &'domains StructuralDomainDeclaration,
        destination: &'domains StructuralDomainDeclaration,
    ) -> bool {
        let resolve = |semantic_domain, carrier| {
            let mut matches = domains
                .clone()
                .filter(|row| row.semantic_domain == semantic_domain && row.carrier == carrier);
            let row = matches.next()?;
            matches.next().is_none().then_some(row)
        };
        if resolve(source.semantic_domain, source.carrier).map(|row| row.id) != Some(source.id)
            || resolve(destination.semantic_domain, destination.carrier).map(|row| row.id)
                != Some(destination.id)
        {
            return false;
        }
        let mut pending = vec![(source, destination, false)];
        let mut active = Vec::new();
        let mut visited = Vec::new();
        while let Some((source, destination, completed)) = pending.pop() {
            if completed {
                if active.pop() != Some(source.id) {
                    return false;
                }
                visited.push(source.id);
                continue;
            }
            if active.contains(&source.id) {
                return false;
            }
            if visited.contains(&source.id) {
                continue;
            }
            if source.semantic_domain != destination.semantic_domain
                || source.complete_predicate_theory != destination.complete_predicate_theory
                || source.predicate != destination.predicate
                || source.content_projection != destination.content_projection
                || source.byte_predicate != destination.byte_predicate
                || source.establishment_routes != destination.establishment_routes
                || source.access_policy != destination.access_policy
            {
                return false;
            }
            let (Some(source_required), Some(destination_required)) = (
                &source.required_memberships,
                &destination.required_memberships,
            ) else {
                return false;
            };
            if source_required != destination_required
                || !source_required.windows(2).all(|pair| pair[0] < pair[1])
            {
                return false;
            }
            active.push(source.id);
            pending.push((source, destination, true));
            for required in source_required.iter().rev() {
                let Some(source_parent) = resolve(*required, source.carrier) else {
                    return false;
                };
                let Some(destination_parent) = resolve(*required, destination.carrier) else {
                    return false;
                };
                pending.push((source_parent, destination_parent, false));
            }
        }
        true
    }
}
pub(crate) mod primitive_store_memberships {
    pub(crate) mod scalar {
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::{
            IntegerMathTerm, Proposition, ScalarRefinement, ScalarTerm, ScalarType,
            StructuralTypeId,
        };
        use terminal_psi::{
            StructuralDomainDeclaration, StructuralDomainPredicate, StructuralFieldType,
            StructuralTypeDeclaration, StructuralTypeShape,
        };

        pub(crate) fn refinement<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            domain: &StructuralDomainDeclaration,
        ) -> Option<ScalarRefinement> {
            if domain.content_projection.is_some() {
                return None;
            }
            let carrier = primitive_carrier(types.clone(), domain.carrier)?;
            let predicate = domain.predicate.as_ref()?;
            if !crate::domain_predicate_is_closed(types, domain.carrier, predicate) {
                return None;
            }
            bind_subject(carrier, predicate)
        }

        pub(crate) fn primitive_carrier<'types>(
            mut types: impl Iterator<Item = &'types StructuralTypeDeclaration>,
            carrier: StructuralTypeId,
        ) -> Option<ScalarType> {
            let declaration = exactly_one(types.by_ref(), |row| row.id == carrier)?;
            match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(carrier) => Some(carrier),
                _ => None,
            }
        }

        fn bind_subject(
            carrier: ScalarType,
            predicate: &StructuralDomainPredicate,
        ) -> Option<ScalarRefinement> {
            enum Pending<'a> {
                Proposition(&'a mut Proposition),
                Scalar(&'a mut ScalarTerm),
                Math(&'a mut IntegerMathTerm),
            }
            let mut proposition = predicate.proposition.clone();
            let mut pending = vec![Pending::Proposition(&mut proposition)];
            let mut remaining = 4096usize;
            while let Some(node) = pending.pop() {
                remaining = remaining.checked_sub(1)?;
                match node {
                    Pending::Proposition(proposition) => match proposition {
                        Proposition::Truth | Proposition::Falsehood => {}
                        semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                            pending.push(Pending::Scalar(right));
                            pending.push(Pending::Scalar(left));
                        }
                        semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                            pending.push(Pending::Math(right));
                            pending.push(Pending::Math(left));
                        }
                        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                            pending.extend(children.iter_mut().map(Pending::Proposition));
                        }
                        Proposition::Implication {
                            premise,
                            conclusion,
                        } => {
                            pending.push(Pending::Proposition(conclusion));
                            pending.push(Pending::Proposition(premise));
                        }
                        _ => return None,
                    },
                    Pending::Math(term) => match term {
                        IntegerMathTerm::Embed(term) => pending.push(Pending::Scalar(term)),
                        IntegerMathTerm::MathValue { .. } => return None,
                        term => pending.extend(term.operands_mut().into_iter().map(Pending::Math)),
                    },
                    Pending::Scalar(term) => {
                        if term.field_version()
                            == Some(semantic_vocabulary::ContentPlaceVersion::Entry)
                        {
                            return None;
                        }
                        let observed = match term {
                            ScalarTerm::BooleanField {
                                version: semantic_vocabulary::ContentPlaceVersion::Current,
                                root,
                                path,
                            } => Some((*root, path.is_empty(), ScalarType::Boolean)),
                            ScalarTerm::IntegerField {
                                version: semantic_vocabulary::ContentPlaceVersion::Current,
                                root,
                                path,
                                scalar_type,
                            } => Some((*root, path.is_empty(), ScalarType::Integer(*scalar_type))),
                            ScalarTerm::IeeeFloatField {
                                version: semantic_vocabulary::ContentPlaceVersion::Current,
                                root,
                                path,
                                format,
                            } => Some((*root, path.is_empty(), ScalarType::IeeeFloat(*format))),
                            ScalarTerm::StructuralRead {
                                root,
                                path,
                                scalar_type,
                            } => Some((*root, path.is_empty(), *scalar_type)),
                            _ => None,
                        };
                        if let Some((root, whole, actual)) = observed {
                            if root != predicate.subject || !whole || actual != carrier {
                                return None;
                            }
                            *term = ScalarTerm::value(ScalarRefinement::SUBJECT, carrier);
                        } else {
                            pending.extend(term.operands_mut().into_iter().map(Pending::Scalar));
                        }
                    }
                }
            }
            ScalarRefinement::new(carrier, proposition).ok()
        }

        pub(crate) fn receive(
            receiving: Option<StructuralFieldType>,
            refinement: ScalarRefinement,
        ) -> Option<Option<StructuralFieldType>> {
            let required = StructuralFieldType::RefinedScalar(refinement.clone());
            if receiving
                .as_ref()
                .is_some_and(|current| crate::scalar_field_entails(current, &required))
            {
                return Some(receiving);
            }
            if refinement.predicate() == &Proposition::Truth {
                return Some(receiving);
            }
            let mut clauses = vec![refinement.predicate().clone()];
            if let Some(current) = receiving {
                if current.scalar_type() != Some(refinement.carrier()) {
                    return None;
                }
                clauses.push(crate::scalar_field_requirement(
                    &current,
                    ScalarRefinement::SUBJECT,
                )?);
            }
            clauses.sort();
            clauses.dedup();
            let predicate = if clauses.len() == 1 {
                clauses.pop()?
            } else {
                Proposition::Conjunction(clauses)
            };
            Some(Some(StructuralFieldType::RefinedScalar(
                ScalarRefinement::new(refinement.carrier(), predicate).ok()?,
            )))
        }

        pub(crate) fn required_domains<'domain>(
            domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
            root: &'domain StructuralDomainDeclaration,
        ) -> Option<Vec<&'domain StructuralDomainDeclaration>> {
            let mut pending = vec![(root.semantic_domain, false)];
            let mut active = Vec::new();
            let mut visited = Vec::new();
            let mut result = Vec::new();
            while let Some((semantic_domain, completed)) = pending.pop() {
                if completed {
                    if active.pop() != Some(semantic_domain) {
                        return None;
                    }
                    visited.push(semantic_domain);
                    continue;
                }
                if active.contains(&semantic_domain) {
                    return None;
                }
                if visited.contains(&semantic_domain) {
                    continue;
                }
                let domain = exactly_one(domains.clone(), |domain| {
                    domain.carrier == root.carrier && domain.semantic_domain == semantic_domain
                })?;
                let required = domain.required_memberships.as_ref()?;
                if !required.windows(2).all(|pair| pair[0] < pair[1]) {
                    return None;
                }
                result.push(domain);
                active.push(semantic_domain);
                pending.push((semantic_domain, true));
                pending.extend(required.iter().rev().map(|domain| (*domain, false)));
            }
            Some(result)
        }

        pub(crate) fn same_theory<'types, 'domain>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
            structural: &'domain StructuralDomainDeclaration,
            scalar: &terminal_psi::ScalarDomainDeclaration,
            catalog: &terminal_psi::ScalarQualificationCatalog,
        ) -> bool {
            let Some(structural_nodes) = required_domains(domains, structural) else {
                return false;
            };
            let Some(scalar_nodes) = scalar.required_domain_closure(&catalog.domains) else {
                return false;
            };
            structural_nodes.len() == scalar_nodes.len()
                && structural_nodes.iter().all(|structural| {
                    let mut matching = scalar_nodes.iter().copied().filter(|domain| {
                        domain.semantic_domain == structural.semantic_domain
                            && domain.carrier == scalar.carrier
                    });
                    let Some(scalar) = matching.next() else {
                        return false;
                    };
                    if matching.next().is_some()
                        || structural.required_memberships != scalar.required_memberships
                        || !structural
                            .establishment_routes
                            .as_deref()
                            .unwrap_or_default()
                            .windows(2)
                            .all(|pair| pair[0] < pair[1])
                        || !scalar
                            .establishment_routes
                            .as_deref()
                            .unwrap_or_default()
                            .windows(2)
                            .all(|pair| pair[0] < pair[1])
                        || structural.establishment_routes != scalar.establishment_routes
                    {
                        return false;
                    }
                    let Some(required) = refinement(types.clone(), structural) else {
                        return false;
                    };
                    if required.carrier() != scalar.carrier {
                        return false;
                    }
                    let proposition = match &scalar.predicate {
                        Some(predicate) => {
                            let mut proposition = predicate.proposition.clone();
                            proposition.remap_value_ids(|value| {
                                if value == predicate.subject {
                                    ScalarRefinement::SUBJECT
                                } else {
                                    value
                                }
                            });
                            proposition
                        }
                        None if scalar.establishment_routes.is_none()
                            && scalar.has_no_required_memberships() =>
                        {
                            Proposition::Truth
                        }
                        None => return false,
                    };
                    let Ok(actual) = ScalarRefinement::new(scalar.carrier, proposition) else {
                        return false;
                    };
                    let required = StructuralFieldType::RefinedScalar(required);
                    let actual = StructuralFieldType::RefinedScalar(actual);
                    crate::scalar_field_entails(&required, &actual)
                        && crate::scalar_field_entails(&actual, &required)
                })
        }
    }

    use crate::exactly_one::exactly_one;
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, StructuralDomainId,
        StructuralTypeId,
    };
    use terminal_psi::{
        ByteSequencePredicate, ByteSequencePredicateSet, StructuralDomainDeclaration,
        StructuralFieldType, StructuralPathQualification, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    pub fn scalar_projection_entails_membership<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        membership: StructuralDomainId,
    ) -> bool {
        let Some(carrier) = crate::qualification_path_carrier(types.clone(), root, path) else {
            return false;
        };
        let Some(domain) = exact_domain(domains, carrier, membership) else {
            return false;
        };
        if !domain.complete_predicate_theory
            || !domain.has_no_required_memberships()
            || domain.establishment_routes.is_some()
            || domain.content_projection.is_some()
            || domain.byte_predicate.is_some()
        {
            return false;
        }
        let Some(restriction) = crate::primitive_projection_restriction(types.clone(), root, path)
            .or_else(|| {
                crate::primitive_read_field(types.clone(), root, path)
                    .map(|field| &field.field_type)
            })
            .filter(|field| crate::scalar_field_requires_evidence(field))
        else {
            return false;
        };
        let Some(required) = scalar::refinement(types, domain) else {
            return false;
        };
        crate::scalar_field_entails(restriction, &StructuralFieldType::RefinedScalar(required))
    }

    pub fn primitive_store_restriction<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        written: &[StructuralPathSegment],
    ) -> Option<Option<StructuralFieldType>> {
        let mut scalar =
            crate::primitive_projection_restriction(types.clone(), root, written).cloned();
        for domain in whole_scalar_domains(
            types.clone(),
            domains.clone(),
            root,
            whole,
            projected,
            additional_input,
            written,
        )? {
            scalar = scalar::receive(scalar, scalar::refinement(types.clone(), domain)?)?;
        }
        let predicates = enclosing_byte_predicates(
            types.clone(),
            domains,
            root,
            whole,
            projected,
            additional_input,
            written,
        )?;
        let mut ascii = ByteSequencePredicateSet::EMPTY;
        ascii.insert(ByteSequencePredicate::AsciiOnly);
        let utf8 = predicates
            .predicates()
            .any(|p| p == ByteSequencePredicate::ValidUtf8);
        let nonzero = predicates
            .predicates()
            .any(|p| p == ByteSequencePredicate::NoNul);
        let ascii = predicates.implies(ascii) || utf8;
        if !ascii && !nonzero {
            return Some(scalar);
        }
        if scalar.is_some() {
            return None;
        }
        Some(Some(StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Unsigned, 8).ok()?,
                IntegerValue::Unsigned(if nonzero { 1 } else { 0 }),
                IntegerValue::Unsigned(if ascii { 127 } else { 255 }),
            )
            .ok()?,
        )))
    }

    fn enclosing_byte_predicates<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        written: &[StructuralPathSegment],
    ) -> Option<ByteSequencePredicateSet> {
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        let memberships = whole.iter().map(|domain| (&[][..], *domain)).chain(
            projected
                .iter()
                .map(|row| (row.path.as_slice(), row.domain)),
        );
        for (path, domain) in memberships {
            if expiring_input_predicate(
                types.clone(),
                domains.clone(),
                root,
                path,
                domain,
                additional_input,
            ) {
                continue;
            }
            if !crate::structural_paths_may_overlap(path, written) {
                continue;
            }
            let prefix = written.get(..path.len())?;
            StructuralPathQualification::selected_suffix(path, prefix)?;
            let suffix = &written[path.len()..];
            let carrier_id = crate::qualification_path_carrier(types.clone(), root, path)?;
            if suffix.is_empty() && scalar::primitive_carrier(types.clone(), carrier_id).is_some() {
                let domain = exact_domain(domains.clone(), carrier_id, domain)?;
                scalar::refinement(types.clone(), domain)?;
                continue;
            }
            // A domain stated only by clauses, `[u8; 3]::Low requires
            // self[0] < 128`, governs no single write: a write inside it opens
            // the owning data type's invariant window, and the checker proves
            // the clauses again before the value is used (dependent_values.md,
            // "Invariant windows").
            let declared = exact_domain(domains.clone(), carrier_id, domain)?;
            if declared.byte_predicate.is_none()
                && declared.content_projection.is_none()
                && declared.establishment_routes.is_none()
                && declared.has_no_required_memberships()
            {
                continue;
            }
            if !matches!(
                suffix,
                [
                    StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                ]
            ) {
                return None;
            }
            let carrier = types.clone().find(|row| row.id == carrier_id)?;
            let StructuralTypeShape::FixedArray { element, .. } = carrier.shape else {
                return None;
            };
            let byte = IntegerType::new(IntegerSign::Unsigned, 8).ok()?;
            if types.clone().find(|row| row.id == element)?.shape
                != StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(
                    byte,
                ))
            {
                return None;
            }
            let declared =
                crate::byte_qualification_predicates_in(domains.clone(), carrier.id, &[domain])?;
            for predicate in declared.predicates() {
                predicates.insert(predicate);
            }
        }
        Some(predicates)
    }

    pub fn primitive_store_byte_predicates<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        written: &[StructuralPathSegment],
    ) -> Option<ByteSequencePredicateSet> {
        enclosing_byte_predicates(
            types,
            domains,
            root,
            whole,
            projected,
            additional_input,
            written,
        )
    }

    fn exact_domain<'domain>(
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration>,
        carrier: StructuralTypeId,
        identity: StructuralDomainId,
    ) -> Option<&'domain StructuralDomainDeclaration> {
        let mut matching = domains.filter(|domain| domain.id == identity);
        let domain = matching.next()?;
        (matching.next().is_none() && domain.carrier == carrier).then_some(domain)
    }

    fn whole_scalar_domains<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        written: &[StructuralPathSegment],
    ) -> Option<Vec<&'domain StructuralDomainDeclaration>> {
        let mut result = Vec::new();
        for (path, identity) in whole.iter().map(|identity| (&[][..], *identity)).chain(
            projected
                .iter()
                .map(|row| (row.path.as_slice(), row.domain)),
        ) {
            if expiring_input_predicate(
                types.clone(),
                domains.clone(),
                root,
                path,
                identity,
                additional_input,
            ) {
                continue;
            }
            if path.len() != written.len() || !crate::structural_paths_may_overlap(path, written) {
                continue;
            }
            StructuralPathQualification::selected_suffix(path, written)?;
            let carrier = crate::qualification_path_carrier(types.clone(), root, path)?;
            if scalar::primitive_carrier(types.clone(), carrier).is_some() {
                let domain = exact_domain(domains.clone(), carrier, identity)?;
                result.extend(scalar::required_domains(domains.clone(), domain)?);
            }
        }
        Some(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn primitive_store_preserves_scalar_memberships<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        catalog: &terminal_psi::ScalarQualificationCatalog,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        written: &[StructuralPathSegment],
        value: &terminal_psi::ValueDeclaration,
    ) -> bool {
        let Some(required) = whole_scalar_domains(
            types.clone(),
            domains.clone(),
            root,
            whole,
            projected,
            additional_input,
            written,
        ) else {
            return false;
        };
        let actual = if value.qualifications.is_empty() {
            &[][..]
        } else {
            let Some(set) = exactly_one(catalog.sets.iter(), |set| set.id == value.qualifications)
            else {
                return false;
            };
            set.domains.as_slice()
        };
        required.into_iter().all(|required| {
            required.establishment_routes.is_none()
                || scalar_domain_matches(
                    types.clone(),
                    domains.clone(),
                    required,
                    catalog,
                    actual,
                    value.scalar_type,
                )
        })
    }

    fn scalar_domain_matches<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        required: &'domain StructuralDomainDeclaration,
        catalog: &terminal_psi::ScalarQualificationCatalog,
        actual: &[semantic_vocabulary::ScalarDomainId],
        carrier: semantic_vocabulary::ScalarType,
    ) -> bool {
        let mut matching = catalog.domains.iter().filter(|domain| {
            domain.semantic_domain == required.semantic_domain && domain.carrier == carrier
        });
        let Some(domain) = matching.next() else {
            return false;
        };
        matching.next().is_none()
            && domain
                .missing_membership_requirements(&catalog.domains, actual)
                .is_some_and(|missing| missing.is_empty())
            && scalar::same_theory(types, domains, required, domain, catalog)
    }

    pub fn primitive_scalar_memberships<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        catalog: &terminal_psi::ScalarQualificationCatalog,
        destination: StructuralTypeId,
        value: &terminal_psi::ValueDeclaration,
    ) -> Option<Vec<StructuralDomainId>> {
        if scalar::primitive_carrier(types.clone(), destination)? != value.scalar_type {
            return None;
        }
        if value.qualifications.is_empty() {
            return Some(Vec::new());
        }
        let mut sets = catalog
            .sets
            .iter()
            .filter(|set| set.id == value.qualifications);
        let actual = sets.next()?.domains.as_slice();
        if sets.next().is_some() {
            return None;
        }
        let mut memberships = Vec::with_capacity(actual.len());
        for identity in actual {
            let declaration = exactly_one(catalog.domains.iter(), |domain| domain.id == *identity)?;
            if declaration.carrier != value.scalar_type {
                return None;
            }
            let mut matching = domains.clone().filter(|domain| {
                domain.carrier == destination
                    && domain.semantic_domain == declaration.semantic_domain
            });
            let domain = matching.next()?;
            if matching.next().is_some()
                || !scalar_domain_matches(
                    types.clone(),
                    domains.clone(),
                    domain,
                    catalog,
                    actual,
                    value.scalar_type,
                )
            {
                return None;
            }
            memberships.push(domain.id);
        }
        memberships.sort();
        memberships.dedup();
        Some(memberships)
    }

    fn expiring_input_predicate<'types, 'domain>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        domains: impl Iterator<Item = &'domain StructuralDomainDeclaration> + Clone,
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
        membership: StructuralDomainId,
        additional: &[StructuralPathQualification],
    ) -> bool {
        if !additional
            .iter()
            .any(|row| row.domain == membership && row.path == path)
        {
            return false;
        }
        crate::qualification_path_carrier(types, root, path)
            .and_then(|carrier| exact_domain(domains, carrier, membership))
            .is_some_and(StructuralDomainDeclaration::is_pure_predicate)
    }
}
pub(crate) mod selected_memberships {
    use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        StructuralAccess, StructuralPathQualification, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    #[allow(clippy::too_many_arguments)]
    pub fn reference_carrier_memberships<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        reference: StructuralTypeId,
        storage: Option<&terminal_psi::ReferenceStorageView>,
    ) -> Result<Vec<StructuralPathQualification>, &'static str> {
        let captured =
            reference_source_memberships(types.clone(), root, whole, projected, path, access)?;
        if storage.is_none() || captured.is_empty() {
            return Ok(captured);
        }
        let StructuralTypeShape::Reference { referent, .. } = types
            .clone()
            .find(|row| row.id == reference)
            .ok_or("reference membership carrier is absent")?
            .shape
        else {
            return Err("representation membership has no reference carrier");
        };
        let source = crate::qualification_path_carrier(types.clone(), root, path)
            .ok_or("reference membership has no declared source carrier")?;
        if crate::borrowed_referent_presentation(types, source, referent)
            == Some(crate::BorrowedReferentPresentation::Unchanged)
        {
            return Ok(captured);
        }
        if access == StructuralAccess::SharedBorrow {
            Ok(Vec::new())
        } else {
            Err("reference reinterpretation cannot erase storage qualification obligations")
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reference_carrier_membership_contract<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        additional_input: &[StructuralPathQualification],
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        reference: StructuralTypeId,
        storage: Option<&terminal_psi::ReferenceStorageView>,
    ) -> Result<
        (
            Vec<StructuralPathQualification>,
            Vec<StructuralPathQualification>,
        ),
        &'static str,
    > {
        let standing_whole = whole
            .iter()
            .copied()
            .filter(|domain| {
                !additional_input
                    .iter()
                    .any(|row| row.path.is_empty() && row.domain == *domain)
            })
            .collect::<Vec<_>>();
        let standing_projected = projected
            .iter()
            .filter(|row| !additional_input.contains(row))
            .cloned()
            .collect::<Vec<_>>();
        let additional = reference_carrier_memberships(
            types.clone(),
            root,
            &[],
            additional_input,
            path,
            StructuralAccess::SharedBorrow,
            reference,
            storage,
        )?;
        let mut captured = reference_carrier_memberships(
            types,
            root,
            &standing_whole,
            &standing_projected,
            path,
            access,
            reference,
            storage,
        )?;
        captured.extend_from_slice(&additional);
        captured.sort();
        captured.dedup();
        Ok((captured, additional))
    }

    pub fn reference_source_memberships<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        whole: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
        path: &[StructuralPathSegment],
        access: StructuralAccess,
    ) -> Result<Vec<StructuralPathQualification>, &'static str> {
        const MAX_PATH_DEPTH: usize = 128;
        const MAX_SELECTION_WORK: usize = 1_048_576;
        if path.len() > MAX_PATH_DEPTH {
            return Err("reference membership selection exceeds the path-depth budget");
        }
        if whole.is_empty() && projected.is_empty() {
            for (position, segment) in path.iter().enumerate() {
                if matches!(
                    segment,
                    StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::AllElements
                ) {
                    let parent =
                        crate::qualification_path_carrier(types.clone(), root, &path[..position])
                            .and_then(|carrier| {
                                crate::structural_declaration(types.clone(), carrier)
                            })
                            .ok_or("reference membership selection has no runtime carrier")?;
                    match parent.shape {
                        StructuralTypeShape::FixedArray {
                            length: terminal_psi::ArrayExtent::Literal(0),
                            ..
                        } => {
                            return Err("reference membership selection has an empty fixed array");
                        }
                        StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::ElementView { .. }
                        | StructuralTypeShape::ByteSequence(_) => {}
                        _ => {
                            return Err("reference membership selection has no collection carrier");
                        }
                    }
                }
            }
            return Ok(Vec::new());
        }
        let mut work = SelectionWork(MAX_SELECTION_WORK);
        work.charge(path.len())?;
        work.charge(whole.len())?;
        for row in projected {
            work.charge(row.path.len().saturating_add(1))?;
        }
        let mut initial = Vec::new();
        initial
            .try_reserve(whole.len().saturating_add(projected.len()))
            .map_err(|_| "reference membership selection allocation failed")?;
        for domain in whole {
            initial.push(MembershipSuffix {
                path: &[],
                domain: *domain,
            });
        }
        for row in projected {
            initial.push(MembershipSuffix {
                path: &row.path,
                domain: row.domain,
            });
        }
        let mut candidates = vec![initial];
        for (position, segment) in path.iter().enumerate() {
            if matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::AllElements
            ) {
                let parent =
                    crate::qualification_path_carrier(types.clone(), root, &path[..position])
                        .and_then(|carrier| crate::structural_declaration(types.clone(), carrier))
                        .ok_or("reference membership selection has no runtime carrier")?;
                let extent = match parent.shape {
                    StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(0),
                        ..
                    } => {
                        return Err("reference membership selection has an empty fixed array");
                    }
                    StructuralTypeShape::FixedArray { length, .. } => {
                        Some(length.literal().ok_or(
                            "reference membership selection requires a closed array extent",
                        )?)
                    }
                    StructuralTypeShape::ElementView { .. } => None,
                    _ => return Err("reference membership selection has no collection carrier"),
                };
                let mut next = Vec::new();
                for rows in candidates {
                    partition_runtime_memberships(rows, segment, extent, &mut next, &mut work)?;
                }
                candidates = next;
            } else {
                for rows in &mut candidates {
                    work.charge(rows.len())?;
                    rows.retain_mut(|row| {
                        let Some(suffix) = StructuralPathQualification::selected_suffix(
                            row.path,
                            std::slice::from_ref(segment),
                        ) else {
                            return false;
                        };
                        row.path = suffix;
                        true
                    });
                }
            }
        }
        let mut available: Option<Vec<StructuralPathQualification>> = None;
        for rows in candidates {
            let mut captured = Vec::new();
            for row in rows {
                work.charge(row.path.len().saturating_add(2))?;
                let mut path = Vec::new();
                path.try_reserve(row.path.len().saturating_add(1))
                    .map_err(|_| "reference membership selection allocation failed")?;
                path.push(StructuralPathSegment::Referent);
                path.extend_from_slice(row.path);
                captured.push(StructuralPathQualification {
                    path,
                    domain: row.domain,
                });
            }
            captured.sort();
            captured.dedup();
            if let Some(common) = &mut available {
                if access != StructuralAccess::SharedBorrow && *common != captured {
                    return Err(
                        "reference selection cannot retain every source membership obligation",
                    );
                }
                common.retain(|row| captured.binary_search(row).is_ok());
            } else {
                available = Some(captured);
            }
        }
        available.ok_or("reference membership selection has no candidate")
    }

    #[derive(Clone, Copy)]
    struct MembershipSuffix<'source> {
        path: &'source [StructuralPathSegment],
        domain: StructuralDomainId,
    }

    struct SelectionWork(usize);

    impl SelectionWork {
        fn charge(&mut self, count: usize) -> Result<(), &'static str> {
            self.0 = self
                .0
                .checked_sub(count)
                .ok_or("reference membership selection exceeds the work budget")?;
            Ok(())
        }
    }

    fn append_candidate<'source>(
        candidates: &mut Vec<Vec<MembershipSuffix<'source>>>,
        mut selected: Vec<MembershipSuffix<'source>>,
        universal: &[MembershipSuffix<'source>],
        work: &mut SelectionWork,
    ) -> Result<(), &'static str> {
        const MAX_CANDIDATES: usize = 4096;
        if candidates.len() >= MAX_CANDIDATES {
            return Err("reference membership selection exceeds the candidate budget");
        }
        work.charge(universal.len().saturating_add(1))?;
        selected
            .try_reserve(universal.len())
            .map_err(|_| "reference membership selection allocation failed")?;
        selected.extend_from_slice(universal);
        candidates
            .try_reserve(1)
            .map_err(|_| "reference membership selection allocation failed")?;
        candidates.push(selected);
        Ok(())
    }

    fn partition_runtime_memberships<'source>(
        rows: Vec<MembershipSuffix<'source>>,
        selector: &StructuralPathSegment,
        extent: Option<u64>,
        candidates: &mut Vec<Vec<MembershipSuffix<'source>>>,
        work: &mut SelectionWork,
    ) -> Result<(), &'static str> {
        work.charge(rows.len())?;
        let mut universal = Vec::new();
        let mut concrete = Vec::new();
        let mut remainder = Vec::new();
        for row in rows {
            let Some((segment, suffix)) = row.path.split_first() else {
                continue;
            };
            let selected = MembershipSuffix {
                path: suffix,
                domain: row.domain,
            };
            match segment {
                StructuralPathSegment::AllElements => universal.push(selected),
                StructuralPathSegment::FixedIndex(index)
                    if extent.is_none_or(|length| *index < length) =>
                {
                    concrete.push((*index, selected));
                }
                _ if extent.is_none() && segment == selector => remainder.push(selected),
                _ => {}
            }
        }
        concrete.sort_unstable_by_key(|(index, _)| *index);
        let mut distinct = 0u64;
        let mut rows = concrete.into_iter().peekable();
        while let Some((index, row)) = rows.next() {
            distinct += 1;
            let mut selected = vec![row];
            while rows
                .peek()
                .is_some_and(|(following, _)| *following == index)
            {
                if let Some((_, row)) = rows.next() {
                    selected.push(row);
                }
            }
            append_candidate(candidates, selected, &universal, work)?;
        }
        if extent.is_none_or(|length| distinct < length) {
            append_candidate(candidates, remainder, &universal, work)?;
        }
        Ok(())
    }
}

pub mod descriptor_results {
    pub(crate) use crate::descriptor_replay as replay;

    pub(crate) use crate::exactly_one::exactly_one;
    pub(crate) use std::collections::{BTreeMap, BTreeSet};

    pub(crate) use semantic_vocabulary::{
        MachineId, PlaceId, StructuralPlaceKind, StructuralTypeId,
    };
    pub(crate) use terminal_psi::{
        ByteSequenceCarrier, Operation, OperationKind, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
    };

    pub(crate) use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
    };
    pub(crate) use crate::storage::borrowed_storage::Projection;
    pub(crate) mod boundary_calls {
        use super::{
            Operation, OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine,
            TerminalModule,
        };
        use super::{owned_shared_byte_descriptor_shape, plain_owned_shape};
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
        use terminal_psi::{
            BoundaryContentGuarantee, BoundaryMachineDeclaration, BoundaryMachineResult,
            OperationResult, StructuralArgument, StructuralTypeDeclaration,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum BoundaryCompletionResult {
            Unit,
            Scalar(ScalarType),
            Structural(StructuralTypeId),
        }

        pub fn boundary_call_preserves_descriptor_storage(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
        ) -> bool {
            let OperationKind::BoundaryCall {
                boundary,
                structural_arguments,
                completion_receipts,
                ..
            } = &operation.kind
            else {
                return false;
            };
            let Some(declaration) = exactly_one(module.boundary_machines.iter(), |declaration| {
                declaration.id == *boundary
            }) else {
                return false;
            };
            let result = match &operation.result {
                OperationResult::Unit => BoundaryCompletionResult::Unit,
                OperationResult::Scalar(result) => {
                    BoundaryCompletionResult::Scalar(result.scalar_type)
                }
                OperationResult::Structural(result) => {
                    BoundaryCompletionResult::Structural(result.structural_type)
                }
            };
            boundary_completion_preserves_descriptor_storage(
                module.structural_types.iter(),
                declaration,
                result,
                !completion_receipts.is_empty(),
                structural_arguments,
                |place| machine.structural_place_type(place),
            )
        }

        pub fn boundary_completion_preserves_descriptor_storage<'types>(
            types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
            declaration: &BoundaryMachineDeclaration,
            result: BoundaryCompletionResult,
            has_completion_receipts: bool,
            arguments: &[StructuralArgument],
            place_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
        ) -> bool {
            if has_completion_receipts
                || declaration.structural_parameters.len() != arguments.len()
                || declaration.content_guarantees.iter().any(|guarantee| {
                    matches!(guarantee, BoundaryContentGuarantee::RetainedBorrow(_))
                })
                || !match (&declaration.result, result) {
                    (BoundaryMachineResult::Unit, BoundaryCompletionResult::Unit) => true,
                    (
                        BoundaryMachineResult::Scalar(expected),
                        BoundaryCompletionResult::Scalar(actual),
                    ) => expected.scalar_type == actual,
                    (
                        BoundaryMachineResult::Structural(expected),
                        BoundaryCompletionResult::Structural(actual),
                    ) => {
                        expected.structural_type == actual
                            && (plain_owned_shape(types.clone(), actual)
                                || (declaration.result_lifetimes.is_some()
                                    && owned_shared_byte_descriptor_shape(types.clone(), actual)))
                    }
                    _ => false,
                }
            {
                return false;
            }
            arguments
                .iter()
                .zip(&declaration.structural_parameters)
                .all(|(argument, parameter)| {
                    if argument.access != parameter.access {
                        return false;
                    }
                    if argument.access == StructuralAccess::SharedBorrow {
                        return true;
                    }
                    let Some(root) = place_type(argument.place) else {
                        return false;
                    };
                    let Some(selected) =
                        crate::runtime_structural_path_tip(types.clone(), root, &argument.path)
                    else {
                        return false;
                    };
                    if argument.access == StructuralAccess::Owned {
                        // A moved immutable descriptor aggregate carries its
                        // loans into the declared result frontier. With no
                        // retained-borrow outcome, moving its carrier does not
                        // make those loans escape beyond the result's use.
                        return declaration.result_lifetimes.is_some()
                            && owned_shared_byte_descriptor_shape(types.clone(), selected.id);
                    }
                    if argument.path.is_empty() {
                        match &selected.shape {
                            StructuralTypeShape::ByteSequence(carrier) if carrier.is_borrowed() => {
                                return true;
                            }
                            StructuralTypeShape::ElementView { element, .. } => {
                                return plain_owned_shape(types.clone(), *element);
                            }
                            _ => {}
                        }
                    }
                    plain_owned_shape(types.clone(), selected.id)
                })
        }
    }
    pub(crate) use crate::descriptors::descriptor_lifetimes;
    pub(crate) use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
    pub use boundary_calls::{
        BoundaryCompletionResult, boundary_call_preserves_descriptor_storage,
        boundary_completion_preserves_descriptor_storage,
    };
    pub(crate) use semantic_vocabulary::Proposition;
    pub(crate) use semantic_vocabulary::PropositionContext;
    pub(crate) use semantic_vocabulary::ScalarTerm;
    pub(crate) use semantic_vocabulary::ValueId;
    pub(crate) mod compatibility {
        use super::*;
        use crate::storage::borrowed_storage::Region;

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub struct DescriptorCompatibilityRequirement {
            pub position: DescriptorPosition,
            pub loan: DescriptorSourceOrigin,
            pub demand: DescriptorCallOrigin,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct DescriptorCompatibilityFailure {
            pub machine: MachineId,
            pub position: DescriptorPosition,
        }

        impl DescriptorAnalysis<'_> {
            pub fn compatibility_requirements(
                &mut self,
                requirements: &BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
            ) -> Result<
                BTreeMap<MachineId, Vec<DescriptorCompatibilityRequirement>>,
                DescriptorCompatibilityFailure,
            > {
                let mut pending = BTreeMap::<_, BTreeSet<_>>::new();
                for (&machine, rows) in requirements {
                    for row in rows {
                        self.compatibility_ingress(machine, row).ok_or(
                            DescriptorCompatibilityFailure {
                                machine,
                                position: row.position,
                            },
                        )?;
                        pending.entry(machine).or_default().insert(row.clone());
                    }
                }
                let mut identities = BTreeSet::new();
                let mut calls = Vec::new();
                for machine in self.compatibility_machines() {
                    if !identities.insert(machine.id) {
                        return Err(DescriptorCompatibilityFailure {
                            machine: machine.id,
                            position: machine.entry,
                        });
                    }
                    calls.extend(machine.known_invocations().filter_map(|(&position, call)| {
                        call.stored_effects_callee()
                            .map(|callee| (machine.id, position, call.owner, callee))
                    }));
                }
                loop {
                    let mut additions = Vec::new();
                    for &(machine, position, owner, callee) in &calls {
                        let Some(requirements) = pending.get(&callee) else {
                            continue;
                        };
                        let failure = || DescriptorCompatibilityFailure { machine, position };
                        for requirement in requirements {
                            let demand = self
                                .substitute_call_storage_at(
                                    machine,
                                    position,
                                    owner,
                                    &requirement.demand,
                                )
                                .ok_or_else(failure)?;
                            let DescriptorCallOrigin::Argument { source, projection } = &demand
                            else {
                                return Err(failure());
                            };
                            if !projection.exact
                                || projection.indirect
                                || projection.window.is_some()
                                || projection.captured_at.is_some()
                            {
                                return Err(failure());
                            }
                            let demanded = Region {
                                root: Some(*source),
                                projection: projection.clone(),
                            };
                            let origins = self
                                .substitute_stored_effect_source_at(
                                    machine,
                                    position,
                                    owner,
                                    &requirement.loan,
                                )
                                .filter(|origins| !origins.is_empty())
                                .ok_or_else(failure)?;
                            for loan in origins {
                                match loan {
                                    DescriptorSourceOrigin::Provider(_) => return Err(failure()),
                                    DescriptorSourceOrigin::Backing(
                                        DescriptorCallOrigin::AccessFormation { .. }
                                        | DescriptorCallOrigin::Invocation { .. },
                                    ) => return Err(failure()),
                                    DescriptorSourceOrigin::Backing(
                                        DescriptorCallOrigin::Empty
                                        | DescriptorCallOrigin::Literal { .. },
                                    ) => {}
                                    DescriptorSourceOrigin::Backing(
                                        DescriptorCallOrigin::Argument { source, projection },
                                    ) => {
                                        if (Region {
                                            root: Some(source),
                                            projection,
                                        })
                                        .overlaps(&demanded)
                                        {
                                            return Err(failure());
                                        }
                                    }
                                    DescriptorSourceOrigin::IncomingSlot(_)
                                    | DescriptorSourceOrigin::IncomingContents { .. } => {
                                        let requirement = DescriptorCompatibilityRequirement {
                                            position,
                                            loan,
                                            demand: demand.clone(),
                                        };
                                        self.compatibility_ingress(machine, &requirement)
                                            .ok_or_else(failure)?;
                                        if !pending
                                            .get(&machine)
                                            .is_some_and(|rows| rows.contains(&requirement))
                                        {
                                            additions.push((machine, requirement));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if additions.is_empty() {
                        return Ok(pending
                            .into_iter()
                            .map(|(machine, rows)| (machine, rows.into_iter().collect()))
                            .collect());
                    }
                    for (machine, requirement) in additions {
                        pending.entry(machine).or_default().insert(requirement);
                    }
                }
            }
        }
    }
    pub(crate) mod contents {
        use super::*;
        pub(crate) mod incoming_memberships {
            use super::*;

            pub(crate) fn select(
                types: &[StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
                contract: PayloadMembershipContract,
                steps: &[DescriptorContentsStep],
            ) -> Option<PayloadMembershipContract> {
                let count = steps
                    .iter()
                    .position(|step| matches!(step, DescriptorContentsStep::Element { .. }))
                    .unwrap_or(steps.len());
                let address = steps[..count]
                    .iter()
                    .map(|step| match step {
                        DescriptorContentsStep::Address(step) => Some(*step),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                let mut contract =
                    super::select_payload_memberships(types, domains, &contract, &address)?;
                let carrier = contract.carrier;
                let Some(DescriptorContentsStep::Element { element, .. }) = steps.get(count) else {
                    super::close_predicate_memberships(types, domains, &mut contract);
                    return Some(contract);
                };
                let suffix = &steps[count + 1..];
                match terminal_psi::find_structural_type(types, carrier)?.shape {
                    StructuralTypeShape::ElementView {
                        element: actual, ..
                    } if actual == *element => {
                        let (qualifications, projected_qualifications) =
                            terminal_psi::StructuralPathQualification::select(
                                &contract.qualifications,
                                &contract.projected_qualifications,
                                &[StructuralPathSegment::AllElements],
                            );
                        select(
                            types,
                            domains,
                            PayloadMembershipContract {
                                scalar_observations: Vec::new(),
                                scalar_requirements: Vec::new(),
                                carrier: *element,
                                qualifications,
                                projected_qualifications,
                            },
                            suffix,
                        )
                    }
                    StructuralTypeShape::FixedArray {
                        element: actual,
                        length: terminal_psi::ArrayExtent::Literal(length),
                    } if actual == *element && length <= 4096 => {
                        let mut common: Option<PayloadMembershipContract> = None;
                        for index in 0..length {
                            let (qualifications, projected_qualifications) =
                                terminal_psi::StructuralPathQualification::select(
                                    &contract.qualifications,
                                    &contract.projected_qualifications,
                                    &[StructuralPathSegment::FixedIndex(index)],
                                );
                            let selected = select(
                                types,
                                domains,
                                PayloadMembershipContract {
                                    scalar_observations: Vec::new(),
                                    scalar_requirements: Vec::new(),
                                    carrier: *element,
                                    qualifications,
                                    projected_qualifications,
                                },
                                suffix,
                            )?;
                            if let Some(common) = &mut common {
                                if common.carrier != selected.carrier {
                                    return None;
                                }
                                common
                                    .qualifications
                                    .retain(|domain| selected.qualifications.contains(domain));
                                common
                                    .projected_qualifications
                                    .retain(|row| selected.projected_qualifications.contains(row));
                            } else {
                                common = Some(selected);
                            }
                        }
                        common
                    }
                    _ => None,
                }
            }
        }
        use crate::descriptors::descriptor_lifetimes::{
            descriptor_location, descriptor_location_type, descriptor_path,
        };

        use super::payload_predicates::introduction_predicate;
        pub use crate::descriptors::descriptor_lifetimes::{
            DescriptorContentsQuery, DescriptorContentsStep,
        };
        use semantic_vocabulary::ScalarRefinement;
        use semantic_vocabulary::{ScalarType, ValueId};
        use terminal_psi::StructuralPathQualification;

        pub struct DescriptorContentsTransfer {
            pub source: DescriptorContentsQuery,
            pub destination: DescriptorContentsQuery,
            pub removes_source: bool,
        }

        pub struct DescriptorContentsOperand<'a> {
            pub root: PlaceId,
            pub root_type: StructuralTypeId,
            pub path: &'a [StructuralPathSegment],
            pub field: Option<semantic_vocabulary::StructuralFieldId>,
        }

        pub fn descriptor_contents_transfers(
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            source: DescriptorContentsOperand<'_>,
            destination: DescriptorContentsOperand<'_>,
            value_type: StructuralTypeId,
            removes_source: bool,
        ) -> Option<Vec<DescriptorContentsTransfer>> {
            let slots = slot_kind.owned_slots(types, source.root, value_type)?;
            if slots.is_empty() {
                return Some(Vec::new());
            }
            let source = descriptor_contents_query(
                types,
                source.root,
                source.root_type,
                source.path,
                source.field,
            )?;
            let destination = descriptor_contents_query(
                types,
                destination.root,
                destination.root_type,
                destination.path,
                destination.field,
            )?;
            let mut transfers = Vec::new();
            for slot in slots {
                let mut source = source.clone();
                let mut destination = destination.clone();
                let suffix = slot.path.into_iter().map(DescriptorContentsStep::Address);
                source.steps.extend(suffix.clone());
                destination.steps.extend(suffix);
                transfers.push(DescriptorContentsTransfer {
                    source,
                    destination,
                    removes_source,
                });
            }
            Some(transfers)
        }

        impl DescriptorContentsQuery {
            pub(crate) fn retains_selector(&self) -> bool {
                self.steps.iter().any(|step| {
                    matches!(
                        step,
                        DescriptorContentsStep::Element {
                            selector: Some(_),
                            ..
                        }
                    )
                })
            }

            pub(crate) fn without_selectors(&self) -> Self {
                Self {
                    root: self.root,
                    steps: self
                        .steps
                        .iter()
                        .map(|step| match step {
                            DescriptorContentsStep::Element { element, .. } => {
                                DescriptorContentsStep::any_element(*element)
                            }
                            step => step.clone(),
                        })
                        .collect(),
                }
            }

            pub(crate) fn is_symbolic_location(&self) -> bool {
                self.retains_selector()
                    && self.steps.iter().all(|step| {
                        !matches!(step, DescriptorContentsStep::Element { selector: None, .. })
                    })
            }

            /// Whether each step stays inside the selected value's own
            /// storage. A reference, view backing or accessor crosses out.
            pub(crate) fn inline_steps(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
            ) -> Option<Vec<bool>> {
                let mut base = root_type;
                let mut path = Vec::new();
                let mut inline = Vec::with_capacity(self.steps.len());
                for step in &self.steps {
                    let container = || {
                        let container = descriptor_location_type(types, base, &path)?;
                        Some(&terminal_psi::find_structural_type(types, container)?.shape)
                    };
                    inline.push(match step {
                        // Storage coordinates do not establish a scalar
                        // value equation across a change of presentation.
                        DescriptorContentsStep::Address(
                            DescriptorPathSegment::RepresentationView { .. }
                            | DescriptorPathSegment::RuntimeIndex { .. }
                            | DescriptorPathSegment::Extent,
                        ) => return None,
                        DescriptorContentsStep::Address(DescriptorPathSegment::Referent) => false,
                        DescriptorContentsStep::Address(DescriptorPathSegment::Case(_)) => true,
                        DescriptorContentsStep::Address(DescriptorPathSegment::Field(_)) => {
                            matches!(path.last(), Some(DescriptorPathSegment::Case(_)))
                                || container().is_some_and(|shape| match shape {
                                    StructuralTypeShape::Record { storage, .. } => !matches!(
                                        storage,
                                        terminal_psi::StructuralRecordStorage::AccessView(_)
                                            | terminal_psi::StructuralRecordStorage::Accessor(_)
                                    ),
                                    StructuralTypeShape::Mixed { .. } => true,
                                    _ => false,
                                })
                        }
                        DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(_))
                        | DescriptorContentsStep::Element { .. } => {
                            container().is_some_and(|shape| {
                                matches!(shape, StructuralTypeShape::FixedArray { .. })
                            })
                        }
                    });
                    match step {
                        DescriptorContentsStep::Address(segment) => path.push(*segment),
                        DescriptorContentsStep::Element { element, .. } => {
                            base = *element;
                            path.clear();
                        }
                    }
                }
                Some(inline)
            }

            pub fn exact_location(&self) -> Option<DescriptorLocation> {
                Some(DescriptorLocation {
                    root: self.root,
                    path: self
                        .steps
                        .iter()
                        .map(|step| match step {
                            DescriptorContentsStep::Address(segment) => Some(*segment),
                            DescriptorContentsStep::Element { .. } => None,
                        })
                        .collect::<Option<_>>()?,
                })
            }

            pub fn from_location(location: &DescriptorLocation) -> Self {
                Self {
                    root: location.root,
                    steps: location
                        .path
                        .iter()
                        .cloned()
                        .map(DescriptorContentsStep::Address)
                        .collect(),
                }
            }

            pub fn observation(&self) -> DescriptorLocation {
                DescriptorLocation {
                    root: self.root,
                    path: self
                        .steps
                        .iter()
                        .map_while(|step| match step {
                            DescriptorContentsStep::Address(segment) => Some(*segment),
                            DescriptorContentsStep::Element { .. } => None,
                        })
                        .collect(),
                }
            }

            pub fn validate(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
            ) -> Option<()> {
                self.validate_using(types, root_type, |carrier, path, _| {
                    descriptor_path(types, carrier, path)?;
                    Some(descriptor_location_type(types, carrier, path))
                })
            }

            /// `location_type` answers `Some` for a path that resolves from
            /// its carrier, holding the structural type at the path's end
            /// when the end has one. A leaf need not: a byte view held in a
            /// field resolves and has no declaration of its own. A query may
            /// end at such a leaf and may not step through it.
            pub(crate) fn validate_using(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
                location_type: impl FnMut(
                    StructuralTypeId,
                    &[DescriptorPathSegment],
                    bool,
                ) -> Option<Option<StructuralTypeId>>,
            ) -> Option<()> {
                self.selected_type_using(types, root_type, location_type)
                    .map(|_| ())
            }

            pub(crate) fn selected_type_using(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
                mut location_type: impl FnMut(
                    StructuralTypeId,
                    &[DescriptorPathSegment],
                    bool,
                ) -> Option<Option<StructuralTypeId>>,
            ) -> Option<Option<StructuralTypeId>> {
                let mut tip = root_type;
                let mut original = true;
                let mut path = Vec::new();
                for step in &self.steps {
                    match step {
                        DescriptorContentsStep::Address(segment) => path.push(*segment),
                        DescriptorContentsStep::Element { element, .. } => {
                            tip = location_type(tip, &path, original)??;
                            let declaration = terminal_psi::find_structural_type(types, tip)?;
                            match declaration.shape {
                                StructuralTypeShape::FixedArray {
                                    element: actual, ..
                                }
                                | StructuralTypeShape::ElementView {
                                    element: actual, ..
                                } if actual == *element => {}
                                _ => return None,
                            }
                            tip = *element;
                            original = false;
                            path.clear();
                        }
                    }
                }
                location_type(tip, &path, original)
            }

            /// Append a checked inline projection using the selected value's
            /// own field declarations. Unknown addresses, windows and past
            /// captures cannot become an exact memory-access query here.
            pub(crate) fn projected(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
                projection: &Projection,
            ) -> Option<Self> {
                if !projection.exact
                    || projection.indirect
                    || projection.window.is_some()
                    || projection.captured_at.is_some()
                {
                    return None;
                }
                if projection.path.is_empty() {
                    self.validate(types, root_type)?;
                    return Some(self.clone());
                }
                let selected =
                    self.selected_type_using(types, root_type, |carrier, path, _| {
                        descriptor_path(types, carrier, path)?;
                        Some(descriptor_location_type(types, carrier, path))
                    })??;
                let suffix =
                    descriptor_contents_query(types, self.root, selected, &projection.path, None)?;
                let mut query = self.clone();
                query.steps.extend(suffix.steps);
                query.validate(types, root_type)?;
                Some(query)
            }

            pub fn conservative_projection(
                &self,
                types: &[StructuralTypeDeclaration],
                root_type: StructuralTypeId,
            ) -> Option<Projection> {
                let prefix = self.observation();
                let mut projection =
                    Projection::new(&descriptor_path(types, root_type, &prefix.path)?);
                projection.exact = false;
                projection.indirect = true;
                Some(projection.without_captured_address())
            }
        }

        pub fn descriptor_contents_query(
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            final_field: Option<semantic_vocabulary::StructuralFieldId>,
        ) -> Option<DescriptorContentsQuery> {
            descriptor_contents_query_with(types, root, root_type, path, final_field, |_| None)
        }

        pub(crate) fn descriptor_storage_use_source(
            types: &[StructuralTypeDeclaration],
            root_type: Option<StructuralTypeId>,
            operation: &terminal_psi::OperationKind,
            used: terminal_psi::StructuralStorageUse<'_>,
            select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
        ) -> crate::descriptor_lifetimes::DescriptorFlowSource {
            use crate::descriptor_lifetimes::DescriptorFlowSource;
            use terminal_psi::StructuralStorageUse;

            let (root, query) = match used {
                StructuralStorageUse::Place(root) => (
                    root,
                    Some(DescriptorContentsQuery::from_location(
                        &DescriptorLocation::whole(root),
                    )),
                ),
                StructuralStorageUse::Projection(source) => (
                    source.root,
                    root_type.and_then(|ty| {
                        descriptor_contents_query_with(
                            types,
                            source.root,
                            ty,
                            source.path,
                            source.field,
                            &select,
                        )
                    }),
                ),
                StructuralStorageUse::Canonical { root, path } => (
                    root,
                    root_type.and_then(|ty| {
                        let location = crate::descriptor_lifetimes::canonical_descriptor_location(
                            types, root, ty, path,
                        )?;
                        Some(DescriptorContentsQuery::from_location(&location))
                    }),
                ),
                StructuralStorageUse::ObservedField { root, path, field } => (
                    root,
                    root_type.and_then(|ty| {
                        let mut selected = path.to_vec();
                        selected.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                            field,
                        ));
                        let location = crate::descriptor_lifetimes::canonical_descriptor_location(
                            types, root, ty, &selected,
                        )?;
                        Some(DescriptorContentsQuery::from_location(&location))
                    }),
                ),
                StructuralStorageUse::Element { descriptor, path } => {
                    let query = (|| {
                        let (index, obligation) = match operation {
                            terminal_psi::OperationKind::ElementViewRead {
                                index,
                                obligation,
                                ..
                            }
                            | terminal_psi::OperationKind::ElementViewWrite {
                                index,
                                obligation,
                                ..
                            } => (*index, *obligation),
                            _ => return None,
                        };
                        let mut selected = descriptor.path.to_vec();
                        if let Some(field) = descriptor.field {
                            let field = crate::projected_field_declaration(
                                types.iter(),
                                root_type?,
                                descriptor.path,
                                field,
                            )?;
                            selected.push(StructuralPathSegment::Field(field.identity));
                        }
                        selected.push(StructuralPathSegment::RuntimeIndex { index, obligation });
                        selected.extend_from_slice(path);
                        descriptor_contents_query_with(
                            types,
                            descriptor.root,
                            root_type?,
                            &selected,
                            None,
                            &select,
                        )
                    })();
                    (descriptor.root, query)
                }
                StructuralStorageUse::Type(_) => unreachable!("a type is not an operation operand"),
            };
            // A byte descriptor's address does not identify the bytes an
            // instruction observes. Until its index/window and backing are
            // represented here, neither a header nor a payload query can be
            // used as an exact byte access. Ordinary liveness is unchanged.
            let query = query.filter(|query| {
                let Some(root_type) = root_type else {
                    return false;
                };
                query
                    .selected_type_using(types, root_type, |carrier, path, _| {
                        let source_path = descriptor_path(types, carrier, path)?;
                        let selected = descriptor_location_type(types, carrier, path);
                        if selected.is_none()
                            && crate::primitive_read_projection_type(
                                types.iter(),
                                carrier,
                                &source_path,
                            )
                            .is_none()
                        {
                            return None;
                        }
                        Some(selected)
                    })
                    .is_some_and(|selected| {
                        selected.is_none_or(|selected| {
                            terminal_psi::find_structural_type(types, selected).is_some_and(
                                |declaration| {
                                    !matches!(
                                        declaration.shape,
                                        StructuralTypeShape::ByteSequence(_)
                                    )
                                },
                            )
                        })
                    })
            });
            DescriptorFlowSource::from_query(root, query)
        }

        fn descriptor_contents_query_with(
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            root_type: StructuralTypeId,
            path: &[StructuralPathSegment],
            final_field: Option<semantic_vocabulary::StructuralFieldId>,
            select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
        ) -> Option<DescriptorContentsQuery> {
            let mut query = DescriptorContentsQuery {
                root,
                steps: Vec::new(),
            };
            let mut tip = root_type;
            let mut static_path = Vec::new();
            for segment in path {
                if let StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    let prefix = descriptor_location(types, root, tip, &static_path, None)?;
                    tip = descriptor_location_type(types, tip, &prefix.path)?;
                    query
                        .steps
                        .extend(prefix.path.into_iter().map(DescriptorContentsStep::Address));
                    let declaration = terminal_psi::find_structural_type(types, tip)?;
                    let element = match declaration.shape {
                        StructuralTypeShape::FixedArray { element, .. }
                        | StructuralTypeShape::ElementView { element, .. } => element,
                        _ => return None,
                    };
                    let selector = select(*index);
                    let fixed = inline_selected_index(&declaration.shape, selector.as_ref());
                    query.steps.push(match fixed {
                        Some(index) => DescriptorContentsStep::Address(
                            DescriptorPathSegment::FixedIndex(index),
                        ),
                        None => DescriptorContentsStep::Element {
                            element,
                            selector: selector.map(Box::new),
                        },
                    });
                    tip = element;
                    static_path.clear();
                } else {
                    static_path.push(segment.clone());
                }
            }
            let suffix = descriptor_location(types, root, tip, &static_path, final_field)?;
            query
                .steps
                .extend(suffix.path.into_iter().map(DescriptorContentsStep::Address));
            query.validate(types, root_type)?;
            Some(query)
        }

        pub(crate) fn inline_selected_index(
            shape: &StructuralTypeShape,
            selector: Option<&semantic_vocabulary::ScalarTerm>,
        ) -> Option<u64> {
            let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(extent),
                ..
            } = shape
            else {
                return None;
            };
            crate::runtime_index_selects(selector?.integer_value()?.1, *extent)
        }

        // Effects cross machine boundaries only in terms of stable scalar
        // inputs. A read of local storage or a renewed block argument cannot
        // accompany the descriptor into the caller.
        pub fn entry_selector(
            term: semantic_vocabulary::ScalarTerm,
            is_parameter: impl Fn(semantic_vocabulary::ValueId) -> bool,
        ) -> Option<semantic_vocabulary::ScalarTerm> {
            use semantic_vocabulary::ScalarTerm;
            let mut pending = vec![&term];
            while let Some(value) = pending.pop() {
                match value {
                    ScalarTerm::Value { id, .. } if is_parameter(*id) => (),
                    ScalarTerm::Value { .. }
                    | ScalarTerm::Application { .. }
                    | ScalarTerm::StructuralRead { .. }
                    | ScalarTerm::BooleanField { .. }
                    | ScalarTerm::IntegerField { .. }
                    | ScalarTerm::IeeeFloatField { .. }
                    | ScalarTerm::ViewExtent { .. } => return None,
                    _ => pending.extend(value.operands()),
                }
            }
            Some(term)
        }

        pub fn descriptor_scalar_arguments(
            formals: impl ExactSizeIterator<Item = (ValueId, semantic_vocabulary::ScalarType)>,
            actuals: &[ValueId],
            expression: impl Fn(ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
        ) -> Vec<(ValueId, semantic_vocabulary::ScalarTerm)> {
            if formals.len() != actuals.len() {
                return Vec::new();
            }
            let mut bindings = formals
                .zip(actuals)
                .filter_map(|((formal, scalar_type), actual)| {
                    let term = expression(*actual)?;
                    (term.scalar_type() == scalar_type).then_some((formal, term))
                })
                .collect::<Vec<_>>();
            bindings.sort_unstable_by_key(|(formal, _)| *formal);
            if bindings.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Vec::new();
            }
            bindings
        }

        pub struct DescriptorStoreFlow<'a> {
            pub position: crate::descriptor_lifetimes::DescriptorPosition,
            pub transfers: &'a mut std::collections::BTreeMap<
                crate::descriptor_lifetimes::DescriptorPosition,
                Vec<DescriptorContentsTransfer>,
            >,
            pub unresolved:
                &'a mut std::collections::BTreeSet<crate::descriptor_lifetimes::DescriptorPosition>,
        }

        pub fn descriptor_leaf_store(
            DescriptorStoreFlow {
                position,
                transfers,
                unresolved,
            }: DescriptorStoreFlow<'_>,
            step: &mut crate::descriptor_lifetimes::DescriptorFlowStep,
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            destination: DescriptorContentsOperand<'_>,
            source: PlaceId,
            select: impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarTerm>,
        ) -> Option<()> {
            let from = DescriptorLocation::whole(source);
            let query = descriptor_contents_query_with(
                types,
                destination.root,
                destination.root_type,
                destination.path,
                destination.field,
                select,
            );
            let location = query
                .as_ref()
                .and_then(DescriptorContentsQuery::exact_location)
                .or_else(|| {
                    descriptor_location(
                        types,
                        destination.root,
                        destination.root_type,
                        destination.path,
                        destination.field,
                    )
                });
            if let Some(location) = location {
                step.definitions.push(location.clone());
                step.derivations.push((from.clone(), location));
            } else {
                step.may_escape = true;
                if let Some(query) = query.filter(|query| {
                    query.retains_selector()
                        && query
                            .inline_steps(types, destination.root_type)
                            .is_some_and(|steps| steps.iter().all(|inline| *inline))
                }) {
                    transfers
                        .entry(position)
                        .or_default()
                        .push(DescriptorContentsTransfer {
                            source: DescriptorContentsQuery::from_location(&from),
                            destination: query,
                            removes_source: false,
                        });
                } else {
                    unresolved.insert(position);
                    step.definitions.extend(slot_kind.overwritten_slots(
                        types,
                        destination.root,
                        destination.root_type,
                        destination.path,
                        destination.field,
                    )?);
                }
            }
            step.replace_value_inputs([from]);
            Some(())
        }

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum DescriptorPayloadTarget {
            Storage {
                location: DescriptorLocation,
                /// An immutable constructor keeps the value captured at its
                /// completed cut. This is not permission to write its current
                /// descriptor or to borrow a caller-selected result source.
                completed: Option<(MachineId, DescriptorPosition)>,
            },
            Incoming(DescriptorContentsQuery),
        }

        impl DescriptorPayloadTarget {
            pub fn current(location: DescriptorLocation) -> Self {
                Self::Storage {
                    location,
                    completed: None,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct PayloadMembershipContract {
            pub scalar_observations: Vec<super::PayloadScalarObservation>,
            pub scalar_requirements: Vec<semantic_vocabulary::Proposition>,
            pub carrier: StructuralTypeId,
            pub qualifications: Vec<semantic_vocabulary::StructuralDomainId>,
            pub projected_qualifications: Vec<StructuralPathQualification>,
        }

        impl PayloadMembershipContract {
            pub fn presented(
                mut self,
                types: &[StructuralTypeDeclaration],
                domains: &[terminal_psi::StructuralDomainDeclaration],
                destination: StructuralTypeId,
            ) -> Option<Self> {
                if crate::borrowed_referent_presentation(types.iter(), self.carrier, destination)
                    != Some(crate::BorrowedReferentPresentation::Unchanged)
                {
                    return None;
                }
                crate::present_structural_memberships(
                    types.iter(),
                    domains.iter(),
                    destination,
                    &mut self.qualifications,
                    &mut self.projected_qualifications,
                )
                .ok()?;
                self.carrier = destination;
                Some(self)
            }
        }

        pub(crate) fn select_payload_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            contract: &PayloadMembershipContract,
            address: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
        ) -> Option<PayloadMembershipContract> {
            let path = descriptor_path(types, contract.carrier, address)?;
            let carrier = crate::qualification_path_carrier(types.iter(), contract.carrier, &path)?;
            let (mut qualifications, projected_qualifications) =
                StructuralPathQualification::select(
                    &contract.qualifications,
                    &contract.projected_qualifications,
                    &path,
                );
            if let Some(predicates) =
                crate::shared_byte_field_copy_predicates(types.iter(), contract.carrier, &path)
            {
                qualifications.extend(byte_predicate_memberships(domains, carrier, predicates));
                qualifications.sort();
                qualifications.dedup();
            }
            let mut selected = PayloadMembershipContract {
                scalar_requirements: contract.scalar_requirements.clone(),
                scalar_observations: contract
                    .scalar_observations
                    .iter()
                    .filter_map(|observation| {
                        Some(super::PayloadScalarObservation {
                            path: observation.path.strip_prefix(address)?.to_vec(),
                            value: observation.value.clone(),
                            entry: observation.entry.clone(),
                        })
                    })
                    .collect(),
                carrier,
                qualifications,
                projected_qualifications,
            };
            super::payload_predicates::close(types, domains, &mut selected);
            Some(selected)
        }

        pub fn payload_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            targets: impl IntoIterator<Item = DescriptorPayloadTarget>,
            mut source: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Option<PayloadMembershipContract> {
            let mut common: Option<PayloadMembershipContract> = None;
            for target in targets {
                let mut selected = match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => select_payload_memberships(
                        types,
                        domains,
                        &source(location.root)?,
                        &location.path,
                    )?,
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => return None,
                    DescriptorPayloadTarget::Incoming(query) => {
                        let contract = source(query.root)?;
                        query.validate(types, contract.carrier)?;
                        incoming_memberships::select(types, domains, contract, &query.steps)?
                    }
                };
                let carrier = selected.carrier;
                close_predicate_memberships(types, domains, &mut selected);
                if let Some(common) = &mut common {
                    if common.carrier != carrier {
                        return None;
                    }
                    common
                        .scalar_requirements
                        .retain(|premise| selected.scalar_requirements.contains(premise));
                    super::PayloadScalarObservation::intersect(
                        &mut common.scalar_observations,
                        &selected.scalar_observations,
                    );
                    common
                        .qualifications
                        .retain(|domain| selected.qualifications.contains(domain));
                    common
                        .projected_qualifications
                        .retain(|row| selected.projected_qualifications.contains(row));
                } else {
                    common = Some(selected);
                }
            }
            common
        }

        pub fn reference_backing_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            identities: &terminal_psi::TerminalIdentityTable,
            analysis: &mut DescriptorAnalysis<'_>,
            machine: MachineId,
            position: DescriptorPosition,
            source: &StructuralArgument,
            storage: Option<&terminal_psi::ReferenceStorageView>,
            result: &terminal_psi::StructuralOperationResult,
            mut source_contract: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<Vec<StructuralPathQualification>, &'static str> {
            let StructuralTypeShape::Reference { referent, access } =
                terminal_psi::find_structural_type(types, result.structural_type)
                    .ok_or("reference result has no declared carrier")?
                    .shape
            else {
                return Err("reference result has no reference carrier");
            };
            if storage.is_some() {
                return Err("reference backing requires its explicit storage interpretation");
            }
            if access != source.access || source.place == result.place {
                return Err("reference backing has no independent matching source");
            }
            let root = source_contract(source.place)
                .ok_or("reference backing has no source declaration")?;
            let path =
                crate::reference_source_storage_path(types.iter(), root.carrier, &source.path);
            let query = descriptor_contents_query(types, source.place, root.carrier, path, None)
                .ok_or("reference backing has no typed contents query")?;
            let targets = analysis
                .payload_targets(machine, position, &query)
                .ok_or("reference backing has an unresolved source alternative")?;
            reference_payload_memberships(
                types,
                domains,
                identities,
                targets,
                referent,
                access,
                |place| {
                    (place != result.place)
                        .then(|| source_contract(place))
                        .flatten()
                },
            )
        }

        fn reference_payload_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            identities: &terminal_psi::TerminalIdentityTable,
            targets: impl IntoIterator<Item = DescriptorPayloadTarget>,
            referent: StructuralTypeId,
            access: StructuralAccess,
            mut source: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<Vec<StructuralPathQualification>, &'static str> {
            let mut common: Option<Vec<StructuralPathQualification>> = None;
            for target in targets {
                let query = match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => DescriptorContentsQuery::from_location(&location),
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => {
                        return Err(
                            "captured constructor contents do not grant live reference memberships",
                        );
                    }
                    DescriptorPayloadTarget::Incoming(query) => query,
                };
                let contract =
                    source(query.root).ok_or("reference payload has no source declaration")?;
                query
                    .validate(types, contract.carrier)
                    .ok_or("reference payload has no typed source query")?;
                let path = contents_membership_path(types, contract.carrier, &query.steps)
                    .ok_or("reference payload has no declared membership path")?;
                let selected =
                    crate::qualification_path_carrier(types.iter(), contract.carrier, &path)
                        .ok_or("reference payload has no declared selected carrier")?;
                if crate::borrowed_referent_presentation(types.iter(), selected, referent)
                    != Some(crate::BorrowedReferentPresentation::Unchanged)
                {
                    return Err("reference payload changes its selected source carrier");
                }
                if !crate::reference_preserves_ancestor_qualifications(
                    types.iter(),
                    domains.iter(),
                    identities,
                    &contract.qualifications,
                    &contract.projected_qualifications,
                    &path,
                    access,
                ) {
                    return Err("reference payload loses an ancestor membership obligation");
                }
                let captured = crate::reference_source_memberships(
                    types.iter(),
                    contract.carrier,
                    &contract.qualifications,
                    &contract.projected_qualifications,
                    &path,
                    access,
                )?;
                let (mut whole, mut projected) = StructuralPathQualification::select(
                    &[],
                    &captured,
                    &[StructuralPathSegment::Referent],
                );
                crate::present_structural_memberships(
                    types.iter(),
                    domains.iter(),
                    referent,
                    &mut whole,
                    &mut projected,
                )?;
                let mut captured = Vec::new();
                StructuralPathQualification::append_under(
                    &whole,
                    &projected,
                    &[StructuralPathSegment::Referent],
                    &mut captured,
                );
                captured.sort();
                captured.dedup();
                if let Some(common) = &mut common {
                    if access != StructuralAccess::SharedBorrow && *common != captured {
                        return Err(
                            "reference payload cannot retain every source membership obligation",
                        );
                    }
                    common.retain(|row| captured.binary_search(row).is_ok());
                } else {
                    common = Some(captured);
                }
            }
            common.ok_or("reference payload has no source candidate")
        }

        pub(crate) fn contents_membership_path(
            types: &[StructuralTypeDeclaration],
            mut carrier: StructuralTypeId,
            steps: &[DescriptorContentsStep],
        ) -> Option<Vec<StructuralPathSegment>> {
            let mut path = Vec::new();
            let mut address = Vec::new();
            for step in steps {
                match step {
                    DescriptorContentsStep::Address(segment) => address.push(*segment),
                    DescriptorContentsStep::Element { element, .. } => {
                        let prefix = descriptor_path(types, carrier, &address)?;
                        path.extend(prefix.iter().cloned());
                        carrier =
                            crate::qualification_path_carrier(types.iter(), carrier, &prefix)?;
                        match terminal_psi::find_structural_type(types, carrier)?.shape {
                            StructuralTypeShape::FixedArray {
                                element: actual, ..
                            }
                            | StructuralTypeShape::ElementView {
                                element: actual, ..
                            } if actual == *element => {}
                            _ => return None,
                        }
                        path.push(StructuralPathSegment::AllElements);
                        carrier = *element;
                        address.clear();
                    }
                }
            }
            path.extend(descriptor_path(types, carrier, &address)?);
            Some(path)
        }

        fn close_predicate_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            contract: &mut PayloadMembershipContract,
        ) {
            super::payload_predicates::close(types, domains, contract);
            admit_declared_scalar_refinement(types, domains, contract);
            contract
                .qualifications
                .extend(implied_predicate_memberships(
                    types,
                    domains,
                    contract.carrier,
                    &contract.qualifications,
                ));
            contract.qualifications.sort();
            contract.qualifications.dedup();
            let mut paths: Vec<_> = contract
                .projected_qualifications
                .iter()
                .map(|row| row.path.clone())
                .collect();
            paths.sort();
            paths.dedup();
            for path in paths {
                let Some(carrier) =
                    crate::qualification_path_carrier(types.iter(), contract.carrier, &path)
                else {
                    continue;
                };
                let whole: Vec<_> = contract
                    .projected_qualifications
                    .iter()
                    .filter(|row| row.path == path)
                    .map(|row| row.domain)
                    .collect();
                for domain in implied_predicate_memberships(types, domains, carrier, &whole) {
                    contract
                        .projected_qualifications
                        .push(StructuralPathQualification {
                            path: path.clone(),
                            domain,
                        });
                }
            }
            contract.projected_qualifications.sort();
            contract.projected_qualifications.dedup();
        }

        fn admit_declared_scalar_refinement(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            contract: &mut PayloadMembershipContract,
        ) {
            let Some(refinement) = types
                .iter()
                .find(|row| row.id == contract.carrier)
                .and_then(|row| match &row.scalar_restriction {
                    Some(terminal_psi::StructuralFieldType::RefinedScalar(refinement)) => {
                        Some(refinement)
                    }
                    _ => None,
                })
            else {
                return;
            };
            for domain in domains {
                if domain.carrier != contract.carrier
                    || domain.content_projection.is_some()
                    || domain.establishment_routes.is_some()
                    || contract.qualifications.contains(&domain.id)
                {
                    continue;
                }
                let Some(predicate) =
                    introduction_predicate(types, domains, domain, &contract.qualifications)
                else {
                    continue;
                };
                let mut proposition = predicate.proposition.clone();
                rewrite_subject_as_refinement_value(
                    &mut proposition,
                    predicate.subject,
                    refinement.carrier(),
                );
                normalize_boolean_comparison(&mut proposition);
                if proposition.same_commutative_predicate(refinement.predicate()) {
                    contract.qualifications.push(domain.id);
                }
            }
        }

        fn rewrite_subject_as_refinement_value(
            proposition: &mut semantic_vocabulary::Proposition,
            subject: semantic_vocabulary::PlaceId,
            carrier: ScalarType,
        ) {
            fn rewrite_term(
                term: &mut semantic_vocabulary::ScalarTerm,
                subject: semantic_vocabulary::PlaceId,
                carrier: ScalarType,
            ) {
                use semantic_vocabulary::ScalarTerm;
                let replacement = match term {
                    ScalarTerm::IntegerField {
                        version: semantic_vocabulary::ContentPlaceVersion::Current,
                        root,
                        path,
                        scalar_type,
                    } if *root == subject
                        && path.is_empty()
                        && ScalarType::Integer(*scalar_type) == carrier =>
                    {
                        Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
                    }
                    ScalarTerm::BooleanField {
                        version: semantic_vocabulary::ContentPlaceVersion::Current,
                        root,
                        path,
                    } if *root == subject && path.is_empty() && carrier == ScalarType::Boolean => {
                        Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
                    }
                    ScalarTerm::IeeeFloatField {
                        version: semantic_vocabulary::ContentPlaceVersion::Current,
                        root,
                        path,
                        format,
                    } if *root == subject
                        && path.is_empty()
                        && carrier == ScalarType::IeeeFloat(*format) =>
                    {
                        Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
                    }
                    ScalarTerm::StructuralRead {
                        root,
                        path,
                        scalar_type,
                    } if *root == subject && path.is_empty() && *scalar_type == carrier => {
                        Some(ScalarTerm::value(ScalarRefinement::SUBJECT, carrier))
                    }
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    *term = replacement;
                    return;
                }
                for operand in term.operands_mut() {
                    rewrite_term(operand, subject, carrier);
                }
            }
            fn rewrite_math(
                term: &mut semantic_vocabulary::IntegerMathTerm,
                subject: semantic_vocabulary::PlaceId,
                carrier: ScalarType,
            ) {
                if let semantic_vocabulary::IntegerMathTerm::Embed(inner) = term {
                    rewrite_term(inner, subject, carrier);
                }
                for operand in term.operands_mut() {
                    rewrite_math(operand, subject, carrier);
                }
            }
            fn rewrite_proposition(
                proposition: &mut semantic_vocabulary::Proposition,
                subject: semantic_vocabulary::PlaceId,
                carrier: ScalarType,
            ) {
                use semantic_vocabulary::Proposition;
                match proposition {
                    semantic_vocabulary::scalar_comparison_operands!(left, right) => {
                        rewrite_term(left, subject, carrier);
                        rewrite_term(right, subject, carrier);
                    }
                    semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                        rewrite_math(left, subject, carrier);
                        rewrite_math(right, subject, carrier);
                    }
                    Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                        for child in children {
                            rewrite_proposition(child, subject, carrier);
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        rewrite_proposition(premise, subject, carrier);
                        rewrite_proposition(conclusion, subject, carrier);
                    }
                    _ => {}
                }
            }
            rewrite_proposition(proposition, subject, carrier);
        }

        fn normalize_boolean_comparison(proposition: &mut semantic_vocabulary::Proposition) {
            use semantic_vocabulary::{Proposition, ScalarTerm};
            match proposition {
                Proposition::Equal(left, right) => {
                    let comparison = match (&*left, &*right) {
                        (ScalarTerm::Boolean(true), comparison)
                        | (comparison, ScalarTerm::Boolean(true)) => comparison.clone(),
                        _ => return,
                    };
                    let Some(normalized) = (match &comparison {
                        ScalarTerm::IntegerEqual { left, right, .. } => Some(Proposition::Equal(
                            left.as_ref().clone(),
                            right.as_ref().clone(),
                        )),
                        ScalarTerm::IntegerLessThan { left, right, .. } => Some(
                            Proposition::LessThan(left.as_ref().clone(), right.as_ref().clone()),
                        ),
                        ScalarTerm::IntegerLessOrEqual { left, right, .. } => Some(
                            Proposition::LessOrEqual(left.as_ref().clone(), right.as_ref().clone()),
                        ),
                        _ => None,
                    }) else {
                        return;
                    };
                    *proposition = normalized;
                }
                Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                    for child in children {
                        normalize_boolean_comparison(child);
                    }
                }
                Proposition::Implication {
                    premise,
                    conclusion,
                } => {
                    normalize_boolean_comparison(premise);
                    normalize_boolean_comparison(conclusion);
                }
                _ => {}
            }
        }

        fn implied_predicate_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            carrier: StructuralTypeId,
            qualifications: &[semantic_vocabulary::StructuralDomainId],
        ) -> Vec<semantic_vocabulary::StructuralDomainId> {
            let mut implied = domains
                .iter()
                .filter(|domain| {
                    domain.carrier == carrier
                        && (domain
                            .missing_membership_requirements(domains, qualifications)
                            .is_some_and(|missing| missing.is_empty())
                            || introduction_predicate(types, domains, domain, qualifications)
                                .is_some_and(|predicate| {
                                    predicate.proposition == semantic_vocabulary::Proposition::Truth
                                }))
                })
                .map(|domain| domain.id)
                .collect::<Vec<_>>();
            if !crate::byte_predicate_carrier(types.iter(), carrier) {
                return implied;
            }
            let mut supplied = terminal_psi::ByteSequencePredicateSet::EMPTY;
            for domain in qualifications {
                if let Some(predicates) =
                    crate::byte_qualification_predicates_in(domains.iter(), carrier, &[*domain])
                {
                    for predicate in predicates.predicates() {
                        supplied.insert(predicate);
                    }
                }
            }
            implied.extend(byte_predicate_memberships(domains, carrier, supplied));
            implied.sort();
            implied.dedup();
            implied
        }

        fn byte_predicate_memberships(
            domains: &[terminal_psi::StructuralDomainDeclaration],
            carrier: StructuralTypeId,
            supplied: terminal_psi::ByteSequencePredicateSet,
        ) -> Vec<semantic_vocabulary::StructuralDomainId> {
            domains
                .iter()
                .filter(|domain| {
                    crate::byte_qualification_predicates_in(domains.iter(), carrier, &[domain.id])
                        .is_some_and(|required| supplied.implies(required))
                })
                .map(|domain| domain.id)
                .collect()
        }

        pub fn terminal_payload_contract(
            machine: &TerminalMachine,
            place: PlaceId,
        ) -> Option<PayloadMembershipContract> {
            if let Some(parameter) = crate::arrival_parameter(machine, place) {
                return Some(PayloadMembershipContract {
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    carrier: parameter.structural_type,
                    qualifications: parameter.qualifications.clone(),
                    projected_qualifications: parameter.projected_qualifications.clone(),
                });
            }
            for operation in machine.operations() {
                if let Some(result) = operation.result.structural()
                    && result.place == place
                {
                    return Some(PayloadMembershipContract {
                        scalar_observations: Vec::new(),
                        scalar_requirements: Vec::new(),
                        carrier: result.structural_type,
                        qualifications: result.qualifications.clone(),
                        projected_qualifications: result.projected_qualifications.clone(),
                    });
                }
                if let OperationKind::EstablishByteSequenceLiteral {
                    destination,
                    qualifications,
                    ..
                } = &operation.kind
                    && *destination == place
                {
                    return Some(PayloadMembershipContract {
                        scalar_observations: Vec::new(),
                        scalar_requirements: Vec::new(),
                        carrier: machine
                            .structural_place(place)?
                            .kind
                            .produced_structural_type()?,
                        qualifications: qualifications.clone(),
                        projected_qualifications: Vec::new(),
                    });
                }
            }
            None
        }

        #[derive(Clone, Debug)]
        pub enum ElementPayloadMemberships {
            Empty,
            Present(PayloadMembershipContract),
        }

        #[allow(clippy::too_many_arguments)]
        pub fn element_payload_memberships(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            machine: &TerminalMachine,
            analysis: &mut DescriptorAnalysis<'_>,
            position: DescriptorPosition,
            source: &StructuralArgument,
            element: StructuralTypeId,
        ) -> Option<ElementPayloadMemberships> {
            element_payload_memberships_in(
                types,
                domains,
                machine.id,
                analysis,
                position,
                source,
                element,
                |place| terminal_payload_contract(machine, place),
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub fn element_payload_memberships_in(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            machine: MachineId,
            analysis: &mut DescriptorAnalysis<'_>,
            position: DescriptorPosition,
            source: &StructuralArgument,
            element: StructuralTypeId,
            mut source_contract: impl FnMut(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Option<ElementPayloadMemberships> {
            let root = source_contract(source.place)?;
            let mut query =
                descriptor_contents_query(types, source.place, root.carrier, &source.path, None)?;
            query
                .steps
                .push(DescriptorContentsStep::any_element(element));
            query.validate(types, root.carrier)?;
            let targets = analysis.payload_targets(machine, position, &query)?;
            if targets.is_empty() {
                return Some(ElementPayloadMemberships::Empty);
            }
            let contract = payload_memberships(types, domains, targets, source_contract)?;
            (contract.carrier == element).then_some(ElementPayloadMemberships::Present(contract))
        }
    }
    pub(crate) mod payload_entry {
        use super::*;
        use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, Proposition};
        use terminal_psi::{ProofNode, ProofRule, StructuralDomainDeclaration};

        pub(crate) mod order {
            use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};
            use terminal_psi::{PrimitiveJudgment, ProofNode, ProofRule};

            pub(crate) fn derive(
                context: &PropositionContext,
                goal: &Proposition,
                premise: &ProofNode,
                assumptions: usize,
            ) -> Option<ProofNode> {
                if matches!(
                    (&premise.conclusion, goal),
                    (Proposition::LessOrEqual(..), Proposition::LessThan(..))
                        | (Proposition::LessThan(..), Proposition::LessOrEqual(..))
                ) {
                    let conversion = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(ProofNode {
                                conclusion: premise.conclusion.clone(),
                                rule: ProofRule::Assumption { index: 0 },
                            }),
                        },
                    };
                    if proof_admission::check_certificate(
                        context,
                        goal,
                        std::slice::from_ref(&premise.conclusion),
                        &[],
                        &conversion,
                    )
                    .is_ok()
                    {
                        return Some(ProofNode {
                            conclusion: goal.clone(),
                            rule: ProofRule::IntegerOrderDiscreteness {
                                relation: Box::new(premise.clone()),
                            },
                        });
                    }
                }
                let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
                    return None;
                };
                proof_admission::check_predicate_evaluation_size([goal, &premise.conclusion])
                    .ok()?;
                let mut relation = match &premise.conclusion {
                    Proposition::IntegerMathLessOrEqual(..) => premise.clone(),
                    Proposition::LessOrEqual(..) => super::bridge(
                        proof_admission::lift_fixed_integer_relation(&premise.conclusion)?,
                        premise.clone(),
                        assumptions,
                    ),
                    _ => return None,
                };
                if let Some(Proposition::IntegerMathLessOrEqual(left, right)) =
                    proof_admission::lift_exact_integer_order(&premise.conclusion)
                {
                    relation = replace_endpoint(context, relation, left, 0)?;
                    relation = replace_endpoint(context, relation, right, 1)?;
                }
                let Proposition::IntegerMathLessOrEqual(left, right) = &relation.conclusion else {
                    return None;
                };
                let addend =
                    IntegerMathTerm::Subtract(Box::new(goal_left.clone()), Box::new(left.clone()));
                let translated_left =
                    IntegerMathTerm::Add(Box::new(left.clone()), Box::new(addend.clone()));
                let translated_right =
                    IntegerMathTerm::Add(Box::new(right.clone()), Box::new(addend.clone()));
                let upper = primitive(
                    context,
                    Proposition::IntegerMathLessOrEqual(
                        translated_right.clone(),
                        goal_right.clone(),
                    ),
                )?;
                relation = ProofNode {
                    conclusion: Proposition::IntegerMathLessOrEqual(
                        translated_left,
                        translated_right,
                    ),
                    rule: ProofRule::IntegerMathOrderTranslation {
                        relation: Box::new(relation),
                        addend,
                    },
                };
                relation = replace_endpoint(context, relation, goal_left.clone(), 0)?;
                Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(relation),
                        middle_less_or_equal_right: Box::new(upper),
                    },
                })
            }

            fn replace_endpoint(
                context: &PropositionContext,
                relation: ProofNode,
                replacement: IntegerMathTerm,
                endpoint: usize,
            ) -> Option<ProofNode> {
                let Proposition::IntegerMathLessOrEqual(left, right) = &relation.conclusion else {
                    return None;
                };
                let original = if endpoint == 0 { left } else { right };
                if *original == replacement {
                    return Some(relation);
                }
                let (equality_left, equality_right) = if original <= &replacement {
                    (original.clone(), replacement.clone())
                } else {
                    (replacement.clone(), original.clone())
                };
                let equality = primitive(
                    context,
                    Proposition::IntegerMathEqual(equality_left, equality_right),
                )?;
                let conclusion = if endpoint == 0 {
                    Proposition::IntegerMathLessOrEqual(replacement, right.clone())
                } else {
                    Proposition::IntegerMathLessOrEqual(left.clone(), replacement)
                };
                Some(ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(equality),
                        endpoint,
                    },
                })
            }

            fn primitive(
                context: &PropositionContext,
                conclusion: Proposition,
            ) -> Option<ProofNode> {
                proof_admission::decide_primitive(
                    context,
                    &conclusion,
                    PrimitiveJudgment::ClosedIntegerRelation,
                )
                .ok()?;
                Some(ProofNode {
                    conclusion,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                })
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use semantic_vocabulary::{
                    IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
                };

                #[test]
                fn exact_range_rearrangement_retains_subject_policy_and_premise() {
                    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                    let context = PropositionContext::from_value_types((1..=3).map(|identity| {
                        (
                            ValueId::new(identity).unwrap(),
                            ScalarType::Integer(integer),
                        )
                    }))
                    .unwrap();
                    let value = |identity| {
                        ScalarTerm::value(
                            ValueId::new(identity).unwrap(),
                            ScalarType::Integer(integer),
                        )
                    };
                    let maximum =
                        ScalarTerm::integer(integer, IntegerValue::Unsigned(u64::MAX.into()))
                            .unwrap();
                    let difference = ScalarTerm::ExactIntegerSubtract {
                        scalar_type: integer,
                        left: Box::new(maximum.clone()),
                        right: Box::new(value(2)),
                    };
                    let premise = Proposition::LessOrEqual(value(1), difference);
                    let goal = Proposition::IntegerMathLessOrEqual(
                        IntegerMathTerm::Add(
                            Box::new(IntegerMathTerm::embed(value(1)).unwrap()),
                            Box::new(IntegerMathTerm::embed(value(2)).unwrap()),
                        ),
                        IntegerMathTerm::embed(maximum.clone()).unwrap(),
                    );
                    let proof =
                        super::super::prove(&context, &goal, std::slice::from_ref(&premise), 0)
                            .unwrap();
                    proof_admission::check_certificate(
                        &context,
                        &goal,
                        std::slice::from_ref(&premise),
                        &[],
                        &proof,
                    )
                    .unwrap();
                    proof_admission::verify_bounded_certificate(
                        &context,
                        &goal,
                        std::slice::from_ref(&premise),
                        &[],
                        &proof,
                        &mut proof_admission::Budget::default(),
                    )
                    .unwrap();
                    assert!(
                        proof_admission::check_certificate(&context, &goal, &[], &[], &proof)
                            .is_err()
                    );
                    let wrong_subject = Proposition::LessOrEqual(
                        value(3),
                        match &premise {
                            Proposition::LessOrEqual(_, right) => right.clone(),
                            _ => unreachable!(),
                        },
                    );
                    assert!(super::super::prove(&context, &goal, &[wrong_subject], 0).is_none());
                    let wrapping = Proposition::LessOrEqual(
                        value(1),
                        ScalarTerm::WrappingIntegerSubtract {
                            scalar_type: integer,
                            left: Box::new(maximum),
                            right: Box::new(value(2)),
                        },
                    );
                    assert!(super::super::prove(&context, &goal, &[wrapping], 0).is_none());
                }
            }
        }

        pub(crate) fn close(
            types: &[StructuralTypeDeclaration],
            domains: &[StructuralDomainDeclaration],
            machine: &DescriptorMachine,
            root: PlaceId,
            contract: &mut PayloadMembershipContract,
        ) {
            let Some((context, requirements)) = &machine.payload_entry_predicates else {
                return;
            };
            let mut pending = vec![(contract.carrier, Vec::new(), Vec::new(), 0usize)];
            let mut remaining = 4096usize;
            while let Some((carrier, path, storage, depth)) = pending.pop() {
                let Some(next) = remaining.checked_sub(1) else {
                    break;
                };
                remaining = next;
                for domain in domains.iter().filter(|domain| {
                    domain.carrier == carrier
                        && domain.content_projection.is_none()
                        && domain.establishment_routes.is_none()
                }) {
                    let (supplied, _) = terminal_psi::StructuralPathQualification::select(
                        &contract.qualifications,
                        &contract.projected_qualifications,
                        &storage,
                    );
                    let Some(predicate) = super::payload_predicates::introduction_predicate(
                        types, domains, domain, &supplied,
                    ) else {
                        continue;
                    };
                    let goal = crate::substitution::substitute_proposition_structural_places(
                        &predicate.proposition,
                        &BTreeMap::from([(predicate.subject, (root, path.clone()))]),
                    );
                    let Ok(checked) =
                        proof_admission::check_predicate_denotations_with_value_equalities(
                            context,
                            &goal,
                            requirements,
                            &[],
                        )
                    else {
                        continue;
                    };
                    let Some(proof) = prove(context, checked.goal(), checked.requirements(), 0)
                    else {
                        continue;
                    };
                    if checked.check_certificate(context, &proof).is_err() {
                        continue;
                    }
                    if storage.is_empty() {
                        contract.qualifications.push(domain.id);
                    } else {
                        contract.projected_qualifications.push(
                            terminal_psi::StructuralPathQualification {
                                path: storage.clone(),
                                domain: domain.id,
                            },
                        );
                    }
                }
                if depth >= 64 {
                    continue;
                }
                let Some(declaration) = terminal_psi::find_structural_type(types, carrier) else {
                    continue;
                };
                if let Some(fields) = declaration.shape.direct_fields() {
                    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                        if pending.len() >= remaining {
                            break;
                        }
                        if let terminal_psi::StructuralFieldType::Structural(child) =
                            field.field_type
                        {
                            let mut child_path = path.clone();
                            child_path.push(Segment::Field(field.id));
                            let mut child_storage = storage.clone();
                            child_storage.push(StructuralPathSegment::Field(field.identity));
                            pending.push((child, child_path, child_storage, depth + 1));
                        }
                    }
                }
                if let StructuralTypeShape::FixedArray {
                    element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = declaration.shape
                    && length <= 4096
                {
                    for index in 0..length {
                        if pending.len() >= remaining {
                            break;
                        }
                        let mut child_path = path.clone();
                        child_path.push(Segment::FixedIndex(index));
                        let mut child_storage = storage.clone();
                        child_storage.push(StructuralPathSegment::FixedIndex(index));
                        pending.push((element, child_path, child_storage, depth + 1));
                    }
                }
            }
            contract.qualifications.sort();
            contract.qualifications.dedup();
            contract.projected_qualifications.sort();
            contract.projected_qualifications.dedup();
        }

        pub(crate) fn prove(
            context: &semantic_vocabulary::PropositionContext,
            goal: &Proposition,
            requirements: &[Proposition],
            depth: usize,
        ) -> Option<ProofNode> {
            prove_with_budget(context, goal, requirements, depth, &mut 4096)
        }

        fn prove_with_budget(
            context: &semantic_vocabulary::PropositionContext,
            goal: &Proposition,
            requirements: &[Proposition],
            depth: usize,
            remaining: &mut usize,
        ) -> Option<ProofNode> {
            *remaining = remaining.checked_sub(1)?;
            if depth > 64 {
                return None;
            }
            if let Some(judgment) = [
                terminal_psi::PrimitiveJudgment::Truth,
                terminal_psi::PrimitiveJudgment::ReflexiveEquality,
                terminal_psi::PrimitiveJudgment::ClosedIntegerRelation,
                terminal_psi::PrimitiveJudgment::IntegerCarrierBound,
            ]
            .into_iter()
            .find(|judgment| proof_admission::decide_primitive(context, goal, *judgment).is_ok())
            {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(judgment),
                });
            }
            if let Some(lowered) = proof_admission::lower_integer_math_relation(goal)
                && let Some(proof) =
                    prove_with_budget(context, &lowered, requirements, depth + 1, remaining)
            {
                return Some(bridge(goal.clone(), proof, requirements.len()));
            }
            for (index, requirement) in requirements.iter().enumerate() {
                let proof = ProofNode {
                    conclusion: requirement.clone(),
                    rule: ProofRule::Assumption { index },
                };
                if let Some(proof) = select(
                    context,
                    goal,
                    proof,
                    requirements.len(),
                    depth + 1,
                    remaining,
                ) {
                    return Some(proof);
                }
            }
            if let Proposition::Conjunction(children) = goal {
                let proofs = children
                    .iter()
                    .map(|child| {
                        prove_with_budget(context, child, requirements, depth + 1, remaining)
                    })
                    .collect::<Option<Vec<_>>>()?;
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::ConjunctionIntroduction(proofs),
                });
            }
            if let Proposition::Disjunction(children) = goal {
                for (index, child) in children.iter().enumerate() {
                    if let Some(disjunct) =
                        prove_with_budget(context, child, requirements, depth + 1, remaining)
                    {
                        return Some(ProofNode {
                            conclusion: goal.clone(),
                            rule: ProofRule::DisjunctionIntroduction {
                                disjunct: Box::new(disjunct),
                                index,
                            },
                        });
                    }
                }
            }
            None
        }

        fn select(
            context: &semantic_vocabulary::PropositionContext,
            goal: &Proposition,
            proof: ProofNode,
            assumptions: usize,
            depth: usize,
            remaining: &mut usize,
        ) -> Option<ProofNode> {
            *remaining = remaining.checked_sub(1)?;
            if proof.conclusion == *goal {
                return Some(proof);
            }
            if depth > 64 {
                return None;
            }
            if proof_admission::lift_fixed_integer_relation(&proof.conclusion).as_ref()
                == Some(goal)
                || proof_admission::lower_integer_math_relation(&proof.conclusion).as_ref()
                    == Some(goal)
            {
                return Some(bridge(goal.clone(), proof, assumptions));
            }
            if let Some(derived) = order::derive(context, goal, &proof, assumptions) {
                return Some(derived);
            }
            if let Proposition::Conjunction(children) = &proof.conclusion {
                for (conjunct, child) in children.iter().enumerate() {
                    let selected = ProofNode {
                        conclusion: child.clone(),
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(proof.clone()),
                            conjunct,
                        },
                    };
                    if let Some(selected) =
                        select(context, goal, selected, assumptions, depth + 1, remaining)
                    {
                        return Some(selected);
                    }
                }
            }
            if let Proposition::Disjunction(children) = &proof.conclusion {
                let branches = children
                    .iter()
                    .map(|child| {
                        select(
                            context,
                            goal,
                            ProofNode {
                                conclusion: child.clone(),
                                rule: ProofRule::Assumption { index: assumptions },
                            },
                            assumptions + 1,
                            depth + 1,
                            remaining,
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(branches) = branches {
                    return Some(ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::DisjunctionElimination {
                            disjunction: Box::new(proof),
                            branches,
                        },
                    });
                }
            }
            if let Proposition::Disjunction(children) = goal {
                for (index, child) in children.iter().enumerate() {
                    if let Some(disjunct) = select(
                        context,
                        child,
                        proof.clone(),
                        assumptions,
                        depth + 1,
                        remaining,
                    ) {
                        return Some(ProofNode {
                            conclusion: goal.clone(),
                            rule: ProofRule::DisjunctionIntroduction {
                                disjunct: Box::new(disjunct),
                                index,
                            },
                        });
                    }
                }
            }
            None
        }

        fn bridge(goal: Proposition, proof: ProofNode, assumptions: usize) -> ProofNode {
            ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(ProofNode {
                        conclusion: Proposition::Implication {
                            premise: Box::new(proof.conclusion.clone()),
                            conclusion: Box::new(goal.clone()),
                        },
                        rule: ProofRule::ImplicationIntroduction {
                            body: Box::new(ProofNode {
                                conclusion: goal,
                                rule: ProofRule::Assumption { index: assumptions },
                            }),
                        },
                    }),
                    premise: Box::new(proof),
                },
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use proof_admission::check_certificate;
            use semantic_vocabulary::{PropositionContext, ScalarTerm, ScalarType, ValueId};

            #[test]
            fn disjunction_transport_checks_every_alternative_in_its_own_scope() {
                use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
                let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let values = [1, 2, 3].map(|index| ValueId::new(index).unwrap());
                let context = PropositionContext::from_value_types(
                    values.map(|value| (value, ScalarType::Integer(integer))),
                )
                .unwrap();
                let finite = values.map(|value| {
                    Proposition::LessThan(
                        ScalarTerm::value(value, ScalarType::Integer(integer)),
                        ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
                    )
                });
                let mathematical = finite.each_ref().map(|proposition| {
                    proof_admission::lift_fixed_integer_relation(proposition).unwrap()
                });
                let requirements = [Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::Disjunction(finite[..2].to_vec()),
                ])];
                let goal = Proposition::Disjunction(mathematical[..2].to_vec());
                let proof = prove(&context, &goal, &requirements, 0).unwrap();
                check_certificate(&context, &goal, &requirements, &[], &proof).unwrap();
                proof_admission::verify_bounded_certificate(
                    &context,
                    &goal,
                    &requirements,
                    &[],
                    &proof,
                    &mut proof_admission::Budget::default(),
                )
                .unwrap();
                assert!(prove(&context, &mathematical[0], &requirements, 0).is_none());
                let wrong = Proposition::Disjunction(vec![
                    mathematical[0].clone(),
                    mathematical[2].clone(),
                ]);
                assert!(prove(&context, &wrong, &requirements, 0).is_none());
            }

            #[test]
            fn disjunction_proof_uses_one_established_alternative_without_inventing_the_other() {
                let left = ValueId::new(1).unwrap();
                let right = ValueId::new(2).unwrap();
                let context = PropositionContext::from_value_types([
                    (left, ScalarType::Boolean),
                    (right, ScalarType::Boolean),
                ])
                .unwrap();
                let predicates = [left, right].map(|value| {
                    Proposition::Equal(
                        ScalarTerm::value(value, ScalarType::Boolean),
                        ScalarTerm::Boolean(true),
                    )
                });
                let goal = Proposition::Disjunction(predicates.to_vec());
                for predicate in &predicates {
                    let requirements = [predicate.clone()];
                    let checked =
                        proof_admission::check_predicate_denotations_with_value_equalities(
                            &context,
                            &goal,
                            &requirements,
                            &[],
                        )
                        .unwrap();
                    let proof = prove(&context, checked.goal(), checked.requirements(), 0).unwrap();
                    assert!(checked.check_certificate(&context, &proof).is_ok());
                }
                assert!(prove(&context, &goal, &[], 0).is_none());
                assert!(
                    prove(
                        &context,
                        &Proposition::Conjunction(predicates.to_vec()),
                        &predicates[..1],
                        0
                    )
                    .is_none()
                );
            }

            #[test]
            fn selected_finite_relation_uses_the_kernel_mathematical_citation_bridge() {
                use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
                let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let value = ValueId::new(1).unwrap();
                let other = ValueId::new(2).unwrap();
                let context = PropositionContext::from_value_types([
                    (value, ScalarType::Integer(integer)),
                    (other, ScalarType::Integer(integer)),
                ])
                .unwrap();
                let finite = Proposition::LessThan(
                    ScalarTerm::value(value, ScalarType::Integer(integer)),
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
                );
                let goal = proof_admission::lift_fixed_integer_relation(&finite).unwrap();
                let requirements = [Proposition::Conjunction(vec![
                    Proposition::Truth,
                    finite.clone(),
                ])];
                let proof = prove(&context, &goal, &requirements, 0).unwrap();
                assert!(check_certificate(&context, &goal, &requirements, &[], &proof).is_ok());
                assert!(prove(&context, &goal, &[], 0).is_none());
                let reverse_requirements = [goal];
                let reverse = prove(&context, &finite, &reverse_requirements, 0).unwrap();
                assert!(
                    check_certificate(&context, &finite, &reverse_requirements, &[], &reverse)
                        .is_ok()
                );
                let wrong = Proposition::LessThan(
                    ScalarTerm::value(other, ScalarType::Integer(integer)),
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap(),
                );
                assert!(prove(&context, &wrong, &reverse_requirements, 0).is_none());
            }
        }
    }
    pub(crate) use payload_entry::prove as prove_payload_predicate;
    pub(crate) mod payload_history {

        pub(crate) mod entry_snapshots {
            use super::super::payload_predicates::observation_type;
            use super::super::{PayloadEntryScalarIdentity, PayloadScalarObservation};
            use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
            use semantic_vocabulary::{PlaceId, StructuralTypeId};
            use terminal_psi::{
                StructuralAccess, StructuralParameterDeclaration, StructuralTypeDeclaration,
                StructuralTypeShape,
            };

            pub(crate) fn collect(
                types: &[StructuralTypeDeclaration],
                parameters: &[StructuralParameterDeclaration],
            ) -> Vec<(PlaceId, Vec<PayloadScalarObservation>)> {
                parameters
                    .iter()
                    .filter_map(|parameter| {
                        if parameter.access != StructuralAccess::Owned {
                            return None;
                        }
                        let snapshots =
                            for_parameter(types, parameter.place, parameter.structural_type)?;
                        Some((parameter.place, snapshots))
                    })
                    .collect()
            }

            fn for_parameter(
                types: &[StructuralTypeDeclaration],
                root: PlaceId,
                structural_type: StructuralTypeId,
            ) -> Option<Vec<PayloadScalarObservation>> {
                let mut pending = vec![(structural_type, Vec::new(), Vec::new())];
                let mut snapshots = Vec::new();
                let mut remaining = 4096usize;
                while let Some((carrier, path, ancestors)) = pending.pop() {
                    remaining = remaining.checked_sub(1)?;
                    if ancestors.len() > 64 || ancestors.contains(&carrier) {
                        return None;
                    }
                    let declaration = terminal_psi::find_structural_type(types, carrier)?;
                    let mut lineage = ancestors;
                    lineage.push(carrier);
                    let mut append = |path: Vec<DescriptorPathSegment>, scalar_type| {
                        snapshots.push(PayloadScalarObservation {
                            path: path.clone(),
                            value: None,
                            entry: Some(PayloadEntryScalarIdentity {
                                root,
                                path,
                                scalar_type,
                            }),
                        });
                    };
                    if let StructuralTypeShape::PrimitiveScalar(scalar_type) = declaration.shape
                        && observation_type(types, structural_type, &path) == Some(scalar_type)
                    {
                        append(path.clone(), scalar_type);
                    }
                    if let Some(fields) = declaration.shape.direct_fields() {
                        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                            let mut selected = path.clone();
                            selected.push(DescriptorPathSegment::Field(field.id));
                            if let Some(child) = field.field_type.as_structural() {
                                pending.push((child, selected, lineage.clone()));
                            } else if let Some(scalar_type) =
                                observation_type(types, structural_type, &selected)
                            {
                                append(selected, scalar_type);
                            }
                        }
                    }
                    if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
                        let length = length.literal()?;
                        if length > remaining as u64 {
                            return None;
                        }
                        for index in 0..length {
                            let mut selected = path.clone();
                            selected.push(DescriptorPathSegment::FixedIndex(index));
                            pending.push((element, selected, lineage.clone()));
                        }
                    }
                }
                Some(snapshots)
            }
        }
        pub(crate) mod premise_table {

            use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
            use std::borrow::Borrow;
            use std::collections::{BTreeMap, BTreeSet};
            use std::ops::Bound;
            use std::sync::Arc;
            use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

            use crate::substitution::{
                substitute_proposition_values, substitute_scalar_term_values,
            };

            const RANK_SPACING: u64 = 1 << 20;

            #[derive(Clone)]
            pub(crate) struct SharedPremise(Arc<PremiseRow>);

            struct PremiseRow {
                proposition: Proposition,
                values: Vec<ValueId>,
                closed: bool,
                substitution_stable: bool,
                rank: AtomicU64,
            }

            impl SharedPremise {
                fn rank(&self) -> u64 {
                    self.0.rank.load(AtomicOrdering::Relaxed)
                }

                pub(crate) fn mentions_any(&self, definitions: &[ValueId]) -> bool {
                    definitions
                        .iter()
                        .any(|value| self.0.values.binary_search(value).is_ok())
                }

                pub(crate) fn survives_payload_retirement(&self, definitions: &[ValueId]) -> bool {
                    self.0.closed && !self.mentions_any(definitions)
                }

                #[cfg(test)]
                pub(crate) fn new(proposition: Proposition) -> Self {
                    thread_local! {
                        static TABLE: std::cell::RefCell<PremiseTable> =
                            std::cell::RefCell::new(PremiseTable::default());
                    }
                    TABLE.with(|table| table.borrow_mut().intern(proposition))
                }
            }

            impl Borrow<Proposition> for SharedPremise {
                fn borrow(&self) -> &Proposition {
                    &self.0.proposition
                }
            }

            impl AsRef<Proposition> for SharedPremise {
                fn as_ref(&self) -> &Proposition {
                    &self.0.proposition
                }
            }

            impl PartialEq for SharedPremise {
                fn eq(&self, other: &Self) -> bool {
                    Arc::ptr_eq(&self.0, &other.0)
                }
            }
            impl Eq for SharedPremise {}
            impl PartialOrd for SharedPremise {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }
            impl Ord for SharedPremise {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.rank().cmp(&other.rank())
                }
            }

            #[derive(Default)]
            pub(crate) struct PremiseTable {
                rows: BTreeSet<SharedPremise>,
            }

            impl PremiseTable {
                pub(crate) fn intern(&mut self, proposition: Proposition) -> SharedPremise {
                    if let Some(existing) = self.rows.get(&proposition) {
                        return existing.clone();
                    }
                    self.insert(proposition)
                }

                pub(crate) fn intern_ref(&mut self, proposition: &Proposition) -> SharedPremise {
                    if let Some(existing) = self.rows.get(proposition) {
                        return existing.clone();
                    }
                    self.insert(proposition.clone())
                }

                fn insert(&mut self, proposition: Proposition) -> SharedPremise {
                    let rank = match self.free_rank(&proposition) {
                        Some(rank) => rank,
                        None => {
                            self.renumber();
                            self.free_rank(&proposition)
                                .expect("renumbered premise ranks leave a gap between neighbors")
                        }
                    };
                    let mut values = Vec::new();
                    let closed = proposition.visit_value_ids(|value| values.push(value));
                    values.sort_unstable();
                    values.dedup();
                    let substitution_stable =
                        substitute_proposition_values(&proposition, &BTreeMap::new())
                            == proposition;
                    let premise = SharedPremise(Arc::new(PremiseRow {
                        proposition,
                        values,
                        closed,
                        substitution_stable,
                        rank: AtomicU64::new(rank),
                    }));
                    self.rows.insert(premise.clone());
                    premise
                }

                fn free_rank(&self, proposition: &Proposition) -> Option<u64> {
                    let below = self
                        .rows
                        .range::<Proposition, _>((Bound::Unbounded, Bound::Excluded(proposition)))
                        .next_back()
                        .map(SharedPremise::rank);
                    let above = self
                        .rows
                        .range::<Proposition, _>((Bound::Excluded(proposition), Bound::Unbounded))
                        .next()
                        .map(SharedPremise::rank);
                    match (below, above) {
                        (None, None) => Some(RANK_SPACING),
                        (Some(below), None) => below.checked_add(RANK_SPACING),
                        (below, Some(above)) => {
                            let below = below.unwrap_or(0);
                            (above - below >= 2).then(|| below + (above - below) / 2)
                        }
                    }
                }

                fn renumber(&mut self) {
                    for (index, premise) in self.rows.iter().enumerate() {
                        let rank = (index as u64 + 1) * RANK_SPACING;
                        premise.0.rank.store(rank, AtomicOrdering::Relaxed);
                    }
                }

                pub(crate) fn arrival_requirements(
                    &mut self,
                    requirements: &[SharedPremise],
                    bindings: &[(ValueId, ScalarTerm)],
                    parameters: &[ValueId],
                ) -> Vec<SharedPremise> {
                    let mut substitutions = BTreeMap::new();
                    for (parameter, actual) in bindings {
                        if let ScalarTerm::Value { id, scalar_type } = actual {
                            substitutions
                                .entry(*id)
                                .or_insert_with(|| ScalarTerm::value(*parameter, *scalar_type));
                        }
                    }
                    let surviving = |value: ValueId| {
                        !parameters.contains(&value) || substitutions.contains_key(&value)
                    };
                    let mut result = Vec::with_capacity(requirements.len() + 2 * bindings.len());
                    for premise in requirements {
                        let row = &premise.0;
                        if row.values.iter().any(|value| !surviving(*value)) {
                            continue;
                        }
                        let unchanged = row.closed
                            && row.substitution_stable
                            && !row
                                .values
                                .iter()
                                .any(|value| substitutions.contains_key(value));
                        result.push(if unchanged {
                            premise.clone()
                        } else {
                            self.intern(substitute_proposition_values(
                                &row.proposition,
                                &substitutions,
                            ))
                        });
                    }
                    for (parameter, actual) in bindings {
                        if actual.any_value_id(|value| !surviving(value)) {
                            continue;
                        }
                        let value = substitute_scalar_term_values(actual, &substitutions);
                        result.push(self.intern(Proposition::Equal(
                            ScalarTerm::value(*parameter, value.scalar_type()),
                            value,
                        )));
                        if !actual.any_value_id(|value| parameters.contains(&value)) {
                            result.push(self.intern(Proposition::Equal(
                                ScalarTerm::value(*parameter, actual.scalar_type()),
                                actual.clone(),
                            )));
                        }
                    }
                    result
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;
                use semantic_vocabulary::ScalarType;

                fn condition(value: u64) -> Proposition {
                    Proposition::Equal(
                        ScalarTerm::Boolean(true),
                        ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
                    )
                }

                #[test]
                fn handle_order_matches_proposition_order_through_renumbering() {
                    let mut table = PremiseTable::default();
                    let mut values = Vec::new();
                    let (mut low, mut high) = (1_000u64, 1_001u64);
                    for round in 0..80 {
                        if round % 2 == 0 {
                            low -= 1;
                            values.push(low);
                        } else {
                            high += 1;
                            values.push(high);
                        }
                    }
                    values.extend((0..64).map(|step| 500 + step));
                    let handles = values
                        .iter()
                        .map(|value| table.intern(condition(*value)))
                        .collect::<Vec<_>>();
                    let mut by_handle = handles.clone();
                    by_handle.sort();
                    let mut by_proposition = values
                        .iter()
                        .map(|value| condition(*value))
                        .collect::<Vec<_>>();
                    by_proposition.sort();
                    assert_eq!(
                        by_handle
                            .iter()
                            .map(|premise| premise.as_ref().clone())
                            .collect::<Vec<_>>(),
                        by_proposition
                    );
                    for (value, handle) in values.iter().zip(&handles) {
                        assert!(table.intern(condition(*value)) == *handle);
                        assert!(table.intern_ref(&condition(*value)) == *handle);
                    }
                    assert!(handles[0] != handles[1]);
                }

                #[test]
                fn arrival_keeps_unrelated_rows_and_substitutes_rebound_values() {
                    let mut table = PremiseTable::default();
                    let unrelated = table.intern(condition(1));
                    let rebound = table.intern(condition(2));
                    let retired = table.intern(condition(3));
                    let actual = ValueId::new(2).unwrap();
                    let formal = ValueId::new(20).unwrap();
                    let arrived = table.arrival_requirements(
                        &[unrelated.clone(), rebound, retired],
                        &[(formal, ScalarTerm::value(actual, ScalarType::Boolean))],
                        &[ValueId::new(3).unwrap(), formal],
                    );
                    assert!(arrived[0] == unrelated);
                    assert_eq!(arrived[1].as_ref(), &condition(20));
                    assert_eq!(
                        arrived[2..]
                            .iter()
                            .map(|premise| premise.as_ref().clone())
                            .collect::<Vec<_>>(),
                        vec![
                            Proposition::Equal(
                                ScalarTerm::value(formal, ScalarType::Boolean),
                                ScalarTerm::value(formal, ScalarType::Boolean),
                            ),
                            Proposition::Equal(
                                ScalarTerm::value(formal, ScalarType::Boolean),
                                ScalarTerm::value(actual, ScalarType::Boolean),
                            ),
                        ]
                    );
                    assert_eq!(arrived.len(), 4);
                }
            }
        }

        use premise_table::{PremiseTable, SharedPremise};

        use super::PayloadScalarObservation;
        use super::contents::select_payload_memberships;
        use super::payload_predicates;
        use super::payload_predicates::observation_type;
        use super::payload_reachability;
        use super::payload_reachability::incoming_flow;
        use super::*;
        use crate::descriptors::descriptor_lifetimes::descriptor_path;
        use crate::qualification_path_carrier;
        use std::borrow::Borrow;
        use std::collections::VecDeque;
        use std::sync::Arc;

        #[derive(Clone, Default, PartialEq, Eq)]
        struct PayloadContracts(BTreeMap<PlaceId, Arc<PayloadMembershipContract>>);

        impl PayloadContracts {
            fn len(&self) -> usize {
                self.0.len()
            }
            fn get(&self, root: &PlaceId) -> Option<&PayloadMembershipContract> {
                self.0.get(root).map(Arc::as_ref)
            }
            fn get_mut(&mut self, root: &PlaceId) -> Option<&mut PayloadMembershipContract> {
                self.0.get_mut(root).map(Arc::make_mut)
            }
            fn insert(&mut self, root: PlaceId, contract: PayloadMembershipContract) {
                self.0.insert(root, Arc::new(contract));
            }
            fn remove(&mut self, root: &PlaceId) -> Option<Arc<PayloadMembershipContract>> {
                self.0.remove(root)
            }
            fn values_mut(&mut self) -> impl Iterator<Item = &mut Arc<PayloadMembershipContract>> {
                self.0.values_mut()
            }
        }

        #[cfg(test)]
        impl<const N: usize> From<[(PlaceId, PayloadMembershipContract); N]> for PayloadContracts {
            fn from(rows: [(PlaceId, PayloadMembershipContract); N]) -> Self {
                Self(
                    rows.into_iter()
                        .map(|(root, contract)| (root, Arc::new(contract)))
                        .collect(),
                )
            }
        }

        #[cfg(test)]
        impl std::ops::Index<&PlaceId> for PayloadContracts {
            type Output = PayloadMembershipContract;
            fn index(&self, root: &PlaceId) -> &Self::Output {
                self.get(root).unwrap()
            }
        }

        #[derive(Clone, Default, PartialEq, Eq)]
        struct State {
            values: PayloadContracts,
            scalar_requirements: Arc<Vec<SharedPremise>>,
            shared_requirements: Arc<Vec<SharedPremise>>,
        }

        impl State {
            fn publish_requirements(&mut self) {
                for contract in self.values.values_mut() {
                    if !contract.scalar_requirements.iter().any(|premise| {
                        self.scalar_requirements
                            .binary_search_by(|candidate| candidate.as_ref().cmp(premise))
                            .is_ok()
                    }) {
                        continue;
                    }
                    let contract = Arc::make_mut(contract);
                    let mut common = self
                        .scalar_requirements
                        .iter()
                        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                        .peekable();
                    contract.scalar_requirements.retain(|premise| {
                        while common.peek().is_some_and(|candidate| *candidate < premise) {
                            common.next();
                        }
                        !common.peek().is_some_and(|candidate| *candidate == premise)
                    });
                }
                self.shared_requirements = self.scalar_requirements.clone();
            }

            fn contract(
                &self,
                root: PlaceId,
                predicate: Option<&semantic_vocabulary::Proposition>,
            ) -> Option<PayloadMembershipContract> {
                let mut contract = self.values.get(&root)?.clone();
                extend_canonical_premises(
                    &mut contract.scalar_requirements,
                    &self.shared_requirements,
                );
                if let Some(predicate) = predicate
                    && !contract.scalar_requirements.contains(predicate)
                {
                    contract.scalar_requirements.push(predicate.clone());
                }
                Some(contract)
            }

            fn meet(&mut self, mut arrival: State) -> bool {
                let previous_shared = std::mem::take(&mut self.shared_requirements);
                let mut shared = previous_shared.clone();
                retain_common_snapshot(&mut shared, &arrival.shared_requirements);
                let mut changed = retain_common_snapshot(
                    &mut self.scalar_requirements,
                    &arrival.scalar_requirements,
                );
                let previous_values = self.values.len();
                self.values.0.retain(|place, contract| {
                    let Some(incoming) = arrival.values.remove(place) else {
                        return false;
                    };
                    if contract.carrier != incoming.carrier {
                        return false;
                    }
                    let contract = Arc::make_mut(contract);
                    let previous_premises =
                        contract.scalar_requirements.len() + previous_shared.len();
                    retain_captured_premises(
                        &mut contract.scalar_requirements,
                        &incoming.scalar_requirements,
                        &arrival.shared_requirements,
                    );
                    let mut incoming_premises = incoming.scalar_requirements.clone();
                    retain_common_premises(&mut incoming_premises, &previous_shared);
                    extend_canonical_premises(
                        &mut contract.scalar_requirements,
                        &incoming_premises,
                    );
                    changed |=
                        contract.scalar_requirements.len() + shared.len() != previous_premises;
                    let previous_counts = (
                        contract.qualifications.len(),
                        contract.projected_qualifications.len(),
                    );
                    changed |= PayloadScalarObservation::intersect(
                        &mut contract.scalar_observations,
                        &incoming.scalar_observations,
                    );
                    retain_common_memberships(
                        &mut contract.qualifications,
                        &incoming.qualifications,
                    );
                    retain_common_memberships(
                        &mut contract.projected_qualifications,
                        &incoming.projected_qualifications,
                    );
                    changed |= previous_counts
                        != (
                            contract.qualifications.len(),
                            contract.projected_qualifications.len(),
                        );
                    true
                });
                self.shared_requirements = shared;
                changed || self.values.len() != previous_values
            }
        }

        struct CachedHistory {
            relevant: payload_reachability::Reachability,
            snapshots: BTreeMap<DescriptorPosition, State>,
        }

        #[derive(Default)]
        struct HistoryReplay {
            // Immutable declaration classification shared by this history's cuts.
            content_independent: Option<Vec<semantic_vocabulary::StructuralDomainId>>,
            premises: PremiseTable,
            cached: Option<CachedHistory>,
            inventory: Option<DescriptorSlotKind>,
        }

        enum PayloadSources<'input> {
            Terminal(&'input TerminalMachine),
            Current {
                identity: MachineId,
                parameters: &'input [terminal_psi::StructuralParameterDeclaration],
                contracts: &'input [(PlaceId, PayloadMembershipContract)],
            },
        }
        impl PayloadSources<'_> {
            fn identity(&self) -> MachineId {
                match self {
                    Self::Terminal(machine) => machine.id,
                    Self::Current { identity, .. } => *identity,
                }
            }
            fn parameters(&self) -> &[terminal_psi::StructuralParameterDeclaration] {
                match self {
                    Self::Terminal(machine) => &machine.structural_parameters,
                    Self::Current { parameters, .. } => parameters,
                }
            }
            fn contract(&self, root: PlaceId) -> Option<PayloadMembershipContract> {
                match self {
                    Self::Terminal(machine) => super::terminal_payload_contract(machine, root),
                    Self::Current { contracts, .. } => {
                        let mut rows = contracts.iter().filter(|(place, _)| *place == root);
                        let (_, contract) = rows.next()?;
                        rows.next().is_none().then(|| contract.clone())
                    }
                }
            }
        }

        pub struct PayloadMembershipHistory<'input> {
            types: &'input [StructuralTypeDeclaration],
            domains: &'input [terminal_psi::StructuralDomainDeclaration],
            graph: &'input [DescriptorMachine],
            descriptor: Option<&'input DescriptorMachine>,
            sources: PayloadSources<'input>,
            incoming: Option<Option<payload_reachability::IncomingFlow>>,
            analysis: Option<DescriptorAnalysis<'input>>,
            replay: HistoryReplay,
            entry_snapshots: Option<Vec<(PlaceId, Vec<PayloadScalarObservation>)>>,
        }
        impl<'input> PayloadMembershipHistory<'input> {
            fn require_analysis_context(
                &mut self,
                analysis: &DescriptorAnalysis<'_>,
            ) -> Result<(), &'static str> {
                if !analysis.uses_source_graph(self.types, self.graph) {
                    return Err(
                        "payload history analysis belongs to another immutable source graph",
                    );
                }
                match self.replay.inventory {
                    Some(inventory) if inventory != analysis.slot_kind() => {
                        Err("payload history analysis changed its descriptor inventory")
                    }
                    Some(_) => Ok(()),
                    None => {
                        self.replay.inventory = Some(analysis.slot_kind());
                        Ok(())
                    }
                }
            }

            pub fn new(
                types: &'input [StructuralTypeDeclaration],
                domains: &'input [terminal_psi::StructuralDomainDeclaration],
                graph: &'input [DescriptorMachine],
                machine: &'input TerminalMachine,
            ) -> Self {
                Self {
                    types,
                    domains,
                    graph,
                    descriptor: descriptor_machine(graph, machine.id),
                    sources: PayloadSources::Terminal(machine),
                    incoming: None,
                    analysis: None,
                    replay: HistoryReplay::default(),
                    entry_snapshots: None,
                }
            }
            pub fn for_current_contracts(
                types: &'input [StructuralTypeDeclaration],
                domains: &'input [terminal_psi::StructuralDomainDeclaration],
                graph: &'input [DescriptorMachine],
                identity: MachineId,
                parameters: &'input [terminal_psi::StructuralParameterDeclaration],
                contracts: &'input [(PlaceId, PayloadMembershipContract)],
            ) -> Result<Self, &'static str> {
                let current = descriptor_machine(graph, identity)
                    .ok_or("current payload machine is absent")?;
                for (index, (root, contract)) in contracts.iter().enumerate() {
                    if current.place_types.get(root) != Some(&contract.carrier)
                        || contracts[..index]
                            .iter()
                            .any(|(previous, _)| previous == root)
                    {
                        return Err(
                            "current payload declaration disagrees with its exact graph carrier",
                        );
                    }
                }
                Ok(Self {
                    types,
                    domains,
                    graph,
                    descriptor: Some(current),
                    incoming: None,
                    analysis: None,
                    sources: PayloadSources::Current {
                        identity,
                        parameters,
                        contracts,
                    },
                    replay: HistoryReplay::default(),
                    entry_snapshots: None,
                })
            }
            pub fn current_selected_payload_contract(
                &mut self,
                position: DescriptorPosition,
                root: PlaceId,
                path: &[StructuralPathSegment],
            ) -> Result<PayloadMembershipContract, &'static str> {
                let mut analysis = self
                    .analysis
                    .take()
                    .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
                let result =
                    self.current_selected_payload_contract_in(&mut analysis, position, root, path);
                self.analysis = Some(analysis);
                result
            }

            pub fn current_selected_payload_contract_in(
                &mut self,
                analysis: &mut DescriptorAnalysis<'_>,
                position: DescriptorPosition,
                root: PlaceId,
                path: &[StructuralPathSegment],
            ) -> Result<PayloadMembershipContract, &'static str> {
                self.require_analysis_context(analysis)?;
                let identity = self.sources.identity();
                let sources = &self.sources;
                let source = |place| sources.contract(place);
                let root_contract = source(root).ok_or("selected payload root is absent")?;
                let query =
                    descriptor_contents_query(self.types, root, root_contract.carrier, path, None)
                        .ok_or("selected payload path has no typed coordinate")?;
                let targets = analysis
                    .payload_targets(identity, position, &query)
                    .ok_or("selected payload backing is unresolved")?;
                let descriptor = self.descriptor;
                let incoming = self
                    .incoming
                    .get_or_insert_with(|| {
                        descriptor.and_then(|machine| incoming_flow(&machine.flow))
                    })
                    .as_ref();
                let mut failure = None;
                let entry_snapshots = self.entry_snapshots.as_deref().unwrap_or(&[]);
                let replay = &mut self.replay;
                let contract = payload_memberships(self.types, self.domains, targets, |place| {
                    current_payload_contract_reusing(
                        self.types,
                        self.domains,
                        descriptor,
                        incoming,
                        analysis,
                        position,
                        place,
                        &source,
                        entry_snapshots,
                        replay,
                    )
                    .map_err(|reason| failure = Some(reason))
                    .ok()
                });
                contract.ok_or(failure.unwrap_or("selected payload memberships are unresolved"))
            }

            pub fn current_payload_contract(
                &mut self,
                position: DescriptorPosition,
                root: PlaceId,
            ) -> Result<PayloadMembershipContract, &'static str> {
                let mut analysis = self
                    .analysis
                    .take()
                    .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
                let result = self.current_payload_contract_in(&mut analysis, position, root);
                self.analysis = Some(analysis);
                result
            }

            pub fn current_payload_contract_in(
                &mut self,
                analysis: &mut DescriptorAnalysis<'_>,
                position: DescriptorPosition,
                root: PlaceId,
            ) -> Result<PayloadMembershipContract, &'static str> {
                self.require_analysis_context(analysis)?;
                let sources = &self.sources;
                let descriptor = self.descriptor;
                let incoming = self
                    .incoming
                    .get_or_insert_with(|| {
                        descriptor.and_then(|machine| incoming_flow(&machine.flow))
                    })
                    .as_ref();
                current_payload_contract_reusing(
                    self.types,
                    self.domains,
                    descriptor,
                    incoming,
                    analysis,
                    position,
                    root,
                    &|place| sources.contract(place),
                    self.entry_snapshots.as_deref().unwrap_or(&[]),
                    &mut self.replay,
                )
            }
            pub fn scalar_snapshot(
                &mut self,
                position: DescriptorPosition,
                root: PlaceId,
                path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
            ) -> Result<PayloadScalarObservation, &'static str> {
                let mut analysis = self
                    .analysis
                    .take()
                    .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
                let result = self.scalar_snapshot_in(&mut analysis, position, root, path);
                self.analysis = Some(analysis);
                result
            }

            pub fn scalar_snapshot_in(
                &mut self,
                analysis: &mut DescriptorAnalysis<'_>,
                position: DescriptorPosition,
                root: PlaceId,
                path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
            ) -> Result<PayloadScalarObservation, &'static str> {
                self.require_analysis_context(analysis)?;
                if self.entry_snapshots.is_none() {
                    self.entry_snapshots = Some(entry_snapshots::collect(
                        self.types,
                        self.sources.parameters(),
                    ));
                    self.replay.cached = None;
                }
                let contract = self.current_payload_contract_in(analysis, position, root)?;
                let expected = observation_type(self.types, contract.carrier, path)
                    .ok_or("snapshot subject is not an exact primitive value")?;
                let mut observations = contract
                    .scalar_observations
                    .iter()
                    .filter(|row| row.path == path);
                let observed = observations
                    .next()
                    .ok_or("snapshot subject has no retained value identity")?;
                if observations.next().is_some()
                    || observed
                        .value
                        .as_ref()
                        .is_some_and(|value| value.scalar_type() != expected)
                    || observed
                        .entry
                        .as_ref()
                        .is_some_and(|entry| entry.scalar_type != expected)
                    || (observed.value.is_none() && observed.entry.is_none())
                {
                    return Err("snapshot subject has ambiguous or mismatched scalar identity");
                }
                Ok(observed.clone())
            }
        }

        pub fn current_selected_payload_contract(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            position: DescriptorPosition,
            root: PlaceId,
            path: &[StructuralPathSegment],
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<PayloadMembershipContract, &'static str> {
            let root_contract = source(root).ok_or("selected payload root is absent")?;
            let query = descriptor_contents_query(types, root, root_contract.carrier, path, None)
                .ok_or("selected payload path has no typed coordinate")?;
            let mut analysis = DescriptorAnalysis::new(types, graph);
            let targets = analysis
                .payload_targets(identity, position, &query)
                .ok_or("selected payload backing is unresolved")?;
            let descriptor = descriptor_machine(graph, identity);
            let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
            let mut replay = HistoryReplay::default();
            let mut history_failure = None;
            let contract = payload_memberships(types, domains, targets, |place| {
                current_payload_contract_reusing(
                    types,
                    domains,
                    descriptor,
                    incoming.as_ref(),
                    &mut analysis,
                    position,
                    place,
                    source,
                    &[],
                    &mut replay,
                )
                .map_err(|reason| history_failure = Some(reason))
                .ok()
            });
            contract.ok_or(history_failure.unwrap_or("selected payload memberships are unresolved"))
        }

        pub fn current_payload_contract(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            position: DescriptorPosition,
            root: PlaceId,
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<PayloadMembershipContract, &'static str> {
            let mut analysis = DescriptorAnalysis::new(types, graph);
            let descriptor = descriptor_machine(graph, identity);
            let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
            current_payload_contract_reusing(
                types,
                domains,
                descriptor,
                incoming.as_ref(),
                &mut analysis,
                position,
                root,
                source,
                &[],
                &mut HistoryReplay::default(),
            )
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum PayloadMembershipCut {
            BeforeOperation,
            NormalCompletion,
        }

        pub(crate) fn completed_payload_contract(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            position: DescriptorPosition,
            root: PlaceId,
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
        ) -> Result<PayloadMembershipContract, &'static str> {
            let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(types, graph);
            let descriptor = descriptor_machine(graph, identity);
            let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
            payload_contract_at_cut(
                types,
                domains,
                descriptor,
                incoming.as_ref(),
                &mut analysis,
                position,
                root,
                source,
                &[],
                &mut HistoryReplay::default(),
                PayloadMembershipCut::NormalCompletion,
            )
        }

        fn current_payload_contract_reusing(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            descriptor: Option<&DescriptorMachine>,
            incoming: Option<&payload_reachability::IncomingFlow>,
            analysis: &mut DescriptorAnalysis<'_>,
            position: DescriptorPosition,
            root: PlaceId,
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
            entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
            replay: &mut HistoryReplay,
        ) -> Result<PayloadMembershipContract, &'static str> {
            payload_contract_at_cut(
                types,
                domains,
                descriptor,
                incoming,
                analysis,
                position,
                root,
                source,
                entry_snapshots,
                replay,
                PayloadMembershipCut::BeforeOperation,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn payload_contract_at_cut(
            types: &[StructuralTypeDeclaration],
            domains: &[terminal_psi::StructuralDomainDeclaration],
            descriptor: Option<&DescriptorMachine>,
            incoming: Option<&payload_reachability::IncomingFlow>,
            analysis: &mut DescriptorAnalysis<'_>,
            position: DescriptorPosition,
            root: PlaceId,
            source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
            entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
            replay: &mut HistoryReplay,
            cut: PayloadMembershipCut,
        ) -> Result<PayloadMembershipContract, &'static str> {
            let content_independent = replay.content_independent.get_or_insert_with(|| {
                terminal_psi::StructuralDomainDeclaration::content_independent_memberships(domains)
            });
            let mut failure = "descriptor machine is absent";
            let result = (|| {
                let machine = descriptor?;
                let identity = machine.id;
                failure = "payload query reachability is invalid or exceeds its finite budget";
                let mut relevant = match incoming {
                    Some(incoming) => payload_reachability::predecessors_with_incoming(
                        machine, incoming, position,
                    )?,
                    None => payload_reachability::predecessors(machine, position)?,
                };
                let premises = &mut replay.premises;
                let previous = replay.cached.take();
                let frontier = previous.as_ref().and_then(|previous| {
                    relevant.compatible_frontier(&previous.relevant, &machine.flow)
                });
                let (mut snapshots, mut pending, previous_relevant) = if let Some(frontier) =
                    frontier
                {
                    let previous = previous.expect("frontier owns the preceding immutable history");
                    (previous.snapshots, frontier, Some(previous.relevant))
                } else {
                    drop(previous);
                    let mut initial = State::default();
                    if let Some((_, requirements)) = &machine.payload_entry_predicates {
                        initial.scalar_requirements = Arc::new(
                            super::scalar_contracts::scalar_conjuncts(requirements)
                                .into_iter()
                                .map(|premise| premises.intern(premise))
                                .collect(),
                        );
                    }
                    canonicalize_snapshot(&mut initial.scalar_requirements);
                    for place in &machine.places {
                        if matches!(place.definition, DescriptorDefinition::Parameter { .. }) {
                            failure = "entry payload contract is absent";
                            let mut contract = source(place.place)?;
                            if let Some((_, snapshots)) = entry_snapshots
                                .iter()
                                .find(|(root, _)| *root == place.place)
                            {
                                contract
                                    .scalar_observations
                                    .extend(snapshots.iter().cloned());
                            }
                            super::payload_entry::close(
                                types,
                                domains,
                                machine,
                                place.place,
                                &mut contract,
                            );
                            contract.scalar_requirements.sort();
                            contract.scalar_requirements.dedup();
                            initial.values.insert(place.place, contract);
                        }
                    }
                    (
                        BTreeMap::from([(machine.entry, initial)]),
                        VecDeque::from([machine.entry]),
                        None,
                    )
                };
                let mut completed = None;
                let mut stores = Vec::new();
                let mut bindings = Vec::new();
                let mut remaining = 65_536usize;
                while let Some(current) = pending.pop_front() {
                    failure = "payload history worklist exceeded its finite budget";
                    remaining = remaining.checked_sub(1)?;
                    if !relevant.contains(current) {
                        continue;
                    }
                    failure = "payload history flow position is absent";
                    let mut before = snapshots.get(&current)?.clone();
                    if let Some(definitions) = machine.payload_scalar_definitions.get(&current) {
                        retire_scalar_instances(&mut before, definitions);
                    }
                    let step = machine.flow.get(&current)?;
                    let mut source_values = BTreeMap::new();
                    for (actual, destination) in &step.derivations {
                        if machine.storage_derivations.contains(&(
                            current,
                            actual.clone(),
                            destination.clone(),
                        )) {
                            continue;
                        }
                        if let Some(contract) = before.values.get(&actual.root) {
                            source_values.entry(actual.root).or_insert_with(|| {
                                let mut contract = contract.clone();
                                extend_canonical_premises(
                                    &mut contract.scalar_requirements,
                                    &before.shared_requirements,
                                );
                                contract
                            });
                        }
                    }
                    let mut after = before;
                    if machine.known_invocation(&current).is_some() {
                        let writes = match analysis.storage_writes(identity, current) {
                            Ok(writes) => writes,
                            Err(reason) => {
                                failure = reason;
                                return None;
                            }
                        };
                        match writes {
                            Some(writes) => {
                                for location in writes {
                                    if let Some(contract) = after.values.get_mut(&location.root) {
                                        failure = "call payload write has no typed coordinate";
                                        let path = descriptor_path(
                                            types,
                                            contract.carrier,
                                            &location.path,
                                        )?;
                                        retire(
                                            content_independent,
                                            contract,
                                            &path,
                                            &location.path,
                                        );
                                    }
                                }
                            }
                            None => clear(&mut after.values),
                        }
                    } else if let Some(writes) = machine.payload_invalidations.get(&current) {
                        for query in writes {
                            let targets = query.as_ref().and_then(|query| {
                                analysis.payload_targets(identity, current, query)
                            });
                            match targets {
                                Some(targets) => {
                                    for target in targets {
                                        match target {
                                            DescriptorPayloadTarget::Storage {
                                                location,
                                                completed: None,
                                            } => {
                                                if let Some(contract) =
                                                    after.values.get_mut(&location.root)
                                                {
                                                    failure = "payload write path has no typed coordinate";
                                                    let path = descriptor_path(
                                                        types,
                                                        contract.carrier,
                                                        &location.path,
                                                    )?;
                                                    retire(
                                                        content_independent,
                                                        contract,
                                                        &path,
                                                        &location.path,
                                                    );
                                                }
                                            }
                                            DescriptorPayloadTarget::Storage {
                                                completed: Some(_),
                                                ..
                                            }
                                            | DescriptorPayloadTarget::Incoming(_) => {
                                                clear(&mut after.values)
                                            }
                                        }
                                    }
                                }
                                None => clear(&mut after.values),
                            }
                        }
                    }
                    stores.clear();
                    for (actual, destination) in &step.derivations {
                        if machine.payload_producers.get(&current) == Some(&destination.root)
                            || machine.storage_derivations.contains(&(
                                current,
                                actual.clone(),
                                destination.clone(),
                            ))
                        {
                            continue;
                        }
                        let mut incoming = source_values.get(&actual.root).and_then(|contract| {
                            select_payload_memberships(types, domains, contract, &actual.path)
                        });
                        failure = "structural store backing is unresolved";
                        let destinations =
                            analysis.storage_origins(identity, current, destination)?;
                        let strong = destinations.len() == 1;
                        let mut destinations = destinations.into_iter().peekable();
                        while let Some(destination) = destinations.next() {
                            let selected = if destinations.peek().is_some() {
                                incoming.clone()
                            } else {
                                incoming.take()
                            };
                            stores.push((destination, selected, strong));
                        }
                    }
                    for index in 0..stores.len() {
                        let destination = &stores[index].0;
                        if stores.iter().enumerate().any(|(other, (candidate, _, _))| {
                            index != other
                                && candidate.root == destination.root
                                && (candidate.path.starts_with(&destination.path)
                                    || destination.path.starts_with(&candidate.path))
                        }) {
                            stores[index].2 = false;
                        }
                    }
                    for (destination, incoming, strong) in stores.drain(..) {
                        let Some(contract) = after.values.get_mut(&destination.root) else {
                            continue;
                        };
                        failure = "structural store path has no typed coordinate";
                        let path = descriptor_path(types, contract.carrier, &destination.path)?;
                        retire(content_independent, contract, &path, &destination.path);
                        if !strong {
                            continue;
                        }
                        let Some(incoming) = incoming else {
                            continue;
                        };
                        failure = "structural store carrier does not match its actual source";
                        let destination_carrier =
                            qualification_path_carrier(types.iter(), contract.carrier, &path)?;
                        let incoming = incoming.presented(types, domains, destination_carrier)?;
                        if path.is_empty() {
                            *contract = incoming;
                        } else {
                            append_scalar_observations(contract, &incoming, &destination.path);
                            terminal_psi::StructuralPathQualification::append_under(
                                &incoming.qualifications,
                                &incoming.projected_qualifications,
                                &path,
                                &mut contract.projected_qualifications,
                            );
                        }
                    }
                    if let Some(producer) = machine.payload_producers.get(&current) {
                        failure = "established payload contract is absent";
                        let mut produced = source(*producer)?;
                        failure = "established payload contract has a different carrier";
                        if machine.place_types.get(producer) != Some(&produced.carrier) {
                            return None;
                        }
                        produced.scalar_requirements.sort();
                        produced.scalar_requirements.dedup();
                        let completion = (identity, (current.0, current.1.checked_add(1)?));
                        for (actual, destination) in &step.derivations {
                            if destination.root != *producer
                                || machine.storage_derivations.contains(&(
                                    current,
                                    actual.clone(),
                                    destination.clone(),
                                ))
                            {
                                continue;
                            }
                            if let Some(incoming) =
                                source_values.get(&actual.root).and_then(|contract| {
                                    select_payload_memberships(
                                        types,
                                        domains,
                                        contract,
                                        &actual.path,
                                    )
                                })
                            {
                                // A sequence constructor authenticates the exact ordinal of
                                // each operand. Plain type paths lack that storage context.
                                failure = "payload producer destination has no typed coordinate";
                                let (path, destination_carrier) = machine.resolve_location(
                                    types,
                                    destination,
                                    Some(completion),
                                )?;
                                let Some(incoming) =
                                    incoming.presented(types, domains, destination_carrier?)
                                else {
                                    continue;
                                };
                                append_scalar_observations(
                                    &mut produced,
                                    &incoming,
                                    &destination.path,
                                );
                                if path.is_empty() {
                                    produced.qualifications.extend(&incoming.qualifications);
                                    produced
                                        .projected_qualifications
                                        .extend(incoming.projected_qualifications.clone());
                                } else {
                                    terminal_psi::StructuralPathQualification::append_under(
                                        &incoming.qualifications,
                                        &incoming.projected_qualifications,
                                        &path,
                                        &mut produced.projected_qualifications,
                                    );
                                }
                                produced.qualifications.sort();
                                produced.qualifications.dedup();
                                produced.projected_qualifications.sort();
                                produced.projected_qualifications.dedup();
                            }
                        }
                        after.values.insert(*producer, produced);
                    }
                    drop(source_values);
                    if let Some(writes) = machine.scalar_payload_writes.get(&current) {
                        for (destination, value) in writes {
                            let resolved;
                            let destination = if machine.payload_producers.get(&current)
                                == Some(&destination.root)
                            {
                                destination
                            } else {
                                let Some(destinations) =
                                    analysis.storage_origins(identity, current, destination)
                                else {
                                    continue;
                                };
                                if destinations.len() != 1 {
                                    continue;
                                }
                                resolved = destinations.into_iter().next()?;
                                &resolved
                            };
                            let Some(contract) = after.values.get_mut(&destination.root) else {
                                continue;
                            };
                            if observation_type(types, contract.carrier, &destination.path)
                                != Some(value.scalar_type())
                            {
                                continue;
                            }
                            contract.scalar_observations.retain(|observation| {
                                !observation.path.starts_with(&destination.path)
                                    && !destination.path.starts_with(&observation.path)
                            });
                            contract.scalar_observations.push(PayloadScalarObservation {
                                path: destination.path.clone(),
                                value: Some(value.clone()),
                                entry: None,
                            });
                        }
                    }
                    if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                        for (location, value) in reads {
                            if let Some(contract) = after.values.get_mut(&location.root)
                                && let Some(equality) =
                                    observe_scalar(contract, &location.path, value)
                            {
                                Arc::make_mut(&mut after.scalar_requirements)
                                    .push(premises.intern(equality));
                            }
                        }
                    }
                    if let Some(requirements) = machine.payload_scalar_postconditions.get(&current)
                        && !requirements.is_empty()
                    {
                        Arc::make_mut(&mut after.scalar_requirements).extend(
                            requirements
                                .iter()
                                .map(|premise| premises.intern_ref(premise)),
                        );
                    }
                    canonicalize_snapshot(&mut after.scalar_requirements);
                    after.publish_requirements();
                    for contract in after.values.values_mut() {
                        if !payload_predicates::closure_may_change(types, domains, contract) {
                            continue;
                        }
                        payload_predicates::close_with_shared_requirements(
                            types,
                            domains,
                            Arc::make_mut(contract),
                            &after.shared_requirements,
                            None,
                        );
                    }
                    if cut == PayloadMembershipCut::NormalCompletion && current == position {
                        completed = Some(after.clone());
                    }
                    let mut successors = step
                        .successors
                        .iter()
                        .enumerate()
                        .filter(|(ordinal, edge)| {
                            relevant.includes_edge(current, *ordinal)
                                && !previous_relevant
                                    .as_ref()
                                    .is_some_and(|previous| previous.contains(edge.target))
                        })
                        .peekable();
                    while let Some((ordinal, edge)) = successors.next() {
                        let mut arrival = if successors.peek().is_some() {
                            after.clone()
                        } else {
                            std::mem::take(&mut after)
                        };
                        let edge_predicate = machine
                            .payload_edge_predicates
                            .get(&current)
                            .and_then(|rows| rows.get(ordinal))
                            .and_then(Option::as_ref);
                        if let Some(predicate) = edge_predicate {
                            Arc::make_mut(&mut arrival.scalar_requirements).extend(
                                super::scalar_contracts::scalar_conjuncts(std::slice::from_ref(
                                    predicate,
                                ))
                                .into_iter()
                                .map(|premise| premises.intern(premise)),
                            );
                            canonicalize_snapshot(&mut arrival.scalar_requirements);
                            for contract in arrival.values.values_mut() {
                                if !payload_predicates::closure_may_change(types, domains, contract)
                                {
                                    continue;
                                }
                                payload_predicates::close_with_shared_requirements(
                                    types,
                                    domains,
                                    Arc::make_mut(contract),
                                    &arrival.shared_requirements,
                                    Some(predicate),
                                );
                            }
                        }
                        bindings.clear();
                        for (actual, parameter) in edge.derivations() {
                            failure = "edge formal is not a whole payload root";
                            if !parameter.path.is_empty() {
                                return None;
                            }
                            if let Some(contract) = arrival
                                .contract(actual.root, edge_predicate)
                                .and_then(|contract| {
                                    select_payload_memberships(
                                        types,
                                        domains,
                                        &contract,
                                        &actual.path,
                                    )
                                })
                            {
                                let Some(contract) = contract.presented(
                                    types,
                                    domains,
                                    *machine.place_types.get(&parameter.root)?,
                                ) else {
                                    continue;
                                };
                                bindings.push((parameter.root, contract));
                            }
                        }
                        for binding in &edge.bindings {
                            let crate::descriptors::descriptor_lifetimes::DescriptorFlowSource::Contents(
                                query,
                            ) = &binding.source
                            else {
                                continue;
                            };
                            let contract = analysis
                                .arrival_payload_targets(identity, current, query)
                                .and_then(|targets| {
                                    payload_memberships(types, domains, targets, |root| {
                                        arrival.contract(root, edge_predicate)
                                    })
                                });
                            if let Some(contract) = contract {
                                failure = "selected edge actual and formal payload carriers differ";
                                let contract = contract.presented(
                                    types,
                                    domains,
                                    *machine.place_types.get(&binding.parameter.root)?,
                                )?;
                                bindings.push((binding.parameter.root, contract));
                            }
                        }
                        if let Some(predicate) = edge_predicate {
                            for contract in arrival.values.values_mut() {
                                if !contract.scalar_requirements.contains(predicate)
                                    && !arrival
                                        .shared_requirements
                                        .iter()
                                        .any(|premise| premise.as_ref() == predicate)
                                {
                                    Arc::make_mut(contract)
                                        .scalar_requirements
                                        .push(predicate.clone());
                                }
                            }
                        }
                        for parameter in &edge.parameters {
                            arrival.values.remove(&parameter.root);
                        }
                        for (parameter, contract) in bindings.drain(..) {
                            arrival.values.insert(parameter, contract);
                        }
                        if edge.target.1 == 0
                            && let Some(parameters) =
                                machine.payload_scalar_parameters.get(&edge.target.0)
                        {
                            let incoming_requirements = arrival.scalar_requirements.clone();
                            retire_scalar_instances(&mut arrival, parameters);
                            if let Some(bindings) = machine
                                .payload_scalar_bindings
                                .get(&current)
                                .and_then(|edges| edges.get(ordinal))
                            {
                                let rebound = premises.arrival_requirements(
                                    &incoming_requirements,
                                    bindings,
                                    parameters,
                                );
                                if !contains_rebound_premises(
                                    &arrival.scalar_requirements,
                                    &rebound,
                                ) {
                                    Arc::make_mut(&mut arrival.scalar_requirements).extend(rebound);
                                }
                                canonicalize_snapshot(&mut arrival.scalar_requirements);
                            }
                        }
                        for contract in arrival.values.values_mut() {
                            if edge_predicate.is_some()
                                && contract
                                    .scalar_requirements
                                    .windows(2)
                                    .any(|pair| pair[0] >= pair[1])
                            {
                                let contract = Arc::make_mut(contract);
                                contract.scalar_requirements.sort();
                                contract.scalar_requirements.dedup();
                            }
                        }
                        arrival.publish_requirements();
                        for contract in arrival.values.values_mut() {
                            if !payload_predicates::closure_may_change(types, domains, contract) {
                                continue;
                            }
                            payload_predicates::close_with_shared_requirements(
                                types,
                                domains,
                                Arc::make_mut(contract),
                                &arrival.shared_requirements,
                                None,
                            );
                        }
                        let changed = if let Some(snapshot) = snapshots.get_mut(&edge.target) {
                            snapshot.meet(arrival)
                        } else {
                            snapshots.insert(edge.target, arrival);
                            true
                        };
                        if changed && !pending.contains(&edge.target) {
                            pending.push_back(edge.target);
                        }
                    }
                }
                if let Some(previous) = previous_relevant {
                    relevant.merge(previous);
                }
                replay.cached = Some(CachedHistory {
                    relevant,
                    snapshots,
                });
                failure = "requested payload root has no reached incoming membership history";
                let mut current = match cut {
                    PayloadMembershipCut::BeforeOperation => replay
                        .cached
                        .as_ref()?
                        .snapshots
                        .get(&position)?
                        .contract(root, None)?,
                    PayloadMembershipCut::NormalCompletion => {
                        completed.as_ref()?.contract(root, None)?
                    }
                };
                if cut == PayloadMembershipCut::NormalCompletion {
                    return Some(current);
                }
                failure = "requested source has no declared payload contract";
                let declared = source(root)?;
                failure = "current and declared source payload carriers differ";
                if current.carrier != declared.carrier {
                    return None;
                }
                let byte_domain = |carrier, domain| {
                    crate::byte_predicate_carrier(types.iter(), carrier)
                        && crate::byte_qualification_predicates_in(
                            domains.iter(),
                            carrier,
                            &[domain],
                        )
                        .is_some()
                };
                current.qualifications.extend(
                    declared
                        .qualifications
                        .into_iter()
                        .filter(|domain| byte_domain(declared.carrier, *domain)),
                );
                current.projected_qualifications.extend(
                    declared.projected_qualifications.into_iter().filter(|row| {
                        qualification_path_carrier(types.iter(), declared.carrier, &row.path)
                            .is_some_and(|carrier| byte_domain(carrier, row.domain))
                    }),
                );
                current.qualifications.sort();
                current.qualifications.dedup();
                current.projected_qualifications.sort();
                current.projected_qualifications.dedup();
                Some(current)
            })();
            result.ok_or(failure)
        }

        fn contains_rebound_premises(
            retained: &[SharedPremise],
            rebound: &[SharedPremise],
        ) -> bool {
            if rebound.windows(2).any(|pair| pair[0] > pair[1]) {
                return rebound
                    .iter()
                    .all(|premise| retained.binary_search(premise).is_ok());
            }
            let mut remaining = retained.iter().peekable();
            rebound
                .iter()
                .all(|premise| next_premise_is(&mut remaining, premise))
        }

        fn next_premise_is<'a, Premise: PartialOrd + 'a>(
            remaining: &mut std::iter::Peekable<impl Iterator<Item = &'a Premise>>,
            premise: &Premise,
        ) -> bool {
            while matches!(remaining.peek(), Some(candidate) if *candidate < premise) {
                remaining.next();
            }
            matches!(remaining.peek(), Some(candidate) if *candidate == premise)
        }

        pub(crate) fn extend_canonical_premises<
            Additional: Borrow<semantic_vocabulary::Proposition>,
        >(
            retained: &mut Vec<semantic_vocabulary::Proposition>,
            additional: &[Additional],
        ) {
            let mut existing = retained.iter().peekable();
            let introduces_premise = additional
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .any(|premise| !next_premise_is(&mut existing, premise));
            if !introduces_premise {
                return;
            }
            let mut combined = Vec::with_capacity(retained.len() + additional.len());
            let mut existing = std::mem::take(retained).into_iter().peekable();
            let mut additional = additional
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            while let (Some(left), Some(right)) = (existing.peek(), additional.peek()) {
                match left.cmp(right) {
                    std::cmp::Ordering::Less => combined.extend(existing.next()),
                    std::cmp::Ordering::Greater => combined.extend(additional.next().cloned()),
                    std::cmp::Ordering::Equal => {
                        combined.extend(existing.next());
                        additional.next();
                    }
                }
            }
            combined.extend(existing);
            combined.extend(additional.cloned());
            *retained = combined;
        }

        fn retain_common_snapshot(
            retained: &mut Arc<Vec<SharedPremise>>,
            incoming: &[SharedPremise],
        ) -> bool {
            let mut remaining = incoming.iter().peekable();
            let unchanged = retained
                .iter()
                .all(|premise| next_premise_is(&mut remaining, premise));
            if unchanged {
                return false;
            }
            let mut remaining = incoming.iter().peekable();
            Arc::make_mut(retained).retain(|premise| next_premise_is(&mut remaining, premise));
            true
        }

        fn canonicalize_snapshot(premises: &mut Arc<Vec<SharedPremise>>) {
            if premises.windows(2).any(|pair| pair[0] >= pair[1]) {
                let premises = Arc::make_mut(premises);
                premises.sort();
                premises.dedup();
            }
        }

        fn retain_common_premises<
            Retained: Borrow<semantic_vocabulary::Proposition>,
            Incoming: Borrow<semantic_vocabulary::Proposition>,
        >(
            retained: &mut Vec<Retained>,
            incoming: &[Incoming],
        ) -> bool {
            let previous = retained.len();
            let mut remaining = incoming
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            retained.retain(|premise| {
                let premise: &semantic_vocabulary::Proposition = premise.borrow();
                next_premise_is(&mut remaining, premise)
            });
            retained.len() != previous
        }

        fn retain_captured_premises<
            Local: Borrow<semantic_vocabulary::Proposition>,
            Common: Borrow<semantic_vocabulary::Proposition>,
        >(
            incoming: &mut Vec<semantic_vocabulary::Proposition>,
            local: &[Local],
            common: &[Common],
        ) {
            let mut local = local
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            let mut common = common
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            incoming.retain(|premise| {
                while local.peek().is_some_and(|candidate| *candidate < premise) {
                    local.next();
                }
                while common.peek().is_some_and(|candidate| *candidate < premise) {
                    common.next();
                }
                local.peek().is_some_and(|candidate| *candidate == premise)
                    || common.peek().is_some_and(|candidate| *candidate == premise)
            });
        }

        fn retain_common_memberships<Membership: Ord>(
            retained: &mut Vec<Membership>,
            incoming: &[Membership],
        ) {
            if retained.is_empty() {
                return;
            }
            if incoming.is_empty() {
                retained.clear();
                return;
            }
            if retained.as_slice() == incoming {
                return;
            }
            let mut incoming_memberships: Vec<_> = incoming.iter().collect();
            incoming_memberships.sort_unstable();
            retained.retain(|membership| incoming_memberships.binary_search(&membership).is_ok());
        }

        fn retire(
            content_independent: &[semantic_vocabulary::StructuralDomainId],
            contract: &mut PayloadMembershipContract,
            path: &[StructuralPathSegment],
            scalar_path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
        ) {
            contract.scalar_observations.retain(|observation| {
                !observation.path.starts_with(scalar_path)
                    && !scalar_path.starts_with(&observation.path)
            });
            let survives = |domain| content_independent.binary_search(&domain).is_ok();
            // A write beneath an already installed restriction does not issue
            // a new membership. Exact/ancestor replacement still retires it;
            // only the incoming value's checked transfer can supply it again.
            contract
                .qualifications
                .retain(|domain| !path.is_empty() && survives(*domain));
            contract.projected_qualifications.retain(|row| {
                !crate::structural_paths_may_overlap(&row.path, path)
                    || (path.len() > row.path.len()
                        && terminal_psi::StructuralPathQualification::selected_suffix(
                            &row.path,
                            &path[..row.path.len()],
                        )
                        .is_some()
                        && survives(row.domain))
            });
        }

        fn observe_scalar(
            contract: &mut PayloadMembershipContract,
            path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
            value: &semantic_vocabulary::ScalarTerm,
        ) -> Option<semantic_vocabulary::Proposition> {
            if let Some(previous) = contract.scalar_observations.iter_mut().find(|observation| {
                observation.path == path
                    && matches!(
                        value.scalar_type(),
                        semantic_vocabulary::ScalarType::Integer(_)
                            | semantic_vocabulary::ScalarType::Boolean
                    )
            }) {
                if let Some(observed) = &previous.value {
                    if observed.scalar_type() == value.scalar_type() {
                        return (observed != value).then(|| {
                            semantic_vocabulary::Proposition::Equal(value.clone(), observed.clone())
                        });
                    }
                } else if previous
                    .entry
                    .as_ref()
                    .is_some_and(|entry| entry.scalar_type == value.scalar_type())
                {
                    previous.value = Some(value.clone());
                    return None;
                }
            }
            contract
                .scalar_observations
                .retain(|observation| observation.path != path);
            contract.scalar_observations.push(PayloadScalarObservation {
                path: path.to_vec(),
                value: Some(value.clone()),
                entry: None,
            });
            None
        }

        fn clear(state: &mut PayloadContracts) {
            for contract in state.values_mut() {
                if contract.qualifications.is_empty()
                    && contract.projected_qualifications.is_empty()
                    && contract.scalar_observations.is_empty()
                {
                    continue;
                }
                let contract = Arc::make_mut(contract);
                contract.qualifications.clear();
                contract.projected_qualifications.clear();
                contract.scalar_observations.clear();
            }
        }

        fn append_scalar_observations(
            destination: &mut PayloadMembershipContract,
            source: &PayloadMembershipContract,
            prefix: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
        ) {
            extend_canonical_premises(
                &mut destination.scalar_requirements,
                &source.scalar_requirements,
            );
            for observation in &source.scalar_observations {
                let mut path = prefix.to_vec();
                path.extend_from_slice(&observation.path);
                destination
                    .scalar_observations
                    .retain(|previous| previous.path != path);
                destination
                    .scalar_observations
                    .push(PayloadScalarObservation {
                        path,
                        value: observation.value.clone(),
                        entry: observation.entry.clone(),
                    });
            }
        }

        fn retire_scalar_instances(
            state: &mut State,
            definitions: &[semantic_vocabulary::ValueId],
        ) {
            if definitions.is_empty() {
                return;
            }
            if state
                .scalar_requirements
                .iter()
                .any(|premise| premise.mentions_any(definitions))
            {
                Arc::make_mut(&mut state.scalar_requirements)
                    .retain(|premise| !premise.mentions_any(definitions));
            }
            if state
                .shared_requirements
                .iter()
                .any(|premise| !premise.survives_payload_retirement(definitions))
            {
                Arc::make_mut(&mut state.shared_requirements)
                    .retain(|premise| premise.survives_payload_retirement(definitions));
            }
            for contract in state.values.values_mut() {
                if !contract.scalar_observations.iter().any(|observation| {
                    (observation.value.is_none() && observation.entry.is_none())
                        || observation.value.as_ref().is_some_and(|value| {
                            value.any_value_id(|value| definitions.contains(&value))
                        })
                }) && !contract
                    .scalar_requirements
                    .iter()
                    .any(|premise| !survives_payload_retirement(premise, definitions))
                {
                    continue;
                }
                let contract = Arc::make_mut(contract);
                contract.scalar_observations.retain_mut(|observation| {
                    if observation.value.as_ref().is_some_and(|value| {
                        value.any_value_id(|value| definitions.contains(&value))
                    }) {
                        observation.value = None;
                    }
                    observation.value.is_some() || observation.entry.is_some()
                });
                retire_payload_premises(&mut contract.scalar_requirements, definitions);
            }
        }

        fn retire_payload_premises<Premise: Borrow<semantic_vocabulary::Proposition>>(
            premises: &mut Vec<Premise>,
            definitions: &[semantic_vocabulary::ValueId],
        ) {
            premises.retain(|premise| survives_payload_retirement(premise.borrow(), definitions));
        }

        fn survives_payload_retirement(
            premise: &semantic_vocabulary::Proposition,
            definitions: &[semantic_vocabulary::ValueId],
        ) -> bool {
            let mut changed = false;
            let closed = premise.visit_value_ids(|value| {
                changed |= definitions.contains(&value);
            });
            closed && !changed
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::descriptors::descriptor_lifetimes::{
                DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource,
                DescriptorPathSegment,
            };
            use semantic_vocabulary::{
                BlockId, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
            };

            fn position(block: u64) -> DescriptorPosition {
                (BlockId::new(block).unwrap(), 0)
            }

            fn condition(value: u64) -> Proposition {
                Proposition::Equal(
                    ScalarTerm::Boolean(true),
                    ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
                )
            }

            fn forked_history(branches: &[u64]) -> (DescriptorMachine, PayloadMembershipContract) {
                let root = PlaceId::new(1).unwrap();
                let carrier = StructuralTypeId::new(1).unwrap();
                let step = |targets: &[u64]| DescriptorFlowStep {
                    successors: targets
                        .iter()
                        .map(|target| DescriptorFlowEdge {
                            edge: None,
                            effects: Vec::new(),
                            definitions: Vec::new(),
                            target: position(*target),
                            parameters: Vec::new(),
                            bindings: Vec::new(),
                        })
                        .collect(),
                    ..Default::default()
                };
                let machine = DescriptorMachine {
                    id: MachineId::new(1).unwrap(),
                    returns: None,
                    places: vec![DescriptorPlace {
                        producer: None,
                        place: root,
                        definition: DescriptorDefinition::Parameter {
                            position: 0,
                            access: StructuralAccess::Owned,
                            structural_type: carrier,
                        },
                    }],
                    place_types: BTreeMap::from([(root, carrier)]),
                    entry: position(1),
                    flow: BTreeMap::from([
                        (position(1), step(branches)),
                        (position(2), step(&[4])),
                        (position(3), step(&[4])),
                        (position(4), step(&[])),
                    ]),
                    originations: BTreeMap::new(),
                    projections: BTreeMap::new(),
                    contents_queries: BTreeMap::new(),
                    contents_transfers: BTreeMap::new(),
                    payload_producers: BTreeMap::new(),
                    payload_entry_predicates: Some((
                        semantic_vocabulary::PropositionContext::from_value_types([
                            (ValueId::new(1).unwrap(), ScalarType::Boolean),
                            (ValueId::new(2).unwrap(), ScalarType::Boolean),
                        ])
                        .unwrap(),
                        vec![condition(2)],
                    )),
                    payload_scalar_definitions: BTreeMap::new(),
                    payload_scalar_postconditions: BTreeMap::from([(
                        position(2),
                        vec![condition(1)],
                    )]),
                    payload_scalar_parameters: BTreeMap::new(),
                    scalar_payload_reads: BTreeMap::new(),
                    payload_edge_predicates: BTreeMap::new(),
                    payload_scalar_bindings: BTreeMap::new(),
                    scalar_payload_writes: BTreeMap::new(),
                    payload_invalidations: BTreeMap::new(),
                    case_constructors: BTreeMap::new(),
                    case_successors: BTreeMap::new(),
                    unresolved_contents_writes: BTreeSet::new(),
                    storage_derivations: BTreeSet::new(),
                    calls: BTreeMap::new(),
                    normal_exits: vec![position(4)],
                };
                let contract = PayloadMembershipContract {
                    carrier,
                    scalar_requirements: vec![condition(1)],
                    scalar_observations: Vec::new(),
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                (machine, contract)
            }

            #[test]
            fn shared_history_refuses_another_inventory_or_same_coordinate_graph() {
                let (machine, contract) = forked_history(&[2, 3]);
                let identity = machine.id;
                let entry = machine.entry;
                let graph = [machine];
                let root = PlaceId::new(1).unwrap();
                let contracts = [(root, contract)];
                let mut history = PayloadMembershipHistory::for_current_contracts(
                    &[],
                    &[],
                    &graph,
                    identity,
                    &[],
                    &contracts,
                )
                .unwrap();
                let mut views = DescriptorAnalysis::new(&[], &graph);
                history
                    .current_payload_contract_in(&mut views, entry, root)
                    .unwrap();
                assert!(history.replay.cached.is_some());

                let mut borrowed = DescriptorAnalysis::with_borrowed_storage_slots(&[], &graph);
                assert_eq!(
                    history
                        .current_payload_contract_in(&mut borrowed, entry, root)
                        .err(),
                    Some("payload history analysis changed its descriptor inventory")
                );
                let other_graph = [forked_history(&[2, 3]).0];
                let mut same_coordinates = DescriptorAnalysis::new(&[], &other_graph);
                assert_eq!(
                    history
                        .current_payload_contract_in(&mut same_coordinates, entry, root)
                        .err(),
                    Some("payload history analysis belongs to another immutable source graph")
                );
                assert!(history.replay.cached.is_some());
                history
                    .current_payload_contract_in(&mut views, entry, root)
                    .unwrap();
                assert_eq!(history.replay.inventory, Some(DescriptorSlotKind::Views));
            }

            #[test]
            fn entry_observation_does_not_inherit_an_unpublished_ambient_premise() {
                let (machine, source) = forked_history(&[2, 3]);
                let observed = current_payload_contract(
                    &[],
                    &[],
                    std::slice::from_ref(&machine),
                    machine.id,
                    machine.entry,
                    PlaceId::new(1).unwrap(),
                    &|_| Some(source.clone()),
                )
                .unwrap();
                assert_eq!(observed.scalar_requirements, vec![condition(1)]);
            }

            #[test]
            fn joined_payload_keeps_both_paths_facts_but_not_a_retired_instance() {
                for branches in [[2, 3], [3, 2]] {
                    for retire in [false, true] {
                        let (mut machine, source) = forked_history(&branches);
                        if retire {
                            machine
                                .payload_scalar_definitions
                                .insert(position(3), vec![ValueId::new(1).unwrap()]);
                        }
                        let observed = current_payload_contract(
                            &[],
                            &[],
                            std::slice::from_ref(&machine),
                            machine.id,
                            position(4),
                            PlaceId::new(1).unwrap(),
                            &|_| Some(source.clone()),
                        )
                        .unwrap();
                        let mut expected = vec![condition(2)];
                        if !retire {
                            expected.push(condition(1));
                        }
                        expected.sort();
                        assert_eq!(observed.scalar_requirements, expected);
                    }
                }
            }

            #[test]
            fn predicate_edge_captures_both_arrival_routes_before_instance_retirement() {
                use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

                let root = PlaceId::new(1).unwrap();
                let selected = PlaceId::new(2).unwrap();
                let predicate = Proposition::Conjunction(vec![condition(3), condition(4)]);
                for retire in [false, true] {
                    let (mut machine, source) = forked_history(&[2]);
                    let _ = machine.flow.remove(&position(3));
                    let types = [StructuralTypeDeclaration {
                        declaration_identity: Default::default(),
                        requires: Vec::new(),
                        quotient_carrier: None,
                        id: source.carrier,
                        identity: TerminalIdentityIndex(1),
                        owns_linear_claim: false,
                        no_code_affine_cleanup: true,
                        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                        descriptor_cleanup: true,
                        nominal_cleanup: None,
                        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                        scalar_restriction: None,
                    }];
                    machine.places.push(DescriptorPlace {
                        producer: None,
                        place: selected,
                        definition: DescriptorDefinition::Local,
                    });
                    machine.place_types.insert(selected, source.carrier);
                    let edge = &mut machine.flow.get_mut(&position(2)).unwrap().successors[0];
                    edge.parameters = vec![
                        DescriptorLocation::whole(root),
                        DescriptorLocation::whole(selected),
                    ];
                    edge.bindings = vec![
                        DescriptorFlowBinding::new(
                            DescriptorLocation::whole(root),
                            DescriptorLocation::whole(root),
                            StructuralAccess::Owned,
                            StructuralAccess::Owned,
                        )
                        .unwrap(),
                        DescriptorFlowBinding::new(
                            DescriptorFlowSource::Contents(DescriptorContentsQuery {
                                root,
                                steps: Vec::new(),
                            }),
                            DescriptorLocation::whole(selected),
                            StructuralAccess::Owned,
                            StructuralAccess::Owned,
                        )
                        .unwrap(),
                    ];
                    machine
                        .payload_edge_predicates
                        .insert(position(2), vec![Some(predicate.clone())]);
                    if retire {
                        machine
                            .payload_scalar_parameters
                            .insert(position(4).0, vec![ValueId::new(3).unwrap()]);
                    }
                    let mut expected = vec![condition(1), condition(2), condition(4)];
                    if !retire {
                        expected.extend([condition(3), predicate.clone()]);
                    }
                    expected.sort();
                    for payload in [root, selected] {
                        let observed = current_payload_contract(
                            &types,
                            &[],
                            std::slice::from_ref(&machine),
                            machine.id,
                            position(4),
                            payload,
                            &|_| Some(source.clone()),
                        )
                        .unwrap();
                        assert_eq!(observed.scalar_requirements, expected);
                    }
                }
            }

            #[test]
            fn published_payload_retains_capacity_and_its_complete_theory() {
                let root = PlaceId::new(1).unwrap();
                let carrier = StructuralTypeId::new(1).unwrap();
                let mut requirements = Vec::with_capacity(32);
                requirements.extend([condition(1), condition(2)]);
                requirements.sort();
                let retained_capacity = requirements.capacity();
                let contract = PayloadMembershipContract {
                    carrier,
                    scalar_observations: Vec::new(),
                    scalar_requirements: requirements,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let mut state = State {
                    values: PayloadContracts::from([(root, contract)]),
                    scalar_requirements: Arc::new(vec![SharedPremise::new(condition(1))]),
                    ..Default::default()
                };
                let before = state.contract(root, None).unwrap();
                state.publish_requirements();
                assert_eq!(state.contract(root, None).unwrap(), before);
                let local = &state.values[&root].scalar_requirements;
                assert_eq!(local, &[condition(2)]);
                assert_eq!(local.capacity(), retained_capacity);
                state.publish_requirements();
                assert_eq!(state.contract(root, None).unwrap(), before);
                assert_eq!(
                    state.values[&root].scalar_requirements.capacity(),
                    retained_capacity
                );
            }

            #[test]
            fn equal_raw_observations_still_normalize_at_meet() {
                let root = PlaceId::new(1).unwrap();
                let observed = super::super::PayloadScalarObservation {
                    path: Vec::new(),
                    value: Some(ScalarTerm::Boolean(true)),
                    entry: None,
                };
                for observations in [
                    vec![observed.clone(), observed],
                    vec![super::super::PayloadScalarObservation {
                        path: Vec::new(),
                        value: None,
                        entry: None,
                    }],
                ] {
                    for different_ambient in [false, true] {
                        let mut retained = State {
                            values: PayloadContracts::from([(
                                root,
                                PayloadMembershipContract {
                                    carrier: StructuralTypeId::new(1).unwrap(),
                                    scalar_observations: observations.clone(),
                                    scalar_requirements: Vec::new(),
                                    qualifications: Vec::new(),
                                    projected_qualifications: Vec::new(),
                                },
                            )]),
                            ..Default::default()
                        };
                        let mut arrival = retained.clone();
                        if different_ambient {
                            arrival.scalar_requirements =
                                Arc::new(vec![SharedPremise::new(condition(1))]);
                        }
                        let changed = retained.meet(arrival);
                        assert!(
                            retained
                                .values
                                .get(&root)
                                .unwrap()
                                .scalar_observations
                                .is_empty()
                        );
                        assert_eq!(changed, observations.len() == 2);
                    }
                }
            }

            #[test]
            fn untouched_payload_rows_stay_shared_and_changed_rows_detach() {
                let root = PlaceId::new(1).unwrap();
                let other = PlaceId::new(2).unwrap();
                let contract = PayloadMembershipContract {
                    carrier: StructuralTypeId::new(1).unwrap(),
                    scalar_observations: Vec::new(),
                    scalar_requirements: vec![condition(1)],
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let original = State {
                    values: PayloadContracts::from([(root, contract.clone()), (other, contract)]),
                    ..Default::default()
                };
                let mut later = original.clone();
                later.publish_requirements();
                retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
                clear(&mut later.values);
                assert!(!super::super::payload_predicates::closure_may_change(
                    &[],
                    &[],
                    later.values.get(&root).unwrap()
                ));
                for root in [root, other] {
                    assert!(Arc::ptr_eq(
                        &original.values.0[&root],
                        &later.values.0[&root]
                    ));
                }
                later
                    .values
                    .get_mut(&root)
                    .unwrap()
                    .scalar_requirements
                    .push(condition(2));
                assert!(!Arc::ptr_eq(
                    &original.values.0[&root],
                    &later.values.0[&root]
                ));
                assert!(Arc::ptr_eq(
                    &original.values.0[&other],
                    &later.values.0[&other]
                ));
                assert_eq!(
                    original.values.get(&root).unwrap().scalar_requirements,
                    vec![condition(1)]
                );
                assert_eq!(
                    later.values.get(&root).unwrap().scalar_requirements,
                    vec![condition(1), condition(2)]
                );
            }

            #[test]
            fn publication_and_retirement_detach_only_affected_payload_rows() {
                let root = PlaceId::new(1).unwrap();
                let other = PlaceId::new(2).unwrap();
                let contract = |premise| PayloadMembershipContract {
                    carrier: StructuralTypeId::new(1).unwrap(),
                    scalar_observations: Vec::new(),
                    scalar_requirements: vec![condition(premise)],
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let original = State {
                    values: PayloadContracts::from([(root, contract(1)), (other, contract(2))]),
                    ..Default::default()
                };
                let mut published = original.clone();
                published.scalar_requirements = Arc::new(vec![SharedPremise::new(condition(1))]);
                published.publish_requirements();
                assert!(!Arc::ptr_eq(
                    &original.values.0[&root],
                    &published.values.0[&root]
                ));
                assert!(Arc::ptr_eq(
                    &original.values.0[&other],
                    &published.values.0[&other]
                ));
                assert!(
                    published
                        .values
                        .get(&root)
                        .unwrap()
                        .scalar_requirements
                        .is_empty()
                );
                let mut retired = original.clone();
                retire_scalar_instances(&mut retired, &[ValueId::new(1).unwrap()]);
                assert!(!Arc::ptr_eq(
                    &original.values.0[&root],
                    &retired.values.0[&root]
                ));
                assert!(Arc::ptr_eq(
                    &original.values.0[&other],
                    &retired.values.0[&other]
                ));
                assert!(
                    retired
                        .values
                        .get(&root)
                        .unwrap()
                        .scalar_requirements
                        .is_empty()
                );
                assert_eq!(
                    original.values.get(&root).unwrap().scalar_requirements,
                    vec![condition(1)]
                );
            }

            // Only contents strictly inside an existing subject may change
            // without replacing that subject's issuer-bound membership.
            #[test]
            fn content_writes_keep_only_existing_content_independent_memberships() {
                use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
                use terminal_psi::{
                    StructuralDomainDeclaration, StructuralDomainPredicate,
                    StructuralPathQualification,
                };
                let id = |raw| StructuralDomainId::new(raw).unwrap();
                let semantic = |raw| DomainSemanticId::new(raw).unwrap();
                let carrier = StructuralTypeId::new(1).unwrap();
                let domain = |raw| StructuralDomainDeclaration {
                    required_memberships: Some(Vec::new()),
                    complete_predicate_theory: true,
                    predicate: Some(StructuralDomainPredicate {
                        subject: PlaceId::new(1).unwrap(),
                        proposition: Proposition::Truth,
                    }),
                    id: id(raw),
                    semantic_domain: semantic(raw),
                    identity: Default::default(),
                    subject_identity: Default::default(),
                    carrier,
                    content_projection: None,
                    byte_predicate: None,
                    establishment_routes: Some(Vec::new()),
                    access_policy: None,
                };
                let mut predicate = domain(2);
                predicate.predicate = Some(StructuralDomainPredicate {
                    subject: PlaceId::new(1).unwrap(),
                    proposition: Proposition::Falsehood,
                });
                let mut incomplete = domain(3);
                incomplete.complete_predicate_theory = false;
                let mut predicate_dependency = domain(4);
                predicate_dependency.required_memberships = Some(vec![semantic(2)]);
                let mut independent_dependency = domain(5);
                independent_dependency.required_memberships = Some(vec![semantic(1)]);
                let mut unknown = domain(6);
                unknown.required_memberships = None;
                let mut bytes = domain(7);
                bytes.byte_predicate = Some(terminal_psi::ByteSequencePredicate::AsciiOnly);
                let domains = [
                    domain(1),
                    predicate,
                    incomplete,
                    predicate_dependency,
                    independent_dependency,
                    unknown,
                    bytes,
                ];
                let content_independent =
                    StructuralDomainDeclaration::content_independent_memberships(&domains);
                assert_eq!(content_independent, [id(1), id(5)]);
                let mut malformed = domains.to_vec();
                let mut cycle = domain(8);
                cycle.required_memberships = Some(vec![semantic(8)]);
                let mut missing = domain(9);
                missing.required_memberships = Some(vec![semantic(99)]);
                let mut missing_predicate = domain(10);
                missing_predicate.predicate = None;
                malformed.extend([cycle, missing, missing_predicate]);
                assert_eq!(
                    StructuralDomainDeclaration::content_independent_memberships(&malformed),
                    content_independent
                );
                malformed.push(domain(1));
                assert!(
                    StructuralDomainDeclaration::content_independent_memberships(&malformed)
                        .is_empty()
                );
                let mut ambiguous = domains.to_vec();
                let mut duplicate_semantic = domain(10);
                duplicate_semantic.semantic_domain = semantic(1);
                ambiguous.push(duplicate_semantic);
                assert!(
                    StructuralDomainDeclaration::content_independent_memberships(&ambiguous)
                        .is_empty()
                );
                let first = StructuralPathSegment::FixedIndex(0);
                let second = StructuralPathSegment::FixedIndex(1);
                let original = PayloadMembershipContract {
                    carrier,
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    qualifications: (1..=7).map(id).collect(),
                    projected_qualifications: (1..=7)
                        .map(|raw| StructuralPathQualification {
                            path: vec![first.clone()],
                            domain: id(raw),
                        })
                        .chain([StructuralPathQualification {
                            path: vec![second.clone()],
                            domain: id(2),
                        }])
                        .collect(),
                };
                let mut changed = original.clone();
                retire(
                    &content_independent,
                    &mut changed,
                    std::slice::from_ref(&first),
                    &[DescriptorPathSegment::FixedIndex(0)],
                );
                assert_eq!(changed.qualifications, [id(1), id(5)]);
                assert_eq!(
                    changed.projected_qualifications,
                    [StructuralPathQualification {
                        path: vec![second],
                        domain: id(2)
                    }],
                    "an exact field replacement loses its tag; an unrelated sibling survives"
                );
                let mut nested = original.clone();
                retire(
                    &content_independent,
                    &mut nested,
                    &[first.clone(), StructuralPathSegment::FixedIndex(0)],
                    &[
                        DescriptorPathSegment::FixedIndex(0),
                        DescriptorPathSegment::FixedIndex(0),
                    ],
                );
                assert_eq!(nested.qualifications, [id(1), id(5)]);
                assert_eq!(
                    nested
                        .projected_qualifications
                        .iter()
                        .map(|row| row.domain)
                        .collect::<Vec<_>>(),
                    [id(1), id(5), id(2)]
                );
                let mut replaced = original.clone();
                retire(&content_independent, &mut replaced, &[], &[]);
                assert!(
                    replaced.qualifications.is_empty()
                        && replaced.projected_qualifications.is_empty(),
                    "whole replacement or moved-out slot reuse cannot reissue an old tag"
                );
                let root = PlaceId::new(1).unwrap();
                let mut unknown_write = PayloadContracts::from([(root, original)]);
                clear(&mut unknown_write);
                assert!(
                    unknown_write.get(&root).unwrap().qualifications.is_empty(),
                    "unknown effects retain no content-independent membership shortcut"
                );
                retire(
                    &content_independent,
                    &mut replaced,
                    &[first],
                    &[DescriptorPathSegment::FixedIndex(0)],
                );
                assert!(
                    replaced.qualifications.is_empty(),
                    "an unqualified new producer gains nothing"
                );
            }

            #[test]
            fn retained_payload_snapshot_survives_later_writes_retirement_and_closure() {
                use semantic_vocabulary::{PropositionId, StructuralDomainId};

                let root = PlaceId::new(1).unwrap();
                let path = vec![DescriptorPathSegment::FixedIndex(0)];
                let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Boolean);
                let domain = |id| StructuralDomainId::new(id).unwrap();
                let mut requirements = vec![
                    condition(1),
                    Proposition::Atom(PropositionId::new(1).unwrap()),
                    Proposition::Conjunction(vec![
                        Proposition::Atom(PropositionId::new(1).unwrap()),
                        condition(2),
                    ]),
                ];
                requirements.sort();
                let shared = Arc::new(
                    requirements
                        .iter()
                        .cloned()
                        .map(SharedPremise::new)
                        .collect(),
                );
                let original = PayloadMembershipContract {
                    carrier: StructuralTypeId::new(1).unwrap(),
                    scalar_observations: vec![super::super::PayloadScalarObservation {
                        path: path.clone(),
                        value: Some(value(1)),
                        entry: None,
                    }],
                    scalar_requirements: requirements,
                    qualifications: vec![domain(2), domain(1), domain(2)],
                    projected_qualifications: Vec::new(),
                };
                let retained = State {
                    values: PayloadContracts::from([(root, original.clone())]),
                    scalar_requirements: Arc::clone(&shared),
                    shared_requirements: shared,
                };
                enum Change {
                    Store,
                    UnknownWrite,
                    RetireInstance,
                    Close,
                }
                for change in [
                    Change::Store,
                    Change::UnknownWrite,
                    Change::RetireInstance,
                    Change::Close,
                ] {
                    let mut later = retained.clone();
                    let mut expected = original.clone();
                    match change {
                        Change::Store => {
                            let contract = later.values.get_mut(&root).unwrap();
                            retire(
                                &[],
                                contract,
                                &[StructuralPathSegment::FixedIndex(0)],
                                &path,
                            );
                            assert_eq!(observe_scalar(contract, &path, &value(2)), None);
                            expected.scalar_observations[0].value = Some(value(2));
                            expected.qualifications.clear();
                        }
                        Change::UnknownWrite => {
                            clear(&mut later.values);
                            expected.scalar_observations.clear();
                            expected.qualifications.clear();
                        }
                        Change::RetireInstance => {
                            retire_scalar_instances(&mut later, &[ValueId::new(1).unwrap()]);
                            expected.scalar_observations.clear();
                            expected.scalar_requirements.clear();
                            assert!(later.shared_requirements.is_empty());
                            assert_eq!(later.scalar_requirements.len(), 2);
                            assert!(later.scalar_requirements.iter().all(|premise| {
                                !premise
                                    .as_ref()
                                    .any_value_id(|value| value == ValueId::new(1).unwrap())
                            }));
                        }
                        Change::Close => {
                            super::super::payload_predicates::close_with_shared_requirements::<
                                Proposition,
                            >(
                                &[], &[], later.values.get_mut(&root).unwrap(), &[], None
                            );
                            expected.qualifications = vec![domain(1), domain(2)];
                        }
                    }
                    assert!(!Arc::ptr_eq(
                        &retained.values.0[&root],
                        &later.values.0[&root]
                    ));
                    assert_eq!(retained.contract(root, None).unwrap(), original);
                    assert_eq!(later.contract(root, None).unwrap(), expected);
                }
                let mut later = retained.clone();
                retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
                assert_eq!(later.scalar_requirements.len(), 2);
                assert!(later.scalar_requirements.iter().all(|premise| {
                    !premise
                        .as_ref()
                        .any_value_id(|value| value == ValueId::new(2).unwrap())
                }));
                assert_eq!(later.shared_requirements.len(), 1);
                assert_eq!(later.shared_requirements[0].as_ref(), &condition(1));
            }

            #[test]
            fn membership_filter_preserves_equal_raw_rosters_and_filters_different_ones() {
                for original in [vec![], vec![1], vec![3, 1, 3, 2, 1]] {
                    let mut retained = original.clone();
                    retain_common_memberships(&mut retained, &original);
                    assert_eq!(retained, original);
                }
                for (original, incoming) in [
                    (vec![3, 1, 3, 2, 1], vec![2, 3, 2]),
                    (vec![3, 1, 3, 2, 1], vec![1, 2, 3]),
                    (vec![3, 1], vec![]),
                    (vec![], vec![3, 1]),
                    (vec![3, 1], vec![2]),
                ] {
                    let expected: Vec<_> = original
                        .iter()
                        .copied()
                        .filter(|value| incoming.contains(value))
                        .collect();
                    let mut retained = original;
                    retain_common_memberships(&mut retained, &incoming);
                    assert_eq!(retained, expected);
                }
            }

            #[test]
            fn rereads_preserve_common_observation_but_writes_and_instances_retire_it() {
                let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
                let value = |identity| {
                    ScalarTerm::value(
                        ValueId::new(identity).unwrap(),
                        ScalarType::Integer(integer),
                    )
                };
                let path = vec![DescriptorPathSegment::FixedIndex(0)];
                let mut contract = PayloadMembershipContract {
                    carrier: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                assert_eq!(observe_scalar(&mut contract, &path, &value(1)), None);
                let mut other_branch = contract.clone();
                assert_eq!(
                    observe_scalar(&mut contract, &path, &value(2)),
                    Some(Proposition::Equal(value(2), value(1)))
                );
                assert_eq!(
                    observe_scalar(&mut other_branch, &path, &value(3)),
                    Some(Proposition::Equal(value(3), value(1)))
                );
                assert_eq!(
                    contract.scalar_observations,
                    other_branch.scalar_observations
                );
                retire(
                    &[],
                    &mut contract,
                    &[StructuralPathSegment::FixedIndex(0)],
                    &path,
                );
                assert_eq!(observe_scalar(&mut contract, &path, &value(4)), None);
                assert_eq!(contract.scalar_observations[0].value, Some(value(4)));
                let root = PlaceId::new(1).unwrap();
                let mut state = State {
                    values: PayloadContracts::from([(root, other_branch)]),
                    scalar_requirements: Arc::default(),
                    shared_requirements: Arc::default(),
                };
                retire_scalar_instances(&mut state, &[ValueId::new(1).unwrap()]);
                assert_eq!(
                    observe_scalar(state.values.get_mut(&root).unwrap(), &path, &value(5)),
                    None
                );
            }
        }
    }
    pub(crate) use payload_history::completed_payload_contract;
    pub(crate) mod payload_predicates {
        use super::PayloadMembershipContract;
        use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
        use semantic_vocabulary::ValueId;
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, Proposition, PropositionContext, ScalarTerm,
            ScalarType, StructuralPlaceKind,
        };
        use std::borrow::{Borrow, Cow};
        use terminal_psi::{StructuralDomainDeclaration, StructuralTypeDeclaration};

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct PayloadScalarObservation {
            pub path: Vec<DescriptorPathSegment>,
            pub value: Option<ScalarTerm>,
            pub entry: Option<PayloadEntryScalarIdentity>,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct PayloadEntryScalarIdentity {
            pub root: semantic_vocabulary::PlaceId,
            pub path: Vec<DescriptorPathSegment>,
            pub scalar_type: ScalarType,
        }

        impl PayloadScalarObservation {
            pub(crate) fn intersect(retained: &mut Vec<Self>, incoming: &[Self]) -> bool {
                let mut changed = false;
                retained.retain_mut(|observation| {
                    let mut rows = incoming.iter().filter(|row| row.path == observation.path);
                    let Some(row) = rows.next() else {
                        changed = true;
                        return false;
                    };
                    if rows.next().is_some() {
                        changed = true;
                        return false;
                    }
                    if observation.value != row.value && observation.value.take().is_some() {
                        changed = true;
                    }
                    if observation.entry != row.entry && observation.entry.take().is_some() {
                        changed = true;
                    }
                    observation.value.is_some() || observation.entry.is_some()
                });
                changed
            }
        }

        pub(crate) fn canonical_path(
            path: &[DescriptorPathSegment],
        ) -> Option<Vec<CanonicalStructuralPathSegment>> {
            path.iter()
                .map(|segment| match segment {
                    DescriptorPathSegment::Field(field) => {
                        Some(CanonicalStructuralPathSegment::Field(*field))
                    }
                    DescriptorPathSegment::FixedIndex(index) => {
                        Some(CanonicalStructuralPathSegment::FixedIndex(*index))
                    }
                    DescriptorPathSegment::Case(case) => {
                        Some(CanonicalStructuralPathSegment::Case(*case))
                    }
                    DescriptorPathSegment::Referent => {
                        Some(CanonicalStructuralPathSegment::Referent)
                    }
                    DescriptorPathSegment::RepresentationView { .. }
                    | DescriptorPathSegment::RuntimeIndex { .. }
                    | DescriptorPathSegment::Extent => None,
                })
                .collect()
        }

        #[cfg(test)]
        #[test]
        fn storage_presentations_and_selectors_do_not_become_static_proof_paths() {
            let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
            assert_eq!(
                canonical_path(&[DescriptorPathSegment::Field(field)]),
                Some(vec![CanonicalStructuralPathSegment::Field(field)])
            );
            for segment in [
                DescriptorPathSegment::RepresentationView {
                    structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    stored_scalar: None,
                },
                DescriptorPathSegment::RuntimeIndex {
                    selector: semantic_vocabulary::ValueId::new(1).unwrap(),
                },
                DescriptorPathSegment::Extent,
            ] {
                assert_eq!(
                    canonical_path(&[DescriptorPathSegment::Field(field), segment]),
                    None
                );
            }
        }

        pub(crate) fn observation_type(
            types: &[StructuralTypeDeclaration],
            carrier: semantic_vocabulary::StructuralTypeId,
            path: &[DescriptorPathSegment],
        ) -> Option<ScalarType> {
            let path =
                crate::descriptors::descriptor_lifetimes::descriptor_path(types, carrier, path)?;
            crate::primitive_read_projection_type(types.iter(), carrier, &path)
        }

        pub(crate) fn close(
            types: &[StructuralTypeDeclaration],
            domains: &[StructuralDomainDeclaration],
            contract: &mut PayloadMembershipContract,
        ) {
            close_with_shared_requirements::<Proposition>(types, domains, contract, &[], None);
        }

        pub(crate) fn closure_may_change(
            types: &[StructuralTypeDeclaration],
            domains: &[StructuralDomainDeclaration],
            contract: &PayloadMembershipContract,
        ) -> bool {
            contract
                .qualifications
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
                || domains.iter().any(|domain| {
                    domain.carrier == contract.carrier
                        && domain.content_projection.is_none()
                        && domain.establishment_routes.is_none()
                        && !contract.qualifications.contains(&domain.id)
                        && introduction_predicate(types, domains, domain, &contract.qualifications)
                            .is_some()
                })
        }

        pub(crate) fn close_with_shared_requirements<Shared: Borrow<Proposition>>(
            types: &[StructuralTypeDeclaration],
            domains: &[StructuralDomainDeclaration],
            contract: &mut PayloadMembershipContract,
            shared: &[Shared],
            edge_predicate: Option<&Proposition>,
        ) {
            let mut requirements = None;
            for domain in domains {
                if domain.carrier != contract.carrier
                    || domain.content_projection.is_some()
                    || domain.establishment_routes.is_some()
                    || contract.qualifications.contains(&domain.id)
                {
                    continue;
                }
                let Some(predicate) =
                    introduction_predicate(types, domains, domain, &contract.qualifications)
                else {
                    continue;
                };
                let requirements = requirements.get_or_insert_with(|| {
                    if shared.is_empty() && edge_predicate.is_none() {
                        Cow::Borrowed(contract.scalar_requirements.as_slice())
                    } else {
                        let mut premises = contract.scalar_requirements.clone();
                        super::payload_history::extend_canonical_premises(&mut premises, shared);
                        if let Some(predicate) = edge_predicate
                            && !premises.contains(predicate)
                        {
                            premises.push(predicate.clone());
                        }
                        Cow::Owned(premises)
                    }
                });
                let mut value_types = Vec::new();
                for observation in &contract.scalar_observations {
                    if let Some(value) = &observation.value {
                        collect_value_types(value, &mut value_types);
                    }
                }
                for premise in requirements.iter() {
                    collect_proposition_types(premise, &mut value_types);
                }
                let Ok(context) = PropositionContext::from_value_types_and_places(
                    value_types,
                    [(
                        predicate.subject,
                        StructuralPlaceKind::Parameter { position: 0 },
                    )],
                ) else {
                    continue;
                };
                let mut equations = requirements.to_vec();
                for observation in &contract.scalar_observations {
                    let Some(value) = &observation.value else {
                        continue;
                    };
                    let Some(path) = canonical_path(&observation.path) else {
                        continue;
                    };
                    let scalar_type = value.scalar_type();
                    if observation_type(types, contract.carrier, &observation.path)
                        != Some(scalar_type)
                    {
                        continue;
                    }
                    let observed = match scalar_type {
                        ScalarType::Integer(integer) => {
                            ScalarTerm::integer_field_path(predicate.subject, path, integer)
                        }
                        ScalarType::Boolean => {
                            ScalarTerm::boolean_field_path(predicate.subject, path)
                        }
                        ScalarType::IeeeFloat(_) | ScalarType::Callable(_) => continue,
                    };
                    equations.push(Proposition::Equal(observed, value.clone()));
                }
                predicate.proposition.visit_scalar_terms(|term| {
                    if let Some(value) =
                        observed_byte_predicate(types, contract, predicate.subject, term)
                    {
                        equations
                            .push(Proposition::Equal(term.clone(), ScalarTerm::Boolean(value)));
                    }
                });
                let checked = proof_admission::check_predicate_denotations_with_value_equalities(
                    &context,
                    &predicate.proposition,
                    requirements,
                    &equations,
                );
                if checked.is_ok_and(|checked| {
                    super::payload_entry::prove(&context, checked.goal(), checked.requirements(), 0)
                        .is_some_and(|proof| checked.check_certificate(&context, &proof).is_ok())
                }) {
                    contract.qualifications.push(domain.id);
                }
            }
            contract.qualifications.sort();
            contract.qualifications.dedup();
        }

        fn collect_value_types(term: &ScalarTerm, output: &mut Vec<(ValueId, ScalarType)>) {
            term.visit_terms(|term| {
                if let ScalarTerm::Value { id, scalar_type } = term {
                    output.push((*id, *scalar_type));
                }
            });
        }

        fn collect_proposition_types(
            proposition: &Proposition,
            output: &mut Vec<(ValueId, ScalarType)>,
        ) {
            proposition.visit_scalar_terms(|term| {
                if let ScalarTerm::Value { id, scalar_type } = term {
                    output.push((*id, *scalar_type));
                }
            });
        }

        fn observed_byte_predicate(
            types: &[StructuralTypeDeclaration],
            contract: &PayloadMembershipContract,
            subject: semantic_vocabulary::PlaceId,
            term: &ScalarTerm,
        ) -> Option<bool> {
            let ScalarTerm::ByteSequencePredicate {
                predicate,
                root,
                path,
            } = term
            else {
                return None;
            };
            if *root != subject {
                return None;
            }
            let selected =
                crate::canonical_structural_path_tip(types.iter(), contract.carrier, path)?;
            let terminal_psi::StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = selected.shape
            else {
                return None;
            };
            if !crate::byte_predicate_carrier(types.iter(), selected.id)
                || length > contract.scalar_observations.len() as u64
            {
                return None;
            }
            let mut bytes = Vec::new();
            for index in 0..length {
                let mut element_path = path.clone();
                element_path.push(CanonicalStructuralPathSegment::FixedIndex(index));
                let mut matching = contract.scalar_observations.iter().filter(|observation| {
                    canonical_path(&observation.path).as_ref() == Some(&element_path)
                });
                let observation = matching.next()?;
                let observed = observation.value.as_ref()?;
                if matching.next().is_some()
                    || observation_type(types, contract.carrier, &observation.path)
                        != Some(observed.scalar_type())
                {
                    return None;
                }
                let ScalarTerm::Integer { value, .. } = observed else {
                    return None;
                };
                let byte = match value {
                    semantic_vocabulary::IntegerValue::Unsigned(value) => {
                        u8::try_from(*value).ok()?
                    }
                    semantic_vocabulary::IntegerValue::Signed(value) => {
                        u8::try_from(*value).ok()?
                    }
                };
                bytes.push(byte);
            }
            Some(predicate.holds_for(&bytes))
        }

        pub(crate) fn terminal_scalar_literal(
            index: &crate::MachineScalarIndex<'_>,
            value: ValueId,
        ) -> Option<ScalarTerm> {
            use terminal_psi::OperationKind;
            if let Some(literal) =
                index
                    .definitions(value)
                    .find_map(|operation| match operation.kind {
                        OperationKind::BooleanConstant { value } => {
                            Some(ScalarTerm::Boolean(value))
                        }
                        _ => None,
                    })
            {
                return Some(literal);
            }
            let (scalar_type, literal) = index.constant_value(value)?;
            let ScalarType::Integer(integer) = scalar_type else {
                return None;
            };
            Some(ScalarTerm::Integer {
                scalar_type: integer,
                value: literal,
            })
        }

        pub fn scalar_payload_expression(
            value: ValueId,
            origin: &impl Fn(ValueId) -> Option<(ScalarType, Option<ScalarTerm>)>,
        ) -> Option<ScalarTerm> {
            fn expand(
                term: &mut ScalarTerm,
                origin: &impl Fn(ValueId) -> Option<(ScalarType, Option<ScalarTerm>)>,
                remaining: &mut usize,
                depth: usize,
            ) -> Option<()> {
                if depth > crate::substitution::SCALAR_EXPRESSION_DEPTH {
                    return None;
                }
                *remaining = remaining.checked_sub(1)?;
                if let ScalarTerm::Value { id, scalar_type } = term {
                    let (actual, denotation) = origin(*id)?;
                    if actual != *scalar_type {
                        return None;
                    }
                    if let Some(denotation) = denotation {
                        *term = denotation;
                        return expand(term, origin, remaining, depth + 1);
                    }
                    return Some(());
                }
                for operand in term.operands_mut() {
                    expand(operand, origin, remaining, depth + 1)?;
                }
                Some(())
            }
            let (scalar_type, _) = origin(value)?;
            let mut term = ScalarTerm::value(value, scalar_type);
            let mut remaining = crate::substitution::SCALAR_EXPRESSION_NODES;
            expand(&mut term, origin, &mut remaining, 0)?;
            Some(term)
        }

        pub struct TerminalScalarExpressions<'machine> {
            index: crate::MachineScalarIndex<'machine>,
            types: std::collections::BTreeMap<ValueId, ScalarType>,
        }

        impl<'machine> TerminalScalarExpressions<'machine> {
            pub fn new(machine: &'machine terminal_psi::TerminalMachine) -> Self {
                let types: std::collections::BTreeMap<_, _> = machine
                    .parameters
                    .iter()
                    .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                    .chain(
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.scalar_ref()),
                    )
                    .map(|value| (value.id, value.scalar_type))
                    .collect();
                Self {
                    index: crate::MachineScalarIndex::new(machine),
                    types,
                }
            }

            pub fn scalar_types(&self) -> impl Iterator<Item = (ValueId, ScalarType)> + '_ {
                self.types
                    .iter()
                    .map(|(&value, &scalar_type)| (value, scalar_type))
            }

            pub fn scalar_type(&self, value: ValueId) -> Option<ScalarType> {
                self.types.get(&value).copied()
            }

            pub fn unexpanded_operation(
                &self,
                operation: &terminal_psi::Operation,
            ) -> Result<Option<ScalarTerm>, String> {
                use terminal_psi::OperationKind;
                match operation.kind {
                    OperationKind::IntegerConstant { value } => {
                        let ScalarType::Integer(scalar_type) =
                            operation.result.expect_scalar().scalar_type
                        else {
                            return Err("integer literal has a noninteger scalar carrier".into());
                        };
                        return Ok(Some(ScalarTerm::Integer { scalar_type, value }));
                    }
                    OperationKind::BooleanConstant { value } => {
                        return Ok(Some(ScalarTerm::Boolean(value)));
                    }
                    OperationKind::ReadScalarApplication { application } => {
                        return Ok(Some(ScalarTerm::Application {
                            application,
                            scalar_type: operation.result.expect_scalar().scalar_type,
                        }));
                    }
                    _ => {}
                }
                let equation = crate::goal_free_scalar_leaf_semantics(operation, &self.types)
                    .map_err(|reason| {
                        format!("scalar operation has invalid canonical semantics: {reason:?}")
                    })?
                    .map(|semantics| semantics.result_equation().clone())
                    .or(
                        crate::proof_bearing_scalar_leaf_semantics(operation, &self.types)
                            .map_err(|reason| {
                                format!(
                                    "scalar operation has invalid checked semantics: {reason:?}"
                                )
                            })?
                            .map(|semantics| semantics.result_equation().clone()),
                    );
                Ok(match equation {
                    Some(Proposition::Equal(_, right)) => Some(right),
                    _ => None,
                })
            }

            pub(crate) fn expression(
                &self,
                value: semantic_vocabulary::ValueId,
            ) -> Option<ScalarTerm> {
                let index = &self.index;
                let types = &self.types;
                scalar_payload_expression(value, &|value| {
                    let scalar_type = *types.get(&value)?;
                    if let Some(literal) = terminal_scalar_literal(index, value) {
                        return Some((scalar_type, Some(literal)));
                    }
                    let denotation = index
                        .definitions(value)
                        .next()
                        .and_then(|operation| {
                            crate::goal_free_scalar_leaf_semantics(operation, types)
                                .ok()
                                .flatten()
                                .map(|semantics| semantics.result_equation().clone())
                                .or_else(|| {
                                    crate::proof_bearing_scalar_leaf_semantics(operation, types)
                                        .ok()
                                        .flatten()
                                        .map(|semantics| semantics.result_equation().clone())
                                })
                        })
                        .and_then(|equation| match equation {
                            Proposition::Equal(_, right) => Some(right),
                            _ => None,
                        });
                    Some((scalar_type, denotation))
                })
            }
        }

        pub(crate) fn terminal_scalar_snapshots(
            machine: &terminal_psi::TerminalMachine,
            values: &[ValueId],
        ) -> Vec<(ValueId, ScalarTerm)> {
            let expressions = TerminalScalarExpressions::new(machine);
            values
                .iter()
                .filter_map(|value| {
                    expressions
                        .expression(*value)
                        .map(|expression| (*value, expression))
                })
                .collect()
        }

        pub(crate) fn introduction_predicate(
            types: &[StructuralTypeDeclaration],
            domains: &[StructuralDomainDeclaration],
            domain: &StructuralDomainDeclaration,
            supplied: &[semantic_vocabulary::StructuralDomainId],
        ) -> Option<terminal_psi::StructuralDomainPredicate> {
            let subject = domain.predicate.as_ref()?.subject;
            let mut clauses = Vec::new();
            for missing in domain.missing_membership_requirements(domains, supplied)? {
                if missing.content_projection.is_some() || missing.establishment_routes.is_some() {
                    return None;
                }
                let predicate = missing.predicate.as_ref()?;
                if !crate::domain_predicate_is_closed(types.iter(), missing.carrier, predicate) {
                    return None;
                }
                let proposition = crate::substitution::substitute_proposition_structural_places(
                    &predicate.proposition,
                    &std::collections::BTreeMap::from([(predicate.subject, (subject, Vec::new()))]),
                );
                if proposition != Proposition::Truth {
                    clauses.push(proposition);
                }
            }
            clauses.sort();
            clauses.dedup();
            let proposition = match clauses.len() {
                0 => Proposition::Truth,
                1 => clauses.pop()?,
                _ => Proposition::Conjunction(clauses),
            };
            Some(terminal_psi::StructuralDomainPredicate {
                subject,
                proposition,
            })
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use semantic_vocabulary::{
                ByteSequencePredicate, IntegerSign, IntegerType, IntegerValue, PlaceId,
                StructuralTypeId,
            };
            use std::slice::from_ref;
            use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

            #[test]
            fn byte_predicate_observations_require_complete_current_typed_contents() {
                let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
                let scalar = StructuralTypeId::new(1).unwrap();
                let array = StructuralTypeId::new(2).unwrap();
                let subject = PlaceId::new(1).unwrap();
                let declaration = |id, shape| StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex(id.get() as u32),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape,
                    scalar_restriction: None,
                };
                let types = vec![
                    declaration(
                        scalar,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
                    ),
                    declaration(
                        array,
                        StructuralTypeShape::FixedArray {
                            element: scalar,
                            length: 2.into(),
                        },
                    ),
                ];
                let term = ScalarTerm::ByteSequencePredicate {
                    predicate: ByteSequencePredicate::AsciiOnly,
                    root: subject,
                    path: Vec::new(),
                };
                let mut contract = PayloadMembershipContract {
                    scalar_observations: [65u8, 66]
                        .into_iter()
                        .enumerate()
                        .map(|(index, value)| PayloadScalarObservation {
                            path: vec![DescriptorPathSegment::FixedIndex(index as u64)],
                            value: Some(
                                ScalarTerm::integer(integer, IntegerValue::Unsigned(value.into()))
                                    .unwrap(),
                            ),
                            entry: None,
                        })
                        .collect(),
                    scalar_requirements: Vec::new(),
                    carrier: array,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let classify = |contract: &PayloadMembershipContract| {
                    observed_byte_predicate(&types, contract, subject, &term)
                };
                assert_eq!(classify(&contract), Some(true));
                assert_eq!(
                    observed_byte_predicate(&types, &contract, PlaceId::new(2).unwrap(), &term),
                    None
                );
                let mut missing = contract.clone();
                missing.scalar_observations.pop();
                assert_eq!(classify(&missing), None);
                let mut duplicate = contract.clone();
                duplicate
                    .scalar_observations
                    .push(duplicate.scalar_observations[0].clone());
                assert_eq!(classify(&duplicate), None);
                let mut wrong_type = contract.clone();
                wrong_type.scalar_observations[1].value = Some(ScalarTerm::Boolean(true));
                assert_eq!(classify(&wrong_type), None);
                contract.scalar_observations[1].value =
                    Some(ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap());
                assert_eq!(classify(&contract), Some(false));
            }

            #[test]
            fn closure_gate_keeps_normalization_without_an_eligible_domain() {
                use semantic_vocabulary::StructuralDomainId;
                let first = StructuralDomainId::new(1).unwrap();
                let second = StructuralDomainId::new(2).unwrap();
                let mut contract = PayloadMembershipContract {
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    carrier: StructuralTypeId::new(1).unwrap(),
                    qualifications: vec![second, first, first],
                    projected_qualifications: Vec::new(),
                };
                assert!(closure_may_change(&[], &[], &contract));
                close(&[], &[], &mut contract);
                assert_eq!(contract.qualifications, vec![first, second]);
                assert!(!closure_may_change(&[], &[], &contract));
                let unchanged = contract.clone();
                close(&[], &[], &mut contract);
                assert_eq!(contract, unchanged);
            }

            #[test]
            fn closure_gate_preserves_eligible_and_opaque_domain_behavior() {
                use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
                use terminal_psi::StructuralDomainPredicate;

                let carrier = StructuralTypeId::new(1).unwrap();
                let domain_id = StructuralDomainId::new(1).unwrap();
                let types = vec![StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id: carrier,
                    identity: TerminalIdentityIndex(1),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    scalar_restriction: None,
                }];
                let domain = StructuralDomainDeclaration {
                    subject_identity: Default::default(),
                    access_policy: None,
                    required_memberships: Some(Vec::new()),
                    complete_predicate_theory: true,
                    predicate: Some(StructuralDomainPredicate {
                        subject: PlaceId::new(1).unwrap(),
                        proposition: Proposition::Truth,
                    }),
                    id: domain_id,
                    semantic_domain: DomainSemanticId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(2),
                    carrier,
                    content_projection: None,
                    byte_predicate: None,
                    establishment_routes: None,
                };
                let empty = PayloadMembershipContract {
                    scalar_observations: Vec::new(),
                    scalar_requirements: Vec::new(),
                    carrier,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let mut contract = empty.clone();
                assert!(closure_may_change(&types, from_ref(&domain), &contract));
                close(&types, from_ref(&domain), &mut contract);
                assert_eq!(contract.qualifications, vec![domain_id]);
                assert!(!closure_may_change(&types, from_ref(&domain), &contract));

                let mut opaque = domain.clone();
                opaque.predicate = None;
                let mut unknown = domain.clone();
                unknown.required_memberships = None;
                for excluded in [opaque, unknown] {
                    let mut contract = empty.clone();
                    let domains = from_ref(&excluded);
                    assert!(!closure_may_change(&types, domains, &contract));
                    close(&types, domains, &mut contract);
                    assert_eq!(contract, empty);
                }
                let mut unprovable = domain;
                unprovable.predicate.as_mut().unwrap().proposition = Proposition::Falsehood;
                let domains = from_ref(&unprovable);
                let mut contract = empty.clone();
                assert!(closure_may_change(&types, domains, &contract));
                close(&types, domains, &mut contract);
                assert_eq!(contract, empty);
            }
        }
    }
    pub(crate) mod payload_reachability {
        use super::*;
        use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, ValueId};
        use std::collections::VecDeque;

        type Conditions = BTreeMap<ValueId, bool>;

        pub(crate) struct Reachability {
            positions: BTreeSet<DescriptorPosition>,
            edges: BTreeSet<(DescriptorPosition, usize)>,
        }

        impl Reachability {
            #[cfg(test)]
            pub(crate) fn extension_frontier(
                &self,
                previous: &Self,
                flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            ) -> Option<VecDeque<DescriptorPosition>> {
                if !previous.positions.is_subset(&self.positions)
                    || !previous.edges.is_subset(&self.edges)
                {
                    return None;
                }
                for &(position, ordinal) in &self.edges {
                    let edge = flow.get(&position)?.successors.get(ordinal)?;
                    if previous.contains(edge.target) && !previous.includes_edge(position, ordinal)
                    {
                        return None;
                    }
                }
                Some(
                    previous
                        .positions
                        .iter()
                        .copied()
                        .filter(|position| {
                            flow.get(position).is_some_and(|step| {
                                step.successors.iter().enumerate().any(|(ordinal, edge)| {
                                    self.includes_edge(*position, ordinal)
                                        && !previous.contains(edge.target)
                                })
                            })
                        })
                        .collect(),
                )
            }

            pub(crate) fn compatible_frontier(
                &self,
                previous: &Self,
                flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            ) -> Option<VecDeque<DescriptorPosition>> {
                for &(position, ordinal) in &self.edges {
                    let edge = flow.get(&position)?.successors.get(ordinal)?;
                    if previous.contains(edge.target) && !previous.includes_edge(position, ordinal)
                    {
                        return None;
                    }
                }
                for &(position, ordinal) in &previous.edges {
                    let edge = flow.get(&position)?.successors.get(ordinal)?;
                    if self.contains(edge.target) && !self.includes_edge(position, ordinal) {
                        return None;
                    }
                }
                Some(
                    previous
                        .positions
                        .iter()
                        .copied()
                        .filter(|position| {
                            self.contains(*position)
                                && flow.get(position).is_some_and(|step| {
                                    step.successors.iter().enumerate().any(|(ordinal, edge)| {
                                        self.includes_edge(*position, ordinal)
                                            && !previous.contains(edge.target)
                                    })
                                })
                        })
                        .collect(),
                )
            }

            pub(crate) fn merge(&mut self, mut previous: Self) {
                self.positions.append(&mut previous.positions);
                self.edges.append(&mut previous.edges);
            }

            pub(crate) fn contains(&self, position: DescriptorPosition) -> bool {
                self.positions.contains(&position)
            }

            pub(crate) fn includes_edge(
                &self,
                position: DescriptorPosition,
                ordinal: usize,
            ) -> bool {
                self.edges.contains(&(position, ordinal))
            }
        }

        pub(crate) type IncomingFlow =
            BTreeMap<DescriptorPosition, Vec<(DescriptorPosition, usize)>>;

        pub(crate) fn incoming_flow(
            flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        ) -> Option<IncomingFlow> {
            let mut incoming = BTreeMap::<_, Vec<_>>::new();
            for (&position, step) in flow {
                for (ordinal, edge) in step.successors.iter().enumerate() {
                    flow.get(&edge.target)?;
                    incoming
                        .entry(edge.target)
                        .or_default()
                        .push((position, ordinal));
                }
            }
            Some(incoming)
        }

        pub(crate) fn predecessors(
            machine: &DescriptorMachine,
            query: DescriptorPosition,
        ) -> Option<Reachability> {
            let incoming = incoming_flow(&machine.flow)?;
            predecessors_with_incoming(machine, &incoming, query)
        }

        pub(crate) fn predecessors_with_incoming(
            machine: &DescriptorMachine,
            incoming: &IncomingFlow,
            query: DescriptorPosition,
        ) -> Option<Reachability> {
            QueryFlow {
                flow: &machine.flow,
                predicates: &machine.payload_edge_predicates,
                bindings: &machine.payload_scalar_bindings,
                parameters: &machine.payload_scalar_parameters,
                definitions: &machine.payload_scalar_definitions,
            }
            .predecessors_with_incoming(incoming, query)
        }

        struct QueryFlow<'flow> {
            flow: &'flow BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            predicates: &'flow BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
            bindings: &'flow BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
            parameters: &'flow BTreeMap<semantic_vocabulary::BlockId, Vec<ValueId>>,
            definitions: &'flow BTreeMap<DescriptorPosition, Vec<ValueId>>,
        }

        impl QueryFlow<'_> {
            #[cfg(test)]
            fn predecessors(&self, query: DescriptorPosition) -> Option<Reachability> {
                let incoming = incoming_flow(self.flow)?;
                self.predecessors_with_incoming(&incoming, query)
            }

            fn predecessors_with_incoming(
                &self,
                incoming: &IncomingFlow,
                query: DescriptorPosition,
            ) -> Option<Reachability> {
                self.flow.get(&query)?;
                let mut conditions = BTreeMap::from([(query, Conditions::new())]);
                let mut edges = BTreeSet::new();
                let mut pending = VecDeque::from([query]);
                let mut remaining = 65_536usize;
                while let Some(destination) = pending.pop_front() {
                    remaining = remaining.checked_sub(1)?;
                    for &(position, ordinal) in incoming.get(&destination).into_iter().flatten() {
                        let Some(arrival) = self.before_edge(
                            position,
                            ordinal,
                            destination,
                            conditions.get(&destination)?,
                        ) else {
                            continue;
                        };
                        edges.insert((position, ordinal));
                        let changed = if let Some(previous) = conditions.get_mut(&position) {
                            let count = previous.len();
                            previous.retain(|value, polarity| arrival.get(value) == Some(polarity));
                            previous.len() != count
                        } else {
                            conditions.insert(position, arrival);
                            true
                        };
                        if changed && !pending.contains(&position) {
                            pending.push_back(position);
                        }
                    }
                }
                Some(Reachability {
                    positions: conditions.into_keys().collect(),
                    edges,
                })
            }

            fn before_edge(
                &self,
                position: DescriptorPosition,
                ordinal: usize,
                destination: DescriptorPosition,
                after: &Conditions,
            ) -> Option<Conditions> {
                let mut before = after.clone();
                if destination.1 == 0 {
                    if let Some(parameters) = self.parameters.get(&destination.0) {
                        before.retain(|value, _| !parameters.contains(value));
                    }
                    if let Some(bindings) = self
                        .bindings
                        .get(&position)
                        .and_then(|edges| edges.get(ordinal))
                    {
                        for (parameter, actual) in bindings {
                            if let Some(&polarity) = after.get(parameter) {
                                require(&mut before, actual, polarity)?;
                            }
                        }
                    }
                }
                if let Some(Some(Proposition::Equal(left, right))) = self
                    .predicates
                    .get(&position)
                    .and_then(|edges| edges.get(ordinal))
                {
                    match (left, right) {
                        (ScalarTerm::Boolean(polarity), term)
                        | (term, ScalarTerm::Boolean(polarity)) => {
                            require(&mut before, term, *polarity)?
                        }
                        _ => {}
                    }
                }
                if let Some(definitions) = self.definitions.get(&position) {
                    before.retain(|value, _| !definitions.contains(value));
                }
                Some(before)
            }
        }

        fn require(
            conditions: &mut Conditions,
            mut term: &ScalarTerm,
            mut polarity: bool,
        ) -> Option<()> {
            for _ in 0..64 {
                match term {
                    ScalarTerm::Boolean(value) => return (*value == polarity).then_some(()),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Boolean,
                    } => {
                        return match conditions.insert(*id, polarity) {
                            Some(previous) if previous != polarity => None,
                            _ => Some(()),
                        };
                    }
                    ScalarTerm::BooleanNot { operand } => {
                        term = operand;
                        polarity = !polarity;
                    }
                    _ => return Some(()),
                }
            }
            Some(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::descriptors::descriptor_lifetimes::DescriptorFlowEdge;
            use semantic_vocabulary::BlockId;

            fn position(block: u64) -> DescriptorPosition {
                (BlockId::new(block).unwrap(), 0)
            }
            fn value(index: u64) -> ValueId {
                ValueId::new(index).unwrap()
            }
            fn boolean(index: u64) -> ScalarTerm {
                ScalarTerm::value(value(index), ScalarType::Boolean)
            }
            fn selected(index: u64, polarity: bool) -> Option<Proposition> {
                Some(Proposition::Equal(
                    ScalarTerm::Boolean(polarity),
                    boolean(index),
                ))
            }
            fn step(targets: &[u64]) -> DescriptorFlowStep {
                DescriptorFlowStep {
                    successors: targets
                        .iter()
                        .map(|target| DescriptorFlowEdge {
                            edge: None,
                            effects: Vec::new(),
                            definitions: Vec::new(),
                            target: position(*target),
                            parameters: Vec::new(),
                            bindings: Vec::new(),
                        })
                        .collect(),
                    ..Default::default()
                }
            }

            #[derive(Default)]
            struct Graph {
                flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
                predicates: BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
                bindings: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
                parameters: BTreeMap<BlockId, Vec<ValueId>>,
                definitions: BTreeMap<DescriptorPosition, Vec<ValueId>>,
            }
            impl Graph {
                fn query(&self) -> QueryFlow<'_> {
                    QueryFlow {
                        flow: &self.flow,
                        predicates: &self.predicates,
                        bindings: &self.bindings,
                        parameters: &self.parameters,
                        definitions: &self.definitions,
                    }
                }
            }

            #[test]
            fn differently_bound_edges_to_one_target_remain_distinct() {
                let mut graph = Graph {
                    flow: BTreeMap::from([
                        (position(1), step(&[2, 2])),
                        (position(2), step(&[3, 4])),
                        (position(3), step(&[])),
                        (position(4), step(&[])),
                    ]),
                    predicates: BTreeMap::from([(
                        position(2),
                        vec![selected(1, true), selected(1, false)],
                    )]),
                    bindings: BTreeMap::from([(
                        position(1),
                        vec![
                            vec![(value(1), ScalarTerm::Boolean(true))],
                            vec![(value(1), ScalarTerm::Boolean(false))],
                        ],
                    )]),
                    parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
                    ..Default::default()
                };
                let reachable = graph.query().predecessors(position(3)).unwrap();
                assert!(reachable.includes_edge(position(1), 0));
                assert!(!reachable.includes_edge(position(1), 1));
                assert!(!reachable.contains(position(4)));
                graph.bindings.get_mut(&position(1)).unwrap()[1][0].1 = boolean(7);
                let reachable = graph.query().predecessors(position(3)).unwrap();
                assert!(reachable.includes_edge(position(1), 1));
                graph.bindings.get_mut(&position(1)).unwrap()[1][0].1 =
                    ScalarTerm::boolean_equal(boolean(7), boolean(8)).unwrap();
                assert!(
                    graph
                        .query()
                        .predecessors(position(3))
                        .unwrap()
                        .includes_edge(position(1), 1)
                );
                graph.bindings.clear();
                assert!(
                    graph
                        .query()
                        .predecessors(position(3))
                        .unwrap()
                        .includes_edge(position(1), 1)
                );
            }

            #[test]
            fn pre_definition_and_post_definition_queries_observe_different_instances() {
                let before = position(1);
                let after = (before.0, 1);
                let mut operation = step(&[]);
                operation.successors.push(DescriptorFlowEdge {
                    edge: None,
                    effects: Vec::new(),
                    definitions: Vec::new(),
                    target: after,
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                });
                let graph = Graph {
                    flow: BTreeMap::from([
                        (before, operation),
                        (after, step(&[2, 3])),
                        (position(2), step(&[])),
                        (position(3), step(&[])),
                    ]),
                    predicates: BTreeMap::from([(
                        after,
                        vec![selected(1, true), selected(1, false)],
                    )]),
                    definitions: BTreeMap::from([(before, vec![value(1)])]),
                    ..Default::default()
                };
                assert!(!graph.query().predecessors(before).unwrap().contains(after));
                assert!(graph.query().predecessors(after).unwrap().contains(before));
                let requirements = BTreeMap::from([(value(1), true)]);
                assert_eq!(
                    graph.query().before_edge(before, 0, after, &requirements),
                    Some(BTreeMap::new())
                );
            }

            #[test]
            fn parameter_rebinding_is_simultaneous_and_missing_bindings_forget_demands() {
                let mut graph = Graph {
                    parameters: BTreeMap::from([(position(2).0, vec![value(1), value(2)])]),
                    bindings: BTreeMap::from([(
                        position(1),
                        vec![vec![(value(1), boolean(2)), (value(2), boolean(1))]],
                    )]),
                    ..Default::default()
                };
                let demands = BTreeMap::from([(value(1), true), (value(2), false)]);
                assert_eq!(
                    graph
                        .query()
                        .before_edge(position(1), 0, position(2), &demands),
                    Some(BTreeMap::from([(value(1), false), (value(2), true)]))
                );
                graph.bindings.clear();
                assert_eq!(
                    graph
                        .query()
                        .before_edge(position(1), 0, position(2), &demands),
                    Some(BTreeMap::new())
                );
            }

            #[test]
            fn query_keeps_backedges_and_retires_new_scalar_instances() {
                let mut graph = Graph {
                    flow: BTreeMap::from([
                        (position(1), step(&[2])),
                        (position(2), step(&[3, 5])),
                        (position(3), step(&[4])),
                        (position(4), step(&[2])),
                        (position(5), step(&[])),
                    ]),
                    predicates: BTreeMap::from([(
                        position(2),
                        vec![selected(1, true), selected(1, false)],
                    )]),
                    parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
                    bindings: BTreeMap::from([
                        (
                            position(1),
                            vec![vec![(value(1), ScalarTerm::Boolean(true))]],
                        ),
                        (position(4), vec![vec![(value(1), boolean(7))]]),
                    ]),
                    definitions: BTreeMap::from([(position(4), vec![value(7)])]),
                };
                let reachable = graph.query().predecessors(position(3)).unwrap();
                assert!(reachable.includes_edge(position(3), 0));
                assert!(reachable.includes_edge(position(4), 0));
                assert!(!reachable.contains(position(5)));
                let demands = BTreeMap::from([(value(7), true)]);
                assert_eq!(
                    graph
                        .query()
                        .before_edge(position(4), 0, position(3), &demands),
                    Some(BTreeMap::new())
                );
                graph.definitions.clear();
                assert_eq!(
                    graph
                        .query()
                        .before_edge(position(4), 0, position(3), &demands),
                    Some(demands)
                );
            }

            #[test]
            fn conflicting_unknown_successors_weaken_demands_and_reopen_predecessors() {
                let graph = Graph {
                    flow: BTreeMap::from([
                        (position(1), step(&[2])),
                        (position(2), step(&[3, 4])),
                        (position(3), step(&[5])),
                        (position(4), step(&[6])),
                        (position(6), step(&[5])),
                        (position(5), step(&[])),
                    ]),
                    predicates: BTreeMap::from([(
                        position(2),
                        vec![selected(1, true), selected(1, false)],
                    )]),
                    parameters: BTreeMap::from([(position(2).0, vec![value(1)])]),
                    bindings: BTreeMap::from([(
                        position(1),
                        vec![vec![(value(1), ScalarTerm::Boolean(false))]],
                    )]),
                    ..Default::default()
                };
                let reachable = graph.query().predecessors(position(5)).unwrap();
                assert!(reachable.includes_edge(position(1), 0));
                assert!(reachable.includes_edge(position(2), 0));
                assert!(reachable.includes_edge(position(2), 1));
            }

            #[test]
            fn history_extension_requires_identical_incoming_edges_for_every_retained_position() {
                let graph = Graph {
                    flow: BTreeMap::from([
                        (position(1), step(&[2, 3])),
                        (position(2), step(&[4])),
                        (position(3), step(&[4])),
                        (position(4), step(&[5])),
                        (position(5), step(&[])),
                    ]),
                    ..Default::default()
                };
                let before = graph.query().predecessors(position(4)).unwrap();
                let after = graph.query().predecessors(position(5)).unwrap();
                assert_eq!(
                    after.extension_frontier(&before, &graph.flow).unwrap(),
                    VecDeque::from([position(4)])
                );
                assert!(before.extension_frontier(&after, &graph.flow).is_none());
                assert!(
                    before
                        .extension_frontier(&before, &graph.flow)
                        .unwrap()
                        .is_empty()
                );
                let filtered = Reachability {
                    positions: BTreeSet::from([position(1), position(2), position(4)]),
                    edges: BTreeSet::from([(position(1), 0), (position(2), 0)]),
                };
                assert!(after.extension_frontier(&filtered, &graph.flow).is_none());
                assert!(after.compatible_frontier(&filtered, &graph.flow).is_none());
                assert!(filtered.compatible_frontier(&after, &graph.flow).is_none());

                let mut left = graph.query().predecessors(position(2)).unwrap();
                let right = graph.query().predecessors(position(3)).unwrap();
                assert_eq!(
                    right.compatible_frontier(&left, &graph.flow).unwrap(),
                    VecDeque::from([position(1)])
                );
                left.merge(right);
                assert_eq!(
                    after.compatible_frontier(&left, &graph.flow).unwrap(),
                    VecDeque::from([position(2), position(3)])
                );
            }
        }
    }
    pub(crate) mod payload_versions {

        use super::{
            DescriptorAnalysis, DescriptorContentsQuery, DescriptorMachine,
            DescriptorPayloadTarget, descriptor_machine,
        };
        use crate::descriptors::descriptor_lifetimes::{
            DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
        };
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::MachineId;
        use semantic_vocabulary::ValueId;
        use std::collections::{BTreeMap, VecDeque};
        use terminal_psi::StructuralTypeDeclaration;

        struct ScalarCaptureEdge {
            target: usize,
            parameters: Vec<ValueId>,
        }

        struct ScalarCaptureStep {
            definitions: Vec<ValueId>,
            successors: Vec<ScalarCaptureEdge>,
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum ScalarCaptureFailure {
            NotCompleted,
            UnknownPosition(DescriptorPosition),
            MissingOrRenewed,
            QueryBudget,
            InvalidFlow,
        }

        pub struct ScalarCaptureAnalysis {
            identity: MachineId,
            positions: BTreeMap<DescriptorPosition, usize>,
            coordinates: Vec<DescriptorPosition>,
            entry: usize,
            steps: Vec<ScalarCaptureStep>,
            answers: BTreeMap<(usize, usize, Vec<ValueId>), bool>,
            versions: Vec<Option<bool>>,
            queued: Vec<bool>,
            pending: VecDeque<usize>,
        }

        impl ScalarCaptureAnalysis {
            pub fn new(machine: &DescriptorMachine) -> Result<Self, &'static str> {
                let positions = machine
                    .flow
                    .keys()
                    .copied()
                    .enumerate()
                    .map(|(index, position)| (position, index))
                    .collect::<BTreeMap<_, _>>();
                let entry = *positions
                    .get(&machine.entry)
                    .ok_or("scalar capture graph has no entry")?;
                let mut steps = Vec::with_capacity(positions.len());
                for (&position, step) in &machine.flow {
                    let mut successors = Vec::with_capacity(step.successors.len());
                    for edge in &step.successors {
                        let target = *positions
                            .get(&edge.target)
                            .ok_or("scalar capture graph has an absent target")?;
                        let parameters = if edge.target.1 == 0 {
                            machine
                                .payload_scalar_parameters
                                .get(&edge.target.0)
                                .cloned()
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        successors.push(ScalarCaptureEdge { target, parameters });
                    }
                    steps.push(ScalarCaptureStep {
                        definitions: machine
                            .payload_scalar_definitions
                            .get(&position)
                            .cloned()
                            .unwrap_or_default(),
                        successors,
                    });
                }
                Ok(Self {
                    identity: machine.id,
                    entry,
                    versions: vec![None; steps.len()],
                    queued: vec![false; steps.len()],
                    pending: VecDeque::new(),
                    coordinates: positions.keys().copied().collect(),
                    positions,
                    steps,
                    answers: BTreeMap::new(),
                })
            }

            pub fn identity(&self) -> MachineId {
                self.identity
            }

            pub fn require(
                &mut self,
                source: DescriptorPosition,
                observation: DescriptorPosition,
                values: &[ValueId],
            ) -> Result<(), ScalarCaptureFailure> {
                if source == observation {
                    return Err(ScalarCaptureFailure::NotCompleted);
                }
                let source = *self
                    .positions
                    .get(&source)
                    .ok_or(ScalarCaptureFailure::UnknownPosition(source))?;
                let observation = *self
                    .positions
                    .get(&observation)
                    .ok_or(ScalarCaptureFailure::UnknownPosition(observation))?;
                let mut values = values.to_vec();
                values.sort();
                values.dedup();
                let key = (source, observation, values);
                if let Some(valid) = self.answers.get(&key) {
                    return if *valid {
                        Ok(())
                    } else {
                        Err(ScalarCaptureFailure::MissingOrRenewed)
                    };
                }
                if self.answers.len() >= 65_536 {
                    return Err(ScalarCaptureFailure::QueryBudget);
                }
                let valid = self.solve(source, observation, &key.2, |_, _| true)?;
                self.answers.insert(key, valid);
                if valid {
                    Ok(())
                } else {
                    Err(ScalarCaptureFailure::MissingOrRenewed)
                }
            }

            fn require_on_edges(
                &mut self,
                source: DescriptorPosition,
                observation: DescriptorPosition,
                values: &[ValueId],
                includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
            ) -> Result<bool, ScalarCaptureFailure> {
                let source = *self
                    .positions
                    .get(&source)
                    .ok_or(ScalarCaptureFailure::UnknownPosition(source))?;
                let observation = *self
                    .positions
                    .get(&observation)
                    .ok_or(ScalarCaptureFailure::UnknownPosition(observation))?;
                let mut values = values.to_vec();
                values.sort();
                values.dedup();
                self.solve(source, observation, &values, includes_edge)
            }

            fn solve(
                &mut self,
                source: usize,
                observation: usize,
                values: &[ValueId],
                includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
            ) -> Result<bool, ScalarCaptureFailure> {
                self.versions.fill(None);
                self.queued.fill(false);
                self.pending.clear();
                self.versions[self.entry] = Some(false);
                self.queued[self.entry] = true;
                self.pending.push_back(self.entry);
                while let Some(current) = self.pending.pop_front() {
                    self.queued[current] = false;
                    let before = self.versions[current].ok_or(ScalarCaptureFailure::InvalidFlow)?;
                    let step = &self.steps[current];
                    let after = if current == source {
                        true
                    } else {
                        before
                            && !step
                                .definitions
                                .iter()
                                .any(|value| values.binary_search(value).is_ok())
                    };
                    for (ordinal, edge) in step.successors.iter().enumerate() {
                        if !includes_edge(self.coordinates[current], ordinal) {
                            continue;
                        }
                        let arrival = after
                            && !edge
                                .parameters
                                .iter()
                                .any(|value| values.binary_search(value).is_ok());
                        let joined =
                            self.versions[edge.target].map_or(arrival, |old| old && arrival);
                        if self.versions[edge.target] != Some(joined) {
                            self.versions[edge.target] = Some(joined);
                            if !self.queued[edge.target] {
                                self.queued[edge.target] = true;
                                self.pending.push_back(edge.target);
                            }
                        }
                    }
                }
                Ok(self.versions[observation] == Some(true))
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct ProducedPayloadVersion {
            pub producer: DescriptorPosition,
            pub payload: DescriptorLocation,
        }

        pub fn unchanged_payload_producer(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            observation: DescriptorPosition,
            query: &DescriptorContentsQuery,
        ) -> Result<ProducedPayloadVersion, &'static str> {
            let machine =
                descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
            let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
            let targets = analysis
                .payload_targets(identity, observation, query)
                .ok_or("produced payload borrow has unresolved backing")?;
            let [
                DescriptorPayloadTarget::Storage {
                    location: payload,
                    completed: None,
                },
            ] = targets.as_slice()
            else {
                return Err(
                    "produced payload borrow does not resolve to one original storage home",
                );
            };
            let mut producers = machine
                .payload_producers
                .iter()
                .filter(|(_, root)| **root == payload.root);
            let (&producer, _) = producers
                .next()
                .ok_or("observed payload has no local whole-value producer")?;
            if producers.next().is_some() {
                return Err("observed payload has multiple whole-value producers");
            }
            require_version(machine, &mut analysis, producer, observation, payload)?;
            Ok(ProducedPayloadVersion {
                producer,
                payload: payload.clone(),
            })
        }

        pub fn require_unchanged_produced_payload(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            producer: DescriptorPosition,
            observation: DescriptorPosition,
            payload: &DescriptorLocation,
        ) -> Result<(), &'static str> {
            let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
            require_unchanged_produced_payload_in(
                types,
                graph,
                &mut analysis,
                identity,
                producer,
                observation,
                payload,
            )
        }

        /// Reuse the caller's prepared storage history. A missing or changed
        /// version is an error, including an unresolved write on any arrival.
        pub fn require_unchanged_produced_payload_in(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            analysis: &mut DescriptorAnalysis<'_>,
            identity: MachineId,
            producer: DescriptorPosition,
            observation: DescriptorPosition,
            payload: &DescriptorLocation,
        ) -> Result<(), &'static str> {
            if !analysis.uses_source_graph(types, graph) {
                return Err("produced storage analysis belongs to another immutable source graph");
            }
            let machine =
                descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
            require_version(machine, analysis, producer, observation, payload)
        }

        fn require_version(
            machine: &DescriptorMachine,
            analysis: &mut DescriptorAnalysis<'_>,
            producer: DescriptorPosition,
            observation: DescriptorPosition,
            payload: &DescriptorLocation,
        ) -> Result<(), &'static str> {
            if !payload.path.is_empty()
                || machine.payload_producers.get(&producer) != Some(&payload.root)
                || producer == observation
            {
                return Err("payload query does not name a completed whole-value producer");
            }
            require_unchanged_between(
                machine,
                analysis,
                producer,
                observation,
                &DescriptorPayloadTarget::current(payload.clone()),
            )
        }

        pub fn require_unchanged_observed_storage(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            completed_observation: DescriptorPosition,
            use_position: DescriptorPosition,
            storage: &DescriptorPayloadTarget,
        ) -> Result<(), &'static str> {
            let machine =
                descriptor_machine(graph, identity).ok_or("observed storage machine is absent")?;
            if completed_observation == use_position {
                return Err("storage observation has not completed before its use");
            }
            let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
            require_unchanged_between(
                machine,
                &mut analysis,
                completed_observation,
                use_position,
                storage,
            )
        }

        fn require_unchanged_between(
            machine: &DescriptorMachine,
            analysis: &mut DescriptorAnalysis<'_>,
            producer: DescriptorPosition,
            observation: DescriptorPosition,
            payload: &DescriptorPayloadTarget,
        ) -> Result<(), &'static str> {
            require_unchanged_from(
                machine,
                analysis,
                VersionStart::Completed(producer),
                observation,
                payload,
            )
        }

        pub fn require_unchanged_entry_storage(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            analysis: &mut DescriptorAnalysis<'_>,
            identity: MachineId,
            observation: DescriptorPosition,
            payload: &DescriptorPayloadTarget,
        ) -> Result<(), &'static str> {
            if !analysis.uses_source_graph(types, graph) {
                return Err("entry storage analysis belongs to another immutable source graph");
            }
            let machine =
                descriptor_machine(graph, identity).ok_or("entry payload machine is absent")?;
            require_unchanged_from(machine, analysis, VersionStart::Entry, observation, payload)
        }

        #[derive(Clone, Copy)]
        enum VersionStart {
            Entry,
            Completed(DescriptorPosition),
        }

        fn require_unchanged_from(
            machine: &DescriptorMachine,
            analysis: &mut DescriptorAnalysis<'_>,
            start: VersionStart,
            observation: DescriptorPosition,
            payload: &DescriptorPayloadTarget,
        ) -> Result<(), &'static str> {
            let identity = machine.id;
            let relevant = super::payload_reachability::predecessors(machine, observation)
                .ok_or("produced payload observation has unresolved control flow")?;
            let home = match payload {
                DescriptorPayloadTarget::Storage {
                    location,
                    completed: None,
                } => location.clone(),
                DescriptorPayloadTarget::Storage {
                    completed: Some(_), ..
                } => return Err("a captured constructor value is not current mutable storage"),
                DescriptorPayloadTarget::Incoming(query) => DescriptorLocation::whole(query.root),
            };
            let overlaps =
                |location: &DescriptorLocation| home.contains(location) || location.contains(&home);
            let unchanged = version_flow_from(
                &machine.flow,
                machine.entry,
                start,
                observation,
                |position, ordinal| relevant.includes_edge(position, ordinal),
                |position, step| {
                    if machine.unresolved_contents_writes.contains(&position) {
                        return Ok(true);
                    }
                    if step.definitions.iter().any(overlaps) {
                        return Ok(true);
                    }
                    for query in machine
                        .payload_invalidations
                        .get(&position)
                        .into_iter()
                        .flatten()
                    {
                        let Some(targets) = query
                            .as_ref()
                            .and_then(|query| analysis.payload_targets(identity, position, query))
                        else {
                            return Ok(true);
                        };
                        if targets.iter().any(|target| match target {
                            DescriptorPayloadTarget::Storage {
                                location,
                                completed: None,
                            } => overlaps(location),
                            DescriptorPayloadTarget::Storage {
                                completed: Some(_), ..
                            } => true,
                            DescriptorPayloadTarget::Incoming(_) => true,
                        }) {
                            return Ok(true);
                        }
                    }
                    for (_, destination) in &step.derivations {
                        if machine.payload_producers.get(&position) == Some(&destination.root) {
                            continue;
                        }
                        let Some(destinations) =
                            analysis.storage_origins(identity, position, destination)
                        else {
                            return Ok(true);
                        };
                        if destinations.iter().any(overlaps) {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                },
                |edge| {
                    edge.parameters
                        .iter()
                        .any(|parameter| parameter.contains(&home) || home.contains(parameter))
                },
            )?;
            if unchanged {
                Ok(())
            } else {
                Err(
                    "produced payload is absent, replaced or possibly modified before this observation",
                )
            }
        }

        pub fn require_completed_scalar_capture(
            graph: &[DescriptorMachine],
            identity: MachineId,
            source: DescriptorPosition,
            observation: DescriptorPosition,
            values: &[ValueId],
        ) -> Result<(), &'static str> {
            if source == observation {
                return Err("evidence capture has not completed before use");
            }
            let machine =
                descriptor_machine(graph, identity).ok_or("evidence capture machine is absent")?;
            let relevant = super::payload_reachability::predecessors(machine, observation)
                .ok_or("evidence capture has unresolved incoming control flow")?;
            let mut analysis = ScalarCaptureAnalysis::new(machine)
                .map_err(|_| "payload version flow names an absent position")?;
            let valid = analysis
                .require_on_edges(source, observation, values, |position, ordinal| {
                    relevant.includes_edge(position, ordinal)
                })
                .map_err(|reason| match reason {
                    ScalarCaptureFailure::UnknownPosition(_) => {
                        "payload version flow names an absent position"
                    }
                    ScalarCaptureFailure::InvalidFlow => "payload version arrival is absent",
                    ScalarCaptureFailure::NotCompleted => {
                        "evidence capture has not completed before use"
                    }
                    ScalarCaptureFailure::MissingOrRenewed => {
                        "evidence capture is absent or its SSA instance was renewed before use"
                    }
                    ScalarCaptureFailure::QueryBudget => "scalar capture query budget exceeded",
                })?;
            if valid {
                Ok(())
            } else {
                Err("evidence capture is absent or its SSA instance was renewed before use")
            }
        }

        fn version_flow_from(
            flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
            entry: DescriptorPosition,
            start: VersionStart,
            observation: DescriptorPosition,
            includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
            mut modifies: impl FnMut(
                DescriptorPosition,
                &DescriptorFlowStep,
            ) -> Result<bool, &'static str>,
            replaces_on_arrival: impl Fn(
                &crate::descriptors::descriptor_lifetimes::DescriptorFlowEdge,
            ) -> bool,
        ) -> Result<bool, &'static str> {
            let positions = flow.keys().copied().collect::<Vec<_>>();
            let position_index = |position| {
                positions
                    .binary_search(&position)
                    .map_err(|_| "payload version flow names an absent position")
            };
            let entry_index = position_index(entry)?;
            let observation_index = position_index(observation)?;
            if let VersionStart::Completed(producer) = start {
                position_index(producer)?;
            }
            let mut versions = vec![None; positions.len()];
            let mut writes = vec![None; positions.len()];
            versions[entry_index] = Some(matches!(start, VersionStart::Entry));
            let mut pending = VecDeque::from([entry_index]);
            while let Some(current_index) = pending.pop_front() {
                let current = positions[current_index];
                let step = flow
                    .get(&current)
                    .ok_or("payload version flow lost its step")?;
                let before = versions[current_index].ok_or("payload version arrival is absent")?;
                let after = if matches!(start, VersionStart::Completed(producer) if current == producer)
                {
                    true
                } else if before {
                    let modifies = match writes[current_index] {
                        Some(modifies) => modifies,
                        None => {
                            let result = modifies(current, step)?;
                            writes[current_index] = Some(result);
                            result
                        }
                    };
                    !modifies
                } else {
                    false
                };
                for (ordinal, edge) in step.successors.iter().enumerate() {
                    if !includes_edge(current, ordinal) {
                        continue;
                    }
                    let target_index = position_index(edge.target)?;
                    let arrival = after && !replaces_on_arrival(edge);
                    let joined = versions[target_index].map_or(arrival, |old| old && arrival);
                    if versions[target_index] != Some(joined) {
                        versions[target_index] = Some(joined);
                        if !pending.contains(&target_index) {
                            pending.push_back(target_index);
                        }
                    }
                }
            }
            Ok(versions[observation_index] == Some(true))
        }

        pub fn unchanged_stored_scalar_sources(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            value: ValueId,
            actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, ValueId)>,
        ) -> Option<Vec<ValueId>> {
            unchanged_stored_sources(types, graph, identity, value, actual_writes)
        }

        pub fn unchanged_stored_sources<Source: Ord>(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            identity: MachineId,
            value: ValueId,
            actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
        ) -> Option<Vec<Source>> {
            let machine = descriptor_machine(graph, identity)?;
            let mut reads = machine
                .scalar_payload_reads
                .iter()
                .flat_map(|(position, rows)| {
                    rows.iter().filter_map(move |(location, term)| {
                        matches!(term, semantic_vocabulary::ScalarTerm::Value { id, .. } if *id == value)
                            .then_some((*position, location))
                    })
                });
            let (observation, observed) = reads.next()?;
            if reads.next().is_some() {
                return None;
            }
            let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
            unchanged_stored_projection_sources_in(
                types,
                graph,
                &mut analysis,
                identity,
                observation,
                observed,
                actual_writes,
            )
        }

        pub fn unchanged_stored_projection_sources_in<Source: Ord>(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            analysis: &mut DescriptorAnalysis<'_>,
            identity: MachineId,
            observation: DescriptorPosition,
            observed: &DescriptorLocation,
            actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
        ) -> Option<Vec<Source>> {
            projected_stored_sources_in(
                types,
                graph,
                analysis,
                identity,
                observation,
                observed,
                actual_writes,
                |source, suffix| suffix.is_empty().then_some(source),
            )
        }

        pub fn projected_stored_sources_in<Source: Ord>(
            types: &[StructuralTypeDeclaration],
            graph: &[DescriptorMachine],
            analysis: &mut DescriptorAnalysis<'_>,
            identity: MachineId,
            observation: DescriptorPosition,
            observed: &DescriptorLocation,
            mut actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
            mut project: impl FnMut(
                Source,
                &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
            ) -> Option<Source>,
        ) -> Option<Vec<Source>> {
            if !analysis.uses_source_graph(types, graph) {
                return None;
            }
            let machine = descriptor_machine(graph, identity)?;
            if !machine.flow.contains_key(&observation) {
                return None;
            }
            let home = direct_owned_storage(types, machine, observed)
                .or_else(|| analysis.location(identity, observation, observed))?;
            let mut sources = Vec::new();
            for &producer in machine.flow.keys() {
                if producer == observation {
                    continue;
                }
                for (written, id) in actual_writes(producer) {
                    let destinations =
                        if machine.payload_producers.get(&producer) == Some(&written.root) {
                            vec![written.clone()]
                        } else {
                            let Some(destination) = direct_owned_storage(types, machine, &written)
                                .or_else(|| analysis.location(identity, producer, &written))
                            else {
                                continue;
                            };
                            vec![destination]
                        };
                    let [destination] = destinations.as_slice() else {
                        continue;
                    };
                    if destination.root != home.root || !home.path.starts_with(&destination.path) {
                        continue;
                    }
                    let Some(id) = project(id, &home.path[destination.path.len()..]) else {
                        continue;
                    };
                    if require_unchanged_between(
                        machine,
                        analysis,
                        producer,
                        observation,
                        &DescriptorPayloadTarget::current(home.clone()),
                    )
                    .is_ok()
                    {
                        sources.push(id);
                    }
                }
            }
            sources.sort();
            sources.dedup();
            (!sources.is_empty()).then_some(sources)
        }

        fn direct_owned_storage(
            types: &[StructuralTypeDeclaration],
            machine: &DescriptorMachine,
            location: &DescriptorLocation,
        ) -> Option<DescriptorLocation> {
            let place = exactly_one(machine.places.iter(), |row| row.place == location.root)?;
            if !matches!(place.definition, super::DescriptorDefinition::Local) {
                return None;
            }
            let ty = *machine.place_types.get(&location.root)?;
            if !super::plain_owned_shape(types.iter(), ty) {
                return None;
            }
            crate::descriptors::descriptor_lifetimes::descriptor_path(types, ty, &location.path)?;
            Some(location.clone())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::descriptors::descriptor_lifetimes::{
                DescriptorFlowBinding, DescriptorFlowEdge,
            };
            use semantic_vocabulary::BlockId;

            fn position(value: u64) -> DescriptorPosition {
                (BlockId::new(value).unwrap(), 0)
            }

            fn payload() -> DescriptorLocation {
                DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(1).unwrap())
            }

            fn joins(edge: &DescriptorFlowEdge, location: &DescriptorLocation) -> bool {
                edge.parameters
                    .iter()
                    .any(|parameter| parameter.contains(location) || location.contains(parameter))
            }

            fn copy_graph() -> (Vec<StructuralTypeDeclaration>, DescriptorMachine) {
                use super::super::{DescriptorDefinition, DescriptorPlace};
                use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
                use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};
                let original = payload();
                let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
                let carrier = StructuralTypeId::new(1).unwrap();
                let types = vec![StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id: carrier,
                    identity: TerminalIdentityIndex(1),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                    scalar_restriction: None,
                }];
                let mut flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[5]), (5, &[])]);
                flow.get_mut(&position(2))
                    .unwrap()
                    .definitions
                    .push(original.clone());
                let copying = flow.get_mut(&position(3)).unwrap();
                copying.definitions.push(copy.clone());
                copying.derivations.push((original.clone(), copy.clone()));
                let machine = DescriptorMachine {
                    id: MachineId::new(1).unwrap(),
                    returns: None,
                    places: [original.root, copy.root]
                        .into_iter()
                        .map(|place| DescriptorPlace {
                            producer: None,
                            place,
                            definition: DescriptorDefinition::Local,
                        })
                        .collect(),
                    place_types: BTreeMap::from([(original.root, carrier), (copy.root, carrier)]),
                    entry: position(1),
                    flow,
                    case_constructors: BTreeMap::new(),
                    case_successors: BTreeMap::new(),
                    payload_producers: BTreeMap::from([
                        (position(2), original.root),
                        (position(3), copy.root),
                    ]),
                    originations: BTreeMap::new(),
                    projections: BTreeMap::new(),
                    contents_queries: BTreeMap::new(),
                    contents_transfers: BTreeMap::new(),
                    payload_entry_predicates: None,
                    payload_scalar_definitions: BTreeMap::new(),
                    payload_scalar_postconditions: BTreeMap::new(),
                    payload_scalar_parameters: BTreeMap::new(),
                    scalar_payload_reads: BTreeMap::new(),
                    payload_edge_predicates: BTreeMap::new(),
                    payload_scalar_bindings: BTreeMap::new(),
                    scalar_payload_writes: BTreeMap::new(),
                    payload_invalidations: BTreeMap::new(),
                    unresolved_contents_writes: Default::default(),
                    storage_derivations: Default::default(),
                    calls: BTreeMap::new(),
                    normal_exits: vec![position(5)],
                };
                (types, machine)
            }

            #[test]
            fn entry_storage_refuses_overlapping_clobbers_without_tainting_other_homes_or_earlier_cuts()
             {
                let (types, mut machine) = copy_graph();
                for step in machine.flow.values_mut() {
                    step.definitions.clear();
                    step.derivations.clear();
                }
                let original = payload();
                let other =
                    DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
                let preserved = |machine: &DescriptorMachine| {
                    let graph = std::slice::from_ref(machine);
                    let mut analysis =
                        DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
                    require_unchanged_entry_storage(
                        &types,
                        graph,
                        &mut analysis,
                        machine.id,
                        position(4),
                        &DescriptorPayloadTarget::current(original.clone()),
                    )
                };
                let writes = |machine: &DescriptorMachine, at| {
                    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                        &types,
                        std::slice::from_ref(machine),
                    );
                    analysis.storage_writes(machine.id, at)
                };
                machine.payload_invalidations.insert(
                    position(2),
                    vec![Some(super::super::DescriptorContentsQuery::from_location(
                        &other,
                    ))],
                );
                assert!(preserved(&machine).is_ok());
                assert_eq!(writes(&machine, position(2)).unwrap(), Some(vec![other]));
                machine.payload_invalidations.insert(
                    position(2),
                    vec![Some(super::super::DescriptorContentsQuery::from_location(
                        &original,
                    ))],
                );
                assert!(preserved(&machine).is_err());
                assert_eq!(
                    writes(&machine, position(2)).unwrap(),
                    Some(vec![original.clone()])
                );
                machine
                    .payload_invalidations
                    .insert(position(2), vec![None]);
                assert!(preserved(&machine).is_err());
                assert_eq!(writes(&machine, position(2)).unwrap(), None);
                machine.payload_invalidations.clear();
                machine
                    .payload_invalidations
                    .insert(position(5), vec![None]);
                assert!(preserved(&machine).is_ok());
                assert_eq!(writes(&machine, position(2)).unwrap(), Some(Vec::new()));
                assert_eq!(writes(&machine, position(5)).unwrap(), None);
                assert_eq!(
                    writes(&machine, position(99)).unwrap_err(),
                    "storage write has no operation or edge cut"
                );
            }

            #[test]
            fn scalar_formal_renews_on_arrival_not_between_operations_in_its_block() {
                let (_, mut machine) = copy_graph();
                let entry = position(1);
                let capture = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
                let middle = (capture.0, 1);
                let observation = (capture.0, 2);
                let parameter = ValueId::new(8).unwrap();
                let step = |target: Option<DescriptorPosition>| DescriptorFlowStep {
                    successors: target
                        .into_iter()
                        .map(|target| DescriptorFlowEdge {
                            edge: None,
                            effects: Vec::new(),
                            definitions: Vec::new(),
                            target,
                            parameters: Vec::new(),
                            bindings: Vec::new(),
                        })
                        .collect(),
                    ..Default::default()
                };
                machine.entry = entry;
                machine.flow = BTreeMap::from([
                    (entry, step(Some(capture))),
                    (capture, step(Some(middle))),
                    (middle, step(Some(observation))),
                    (observation, step(None)),
                ]);
                machine.payload_scalar_parameters = BTreeMap::from([(capture.0, vec![parameter])]);
                machine.payload_scalar_definitions.clear();
                let mut analysis = ScalarCaptureAnalysis::new(&machine).unwrap();
                analysis
                    .require(capture, observation, &[parameter])
                    .unwrap();
                assert!(analysis.require(entry, observation, &[parameter]).is_err());
                require_completed_scalar_capture(
                    std::slice::from_ref(&machine),
                    machine.id,
                    capture,
                    observation,
                    &[parameter],
                )
                .unwrap();
            }

            #[test]
            fn prepared_scalar_capture_refuses_renewed_operands_without_reusing_old_answers() {
                let (_, mut machine) = copy_graph();
                let status = ValueId::new(8).unwrap();
                machine
                    .payload_scalar_definitions
                    .insert(position(1), vec![status]);
                let mut original = ScalarCaptureAnalysis::new(&machine).unwrap();
                original
                    .require(position(2), position(5), &[status])
                    .unwrap();
                original
                    .require(position(2), position(5), &[status])
                    .unwrap();
                assert_eq!(original.answers.len(), 1);

                machine
                    .payload_scalar_definitions
                    .insert(position(4), vec![status]);
                let mut renewed = ScalarCaptureAnalysis::new(&machine).unwrap();
                assert!(
                    renewed
                        .require(position(2), position(5), &[status])
                        .is_err()
                );
                assert!(
                    renewed
                        .require(position(2), position(2), &[status])
                        .is_err()
                );
                assert!(renewed.require(position(2), position(5), &[]).is_ok());
            }

            #[test]
            fn entry_version_includes_the_first_operation_write() {
                let (_, machine) = copy_graph();
                let observe = |start, first_writes| {
                    version_flow_from(
                        &machine.flow,
                        machine.entry,
                        start,
                        position(5),
                        |_, _| true,
                        |position, _| Ok(first_writes && position == machine.entry),
                        |_| false,
                    )
                    .unwrap()
                };
                assert!(observe(VersionStart::Entry, false));
                assert!(!observe(VersionStart::Entry, true));
                assert!(observe(VersionStart::Completed(machine.entry), true));
            }

            #[test]
            fn opaque_output_identity_needs_unchanged_actual_storage() {
                use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ScalarType};

                #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
                struct NativeOutput(DescriptorPosition, u32);

                let (types, mut machine) = copy_graph();
                let identity = machine.id;
                let read = ValueId::new(7).unwrap();
                machine.scalar_payload_reads.insert(
                    position(5),
                    vec![(
                        payload(),
                        ScalarTerm::value(
                            read,
                            ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            ),
                        ),
                    )],
                );
                let mut graph = vec![machine];
                let observe = |graph: &[DescriptorMachine]| {
                    unchanged_stored_sources(&types, graph, identity, read, |writer_position| {
                        if writer_position == position(2) {
                            vec![(payload(), NativeOutput(writer_position, 3))]
                        } else {
                            Vec::new()
                        }
                    })
                };
                assert_eq!(observe(&graph), Some(vec![NativeOutput(position(2), 3)]));
                graph[0].unresolved_contents_writes.insert(position(4));
                assert!(observe(&graph).is_none());
            }

            #[test]
            fn copied_projection_names_its_pre_copy_source_and_keeps_its_own_version() {
                use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
                use semantic_vocabulary::StructuralTypeId;
                use terminal_psi::{ArrayExtent, StructuralTypeShape};
                let (mut types, mut machine) = copy_graph();
                let mut element = types[0].clone();
                element.id = StructuralTypeId::new(2).unwrap();
                types[0].shape = StructuralTypeShape::FixedArray {
                    element: element.id,
                    length: ArrayExtent::Literal(2),
                };
                types.push(element);
                let original = payload();
                let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
                let mut observed = copy.clone();
                observed.path.push(DescriptorPathSegment::FixedIndex(1));
                let mut source_leaf = original.clone();
                source_leaf.path = observed.path.clone();
                machine.payload_invalidations.insert(
                    position(4),
                    vec![Some(DescriptorContentsQuery::from_location(&source_leaf))],
                );
                let observe = |machine: &DescriptorMachine| {
                    let graph = std::slice::from_ref(machine);
                    let mut analysis =
                        DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
                    projected_stored_sources_in(
                        &types,
                        graph,
                        &mut analysis,
                        machine.id,
                        position(5),
                        &observed,
                        |cut| {
                            if cut == position(3) {
                                vec![(copy.clone(), (cut, original.clone()))]
                            } else {
                                Vec::new()
                            }
                        },
                        |(capture, mut source), suffix| {
                            source.path.extend_from_slice(suffix);
                            Some((capture, source))
                        },
                    )
                };
                assert_eq!(observe(&machine), Some(vec![(position(3), source_leaf)]));
                machine.payload_invalidations.insert(
                    position(4),
                    vec![Some(DescriptorContentsQuery::from_location(&observed))],
                );
                assert!(observe(&machine).is_none());
            }

            #[test]
            fn equal_copies_keep_distinct_producers_and_writes_retire_only_their_contents() {
                let (types, machine) = copy_graph();
                let identity = machine.id;
                let mut graph = vec![machine];
                let original = DescriptorContentsQuery::from_location(&payload());
                let copy = DescriptorContentsQuery::from_location(&DescriptorLocation::whole(
                    semantic_vocabulary::PlaceId::new(2).unwrap(),
                ));
                let observe = |graph: &[DescriptorMachine], query| {
                    unchanged_payload_producer(&types, graph, identity, position(5), query)
                };
                assert_eq!(observe(&graph, &original).unwrap().producer, position(2));
                assert_eq!(observe(&graph, &copy).unwrap().producer, position(3));
                assert!(
                    require_unchanged_produced_payload(
                        &types,
                        &graph,
                        identity,
                        position(2),
                        position(5),
                        &copy.exact_location().unwrap(),
                    )
                    .is_err()
                );
                graph[0]
                    .payload_invalidations
                    .insert(position(4), vec![Some(copy.clone())]);
                assert!(observe(&graph, &original).is_ok());
                assert!(observe(&graph, &copy).is_err());
                graph[0]
                    .payload_invalidations
                    .insert(position(4), vec![None]);
                assert!(observe(&graph, &original).is_err());
                assert!(observe(&graph, &copy).is_err());
            }

            #[test]
            fn whole_borrowed_arrival_keeps_the_original_producer_and_invalidations() {
                use super::super::DescriptorDefinition;
                use crate::descriptors::descriptor_lifetimes::DescriptorFlowSource;
                use terminal_psi::StructuralAccess;

                let (types, mut machine) = copy_graph();
                let identity = machine.id;
                let borrowed =
                    DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
                machine.originations.insert(
                    position(2),
                    vec![(
                        payload(),
                        super::super::DescriptorCallOrigin::Argument {
                            source: payload().root,
                            projection: super::super::Projection::new(&[]),
                        },
                    )],
                );
                machine.places[1].definition = DescriptorDefinition::Arrival {
                    access: StructuralAccess::SharedBorrow,
                };
                machine.payload_producers.remove(&position(3));
                let capture = machine.flow.get_mut(&position(3)).unwrap();
                capture.definitions.clear();
                capture.derivations.clear();
                capture.successors[0].parameters.push(borrowed.clone());
                capture.successors[0].bindings.push(
                    DescriptorFlowBinding::new(
                        DescriptorFlowSource::Location(payload()),
                        borrowed.clone(),
                        StructuralAccess::SharedBorrow,
                        StructuralAccess::SharedBorrow,
                    )
                    .unwrap(),
                );
                let query = DescriptorContentsQuery::from_location(&borrowed);
                let mut graph = vec![machine];
                let observe = |graph: &[DescriptorMachine]| {
                    unchanged_payload_producer(&types, graph, identity, position(5), &query)
                };
                assert_eq!(
                    observe(&graph).unwrap(),
                    ProducedPayloadVersion {
                        producer: position(2),
                        payload: payload()
                    },
                );
                graph[0]
                    .payload_invalidations
                    .insert(position(4), vec![Some(query.clone())]);
                assert!(observe(&graph).is_err());
                graph[0].payload_invalidations.clear();
                graph[0].places[1].definition = DescriptorDefinition::Arrival {
                    access: StructuralAccess::Owned,
                };
                graph[0].flow.get_mut(&position(3)).unwrap().successors[0].bindings[0] =
                    DescriptorFlowBinding::new(
                        DescriptorFlowSource::Location(payload()),
                        borrowed,
                        StructuralAccess::Owned,
                        StructuralAccess::Owned,
                    )
                    .unwrap();
                assert!(observe(&graph).is_err());
            }

            #[test]
            fn ambiguous_producers_and_unfinished_results_do_not_supply_a_version() {
                let (types, machine) = copy_graph();
                let identity = machine.id;
                let mut graph = vec![machine];
                let original = DescriptorContentsQuery::from_location(&payload());
                assert!(
                    unchanged_payload_producer(&types, &graph, identity, position(2), &original)
                        .is_err()
                );
                graph[0]
                    .payload_producers
                    .insert(position(4), payload().root);
                assert_eq!(
                    unchanged_payload_producer(&types, &graph, identity, position(5), &original),
                    Err("observed payload has multiple whole-value producers"),
                );
            }

            #[test]
            fn completed_shared_observation_preserves_existing_storage_not_replacements() {
                let (types, mut machine) = copy_graph();
                let identity = machine.id;
                machine.payload_producers.remove(&position(2));
                machine
                    .flow
                    .get_mut(&position(2))
                    .unwrap()
                    .definitions
                    .clear();
                let mut graph = vec![machine];
                let observe = |graph: &[DescriptorMachine]| {
                    require_unchanged_observed_storage(
                        &types,
                        graph,
                        identity,
                        position(2),
                        position(5),
                        &DescriptorPayloadTarget::current(payload()),
                    )
                };
                assert!(observe(&graph).is_ok());
                assert!(
                    require_unchanged_produced_payload(
                        &types,
                        &graph,
                        identity,
                        position(2),
                        position(5),
                        &payload()
                    )
                    .is_err()
                );
                assert!(
                    require_unchanged_observed_storage(
                        &types,
                        &graph,
                        identity,
                        position(2),
                        position(2),
                        &DescriptorPayloadTarget::current(payload())
                    )
                    .is_err()
                );
                graph[0].payload_invalidations.insert(
                    position(4),
                    vec![Some(DescriptorContentsQuery::from_location(&payload()))],
                );
                assert!(observe(&graph).is_err());
                graph[0].payload_invalidations.clear();
                graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
                    .parameters
                    .push(payload());
                assert!(
                    observe(&graph).is_err(),
                    "an equal replacement is not this observation"
                );
                graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
                    .parameters
                    .clear();
                graph[0]
                    .flow
                    .get_mut(&position(1))
                    .unwrap()
                    .successors
                    .push(DescriptorFlowEdge {
                        edge: None,
                        effects: Vec::new(),
                        definitions: Vec::new(),
                        target: position(5),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    });
                assert!(
                    observe(&graph).is_err(),
                    "every arrival must have completed the observation"
                );
            }

            fn flow(edges: &[(u64, &[u64])]) -> BTreeMap<DescriptorPosition, DescriptorFlowStep> {
                edges
                    .iter()
                    .map(|(source, targets)| {
                        (
                            position(*source),
                            DescriptorFlowStep {
                                successors: targets
                                    .iter()
                                    .map(|target| DescriptorFlowEdge {
                                        edge: None,
                                        effects: Vec::new(),
                                        definitions: Vec::new(),
                                        target: position(*target),
                                        parameters: Vec::new(),
                                        bindings: Vec::new(),
                                    })
                                    .collect(),
                                ..Default::default()
                            },
                        )
                    })
                    .collect()
            }

            #[test]
            fn all_arrivals_need_the_completed_unchanged_version() {
                let diamond = flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
                let query = |writes: &[u64]| {
                    version_flow_from(
                        &diamond,
                        position(1),
                        VersionStart::Completed(position(2)),
                        position(5),
                        |_, _| true,
                        |current, _| Ok(writes.iter().any(|write| position(*write) == current)),
                        |edge| joins(edge, &payload()),
                    )
                    .unwrap()
                };
                assert!(query(&[]));
                assert!(!query(&[3]));
                assert!(!query(&[4]));
                let bypass = flow(&[(1, &[2, 4]), (2, &[3]), (3, &[5]), (4, &[5]), (5, &[])]);
                assert!(
                    !version_flow_from(
                        &bypass,
                        position(1),
                        VersionStart::Completed(position(2)),
                        position(5),
                        |_, _| true,
                        |_, _| Ok(false),
                        |edge| joins(edge, &payload()),
                    )
                    .unwrap()
                );
            }

            #[test]
            fn transition_replacement_retires_only_the_overlapping_version() {
                let original = payload();
                let other =
                    DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
                let field = DescriptorLocation {
                    root: original.root,
                    path: vec![
                        crate::descriptors::descriptor_lifetimes::DescriptorPathSegment::Field(
                            semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                        ),
                    ],
                };
                for (parameter, retained) in [
                    (other.clone(), true),
                    (original.clone(), false),
                    (field, false),
                ] {
                    let mut diamond =
                        flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
                    let edge = &mut diamond.get_mut(&position(4)).unwrap().successors[0];
                    edge.parameters.push(parameter.clone());
                    edge.bindings.push(
                        DescriptorFlowBinding::new(
                            other.clone(),
                            parameter,
                            terminal_psi::StructuralAccess::Owned,
                            terminal_psi::StructuralAccess::Owned,
                        )
                        .unwrap(),
                    );
                    assert_eq!(
                        version_flow_from(
                            &diamond,
                            position(1),
                            VersionStart::Completed(position(2)),
                            position(5),
                            |_, _| true,
                            |_, _| Ok(false),
                            |edge| joins(edge, &original),
                        )
                        .unwrap(),
                        retained,
                    );
                }
            }

            #[test]
            fn new_loop_completion_replaces_the_previous_dynamic_version() {
                let loop_flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[2, 5]), (5, &[])]);
                assert!(
                    version_flow_from(
                        &loop_flow,
                        position(1),
                        VersionStart::Completed(position(2)),
                        position(3),
                        |_, _| true,
                        |current, _| Ok(current == position(4)),
                        |edge| joins(edge, &payload()),
                    )
                    .unwrap()
                );
                let stale_loop = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[3, 5]), (5, &[])]);
                assert!(
                    !version_flow_from(
                        &stale_loop,
                        position(1),
                        VersionStart::Completed(position(2)),
                        position(3),
                        |_, _| true,
                        |current, _| Ok(current == position(4)),
                        |edge| joins(edge, &payload()),
                    )
                    .unwrap()
                );
            }
        }
    }
    pub(crate) mod scalar_captures {

        use std::collections::BTreeMap;

        use semantic_vocabulary::{
            IdTable, IdTableBuildError, PlaceId, ScalarTerm, ScalarType, StructuralPlaceKind,
            StructuralTypeId, ValueId,
        };
        use terminal_psi::{
            Operation, OperationKind, RecordFieldValue, TerminalMachine, TerminalModule, Terminator,
        };

        use crate::descriptors::descriptor_lifetimes::{
            DescriptorLocation, DescriptorPosition, descriptor_location,
        };

        #[derive(Clone, Debug)]
        pub enum CapturedScalarOperand {
            Value {
                id: ValueId,
                scalar_type: ScalarType,
            },
            Storage(DescriptorLocation),
        }

        impl CapturedScalarOperand {
            pub fn value_term(&self) -> Option<ScalarTerm> {
                match self {
                    Self::Value { id, scalar_type } => Some(ScalarTerm::value(*id, *scalar_type)),
                    Self::Storage(_) => None,
                }
            }
        }

        pub struct TerminalScalarCaptures {
            pub writes: BTreeMap<
                DescriptorPosition,
                Result<Vec<(DescriptorLocation, ScalarTerm)>, &'static str>,
            >,
            pub arrivals: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, CapturedScalarOperand)>>>,
        }

        impl TerminalScalarCaptures {
            pub fn new(
                module: &TerminalModule,
                machine: &TerminalMachine,
            ) -> Result<Self, &'static str> {
                let place_types = place_type_index(machine)?;
                let mut type_rows = Vec::new();
                for value in machine
                    .parameters
                    .iter()
                    .chain(machine.block_parameters())
                    .chain(
                        machine
                            .operations()
                            .filter_map(|operation| operation.result.scalar_ref()),
                    )
                {
                    type_rows.push((value.id, Some(value.scalar_type)));
                }
                type_rows.sort_by_key(|(id, _)| *id);
                let types = IdTable::from_rows_with_gaps(type_rows, 65_536)
                    .map_err(capture_inventory_failure)?;
                let mut block_rows = machine
                    .blocks
                    .iter()
                    .map(|block| (block.id, Some(block)))
                    .collect::<Vec<_>>();
                block_rows.sort_by_key(|(id, _)| *id);
                let blocks = IdTable::from_rows_with_gaps(block_rows, 65_536)
                    .map_err(capture_inventory_failure)?;
                let mut result = Self {
                    writes: BTreeMap::new(),
                    arrivals: BTreeMap::new(),
                };
                for block in &machine.blocks {
                    for (ordinal, operation) in block.operations.iter().enumerate() {
                        if let Some(writes) = scalar_storage_writes(
                            module,
                            |root| place_types.get(root).copied().flatten(),
                            operation,
                        ) {
                            let position = (
                                block.id,
                                u32::try_from(ordinal).map_err(
                                    |_| "scalar write ordinal exceeds its canonical position",
                                )?,
                            );
                            let writes = writes.and_then(|rows| {
                                rows.into_iter()
                                    .map(|(location, actual)| {
                                        let scalar_type =
                                            types.get(actual).copied().flatten().ok_or(
                                                "scalar writer has no evaluated operand binder",
                                            )?;
                                        Ok((location, ScalarTerm::value(actual, scalar_type)))
                                    })
                                    .collect()
                            });
                            result.writes.insert(position, writes);
                        }
                    }
                    let position = (
                        block.id,
                        u32::try_from(block.operations.len())
                            .map_err(|_| "scalar arrival ordinal exceeds its canonical position")?,
                    );
                    let mut arrivals = Vec::new();
                    for (ordinal, binding) in
                        crate::control_graph::forwarded_bindings(block).enumerate()
                    {
                        let target = blocks
                            .get(binding.target)
                            .copied()
                            .flatten()
                            .ok_or("scalar arrival names an absent target block")?;
                        let operands = if let Some(arguments) = binding.arguments {
                            if arguments.len() != target.parameters.len() {
                                return Err("scalar arrival changes its target parameter count");
                            }
                            target
                                .parameters
                                .iter()
                                .zip(arguments)
                                .map(|(formal, actual)| {
                                    let scalar_type = types
                                        .get(*actual)
                                        .copied()
                                        .flatten()
                                        .ok_or("scalar arrival has no actual operand binder")?;
                                    if scalar_type != formal.scalar_type {
                                        return Err(
                                            "scalar arrival changes its actual operand type",
                                        );
                                    }
                                    Ok((
                                        formal.id,
                                        CapturedScalarOperand::Value {
                                            id: *actual,
                                            scalar_type,
                                        },
                                    ))
                                })
                                .collect::<Result<Vec<_>, _>>()?
                        } else {
                            let Terminator::StructuralCase {
                                source,
                                path,
                                cases,
                            } = &block.terminator
                            else {
                                return Err("scalar arrival has no original case-payload capture");
                            };
                            let edge = cases
                                .get(ordinal)
                                .ok_or("scalar arrival lost its selected case edge")?;
                            if edge.target != binding.target
                                || edge.payload_fields.len() != target.parameters.len()
                            {
                                return Err(
                                    "scalar case arrival changes its original edge or parameter count",
                                );
                            }
                            let root = place_types
                                .get(*source)
                                .copied()
                                .flatten()
                                .ok_or("scalar case source has no declared carrier")?;
                            let captured_types = super::case_payload_scalar_types(
                                &module.structural_types,
                                root,
                                path,
                                edge.case,
                                &edge.payload_fields,
                            )
                            .ok_or("scalar case arrival has no exact declared field roster")?;
                            let mut selected = path.clone();
                            selected.push(terminal_psi::StructuralPathSegment::Case(edge.case));
                            target
                                .parameters
                                .iter()
                                .zip(&edge.payload_fields)
                                .zip(captured_types)
                                .map(|((formal, field), scalar_type)| {
                                    if scalar_type != formal.scalar_type {
                                        return Err(
                                            "scalar case arrival changes its declared payload type",
                                        );
                                    }
                                    let location = descriptor_location(
                                        &module.structural_types,
                                        *source,
                                        root,
                                        &selected,
                                        Some(*field),
                                    )
                                    .ok_or("scalar case arrival has no exact payload storage coordinate")?;
                                    Ok((formal.id, CapturedScalarOperand::Storage(location)))
                                })
                                .collect::<Result<Vec<_>, _>>()?
                        };
                        arrivals.push(operands);
                    }
                    if !arrivals.is_empty() {
                        result.arrivals.insert(position, arrivals);
                    }
                }
                Ok(result)
            }
        }

        #[derive(Clone, Debug)]
        pub struct ResidentValueTransfer {
            pub source: DescriptorLocation,
            pub destination: DescriptorLocation,
        }

        pub enum ResidentValueCapture {
            Complete(Vec<ResidentValueTransfer>),
            Unresolved(&'static str),
        }

        pub type ResidentValueCaptures = BTreeMap<DescriptorPosition, ResidentValueCapture>;

        pub fn terminal_resident_value_transfers(
            machine: &TerminalMachine,
            graph: &super::DescriptorMachine,
        ) -> Result<ResidentValueCaptures, &'static str> {
            if graph.id != machine.id || graph.entry != (machine.entry, 0) {
                return Err("resident capture graph belongs to another machine");
            }
            let mut captures = BTreeMap::new();
            for block in &machine.blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let position = (
                        block.id,
                        u32::try_from(ordinal).map_err(
                            |_| "resident capture ordinal exceeds its canonical position",
                        )?,
                    );
                    let step = graph
                        .flow
                        .get(&position)
                        .ok_or("resident capture has no current storage step")?;
                    if matches!(
                        &operation.kind,
                        terminal_psi::structural_leaf_copy_pair_pattern!()
                            | OperationKind::MoveStructuralValue { .. }
                            | OperationKind::TransformContent { .. }
                            | OperationKind::MoveStructuralField { .. }
                            | OperationKind::StoreStructuralField { .. }
                            | OperationKind::EstablishStructuralArray { .. }
                            | OperationKind::EstablishRecord { .. }
                            | OperationKind::EstablishStructuralCase { .. }
                    ) {
                        if step.may_escape {
                            captures.insert(
                                position,
                                ResidentValueCapture::Unresolved(
                                    "resident value capture has an unresolved storage coordinate",
                                ),
                            );
                            continue;
                        }
                        if step.derivations.iter().any(|(source, destination)| {
                            graph.storage_derivations.contains(&(
                                position,
                                source.clone(),
                                destination.clone(),
                            ))
                        }) {
                            captures.insert(
                                position,
                                ResidentValueCapture::Unresolved(
                                    "resident value capture substitutes an address formation",
                                ),
                            );
                            continue;
                        }
                        captures.insert(
                            position,
                            ResidentValueCapture::Complete(
                                step.derivations
                                    .iter()
                                    .map(|(source, destination)| ResidentValueTransfer {
                                        source: source.clone(),
                                        destination: destination.clone(),
                                    })
                                    .collect(),
                            ),
                        );
                    }
                }
            }
            Ok(captures)
        }

        fn place_type_index(
            machine: &TerminalMachine,
        ) -> Result<IdTable<PlaceId, Option<StructuralTypeId>>, &'static str> {
            let mut type_rows = Vec::new();
            let mut parameter_rows = Vec::new();
            for (ordinal, parameter) in machine.structural_parameters.iter().enumerate() {
                if usize::try_from(parameter.position).ok() != Some(ordinal) {
                    return Err("scalar capture signature parameter has no dense coordinate");
                }
                type_rows.push((parameter.place, Some(parameter.structural_type)));
                parameter_rows.push((
                    parameter.place,
                    Some(StructuralPlaceKind::Parameter {
                        position: parameter.position,
                    }),
                ));
            }
            for block in &machine.blocks {
                for (ordinal, parameter) in block.structural_parameters.iter().enumerate() {
                    if usize::try_from(parameter.position).ok() != Some(ordinal) {
                        return Err("scalar capture block parameter has no dense coordinate");
                    }
                    type_rows.push((parameter.place, Some(parameter.structural_type)));
                    parameter_rows.push((
                        parameter.place,
                        Some(StructuralPlaceKind::BlockParameter {
                            block: block.id,
                            position: parameter.position,
                        }),
                    ));
                }
            }
            parameter_rows.sort_by_key(|(id, _)| *id);
            let parameter_kinds = IdTable::from_rows_with_gaps(parameter_rows, 65_536)
                .map_err(capture_inventory_failure)?;
            let mut declared = machine
                .structural_places
                .iter()
                .map(|place| place.id)
                .collect::<Vec<_>>();
            declared.sort_unstable();
            if declared.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err("scalar capture roster repeats a structural place");
            }
            for place in &machine.structural_places {
                if parameter_kinds
                    .get(place.id)
                    .and_then(Option::as_ref)
                    .is_some_and(|kind| *kind != place.kind)
                {
                    return Err("scalar capture parameter changes its declared storage kind");
                }
                match place.kind {
                    kind @ (StructuralPlaceKind::Parameter { .. }
                    | StructuralPlaceKind::BlockParameter { .. }) => {
                        if parameter_kinds.get(place.id).and_then(Option::as_ref) != Some(&kind) {
                            return Err("scalar capture place changes its parameter coordinate");
                        }
                    }
                    StructuralPlaceKind::Result => {
                        let result = machine
                            .result
                            .structural()
                            .filter(|result| result.place == place.id)
                            .ok_or(
                                "scalar capture result place has no matching result signature",
                            )?;
                        type_rows.push((place.id, Some(result.structural_type)));
                    }
                    StructuralPlaceKind::ProviderAttachment { .. } => {}
                    kind => {
                        let carrier = kind
                            .produced_structural_type()
                            .ok_or("scalar capture producer place has no carrier")?;
                        type_rows.push((place.id, Some(carrier)));
                    }
                }
            }
            if parameter_kinds
                .iter()
                .any(|(place, kind)| kind.is_some() && declared.binary_search(&place).is_err())
            {
                return Err("scalar capture parameter has no declared storage place");
            }
            type_rows.sort_by_key(|(id, _)| *id);
            IdTable::from_rows_with_gaps(type_rows, 65_536).map_err(capture_inventory_failure)
        }

        fn capture_inventory_failure(reason: IdTableBuildError) -> &'static str {
            match reason {
                IdTableBuildError::RepeatedOrUnordered { .. } => {
                    "scalar capture inventory has repeated or unordered identities"
                }
                IdTableBuildError::SpanOverflow => {
                    "scalar capture inventory identity span overflows"
                }
                IdTableBuildError::SpanLimit { .. } => {
                    "scalar capture inventory identity span exceeds its allocation bound"
                }
                IdTableBuildError::AllocationFailed => "scalar capture inventory allocation failed",
            }
        }

        pub(crate) fn scalar_storage_writes(
            module: &TerminalModule,
            place_type: impl Fn(PlaceId) -> Option<StructuralTypeId>,
            operation: &Operation,
        ) -> Option<Result<Vec<(DescriptorLocation, ValueId)>, &'static str>> {
            let mut writes = Vec::new();
            let mut write = |root, path: &[terminal_psi::StructuralPathSegment], field, value| {
                let root_type =
                    place_type(root).ok_or("scalar writer has no declared destination carrier")?;
                let location =
                    descriptor_location(&module.structural_types, root, root_type, path, field)
                        .ok_or("scalar writer has no exact supported destination projection")?;
                writes.push((location, value));
                Ok::<_, &'static str>(())
            };
            let capture = || -> Result<_, &'static str> {
                match &operation.kind {
                    OperationKind::WriteOnlyPrimitiveStore {
                        destination,
                        path,
                        value,
                        ..
                    } => write(*destination, path, None, *value)?,
                    OperationKind::StructuralScalarFieldStore {
                        destination,
                        path,
                        field,
                        value,
                        ..
                    } => write(*destination, path, Some(*field), *value)?,
                    OperationKind::EstablishPrimitiveLocal { value, .. } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or("primitive initializer has no structural result")?;
                        write(result.place, &[], None, *value)?;
                    }
                    OperationKind::EstablishScalarArray { elements } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or("scalar array initializer has no structural result")?;
                        for (ordinal, value) in elements.iter().enumerate() {
                            let path = crate::scalar_array_leaf_path(
                                module.structural_types.iter(),
                                result.structural_type,
                                u64::try_from(ordinal)
                                    .map_err(|_| "scalar array initializer ordinal is too large")?,
                            )
                            .ok_or("scalar array initializer has no declared scalar leaf")?;
                            write(result.place, &path, None, *value)?;
                        }
                    }
                    OperationKind::EstablishScalarCase {
                        result_case,
                        fields,
                    } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or("scalar case initializer has no structural result")?;
                        let path = [terminal_psi::StructuralPathSegment::Case(*result_case)];
                        for field in fields {
                            write(result.place, &path, Some(field.field), field.value)?;
                        }
                    }
                    OperationKind::EstablishRecord { fields }
                    | OperationKind::EstablishStructuralCase { fields, .. } => {
                        let result = operation
                            .result
                            .structural()
                            .ok_or("record initializer has no structural result")?;
                        for (ordinal, field) in fields.iter().enumerate() {
                            let RecordFieldValue::Scalar { value, .. } = &field.value else {
                                continue;
                            };
                            if let OperationKind::EstablishStructuralCase { result_case, .. } =
                                &operation.kind
                            {
                                let shape = &module
                                    .structural_type(result.structural_type)
                                    .ok_or("case initializer has no declared carrier")?
                                    .shape;
                                let path = terminal_psi::structural_case_initializer_path(
                                    shape,
                                    *result_case,
                                    ordinal,
                                    field.field,
                                )
                                .ok_or(
                                    "case initializer changes its declared scalar field roster",
                                )?;
                                write(result.place, &path, None, *value)?;
                            } else {
                                write(result.place, &[], Some(field.field), *value)?;
                            }
                        }
                    }
                    _ => return Ok(false),
                }
                Ok(true)
            };
            let mut capture = capture;
            match capture() {
                Ok(false) => None,
                Ok(true) => Some(Ok(writes)),
                Err(reason) => Some(Err(reason)),
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::descriptor_results::{
                DescriptorCallOrigin, DescriptorInvocation, DescriptorInvocationOwner,
                DescriptorInvocationTarget, DescriptorPayloadTarget,
                terminal_borrowed_storage_graph,
            };
            use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
            use crate::descriptors::descriptor_lifetimes::{
                DescriptorContentsQuery, DescriptorContentsStep,
            };
            use crate::storage::borrowed_storage::Projection;
            use semantic_vocabulary::{
                BlockId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
                ScalarQualificationSetId, StructuralTypeId,
            };
            use std::collections::BTreeSet;
            use terminal_psi::{
                Block, ByteSequenceCarrier, OperationResult, StructuralAccess,
                StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
                StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
                TerminalIdentityIndex, TerminalMachineResult, ValueDeclaration,
            };
            fn machine(id: u64) -> TerminalMachine {
                let block = BlockId::new(1).unwrap();
                TerminalMachine {
                    assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
                    id: MachineId::new(id).unwrap(),
                    attachment: None,
                    ranked_scc: None,
                    structural_places: Vec::new(),
                    call_formations: Vec::new(),
                    block_entry_claims: Vec::new(),
                    loan_completions: Vec::new(),
                    content_identity_reshuffles: Vec::new(),
                    content_partition_compositions: Vec::new(),
                    entry: block,
                    blocks: vec![Block::plain(
                        block,
                        Vec::new(),
                        Terminator::ReturnUnit {
                            edge: EdgeId::new(1).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    )],
                    contract: terminal_psi::MachineContract {
                        selected_application: None,
                        declaration: None,
                        parameters: Vec::new(),
                        parameter_order: Vec::new(),
                        structural_parameters: Vec::new(),
                        result: TerminalMachineResult::Unit,
                        entry_claims: Vec::new(),
                        declared_service_reach: None,
                        closed_reach_application: None,
                        published_synchronous_entries: Vec::new(),
                        published_service_ceiling: Vec::new(),
                        content_entry_claims: Vec::new(),
                        ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
                    },
                }
            }

            fn sequence_capture_module() -> TerminalModule {
                let bytes = StructuralTypeId::new(1).unwrap();
                let sequence = StructuralTypeId::new(2).unwrap();
                let declaration = |id, shape| StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex::ABSENT,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape,
                    scalar_restriction: None,
                };
                let mut module = TerminalModule::declarations();
                module.structural_types = vec![
                    declaration(
                        bytes,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: None,
                        }),
                    ),
                    declaration(
                        sequence,
                        StructuralTypeShape::ElementView {
                            element: bytes,
                            access: None,
                        },
                    ),
                ];
                let mut body = machine(1);
                for position in 0..2 {
                    let place = PlaceId::new(position as u64 + 1).unwrap();
                    body.parameter_order
                        .push(terminal_psi::BoundaryParameterKind::Structural);
                    body.structural_parameters
                        .push(StructuralParameterDeclaration {
                            place,
                            position,
                            structural_type: bytes,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::Owned,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            additional_input_qualifications: Vec::new(),
                        });
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::Parameter { position },
                        });
                }
                for (operation, place) in [(1, 3), (2, 4), (3, 5)] {
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: PlaceId::new(place).unwrap(),
                            kind: StructuralPlaceKind::OperationResult {
                                producer: OperationId::new(operation).unwrap(),
                                structural_type: sequence,
                            },
                        });
                }
                let argument = |place| {
                    terminal_psi::StructuralArgument::whole(
                        PlaceId::new(place).unwrap(),
                        StructuralAccess::Owned,
                    )
                };
                body.blocks[0].operations = vec![
                    sequence_result(
                        1,
                        3,
                        sequence,
                        OperationKind::EstablishStructuralArray {
                            elements: vec![argument(1), argument(2)],
                        },
                    ),
                    sequence_result(
                        2,
                        4,
                        sequence,
                        OperationKind::StructuralLeafCopy {
                            source: PlaceId::new(3).unwrap(),
                            path: Vec::new(),
                        },
                    ),
                    sequence_result(
                        3,
                        5,
                        sequence,
                        OperationKind::EstablishStructuralArray {
                            elements: vec![argument(2), argument(2)],
                        },
                    ),
                    Operation::unsuspended(
                        OperationId::new(4).unwrap(),
                        OperationResult::Unit,
                        OperationKind::StoreStructuralField {
                            disposition:
                                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                            destination: PlaceId::new(3).unwrap(),
                            path: Vec::new(),
                            field: None,
                            value: argument(5),
                        },
                    ),
                ];
                module.machines = vec![body];
                module
            }

            fn sequence_result(
                id: u64,
                place: u64,
                ty: StructuralTypeId,
                kind: OperationKind,
            ) -> Operation {
                Operation::unsuspended(
                    OperationId::new(id).unwrap(),
                    OperationResult::Structural(terminal_psi::StructuralOperationResult {
                        place: PlaceId::new(place).unwrap(),
                        structural_type: ty,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind,
                )
            }

            fn reference_flow_module(record: bool) -> TerminalModule {
                let scalar = StructuralTypeId::new(1).unwrap();
                let reference = StructuralTypeId::new(2).unwrap();
                let pair = StructuralTypeId::new(3).unwrap();
                let declaration = |id, copyable, shape| StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex::ABSENT,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: if copyable {
                        terminal_psi::StructuralMultiplicity::Unrestricted
                    } else {
                        terminal_psi::StructuralMultiplicity::Affine
                    },
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape,
                    scalar_restriction: None,
                };
                let field = |number: u32| terminal_psi::StructuralFieldDeclaration {
                    declaration_identity: Default::default(),
                    id: semantic_vocabulary::StructuralFieldId::new(u64::from(number)).unwrap(),
                    identity: TerminalIdentityIndex(number),
                    inline_carrier: None,
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Structural(reference),
                    storage: None,
                };
                let mut module = TerminalModule::declarations();
                module.structural_types = vec![
                    declaration(
                        scalar,
                        true,
                        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    ),
                    declaration(
                        reference,
                        true,
                        StructuralTypeShape::Reference {
                            referent: scalar,
                            access: StructuralAccess::SharedBorrow,
                        },
                    ),
                    declaration(
                        pair,
                        false,
                        StructuralTypeShape::Record {
                            fields: vec![field(1), field(2)],
                            storage: terminal_psi::StructuralRecordStorage::Packed,
                        },
                    ),
                ];
                let mut body = machine(1);
                let ty = if record { pair } else { reference };
                let source = PlaceId::new(1).unwrap();
                body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
                body.structural_parameters
                    .push(StructuralParameterDeclaration {
                        place: source,
                        position: 0,
                        structural_type: ty,
                        multiplicity: if record {
                            StructuralMultiplicity::Affine
                        } else {
                            StructuralMultiplicity::Unrestricted
                        },
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                body.structural_places
                    .push(terminal_psi::StructuralPlaceDeclaration {
                        id: source,
                        kind: StructuralPlaceKind::Parameter { position: 0 },
                    });
                for (operation, place) in [(1, 2), (2, 3)] {
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: PlaceId::new(place).unwrap(),
                            kind: StructuralPlaceKind::OperationResult {
                                producer: OperationId::new(operation).unwrap(),
                                structural_type: ty,
                            },
                        });
                }
                module.machines = vec![body];
                module
            }

            #[test]
            fn successor_diagnostics_keep_actual_operands_and_target_parameters() {
                use crate::descriptor_results::{
                    DescriptorGraphError, terminal_reference_graph_from,
                };
                use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

                let mut module = TerminalModule::declarations();
                let mut body = machine(1);
                let target = BlockId::new(2).unwrap();
                let actual = ValueId::new(1).unwrap();
                let edge = EdgeId::new(1).unwrap();
                body.blocks[0].operations.push(Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    OperationResult::Scalar(ValueDeclaration {
                        id: actual,
                        scalar_type: ScalarType::Boolean,
                        qualifications: ScalarQualificationSetId::ZERO,
                    }),
                    OperationKind::BooleanConstant { value: true },
                ));
                body.blocks[0].terminator = Terminator::Jump {
                    edge,
                    target,
                    arguments: vec![actual],
                    erased_arguments: vec![],
                    erased_proof_arguments: vec![],
                    structural_arguments: vec![],
                    cleanup_actions: vec![],
                };
                let mut arrival = Block::plain(
                    target,
                    vec![],
                    Terminator::ReturnUnit {
                        edge: EdgeId::new(2).unwrap(),
                        cleanup_actions: vec![],
                    },
                );
                arrival.parameters.push(ValueDeclaration {
                    id: ValueId::new(2).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                    qualifications: ScalarQualificationSetId::ZERO,
                });
                body.blocks.push(arrival);
                module.machines.push(body);
                let failure = terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()])
                    .err()
                    .expect("a Boolean actual cannot initialize an integer parameter");
                assert_eq!(
                    failure.detail(),
                    Some("descriptor arrival scalar type differs from target parameter")
                );
                assert!(matches!(failure, DescriptorGraphError::IncompleteMachine {
                    successor: Some((found, destination)), ..
                } if found == edge && destination == target));
                let detail = failure.in_module(&module);
                assert!(detail.contains("terminator Jump"), "{detail}");
                assert!(
                    detail.contains("scalar actuals [(ValueId(1), Some(Boolean))]"),
                    "{detail}"
                );
                assert!(
                    detail.contains("ValueId(2)") && detail.contains("Integer"),
                    "{detail}"
                );

                let Terminator::Jump { arguments, .. } =
                    &mut module.machines[0].blocks[0].terminator
                else {
                    unreachable!()
                };
                arguments.clear();
                let failure = terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()])
                    .err()
                    .expect("missing actuals remain refused");
                assert_eq!(
                    failure.detail(),
                    Some("descriptor arrival scalar arity differs from target parameters")
                );
                let Terminator::Jump { arguments, .. } =
                    &mut module.machines[0].blocks[0].terminator
                else {
                    unreachable!()
                };
                arguments.push(actual);
                module.machines[0].blocks[1].parameters[0].scalar_type = ScalarType::Boolean;
                assert!(
                    terminal_reference_graph_from(&module, [MachineId::new(1).unwrap()]).is_ok()
                );
            }

            #[test]
            fn nonreturning_local_calls_retain_write_effects_and_actual_backing() {
                use crate::descriptor_results::{DescriptorAnalysis, DescriptorSourceOrigin};
                use semantic_vocabulary::{InvocationSite, ScalarType, ValueId};
                // An ordinary borrowed object has exact storage. An owned
                // pointer without its pointee supply must remain unknown.
                for indirect in [false, true] {
                    let path = if indirect {
                        vec![StructuralPathSegment::Referent]
                    } else {
                        vec![]
                    };
                    let mut module = reference_flow_module(false);
                    let scalar = StructuralTypeId::new(1).unwrap();
                    module.structural_types[1].shape = StructuralTypeShape::Reference {
                        referent: scalar,
                        access: StructuralAccess::MutableBorrow,
                    };
                    module.structural_types[1].multiplicity = StructuralMultiplicity::Affine;
                    let actual = PlaceId::new(1).unwrap();
                    let formal = PlaceId::new(7).unwrap();
                    let mut caller = module.machines.remove(0);
                    caller.structural_places.truncate(1);
                    caller.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
                    if !indirect {
                        caller.structural_parameters[0].structural_type = scalar;
                        caller.structural_parameters[0].multiplicity =
                            StructuralMultiplicity::Unrestricted;
                    }
                    let mut helper = machine(u64::MAX);
                    helper.contract.id = ContractId::new(2).unwrap();
                    helper.parameter_order = caller.parameter_order.clone();
                    helper.structural_parameters = caller.structural_parameters.clone();
                    helper.structural_parameters[0].place = formal;
                    if !indirect {
                        helper.structural_parameters[0].access = StructuralAccess::MutableBorrow;
                    }
                    helper.structural_places = caller.structural_places.clone();
                    helper.structural_places[0].id = formal;
                    helper.blocks[0].operations = vec![
                        Operation::unsuspended(
                            OperationId::new(1).unwrap(),
                            OperationResult::Scalar(ValueDeclaration {
                                id: ValueId::new(1).unwrap(),
                                scalar_type: ScalarType::Boolean,
                                qualifications: ScalarQualificationSetId::ZERO,
                            }),
                            OperationKind::BooleanConstant { value: true },
                        ),
                        Operation::unsuspended(
                            OperationId::new(2).unwrap(),
                            OperationResult::Unit,
                            OperationKind::WriteOnlyPrimitiveStore {
                                destination: formal,
                                path: path.clone(),
                                value: ValueId::new(1).unwrap(),
                                range_obligation: None,
                            },
                        ),
                    ];
                    helper.blocks[0].terminator = Terminator::Crash {
                        edge: EdgeId::new(1).unwrap(),
                        cause: semantic_vocabulary::CrashCause::Trap,
                        site_guard: vec![],
                        frontier_lower_bound: vec![],
                    };
                    caller.blocks[0].operations.clear();
                    caller.blocks[0].terminator = Terminator::InvokeNoNormal {
                        edge: EdgeId::new(1).unwrap(),
                        static_reach_binding: None,
                        required_contract: None,
                        callee: helper.id,
                        arguments: vec![],
                        erased_arguments: vec![],
                        erased_proof_arguments: vec![],
                        structural_arguments: vec![terminal_psi::StructuralArgument::whole(
                            actual,
                            if indirect {
                                StructuralAccess::Owned
                            } else {
                                StructuralAccess::MutableBorrow
                            },
                        )],
                        claim_transfers: vec![],
                        requirement_obligations: vec![],
                        crash_continuations: vec![],
                    };
                    module.machines = vec![caller, helper];
                    let caller = module.machines[0].id;
                    let helper = module.machines[1].id;
                    module.machines.reverse();
                    let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                        &module,
                        [caller],
                        &[],
                    )
                    .unwrap();
                    assert_eq!(
                        graph.iter().map(|row| row.id).collect::<Vec<_>>(),
                        [helper, caller],
                        "a sorted sparse lookup must preserve module output order"
                    );
                    let body = graph.iter().find(|row| row.id == caller).unwrap();
                    let cut = (BlockId::new(1).unwrap(), 0);
                    let call = body
                        .known_invocation(&cut)
                        .expect("the exact local terminal call has an account");
                    assert_eq!(
                        semantic_vocabulary::InvocationSite::from(call.owner),
                        InvocationSite::InvokeNoNormal(EdgeId::new(1).unwrap())
                    );
                    assert_eq!(call.callee(), Some(helper));
                    assert!(!body.unresolved_contents_writes.contains(&cut));
                    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(
                        &module.structural_types,
                        &graph,
                    );
                    assert_eq!(
                        analysis.storage_access_frame(caller).unwrap(),
                        Some([].as_slice())
                    );
                    let accesses = analysis
                        .storage_access_frame(helper)
                        .unwrap()
                        .unwrap()
                        .to_vec();
                    assert!(accesses.iter().any(|&input| matches!(
                        analysis.storage_access(helper, input),
                        Some((access_plans::AccessOperation::Write, _))
                    )));
                    if indirect {
                        assert!(
                            analysis.storage_writes(caller, cut).unwrap().is_none(),
                            "an unprovided pointee cannot become a known-empty write"
                        );
                        continue;
                    }
                    let written = analysis.storage_writes(caller, cut).unwrap().unwrap();
                    assert!(
                        written
                            .iter()
                            .any(|row| row.root == actual && row.path.is_empty()),
                        "a write before Trap still changes the actual referent: {written:?}"
                    );
                    let origin = DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                        source: formal,
                        projection: Projection::new(&path),
                    });
                    let actuals = analysis
                        .substitute_call_source(caller, cut, &origin)
                        .unwrap();
                    assert_eq!(
                        actuals,
                        vec![DescriptorSourceOrigin::Backing(
                            DescriptorCallOrigin::Argument {
                                source: actual,
                                projection: Projection::new(&path),
                            }
                        )]
                    );
                    let mut duplicate = module.clone();
                    duplicate.machines.push(
                        module
                            .machines
                            .iter()
                            .find(|row| row.id == helper)
                            .unwrap()
                            .clone(),
                    );
                    assert!(matches!(
                        crate::descriptor_results::terminal_borrowed_storage_graph_from(&duplicate, [caller], &[]),
                        Err(crate::descriptor_results::DescriptorGraphError::MissingOrAmbiguousMachine(id)) if id == helper
                    ));
                    assert!(terminal_borrowed_storage_graph(&duplicate).is_empty());
                    let mut missing = module.clone();
                    missing.machines.retain(|row| row.id != helper);
                    let missing_graph =
                        crate::descriptor_results::terminal_borrowed_storage_graph_from(
                            &missing,
                            [caller],
                            &[],
                        )
                        .unwrap();
                    let mut unknown = DescriptorAnalysis::with_borrowed_storage_slots(
                        &missing.structural_types,
                        &missing_graph,
                    );
                    assert!(unknown.storage_access_frame(caller).unwrap().is_none());
                    assert!(unknown.storage_writes(caller, cut).unwrap().is_none());
                }
            }

            #[test]
            fn borrowing_a_carrier_slot_does_not_copy_its_existing_permission() {
                use crate::descriptor_lifetimes::{
                    descriptor_entry_lifetime, descriptor_location_lifetime,
                };
                let mut module = reference_flow_module(false);
                let reference = StructuralTypeId::new(2).unwrap();
                let child_type = StructuralTypeId::new(3).unwrap();
                module.structural_types[2].shape = StructuralTypeShape::Reference {
                    referent: reference,
                    access: StructuralAccess::SharedBorrow,
                };
                module.structural_types[2].multiplicity =
                    terminal_psi::StructuralMultiplicity::Unrestricted;
                let body = &mut module.machines[0];
                let source = PlaceId::new(1).unwrap();
                let child = PlaceId::new(2).unwrap();
                body.structural_places.pop();
                body.structural_places[1].kind = StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(1).unwrap(),
                    structural_type: child_type,
                };
                let mut formation = sequence_result(
                    1,
                    2,
                    child_type,
                    OperationKind::EstablishReference {
                        source: terminal_psi::StructuralArgument::whole(
                            source,
                            StructuralAccess::SharedBorrow,
                        ),
                        storage: None,
                    },
                );
                let OperationResult::Structural(result) = &mut formation.result else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Affine;
                body.blocks[0].operations = vec![formation];
                let identity = body.id;
                let graph =
                    crate::descriptor_results::terminal_reference_graph_from(&module, [identity])
                        .unwrap();
                let row = &graph[0];
                let after = (row.entry.0, 1);
                let parent = descriptor_entry_lifetime(
                    &row.flow,
                    row.entry,
                    &DescriptorLocation::whole(source),
                );
                let child_life = descriptor_location_lifetime(
                    &row.flow,
                    row.entry,
                    &DescriptorLocation::whole(child),
                );
                assert_eq!(
                    parent.surviving[&after],
                    BTreeSet::from([DescriptorLocation::whole(source)])
                );
                assert_eq!(
                    child_life.surviving[&after],
                    BTreeSet::from([DescriptorLocation::whole(child)])
                );
                assert!(row.flow[&row.entry].authority_derivations.is_empty());
                assert!(
                    !row.flow[&row.entry].derivations.is_empty(),
                    "the child still captures its real carrier storage"
                );
            }

            #[test]
            fn projected_release_retires_only_its_reference_leaf() {
                use crate::descriptor_lifetimes::descriptor_location_lifetime;
                let mut module = reference_flow_module(true);
                let body = &mut module.machines[0];
                let source = PlaceId::new(1).unwrap();
                let carrier = PlaceId::new(2).unwrap();
                let ty = StructuralTypeId::new(3).unwrap();
                let mut moved = sequence_result(
                    1,
                    2,
                    ty,
                    OperationKind::MoveStructuralValue {
                        source: terminal_psi::StructuralArgument::whole(
                            source,
                            StructuralAccess::Owned,
                        ),
                    },
                );
                let OperationResult::Structural(result) = &mut moved.result else {
                    unreachable!();
                };
                result.multiplicity = StructuralMultiplicity::Affine;
                body.structural_places.pop();
                body.blocks[0].operations = vec![
                    moved,
                    Operation::unsuspended(
                        OperationId::new(2).unwrap(),
                        OperationResult::Unit,
                        OperationKind::ReleaseReference {
                            source: carrier,
                            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(1))],
                        },
                    ),
                ];
                let id = body.id;
                let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
                    .unwrap();
                let row = &graph[0];
                let birth = (row.entry.0, 0);
                let release = (row.entry.0, 1);
                let after = (row.entry.0, 2);
                let leaf = |number| DescriptorLocation {
                    root: carrier,
                    path: vec![DescriptorPathSegment::Field(
                        semantic_vocabulary::StructuralFieldId::new(number).unwrap(),
                    )],
                };
                assert_eq!(
                    row.flow[&release].observed_inputs().collect::<Vec<_>>(),
                    vec![&leaf(1)]
                );
                assert_eq!(row.flow[&release].definitions, vec![leaf(1)]);
                let released = descriptor_location_lifetime(&row.flow, birth, &leaf(1));
                let sibling = descriptor_location_lifetime(&row.flow, birth, &leaf(2));
                assert!(!released.authorized[&after].contains(&leaf(1)));
                assert!(sibling.authorized[&after].contains(&leaf(2)));
            }

            #[test]
            fn moving_a_carrier_retires_its_old_name_while_copying_keeps_the_source() {
                use crate::descriptor_lifetimes::descriptor_location_lifetime;
                let mut module = reference_flow_module(false);
                let body = &mut module.machines[0];
                let source = PlaceId::new(1).unwrap();
                let moved = PlaceId::new(2).unwrap();
                let copied = PlaceId::new(3).unwrap();
                let ty = StructuralTypeId::new(2).unwrap();
                body.blocks[0].operations = vec![
                    sequence_result(
                        1,
                        2,
                        ty,
                        OperationKind::MoveStructuralValue {
                            source: terminal_psi::StructuralArgument::whole(
                                source,
                                StructuralAccess::Owned,
                            ),
                        },
                    ),
                    sequence_result(
                        2,
                        3,
                        ty,
                        OperationKind::StructuralLeafCopy {
                            source: moved,
                            path: Vec::new(),
                        },
                    ),
                ];
                let id = body.id;
                let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
                    .unwrap();
                let row = &graph[0];
                let moving = (row.entry.0, 0);
                let copying = (row.entry.0, 1);
                let after = (row.entry.0, 2);
                assert!(
                    row.flow[&moving]
                        .definitions
                        .contains(&DescriptorLocation::whole(source))
                );
                assert!(
                    !row.flow[&copying]
                        .definitions
                        .contains(&DescriptorLocation::whole(moved))
                );
                let lifetime = descriptor_location_lifetime(
                    &row.flow,
                    moving,
                    &DescriptorLocation::whole(moved),
                );
                assert!(lifetime.authorized[&after].contains(&DescriptorLocation::whole(moved)));
                assert!(lifetime.authorized[&after].contains(&DescriptorLocation::whole(copied)));
                let names = crate::descriptors::descriptor_lifetimes::value_locations(
                    &row.flow,
                    moving,
                    &DescriptorLocation::whole(source),
                    true,
                );
                assert!(!names[&copying].contains(&DescriptorLocation::whole(source)));
                assert!(names[&copying].contains(&DescriptorLocation::whole(moved)));
            }

            #[test]
            fn constructing_with_fresh_permission_uses_its_retained_custody() {
                let mut module = reference_flow_module(false);
                let body = &mut module.machines[0];
                let backing = PlaceId::new(1).unwrap();
                let source = PlaceId::new(2).unwrap();
                let carrier = PlaceId::new(3).unwrap();
                let sibling = PlaceId::new(4).unwrap();
                let scalar = StructuralTypeId::new(1).unwrap();
                let reference = StructuralTypeId::new(2).unwrap();
                let pair = StructuralTypeId::new(3).unwrap();
                let mut parameter = body.structural_parameters[0].clone();
                parameter.place = sibling;
                parameter.position = 1;
                body.structural_parameters.push(parameter);
                body.parameter_order
                    .push(terminal_psi::BoundaryParameterKind::Structural);
                body.structural_parameters[0].structural_type = scalar;
                body.structural_parameters[0].access = StructuralAccess::SharedBorrow;
                body.structural_places[2].kind = StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(2).unwrap(),
                    structural_type: pair,
                };
                body.structural_places
                    .push(terminal_psi::StructuralPlaceDeclaration {
                        id: sibling,
                        kind: StructuralPlaceKind::Parameter { position: 1 },
                    });
                let field = |number, place| terminal_psi::RecordFieldInitializer {
                    field: semantic_vocabulary::StructuralFieldId::new(number).unwrap(),
                    value: RecordFieldValue::Structural(terminal_psi::StructuralArgument::whole(
                        place,
                        StructuralAccess::Owned,
                    )),
                };
                body.blocks[0].operations = vec![
                    sequence_result(
                        1,
                        2,
                        reference,
                        OperationKind::EstablishReference {
                            source: terminal_psi::StructuralArgument::whole(
                                backing,
                                StructuralAccess::SharedBorrow,
                            ),
                            storage: None,
                        },
                    ),
                    sequence_result(
                        2,
                        3,
                        pair,
                        OperationKind::EstablishRecord {
                            fields: vec![field(1, source), field(2, sibling)],
                        },
                    ),
                ];
                for operation in &mut body.blocks[0].operations {
                    let OperationResult::Structural(result) = &mut operation.result else {
                        unreachable!();
                    };
                    result.multiplicity = StructuralMultiplicity::Affine;
                }
                let id = body.id;
                let graph = crate::descriptor_results::terminal_reference_graph_from(&module, [id])
                    .unwrap();
                let row = &graph[0];
                let constructor = (row.entry.0, 1);
                let after = (row.entry.0, 2);
                let source = DescriptorLocation::whole(source);
                assert!(row.flow[&constructor].definitions.contains(&source));
                assert!(
                    !row.flow[&constructor]
                        .definitions
                        .contains(&DescriptorLocation::whole(sibling))
                );
                let mut analysis =
                    crate::descriptor_results::DescriptorAnalysis::with_reference_slots(
                        &module.structural_types,
                        &graph,
                    );
                let original = analysis.symbolic_source_origins(id, constructor, &source);
                assert!(original.is_some());
                let stored = DescriptorLocation {
                    root: carrier,
                    path: vec![DescriptorPathSegment::Field(
                        semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    )],
                };
                assert_eq!(
                    analysis.symbolic_source_origins(id, after, &stored),
                    original,
                    "the destination reads the origin before source consumption",
                );
                assert!(
                    analysis
                        .symbolic_source_origins(id, after, &source)
                        .is_none()
                );
                assert!(
                    analysis
                        .symbolic_source_origins(id, after, &DescriptorLocation::whole(sibling))
                        .is_some()
                );
            }

            // Replacing a descriptor cannot retarget a previously copied
            // sequence. Its two elements keep distinct sources at construction.
            #[test]
            fn stored_sequence_ordinals_keep_the_completed_constructor_cut() {
                use crate::descriptor_replay::engine::{Origin, Replay};
                let module = sequence_capture_module();
                let id = module.machines[0].id;
                let block = module.machines[0].entry;
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &module,
                    [id],
                    &[],
                )
                .unwrap();
                let row = &graph[0];
                let mut replay = Replay::with_slot_kind(
                    &module.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(id, 0).unwrap();
                let current = states.get(&(block, 4)).unwrap();
                let origin = |source| {
                    Origin::Backing(DescriptorCallOrigin::Argument {
                        source: PlaceId::new(source).unwrap(),
                        projection: Projection::new(&[]),
                    })
                };
                let query = |root| DescriptorContentsQuery {
                    root: PlaceId::new(root).unwrap(),
                    steps: vec![DescriptorContentsStep::any_element(
                        StructuralTypeId::new(1).unwrap(),
                    )],
                };
                assert_eq!(
                    replay.read_contents(row, current, &query(4)),
                    Some(BTreeSet::from([origin(1), origin(2)]))
                );
                assert_eq!(
                    replay.read_contents(row, current, &query(3)),
                    Some(BTreeSet::from([origin(2)]))
                );
                let targets = replay.contents_targets(row, current, &query(4)).unwrap();
                assert_eq!(
                    targets,
                    (0..2)
                        .map(|index| DescriptorPayloadTarget::Storage {
                            location: DescriptorLocation {
                                root: PlaceId::new(3).unwrap(),
                                path: vec![DescriptorPathSegment::FixedIndex(index)]
                            },
                            completed: Some((id, (block, 1))),
                        })
                        .collect()
                );
                for (index, source) in [(0, 1), (1, 2)] {
                    let ordinal = DescriptorContentsQuery {
                        root: PlaceId::new(3).unwrap(),
                        steps: vec![DescriptorContentsStep::Address(
                            DescriptorPathSegment::FixedIndex(index),
                        )],
                    };
                    assert_eq!(
                        replay.read_contents(row, current, &ordinal),
                        Some(BTreeSet::from([origin(source)]))
                    );
                }
                let outside = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![DescriptorContentsStep::Address(
                        DescriptorPathSegment::FixedIndex(2),
                    )],
                };
                assert!(replay.read_contents(row, current, &outside).is_none());
                assert!(
                    crate::descriptor_lifetimes::descriptor_location(
                        &module.structural_types,
                        PlaceId::new(3).unwrap(),
                        StructuralTypeId::new(2).unwrap(),
                        &[StructuralPathSegment::FixedIndex(0)],
                        None,
                    )
                    .is_none(),
                    "an internal constructor ordinal is not a proved source index"
                );
                let mut replacements = BTreeMap::new();
                assert!(
                    replay
                        .contents_replacements(
                            row,
                            current,
                            &query(4),
                            Some(BTreeSet::from([origin(2)])),
                            &mut replacements
                        )
                        .is_none(),
                    "captured contents do not grant a live write"
                );

                // Equal machine/block/instruction numbers in another immutable
                // owner cannot supply this constructor's history.
                let mut foreign = sequence_capture_module();
                let OperationKind::EstablishStructuralArray { elements } =
                    &mut foreign.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                elements.swap(0, 1);
                let foreign_graph =
                    crate::descriptor_results::terminal_borrowed_storage_graph_from(
                        &foreign,
                        [id],
                        &[],
                    )
                    .unwrap();
                let mut other = Replay::with_slot_kind(
                    &foreign.structural_types,
                    &foreign_graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let other_states = other.states(id, 0).unwrap();
                let wrong = crate::descriptor_replay::engine::DescriptorReadDepth::at(
                    &foreign_graph[0],
                    current,
                    Some(&other_states),
                );
                assert!(
                    replay
                        .read_contents_with_depth(row, current, &query(4), &wrong)
                        .is_none()
                );

                // Local owned construction is not permission to publish a
                // result borrowing that local backing without its public join.
                assert!(
                    replay
                        .exported(
                            row,
                            &Origin::Storage(DescriptorLocation::whole(PlaceId::new(3).unwrap()))
                        )
                        .is_none()
                );
            }

            // Capture retains the pointer value; following that pointer still
            // observes current contents under the already-held mutable loan.
            #[test]
            fn stored_sequence_reference_capture_keeps_the_referent_live() {
                use crate::descriptor_replay::engine::{Origin, Replay};
                let mut module = sequence_capture_module();
                let bytes = StructuralTypeId::new(1).unwrap();
                let sequence = StructuralTypeId::new(2).unwrap();
                let reference = StructuralTypeId::new(3).unwrap();
                let mut pointer = module.structural_types[0].clone();
                pointer.id = reference;
                pointer.shape = StructuralTypeShape::Reference {
                    referent: bytes,
                    access: StructuralAccess::MutableBorrow,
                };
                module.structural_types.push(pointer);
                module.structural_types[1].shape = StructuralTypeShape::ElementView {
                    element: reference,
                    access: None,
                };
                let body = &mut module.machines[0];
                body.structural_parameters[0].structural_type = reference;
                body.structural_places.retain(|place| place.id.get() <= 3);
                body.blocks[0].operations = vec![
                    sequence_result(
                        1,
                        3,
                        sequence,
                        OperationKind::EstablishStructuralArray {
                            elements: vec![terminal_psi::StructuralArgument::whole(
                                PlaceId::new(1).unwrap(),
                                StructuralAccess::Owned,
                            )],
                        },
                    ),
                    Operation::unsuspended(
                        OperationId::new(2).unwrap(),
                        OperationResult::Unit,
                        OperationKind::StoreStructuralField {
                            disposition:
                                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                            destination: PlaceId::new(1).unwrap(),
                            path: vec![StructuralPathSegment::Referent],
                            field: None,
                            value: terminal_psi::StructuralArgument::whole(
                                PlaceId::new(2).unwrap(),
                                StructuralAccess::Owned,
                            ),
                        },
                    ),
                ];
                let id = body.id;
                let block = body.entry;
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &module,
                    [id],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &module.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(id, 0).unwrap();
                let current = states.get(&(block, 2)).unwrap();
                let pointer = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![DescriptorContentsStep::any_element(reference)],
                };
                assert_eq!(
                    replay.read_contents(&graph[0], current, &pointer),
                    Some(BTreeSet::from([Origin::Incoming(
                        DescriptorLocation::whole(PlaceId::new(1).unwrap())
                    )]))
                );
                let mut pointee = pointer;
                pointee.steps.push(DescriptorContentsStep::Address(
                    DescriptorPathSegment::Referent,
                ));
                let direct_query = DescriptorContentsQuery {
                    root: PlaceId::new(1).unwrap(),
                    steps: vec![DescriptorContentsStep::Address(
                        DescriptorPathSegment::Referent,
                    )],
                };
                let replacement = Origin::Backing(DescriptorCallOrigin::Argument {
                    source: PlaceId::new(2).unwrap(),
                    projection: Projection::new(&[]),
                });
                let direct = replay.read_contents(&graph[0], current, &direct_query);
                let through_sequence = replay.read_contents(&graph[0], current, &pointee);
                let constructor = replay.read_contents(
                    &graph[0],
                    states.get(&(block, 1)).unwrap(),
                    &direct_query,
                );
                assert_eq!(
                    direct,
                    Some(BTreeSet::from([replacement.clone()])),
                    "the existing direct read after the write"
                );
                assert_ne!(
                    direct, constructor,
                    "the write changed the live pointee after construction"
                );
                assert_eq!(
                    through_sequence, direct,
                    "a captured pointer must use the same live read, not its old pointee snapshot"
                );
                assert!(
                    replay
                        .contents_targets(&graph[0], current, &pointee)
                        .unwrap()
                        .iter()
                        .all(|target| !matches!(
                            target,
                            DescriptorPayloadTarget::Storage {
                                completed: Some(_),
                                ..
                            }
                        )),
                    "the live referent is not frozen at the pointer's capture cut"
                );

                let mut redirected = module.clone();
                let body = &mut redirected.machines[0];
                for (number, ty) in [(4, reference), (5, bytes)] {
                    let position = u32::try_from(body.structural_parameters.len()).unwrap();
                    let place = PlaceId::new(number).unwrap();
                    body.parameter_order
                        .push(terminal_psi::BoundaryParameterKind::Structural);
                    body.structural_parameters
                        .push(StructuralParameterDeclaration {
                            place,
                            position,
                            structural_type: ty,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::Owned,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            additional_input_qualifications: Vec::new(),
                        });
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: place,
                            kind: StructuralPlaceKind::Parameter { position },
                        });
                }
                for (operation, path, value) in [
                    (3, Vec::new(), 4),
                    (4, vec![StructuralPathSegment::Referent], 5),
                ] {
                    body.blocks[0].operations.push(Operation::unsuspended(
                        OperationId::new(operation).unwrap(),
                        OperationResult::Unit,
                        OperationKind::StoreStructuralField {
                            disposition:
                                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
                            destination: PlaceId::new(1).unwrap(),
                            path,
                            field: None,
                            value: terminal_psi::StructuralArgument::whole(
                                PlaceId::new(value).unwrap(),
                                StructuralAccess::Owned,
                            ),
                        },
                    ));
                }
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &redirected,
                    [id],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &redirected.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(id, 0).unwrap();
                let current = states.get(&(block, 4)).unwrap();
                let next = Origin::Backing(DescriptorCallOrigin::Argument {
                    source: PlaceId::new(5).unwrap(),
                    projection: Projection::new(&[]),
                });
                let direct = replay
                    .read_contents(&graph[0], current, &direct_query)
                    .unwrap();
                let retained = replay.read_contents(&graph[0], current, &pointee).unwrap();
                assert!(
                    direct.contains(&next),
                    "the current variable now names the replacement pointer's referent"
                );
                assert!(
                    retained.contains(&replacement),
                    "the sequence keeps the first pointer and its live referent"
                );
                assert!(
                    !retained.contains(&next),
                    "a current-root shortcut must not retarget the captured pointer"
                );
                assert_ne!(retained, direct);
            }

            // A call reads the actual sequence in its caller's cut. Equal
            // block and operation numbers in the callee do not name that cut.
            #[test]
            fn stored_sequence_call_inputs_keep_the_callers_capture() {
                use crate::descriptor_replay::engine::{Origin, Replay};
                let mut module = sequence_capture_module();
                let bytes = StructuralTypeId::new(1).unwrap();
                let sequence = StructuralTypeId::new(2).unwrap();
                let reader_id = MachineId::new(2).unwrap();
                module.machines[0].structural_places.push(
                    terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(6).unwrap(),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(5).unwrap(),
                            structural_type: bytes,
                        },
                    },
                );
                module.machines[0].blocks[0]
                    .operations
                    .push(sequence_result(
                        5,
                        6,
                        bytes,
                        OperationKind::CallStructural {
                            callee: reader_id,
                            structural_arguments: vec![terminal_psi::StructuralArgument::whole(
                                PlaceId::new(4).unwrap(),
                                StructuralAccess::Owned,
                            )],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                    ));
                let mut reader = machine(2);
                reader.contract.id = ContractId::new(2).unwrap();
                reader.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
                reader.structural_parameters = vec![StructuralParameterDeclaration {
                    place: PlaceId::new(1).unwrap(),
                    position: 0,
                    structural_type: sequence,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    additional_input_qualifications: Vec::new(),
                }];
                reader.structural_places = vec![
                    terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(1).unwrap(),
                        kind: StructuralPlaceKind::Parameter { position: 0 },
                    },
                    terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(3).unwrap(),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(3).unwrap(),
                            structural_type: bytes,
                        },
                    },
                ];
                let integer = semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap();
                let scalar = |id, value| {
                    Operation::unsuspended(
                        OperationId::new(id).unwrap(),
                        OperationResult::Scalar(ValueDeclaration {
                            id: ValueId::new(id).unwrap(),
                            scalar_type: ScalarType::Integer(integer),
                            qualifications: ScalarQualificationSetId::ZERO,
                        }),
                        OperationKind::IntegerConstant {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                        },
                    )
                };
                reader.blocks[0].operations = vec![
                    scalar(1, 0),
                    scalar(2, 2),
                    sequence_result(
                        3,
                        3,
                        bytes,
                        OperationKind::ElementViewRead {
                            source: terminal_psi::StructuralArgument::whole(
                                PlaceId::new(1).unwrap(),
                                StructuralAccess::SharedBorrow,
                            ),
                            index: ValueId::new(1).unwrap(),
                            length: ValueId::new(2).unwrap(),
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                            path: Vec::new(),
                        },
                    ),
                ];
                reader.result =
                    TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
                        place: PlaceId::new(3).unwrap(),
                        structural_type: bytes,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        reference_sources: Vec::new(),
                    });
                reader.blocks[0].terminator = Terminator::ReturnStructural {
                    edge: EdgeId::new(1).unwrap(),
                    source: PlaceId::new(3).unwrap(),
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                module.machines.push(reader);
                let caller = module.machines[0].id;
                let block = module.machines[0].entry;
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &module,
                    [caller],
                    &[],
                )
                .unwrap();
                let mut duplicate_module = module.clone();
                let duplicate = &mut duplicate_module.machines[0];
                let duplicate_block = &mut duplicate.blocks[0];
                let (ordinal, constructor) = duplicate_block
                    .operations
                    .iter_mut()
                    .enumerate()
                    .find(|(_, operation)| {
                        matches!(
                            operation.kind,
                            OperationKind::EstablishStructuralArray { .. }
                        )
                    })
                    .unwrap();
                let OperationKind::EstablishStructuralArray { elements } = &mut constructor.kind
                else {
                    unreachable!()
                };
                elements.push(elements[0].clone());
                let expected: Vec<_> = elements.iter().map(|element| element.place).collect();
                let constructor_position = (duplicate_block.id, u32::try_from(ordinal).unwrap());
                let duplicates =
                    crate::descriptor_results::terminal_borrowed_storage_graph(&duplicate_module);
                let duplicate = duplicates.iter().find(|row| row.id == caller).unwrap();
                assert_eq!(
                    duplicate.flow[&constructor_position]
                        .inputs
                        .iter()
                        .map(|(input, _)| input.as_ref().unwrap().root)
                        .collect::<Vec<_>>(),
                    expected,
                    "captured constructor operands preserve order and duplicate elements"
                );
                let mut replay = Replay::with_slot_kind(
                    &module.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(caller, 0).unwrap();
                let row = graph.iter().find(|row| row.id == caller).unwrap();
                // This owner conservatively tracks every possible indexed
                // origin; the separate bounds owner still owes its proof.
                let expected = (1..=2)
                    .map(|source| {
                        Origin::Backing(DescriptorCallOrigin::Argument {
                            source: PlaceId::new(source).unwrap(),
                            projection: Projection::new(&[]),
                        })
                    })
                    .collect();
                assert_eq!(
                    replay.read(
                        row,
                        states.get(&(block, 5)).unwrap(),
                        &DescriptorLocation::whole(PlaceId::new(6).unwrap())
                    ),
                    Some(expected)
                );

                // A local owned allocation still cannot be substituted for a
                // public owned-result contract or an escaping borrowed source.
                let mut escaping = sequence_capture_module();
                let body = &mut escaping.machines[0];
                body.result =
                    TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
                        place: PlaceId::new(4).unwrap(),
                        structural_type: sequence,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        reference_sources: Vec::new(),
                    });
                body.blocks[0].terminator = Terminator::ReturnStructural {
                    edge: EdgeId::new(1).unwrap(),
                    source: PlaceId::new(4).unwrap(),
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &escaping,
                    [caller],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &escaping.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                assert!(replay.summary(caller, 0).is_none());

                // Moving the roster carries its own storage, while each
                // ordinal must retain the distinct caller view it contains.
                let mut moved = sequence_capture_module();
                moved.structural_types[1].multiplicity =
                    terminal_psi::StructuralMultiplicity::Affine;
                let body = &mut moved.machines[0];
                body.blocks[0].operations.truncate(1);
                body.structural_places.retain(|place| place.id.get() <= 3);
                body.result =
                    TerminalMachineResult::Structural(terminal_psi::StructuralResultDeclaration {
                        place: PlaceId::new(3).unwrap(),
                        structural_type: sequence,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        reference_sources: Vec::new(),
                    });
                body.blocks[0].terminator = Terminator::ReturnStructural {
                    edge: EdgeId::new(1).unwrap(),
                    source: PlaceId::new(3).unwrap(),
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &moved,
                    [caller],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &moved.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let result = replay
                    .summary(caller, 0)
                    .and_then(|summary| summary.result.into_slots())
                    .unwrap();
                assert_eq!(result.len(), 1);
                let [
                    crate::descriptor_replay::DescriptorStoredOrigin::Carried { element, elements },
                ] = result[0].origins.as_slice()
                else {
                    panic!("the returned roster needs its complete element account");
                };
                assert_eq!(*element, bytes);
                assert_eq!(elements.len(), 2);
                for (ordinal, slots) in elements.iter().enumerate() {
                    assert_eq!(slots.len(), 1);
                    assert!(slots[0].path.is_empty());
                    assert!(matches!(slots[0].origins.as_slice(),
                        [crate::descriptor_replay::DescriptorStoredOrigin::Parameter { position, projection }]
                        if *position == ordinal && *projection == Projection::new(&[])));
                }

                // The caller has the same numeric block/instruction/place
                // coordinates as the constructor. Read its rebased account,
                // then move the result and reuse its old descriptor home.
                let mut returned = moved.clone();
                let mut receiver = sequence_capture_module().machines.remove(0);
                receiver.id = MachineId::new(2).unwrap();
                receiver.contract.id = ContractId::new(2).unwrap();
                receiver.blocks[0].operations[0].kind = OperationKind::CallStructural {
                    callee: caller,
                    structural_arguments: (1..=2)
                        .map(|source| {
                            terminal_psi::StructuralArgument::whole(
                                PlaceId::new(source).unwrap(),
                                StructuralAccess::Owned,
                            )
                        })
                        .collect(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                };
                receiver.blocks[0].operations[1].kind = OperationKind::MoveStructuralField {
                    source: PlaceId::new(3).unwrap(),
                    path: Vec::new(),
                    field: None,
                };
                let OperationKind::StoreStructuralField { disposition, .. } =
                    &mut receiver.blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                *disposition = terminal_psi::StructuralStoreDisposition::Vacant;
                let receiver_id = receiver.id;
                let receiver_entry = receiver.entry;
                returned.machines.push(receiver);
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &returned,
                    [receiver_id],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &returned.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(receiver_id, 0).unwrap();
                let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
                let current = states.get(&(receiver_entry, 4)).unwrap();
                for (ordinal, source) in [(0, 1), (1, 2)] {
                    let query = DescriptorContentsQuery {
                        root: PlaceId::new(4).unwrap(),
                        steps: vec![DescriptorContentsStep::Address(
                            DescriptorPathSegment::FixedIndex(ordinal),
                        )],
                    };
                    assert_eq!(
                        replay.read_contents(receiver, current, &query),
                        Some(BTreeSet::from([Origin::Backing(
                            DescriptorCallOrigin::Argument {
                                source: PlaceId::new(source).unwrap(),
                                projection: Projection::new(&[]),
                            }
                        )]))
                    );
                }
                let query = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![DescriptorContentsStep::any_element(bytes)],
                };
                let after_call = states.get(&(receiver_entry, 1)).unwrap();
                let constructor = graph.iter().find(|row| row.id == caller).unwrap();
                let constructor_states = replay.states(caller, 0).unwrap();
                let wrong = crate::descriptor_replay::engine::DescriptorReadDepth::at(
                    constructor,
                    after_call,
                    Some(&constructor_states),
                );
                assert!(
                    replay
                        .read_contents_with_depth(receiver, after_call, &query, &wrong)
                        .is_none(),
                    "callee instruction numbers cannot supply caller capture history"
                );

                // Packing a borrowed view into an owned outer roster does
                // not transfer ownership of the local storage it points at.
                let mut borrowed_leaf = moved.clone();
                let borrowed = StructuralTypeId::new(3).unwrap();
                let outer = StructuralTypeId::new(4).unwrap();
                let mut declaration = borrowed_leaf.structural_types[1].clone();
                declaration.id = borrowed;
                declaration.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
                declaration.shape = StructuralTypeShape::ElementView {
                    element: bytes,
                    access: Some(StructuralAccess::SharedBorrow),
                };
                borrowed_leaf.structural_types.push(declaration);
                let mut declaration = borrowed_leaf.structural_types[1].clone();
                declaration.id = outer;
                declaration.shape = StructuralTypeShape::ElementView {
                    element: borrowed,
                    access: None,
                };
                borrowed_leaf.structural_types.push(declaration);
                let argument = |place, access| {
                    terminal_psi::StructuralArgument::whole(PlaceId::new(place).unwrap(), access)
                };
                let body = &mut borrowed_leaf.machines[0];
                for (operation, place, ty) in [(2, 4, borrowed), (3, 5, outer)] {
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: PlaceId::new(place).unwrap(),
                            kind: StructuralPlaceKind::OperationResult {
                                producer: OperationId::new(operation).unwrap(),
                                structural_type: ty,
                            },
                        });
                }
                body.blocks[0].operations.extend([
                    sequence_result(
                        2,
                        4,
                        borrowed,
                        OperationKind::EstablishElementView {
                            destination: PlaceId::new(4).unwrap(),
                            source: argument(3, StructuralAccess::SharedBorrow),
                            element: bytes,
                            region: terminal_psi::ElementViewRegion::WholeCollection,
                        },
                    ),
                    sequence_result(
                        3,
                        5,
                        outer,
                        OperationKind::CallStructural {
                            callee: reader_id,
                            structural_arguments: vec![argument(4, StructuralAccess::Owned)],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                    ),
                ]);
                let TerminalMachineResult::Structural(result) = &mut body.result else {
                    unreachable!()
                };
                result.place = PlaceId::new(5).unwrap();
                result.structural_type = outer;
                let Terminator::ReturnStructural { source, .. } = &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                *source = PlaceId::new(5).unwrap();
                let mut pack = moved.machines[0].clone();
                pack.id = reader_id;
                pack.contract.id = ContractId::new(2).unwrap();
                pack.parameter_order.truncate(1);
                pack.structural_parameters.truncate(1);
                pack.structural_parameters[0].structural_type = borrowed;
                pack.structural_places.retain(|row| row.id.get() != 2);
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = &mut pack.structural_places[1].kind
                else {
                    unreachable!()
                };
                *structural_type = outer;
                pack.blocks[0].operations = vec![sequence_result(
                    1,
                    3,
                    outer,
                    OperationKind::EstablishStructuralArray {
                        elements: vec![argument(1, StructuralAccess::Owned)],
                    },
                )];
                let TerminalMachineResult::Structural(result) = &mut pack.result else {
                    unreachable!()
                };
                result.structural_type = outer;
                borrowed_leaf.machines.push(pack);
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &borrowed_leaf,
                    [caller],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &borrowed_leaf.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                assert!(
                    replay
                        .summary(reader_id, 0)
                        .and_then(|summary| summary.result.into_slots())
                        .is_some(),
                    "the helper can carry a view borrowed from its caller"
                );
                assert!(
                    replay.summary(caller, 0).is_none(),
                    "re-export must not turn a borrowed local leaf into owned storage"
                );

                // Each return keeps its own extent. A shorter literal roster
                // must merge without claiming the longer one's ordinal.
                let mut alternatives = moved.clone();
                let body = &mut alternatives.machines[0];
                body.parameters = vec![ValueDeclaration {
                    id: semantic_vocabulary::ValueId::new(1).unwrap(),
                    scalar_type: semantic_vocabulary::ScalarType::Boolean,
                    qualifications: ScalarQualificationSetId::ZERO,
                }];
                body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Scalar];
                body.structural_parameters.clear();
                for row in &mut body.structural_places {
                    row.kind = match row.id.get() {
                        1 | 2 => StructuralPlaceKind::ByteSequenceLiteral {
                            declaration_ordinal: (row.id.get() - 1) as u32,
                            structural_type: bytes,
                        },
                        3 => StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(3).unwrap(),
                            structural_type: sequence,
                        },
                        _ => unreachable!(),
                    };
                }
                body.structural_places
                    .push(terminal_psi::StructuralPlaceDeclaration {
                        id: PlaceId::new(5).unwrap(),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: OperationId::new(4).unwrap(),
                            structural_type: sequence,
                        },
                    });
                let mut long = body.blocks[0].clone();
                long.id = BlockId::new(2).unwrap();
                long.operations[0].id = OperationId::new(3).unwrap();
                let mut short = long.clone();
                short.id = BlockId::new(3).unwrap();
                short.operations = vec![sequence_result(
                    4,
                    5,
                    sequence,
                    OperationKind::EstablishStructuralArray {
                        elements: vec![terminal_psi::StructuralArgument::whole(
                            PlaceId::new(1).unwrap(),
                            StructuralAccess::Owned,
                        )],
                    },
                )];
                short.terminator = Terminator::ReturnStructural {
                    edge: EdgeId::new(4).unwrap(),
                    source: PlaceId::new(5).unwrap(),
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                let edge = |number, target| SuccessorEdge {
                    edge: EdgeId::new(number).unwrap(),
                    target: BlockId::new(target).unwrap(),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                body.blocks[0].operations = (1..=2)
                    .map(|number| {
                        Operation::unsuspended(
                            OperationId::new(number).unwrap(),
                            OperationResult::Unit,
                            OperationKind::EstablishByteSequenceLiteral {
                                destination: PlaceId::new(number).unwrap(),
                                bytes: vec![number as u8],
                                qualifications: Vec::new(),
                            },
                        )
                    })
                    .collect();
                body.blocks[0].terminator = Terminator::Conditional {
                    condition: body.parameters[0].id,
                    when_true: edge(2, 2),
                    when_false: edge(3, 3),
                };
                body.blocks.extend([long, short]);
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &alternatives,
                    [caller],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &alternatives.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let result = replay
                    .summary(caller, 0)
                    .and_then(|summary| summary.result.into_slots())
                    .unwrap();
                let mut lengths = result[0]
                    .origins
                    .iter()
                    .map(|origin| {
                        let crate::descriptor_replay::DescriptorStoredOrigin::Carried {
                            elements,
                            ..
                        } = origin
                        else {
                            panic!("each alternative must retain its actual roster");
                        };
                        assert!(
                            elements
                                .iter()
                                .flatten()
                                .all(|slot| slot.origins.iter().all(|origin| matches!(
                                    origin,
                                    crate::descriptor_replay::DescriptorStoredOrigin::Literal { .. }
                                )))
                        );
                        elements.len()
                    })
                    .collect::<Vec<_>>();
                lengths.sort_unstable();
                assert_eq!(lengths, [1, 2]);

                let mut receiver = machine(2);
                receiver.contract.id = ContractId::new(2).unwrap();
                receiver.parameters = alternatives.machines[0].parameters.clone();
                receiver.parameter_order = vec![terminal_psi::BoundaryParameterKind::Scalar];
                receiver.structural_places = vec![terminal_psi::StructuralPlaceDeclaration {
                    id: PlaceId::new(3).unwrap(),
                    kind: StructuralPlaceKind::OperationResult {
                        producer: OperationId::new(1).unwrap(),
                        structural_type: sequence,
                    },
                }];
                receiver.blocks[0].operations = vec![sequence_result(
                    1,
                    3,
                    sequence,
                    OperationKind::CallStructuralWithScalarArguments {
                        callee: caller,
                        arguments: vec![receiver.parameters[0].id],
                        erased_arguments: Vec::new(),
                        erased_proof_arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                        selected_evidence: Vec::new(),
                    },
                )];
                let receiver_id = receiver.id;
                let receiver_entry = receiver.entry;
                alternatives.machines.push(receiver);
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &alternatives,
                    [receiver_id],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &alternatives.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(receiver_id, 0).unwrap();
                let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
                let current = states.get(&(receiver_entry, 1)).unwrap();
                let query = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![DescriptorContentsStep::any_element(bytes)],
                };
                assert_eq!(
                    replay.read_contents(receiver, current, &query),
                    Some(
                        (1..=2)
                            .map(|source| Origin::Backing(DescriptorCallOrigin::Literal {
                                machine: caller,
                                place: PlaceId::new(source).unwrap(),
                            }))
                            .collect()
                    ),
                    "whole-roster enumeration retains both return alternatives"
                );
                let query = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![DescriptorContentsStep::Address(
                        DescriptorPathSegment::FixedIndex(1),
                    )],
                };
                assert!(
                    replay.read_contents(receiver, current, &query).is_none(),
                    "the shorter return cannot supply ordinal one"
                );

                // Follow the contents of each nested roster, including the
                // longer return's second element. A plain enumeration above
                // can read an installed slot without traversing its account.
                let mut nested = alternatives.clone();
                let outer = StructuralTypeId::new(3).unwrap();
                let mut declaration = nested.structural_types[1].clone();
                declaration.id = outer;
                declaration.shape = StructuralTypeShape::ElementView {
                    element: sequence,
                    access: None,
                };
                nested.structural_types.push(declaration);
                let argument = |place| {
                    terminal_psi::StructuralArgument::whole(
                        PlaceId::new(place).unwrap(),
                        StructuralAccess::Owned,
                    )
                };
                let body = &mut nested.machines[0];
                for (operation, place) in [(3, 6), (4, 7)] {
                    body.structural_places
                        .push(terminal_psi::StructuralPlaceDeclaration {
                            id: PlaceId::new(place).unwrap(),
                            kind: StructuralPlaceKind::OperationResult {
                                producer: OperationId::new(operation).unwrap(),
                                structural_type: sequence,
                            },
                        });
                    body.blocks[0].operations.push(sequence_result(
                        operation,
                        place,
                        sequence,
                        OperationKind::EstablishStructuralArray {
                            elements: vec![argument(place - 5)],
                        },
                    ));
                }
                for (block_index, operation, place, inputs) in [
                    (1, 5, 3, vec![argument(6), argument(7)]),
                    (2, 6, 5, vec![argument(6)]),
                ] {
                    body.structural_places
                        .iter_mut()
                        .find(|row| row.id == PlaceId::new(place).unwrap())
                        .unwrap()
                        .kind = StructuralPlaceKind::OperationResult {
                        producer: OperationId::new(operation).unwrap(),
                        structural_type: outer,
                    };
                    body.blocks[block_index].operations = vec![sequence_result(
                        operation,
                        place,
                        outer,
                        OperationKind::EstablishStructuralArray { elements: inputs },
                    )];
                }
                let TerminalMachineResult::Structural(result) = &mut body.result else {
                    unreachable!()
                };
                result.structural_type = outer;
                let receiver = &mut nested.machines[1];
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = &mut receiver.structural_places[0].kind
                else {
                    unreachable!()
                };
                *structural_type = outer;
                let OperationResult::Structural(result) =
                    &mut receiver.blocks[0].operations[0].result
                else {
                    unreachable!()
                };
                result.structural_type = outer;
                let graph = crate::descriptor_results::terminal_borrowed_storage_graph_from(
                    &nested,
                    [receiver_id],
                    &[],
                )
                .unwrap();
                let mut replay = Replay::with_slot_kind(
                    &nested.structural_types,
                    &graph,
                    crate::descriptor_results::DescriptorSlotKind::BorrowedStorage,
                );
                let states = replay.states(receiver_id, 0).unwrap();
                let receiver = graph.iter().find(|row| row.id == receiver_id).unwrap();
                let current = states.get(&(receiver_entry, 1)).unwrap();
                let mut query = DescriptorContentsQuery {
                    root: PlaceId::new(3).unwrap(),
                    steps: vec![
                        DescriptorContentsStep::any_element(sequence),
                        DescriptorContentsStep::any_element(bytes),
                    ],
                };
                assert_eq!(
                    replay.read_contents(receiver, current, &query),
                    Some(
                        (1..=2)
                            .map(|source| Origin::Backing(DescriptorCallOrigin::Literal {
                                machine: caller,
                                place: PlaceId::new(source).unwrap(),
                            }))
                            .collect()
                    ),
                    "nested enumeration follows each alternative's actual element contents",
                );
                query.steps[0] =
                    DescriptorContentsStep::Address(DescriptorPathSegment::FixedIndex(1));
                assert!(
                    replay.read_contents(receiver, current, &query).is_none(),
                    "nested traversal cannot make ordinal one exist in the shorter return"
                );
            }

            #[test]
            fn cleanup_calls_keep_exact_edges_action_order_and_unique_owners() {
                let body = machine(1);
                let mut module = TerminalModule::declarations();
                module.machines.push(body);
                let mut graphs = terminal_borrowed_storage_graph(&module);
                let graph = &mut graphs[0];
                let position = (module.machines[0].entry, 0);
                for (edge, action_ordinal) in [(1, 0), (1, 2), (2, 0), (2, 2)] {
                    let owner = DescriptorInvocationOwner::Cleanup {
                        edge: EdgeId::new(edge).unwrap(),
                        action_ordinal,
                    };
                    let call = || DescriptorInvocation {
                        scalar_arguments: Vec::new(),
                        owner,
                        target: DescriptorInvocationTarget::Machine(MachineId::new(2).unwrap()),
                        arguments: Vec::new(),
                        argument_contents: Vec::new(),
                        result: None,
                    };
                    assert!(graph.insert_call(position, call()).is_some());
                    assert!(graph.insert_call(position, call()).is_none());
                    assert_eq!(graph.invocation(position, owner).unwrap().owner, owner);
                }
                let owners: Vec<_> = graph
                    .calls_at(position)
                    .iter()
                    .map(|call| call.owner)
                    .collect();
                assert_eq!(owners.len(), 4);
                assert!(owners.windows(2).all(|rows| rows[0] < rows[1]));
                assert!(graph.direct_call(position).is_none());
                assert!(
                    graph
                        .invocation(
                            position,
                            DescriptorInvocationOwner::Cleanup {
                                edge: EdgeId::new(1).unwrap(),
                                action_ordinal: 1,
                            }
                        )
                        .is_none()
                );
                // One normal exit cannot secretly select two cleanup edges.
                assert!(graph.validate_invocation_owners().is_none());
                graph.calls.get_mut(&position).unwrap().as_mut().unwrap().retain(|call| {
                    matches!(call.owner, DescriptorInvocationOwner::Cleanup { edge, .. } if edge == EdgeId::new(1).unwrap())
                });
                assert!(graph.validate_invocation_owners().is_some());
                let second = graph.calls_at(position)[1].owner;
                let calls = graph.calls.get_mut(&position).unwrap().as_mut().unwrap();
                calls[1].owner = calls[0].owner;
                let duplicate = calls[0].owner;
                assert!(graph.invocation(position, duplicate).is_none());
                assert!(graph.validate_invocation_owners().is_none());
                let calls = graph.calls.get_mut(&position).unwrap().as_mut().unwrap();
                calls[1].owner = second;
                calls.swap(0, 1);
                assert!(
                    graph.validate_invocation_owners().is_none(),
                    "an unsorted raw roster cannot enter prepared replay"
                );
            }

            #[test]
            fn decomposition_observes_contained_descriptors_without_consuming_them() {
                let view = StructuralTypeId::new(1).unwrap();
                let carrier = StructuralTypeId::new(2).unwrap();
                let source = PlaceId::new(1).unwrap();
                let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
                let mut body = machine(1);
                body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
                body.structural_parameters
                    .push(StructuralParameterDeclaration {
                        place: source,
                        position: 0,
                        structural_type: carrier,
                        multiplicity: StructuralMultiplicity::Affine,
                        access: StructuralAccess::Owned,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                body.structural_places
                    .push(terminal_psi::StructuralPlaceDeclaration {
                        id: source,
                        kind: StructuralPlaceKind::Parameter { position: 0 },
                    });
                body.blocks[0].operations.push(Operation::unsuspended(
                    OperationId::new(1).unwrap(),
                    OperationResult::Unit,
                    OperationKind::DecomposeStructuralValue {
                        subject: terminal_psi::StructuralArgument::whole(
                            source,
                            StructuralAccess::Owned,
                        ),
                    },
                ));
                let declaration = |id, shape| StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex(1),
                    declaration_identity: TerminalIdentityIndex(1),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                    descriptor_cleanup: false,
                    nominal_cleanup: None,
                    shape,
                    scalar_restriction: None,
                };
                let mut nominal = declaration(
                    carrier,
                    StructuralTypeShape::Record {
                        fields: vec![terminal_psi::StructuralFieldDeclaration {
                            id: field,
                            identity: TerminalIdentityIndex(2),
                            declaration_identity: TerminalIdentityIndex(2),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            storage: None,
                            field_type: terminal_psi::StructuralFieldType::Structural(view),
                            inline_carrier: None,
                        }],
                        storage: terminal_psi::StructuralRecordStorage::Packed,
                    },
                );
                nominal.nominal_cleanup = Some(terminal_psi::NominalCleanupSelection {
                    declaration: terminal_psi::NominalDeclarationIdentity::new(nominal.identity),
                    cleanup_machine: body.id,
                });
                let mut module = TerminalModule::declarations();
                module.structural_types = vec![
                    declaration(
                        view,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::MutableBorrow),
                        }),
                    ),
                    nominal,
                ];
                module.machines = vec![body.clone()];
                let graphs = terminal_borrowed_storage_graph(&module);
                assert_eq!(graphs.len(), 1);
                let position = (body.entry, 0);
                let step = &graphs[0].flow[&position];
                assert_eq!(
                    step.observed_inputs().cloned().collect::<Vec<_>>(),
                    [DescriptorLocation::whole(source)]
                );
                assert!(step.definitions.is_empty());
                assert!(step.derivations.is_empty());
                assert!(!step.may_escape);
                assert!(!graphs[0].payload_invalidations.contains_key(&position));
                let next = &graphs[0].flow[&(body.entry, 1)];
                assert!(next.definitions.is_empty());
            }

            // Moving a borrowed sequence value retires its descriptor, not
            // its elements. A real element write still needs a known backing
            // and cannot publish an empty or descriptor-only write frame.
            #[test]
            fn moving_sequence_values_does_not_write_their_backing() {
                let element = StructuralTypeId::new(1).unwrap();
                let sequence = StructuralTypeId::new(2).unwrap();
                let source = PlaceId::new(1).unwrap();
                let destination = PlaceId::new(2).unwrap();
                let operation = OperationId::new(1).unwrap();
                let declaration = |id, shape| StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id,
                    identity: TerminalIdentityIndex::ABSENT,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    descriptor_cleanup: false,
                    nominal_cleanup: None,
                    shape,
                    scalar_restriction: None,
                };
                for shape in [
                    StructuralTypeShape::ElementView {
                        element,
                        access: None,
                    },
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: None,
                    }),
                ] {
                    let mut module = TerminalModule::declarations();
                    module.structural_types = vec![
                        declaration(
                            element,
                            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                        ),
                        declaration(sequence, shape),
                    ];
                    let mut body = machine(1);
                    body.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
                    body.structural_parameters
                        .push(StructuralParameterDeclaration {
                            place: source,
                            position: 0,
                            structural_type: sequence,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::Owned,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            additional_input_qualifications: Vec::new(),
                        });
                    body.structural_places = vec![
                        terminal_psi::StructuralPlaceDeclaration {
                            id: source,
                            kind: StructuralPlaceKind::Parameter { position: 0 },
                        },
                        terminal_psi::StructuralPlaceDeclaration {
                            id: destination,
                            kind: StructuralPlaceKind::OperationResult {
                                producer: operation,
                                structural_type: sequence,
                            },
                        },
                    ];
                    for kind in [
                        OperationKind::MoveStructuralValue {
                            source: terminal_psi::StructuralArgument::whole(
                                source,
                                StructuralAccess::Owned,
                            ),
                        },
                        OperationKind::MoveStructuralField {
                            source,
                            path: Vec::new(),
                            field: None,
                        },
                    ] {
                        body.blocks[0].operations = vec![Operation::unsuspended(
                            operation,
                            OperationResult::Structural(terminal_psi::StructuralOperationResult {
                                place: destination,
                                structural_type: sequence,
                                multiplicity: StructuralMultiplicity::Unrestricted,
                                qualifications: Vec::new(),
                                projected_qualifications: Vec::new(),
                                additional_input_qualifications: Vec::new(),
                                qualification_establishments: Vec::new(),
                                claims: Vec::new(),
                            }),
                            kind,
                        )];
                        module.machines = vec![body.clone()];
                        let graph = terminal_borrowed_storage_graph(&module);
                        assert_eq!(graph.len(), 1);
                        let position = (body.entry, 0);
                        assert!(
                            graph[0].flow[&position]
                                .inputs
                                .iter()
                                .all(|(_, access)| access.is_none()),
                            "moving a whole descriptor does not read its backing"
                        );
                        if matches!(
                            module.structural_types[1].shape,
                            StructuralTypeShape::ByteSequence(_)
                        ) {
                            let length = OperationKind::ByteSequenceLength {
                                source: terminal_psi::StructuralArgument::whole(
                                    source,
                                    StructuralAccess::SharedBorrow,
                                ),
                            };
                            let uses = length.storage_uses();
                            assert!(
                                matches!(
                                    crate::descriptor_results::descriptor_storage_use_source(
                                        &module.structural_types, graph[0].place_types.get(&source).copied(), &length, uses[0].0, |_| None,
                                    ),
                                    crate::descriptor_lifetimes::DescriptorFlowSource::Unavailable(root) if root == source
                                ),
                                "a byte descriptor address is not an exact byte-payload footprint"
                            );
                        }
                        assert_eq!(
                            graph[0].payload_invalidations[&position],
                            [Some(DescriptorContentsQuery::from_location(
                                &DescriptorLocation::whole(source)
                            ))]
                        );
                        let mut analysis = crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                            &module.structural_types, &graph,
                        );
                        assert_eq!(
                            analysis.storage_writes(body.id, position).unwrap(),
                            Some(vec![
                                DescriptorLocation::whole(source),
                                DescriptorLocation::whole(destination),
                            ])
                        );
                        let frames =
                            crate::borrowed_storage::inferred_write_frames(&module).unwrap();
                        assert_eq!(frames.get(body.id), Some(&Some(Vec::new())));
                    }

                    // Move a real field rather than the whole source root.
                    // The sibling's entry contents must survive the same cut,
                    // while the moved field cannot retain its former value.
                    let record = StructuralTypeId::new(3).unwrap();
                    let moved = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
                    let sibling = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
                    let field =
                        |id, identity, field_type| terminal_psi::StructuralFieldDeclaration {
                            declaration_identity: Default::default(),
                            id,
                            identity: TerminalIdentityIndex(identity),
                            inline_carrier: None,
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type,
                            storage: None,
                        };
                    module.structural_types.push(declaration(
                        record,
                        StructuralTypeShape::Record {
                            fields: vec![
                                field(
                                    moved,
                                    1,
                                    terminal_psi::StructuralFieldType::Structural(sequence),
                                ),
                                field(
                                    sibling,
                                    2,
                                    terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                                ),
                            ],
                            storage: terminal_psi::StructuralRecordStorage::Packed,
                        },
                    ));
                    let mut field_body = body.clone();
                    field_body.structural_parameters[0].structural_type = record;
                    field_body.blocks[0].operations[0].kind = OperationKind::MoveStructuralField {
                        source,
                        path: Vec::new(),
                        field: Some(moved),
                    };
                    module.machines = vec![field_body.clone()];
                    let graph = terminal_borrowed_storage_graph(&module);
                    assert_eq!(graph.len(), 1);
                    let accesses: Vec<_> = graph[0].flow[&(field_body.entry, 0)]
                        .inputs
                        .iter()
                        .filter(|(_, access)| access.is_some())
                        .collect();
                    assert_eq!(accesses.len(), 1);
                    let (location, Some((access_plans::AccessOperation::Take, access))) =
                        accesses[0]
                    else {
                        panic!("the projected move retains its take operation");
                    };
                    assert!(
                        location.is_none(),
                        "the event is not another old-content observation"
                    );
                    assert_eq!(
                        graph[0].flow[&(field_body.entry, 0)]
                            .observed_inputs()
                            .collect::<Vec<_>>(),
                        vec![&DescriptorLocation {
                            root: source,
                            path: vec![crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                                moved
                            )],
                        }],
                        "the moved field cannot become its disjoint sibling"
                    );
                    if matches!(
                        module.structural_types[1].shape,
                        StructuralTypeShape::ByteSequence(_)
                    ) {
                        assert!(
                            matches!(access, crate::descriptor_lifetimes::DescriptorFlowSource::Unavailable(root) if *root == source),
                            "the byte-payload footprint remains unavailable"
                        );
                    } else {
                        assert!(
                            matches!(access, crate::descriptor_lifetimes::DescriptorFlowSource::Contents(query)
                            if query.root == source && query.steps == [DescriptorContentsStep::Address(
                                crate::descriptor_lifetimes::DescriptorPathSegment::Field(moved))]),
                            "the element descriptor keeps its actual typed field query"
                        );
                    }
                    let frames = crate::borrowed_storage::inferred_write_frames(&module).unwrap();
                    assert_eq!(frames.get(field_body.id), Some(&Some(Vec::new())));
                    let mut analysis =
                        crate::descriptor_results::DescriptorAnalysis::with_borrowed_storage_slots(
                            &module.structural_types,
                            &graph,
                        );
                    assert_eq!(
                        analysis
                            .storage_writes(field_body.id, (field_body.entry, 0))
                            .unwrap(),
                        Some(vec![
                            DescriptorLocation {
                                root: source,
                                path: vec![
                                    crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                                        moved
                                    )
                                ]
                            },
                            DescriptorLocation::whole(destination),
                        ])
                    );
                    for (field, survives) in [(sibling, true), (moved, false)] {
                        let payload = crate::descriptor_results::DescriptorPayloadTarget::current(
                            DescriptorLocation {
                                root: source,
                                path: vec![
                                    crate::descriptor_lifetimes::DescriptorPathSegment::Field(
                                        field,
                                    ),
                                ],
                            },
                        );
                        assert_eq!(
                            crate::descriptor_results::require_unchanged_entry_storage(
                                &module.structural_types,
                                &graph,
                                &mut analysis,
                                field_body.id,
                                (field_body.entry, 1),
                                &payload,
                            )
                            .is_ok(),
                            survives,
                        );
                    }

                    // An unknown caller-supplied backing is still unknown.
                    // Use the real element-write operation, not a forged
                    // summary, to retain the conservative effect boundary.
                    body.structural_places.retain(|place| place.id == source);
                    module.structural_types[1].shape = StructuralTypeShape::ElementView {
                        element,
                        access: Some(StructuralAccess::MutableBorrow),
                    };
                    let index = ValueId::new(1).unwrap();
                    let value = ValueId::new(2).unwrap();
                    let length = ValueId::new(3).unwrap();
                    let integer = ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    );
                    body.parameters = vec![
                        ValueDeclaration {
                            id: index,
                            scalar_type: integer,
                            qualifications: ScalarQualificationSetId::ZERO,
                        },
                        ValueDeclaration {
                            id: value,
                            scalar_type: ScalarType::Boolean,
                            qualifications: ScalarQualificationSetId::ZERO,
                        },
                        ValueDeclaration {
                            id: length,
                            scalar_type: integer,
                            qualifications: ScalarQualificationSetId::ZERO,
                        },
                    ];
                    body.parameter_order = vec![
                        terminal_psi::BoundaryParameterKind::Structural,
                        terminal_psi::BoundaryParameterKind::Scalar,
                        terminal_psi::BoundaryParameterKind::Scalar,
                        terminal_psi::BoundaryParameterKind::Scalar,
                    ];
                    body.blocks[0].operations = vec![Operation::unsuspended(
                        operation,
                        OperationResult::Unit,
                        OperationKind::ElementViewWrite {
                            destination: terminal_psi::StructuralArgument::whole(
                                source,
                                StructuralAccess::MutableBorrow,
                            ),
                            path: Vec::new(),
                            index,
                            value,
                            length,
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        },
                    )];
                    module.machines = vec![body.clone()];
                    let graph = terminal_borrowed_storage_graph(&module);
                    assert_eq!(graph.len(), 1);
                    assert!(matches!(
                        graph[0].payload_invalidations[&(body.entry, 0)].as_slice(),
                        [Some(DescriptorContentsQuery { steps, .. })]
                            if matches!(steps.as_slice(), [DescriptorContentsStep::Element { .. }])
                    ));
                    let accesses: Vec<_> = graph[0].flow[&(body.entry, 0)]
                        .inputs
                        .iter()
                        .filter_map(|(_, access)| access.as_ref())
                        .collect();
                    assert!(
                        matches!(accesses.as_slice(),
                        [(access_plans::AccessOperation::Write, crate::descriptor_lifetimes::DescriptorFlowSource::Contents(query))]
                            if query.root == source && matches!(query.steps.as_slice(),
                                [DescriptorContentsStep::Element { selector: Some(selector), .. }]
                                    if **selector == semantic_vocabulary::ScalarTerm::value(index, integer))),
                        "the backing write retains its evaluated index at the input cut"
                    );
                    assert_eq!(
                        crate::borrowed_storage::inferred_write_frames(&module)
                            .unwrap()
                            .get(body.id),
                        Some(&None)
                    );
                    // A variable array selection must not hide the later
                    // view-backing write when it is widened to the array.
                    let array = StructuralTypeId::new(4).unwrap();
                    module.structural_types.push(declaration(
                        array,
                        StructuralTypeShape::FixedArray {
                            element: sequence,
                            length: terminal_psi::ArrayExtent::Literal(1),
                        },
                    ));
                    body.structural_parameters[0].structural_type = array;
                    let OperationKind::ElementViewWrite { destination, .. } =
                        &mut body.blocks[0].operations[0].kind
                    else {
                        unreachable!("the same backing-write control")
                    };
                    destination.path = vec![StructuralPathSegment::RuntimeIndex {
                        index,
                        obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
                    }];
                    module.machines = vec![body.clone()];
                    let graph = terminal_borrowed_storage_graph(&module);
                    assert_eq!(graph.len(), 1);
                    assert!(matches!(
                        graph[0].payload_invalidations[&(body.entry, 0)].as_slice(),
                        [Some(DescriptorContentsQuery { steps, .. })]
                            if matches!(steps.as_slice(), [DescriptorContentsStep::Element { .. }, DescriptorContentsStep::Element { .. }])
                    ));
                    assert_eq!(
                        crate::borrowed_storage::inferred_write_frames(&module)
                            .unwrap()
                            .get(body.id),
                        Some(&None)
                    );
                }
            }

            #[test]
            fn stored_and_forwarded_scalars_keep_evaluated_binders_and_both_edge_cuts() {
                let mut module = TerminalModule::declarations();
                let carrier = StructuralTypeId::new(1).unwrap();
                module.structural_types.push(StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    id: carrier,
                    identity: TerminalIdentityIndex(1),
                    owns_linear_claim: false,
                    no_code_affine_cleanup: true,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                    scalar_restriction: None,
                });
                let scalar = |id| ValueDeclaration {
                    id: ValueId::new(id).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    qualifications: ScalarQualificationSetId::ZERO,
                };
                let mut machine = machine(1);
                let original = scalar(1);
                let saved = scalar(2);
                let formal = scalar(3);
                let home = PlaceId::new(1).unwrap();
                machine.parameters.push(original);
                machine.parameter_order = vec![
                    terminal_psi::BoundaryParameterKind::Scalar,
                    terminal_psi::BoundaryParameterKind::Structural,
                ];
                machine
                    .structural_parameters
                    .push(StructuralParameterDeclaration {
                        place: home,
                        position: 0,
                        structural_type: carrier,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::WriteOnlyBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                machine
                    .structural_places
                    .push(terminal_psi::StructuralPlaceDeclaration {
                        id: home,
                        kind: StructuralPlaceKind::Parameter { position: 0 },
                    });
                machine.blocks[0].operations = vec![
                    Operation::unsuspended(
                        OperationId::new(1).unwrap(),
                        OperationResult::Scalar(saved),
                        OperationKind::BooleanNot {
                            operand: original.id,
                        },
                    ),
                    Operation::unsuspended(
                        OperationId::new(2).unwrap(),
                        OperationResult::Unit,
                        OperationKind::WriteOnlyPrimitiveStore {
                            destination: home,
                            path: Vec::new(),
                            value: saved.id,
                            range_obligation: None,
                        },
                    ),
                ];
                let target = BlockId::new(2).unwrap();
                let edge = |id, actual| SuccessorEdge {
                    edge: EdgeId::new(id).unwrap(),
                    target,
                    arguments: vec![actual],
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: Vec::new(),
                    cleanup_actions: Vec::new(),
                };
                machine.blocks[0].terminator = Terminator::Conditional {
                    condition: original.id,
                    when_true: edge(1, saved.id),
                    when_false: edge(2, original.id),
                };
                machine.blocks.push(Block::plain(
                    target,
                    vec![formal],
                    Terminator::ReturnUnit {
                        edge: EdgeId::new(3).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ));
                let captures = TerminalScalarCaptures::new(&module, &machine).unwrap();
                let entry = machine.entry;
                assert_eq!(
                    captures.writes[&(entry, 1)].as_ref().unwrap(),
                    &vec![(
                        DescriptorLocation::whole(home),
                        ScalarTerm::value(saved.id, ScalarType::Boolean)
                    )]
                );
                let edges = &captures.arrivals[&(entry, 2)];
                assert_eq!(edges.len(), 2);
                assert_eq!(edges[0][0].0, formal.id);
                assert_eq!(
                    edges[0][0].1.value_term(),
                    Some(ScalarTerm::value(saved.id, ScalarType::Boolean))
                );
                assert_eq!(
                    edges[1][0].1.value_term(),
                    Some(ScalarTerm::value(original.id, ScalarType::Boolean))
                );
                assert_ne!(
                    edges[0][0].1.value_term(),
                    Some(ScalarTerm::BooleanNot {
                        operand: Box::new(ScalarTerm::value(original.id, ScalarType::Boolean))
                    })
                );
                machine.blocks[0].operations[1].kind = OperationKind::WriteOnlyPrimitiveStore {
                    destination: PlaceId::new(99).unwrap(),
                    path: Vec::new(),
                    value: saved.id,
                    range_obligation: None,
                };
                let incomplete = TerminalScalarCaptures::new(&module, &machine).unwrap();
                assert!(incomplete.writes[&(entry, 1)].is_err());
                machine.structural_places[0].kind = StructuralPlaceKind::ProviderAttachment {
                    attachment: carrier,
                    field: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
                };
                assert!(matches!(
                    TerminalScalarCaptures::new(&module, &machine),
                    Err("scalar capture parameter changes its declared storage kind")
                ));
            }
        }
    }
    pub(crate) mod value_identity {

        use super::*;
        use crate::descriptors::descriptor_lifetimes::{
            DescriptorFlowSource, descriptor_location, descriptor_location_type, value_locations,
        };
        use semantic_vocabulary::{IdTable, InvocationSite, OperationId};

        pub(crate) mod flow {
            use super::*;

            pub(crate) fn adapt(
                module: &TerminalModule,
                machine: &DescriptorMachine,
                operations: &BTreeMap<DescriptorPosition, &Operation>,
                analysis: &mut DescriptorAnalysis<'_>,
                relevant: &payload_reachability::Reachability,
                observation: DescriptorPosition,
            ) -> Result<BTreeMap<DescriptorPosition, DescriptorFlowStep>, &'static str>
            {
                let identity = machine.id;
                let mut flow = BTreeMap::new();
                for (&position, original) in &machine.flow {
                    if !relevant.contains(position) {
                        continue;
                    }
                    if position == observation {
                        flow.insert(position, DescriptorFlowStep::default());
                        continue;
                    }
                    if machine.unresolved_contents_writes.contains(&position) {
                        return Err("value identity crosses an unresolved write");
                    }
                    let mut step = DescriptorFlowStep::default();
                    for definition in &original.definitions {
                        step.definitions.push(
                            analysis
                                .location(identity, position, definition)
                                .ok_or("value identity write has unresolved backing")?,
                        );
                    }
                    for query in machine
                        .payload_invalidations
                        .get(&position)
                        .into_iter()
                        .flatten()
                    {
                        let query = query
                            .as_ref()
                            .ok_or("value identity crosses an unknown payload write")?;
                        let targets = analysis
                            .payload_targets(identity, position, query)
                            .ok_or("value identity payload write has unresolved backing")?;
                        if targets.is_empty() {
                            return Err("value identity payload write has no complete targets");
                        }
                        for target in targets {
                            let DescriptorPayloadTarget::Storage {
                                location: target,
                                completed: None,
                            } = target
                            else {
                                return Err(
                                    "value identity payload write may reach unknown incoming storage",
                                );
                            };
                            step.definitions.push(target);
                        }
                    }
                    if operations
                        .get(&position)
                        .is_some_and(|operation| identity_operation(&operation.kind))
                    {
                        for (source, destination) in &original.derivations {
                            let source = analysis
                                .location(identity, position, source)
                                .ok_or("value identity source has unresolved backing")?;
                            let destination = analysis
                                .location(identity, position, destination)
                                .ok_or("value identity destination has unresolved backing")?;
                            if location_type(module, machine, &source)?
                                != location_type(module, machine, &destination)?
                            {
                                return Err("value identity transfer changes the retained type");
                            }
                            step.derivations.push((source.clone(), destination.clone()));
                            step.authority_derivations.push((source, destination));
                        }
                    }
                    step.inputs = original.inputs.clone();
                    step.may_escape = original.may_escape;
                    for (ordinal, edge) in original.successors.iter().enumerate() {
                        if !relevant.includes_edge(position, ordinal) {
                            continue;
                        }
                        let mut edge = edge.clone();
                        for binding in &mut edge.bindings {
                            let source = match &binding.source {
                                DescriptorFlowSource::Location(source) => {
                                    analysis.location(identity, position, source)
                                }
                                DescriptorFlowSource::Contents(query) => {
                                    let targets = analysis
                                        .arrival_payload_targets(identity, position, query)
                                        .ok_or("value identity arrival has unavailable contents")?;
                                    match targets.as_slice() {
                                        [
                                            DescriptorPayloadTarget::Storage {
                                                location,
                                                completed: None,
                                            },
                                        ] => Some(location.clone()),
                                        _ => None,
                                    }
                                }
                                DescriptorFlowSource::Unavailable(_) => None,
                            }
                            .ok_or("value identity arrival does not name one exact value")?;
                            if location_type(module, machine, &source)?
                                != location_type(module, machine, &binding.parameter)?
                            {
                                return Err("value identity arrival changes the retained type");
                            }
                            binding.source = DescriptorFlowSource::Location(source);
                        }
                        step.successors.push(edge);
                    }
                    flow.insert(position, step);
                }
                require_acyclic(&flow, machine.entry)?;
                Ok(flow)
            }
            pub(crate) fn location_type(
                module: &TerminalModule,
                machine: &DescriptorMachine,
                location: &DescriptorLocation,
            ) -> Result<StructuralTypeId, &'static str> {
                descriptor_location_type(
                    &module.structural_types,
                    *machine
                        .place_types
                        .get(&location.root)
                        .ok_or("value identity location has no retained type")?,
                    &location.path,
                )
                .ok_or("value identity location has an unresolved retained type")
            }

            pub(crate) fn identity_operation(operation: &OperationKind) -> bool {
                matches!(
                    operation,
                    OperationKind::MoveStructuralValue { .. }
                        | OperationKind::MoveStructuralField { .. }
                        | OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::StructuralCaseLeafCopy { .. }
                        | OperationKind::StoreStructuralField { .. }
                )
            }

            pub(crate) fn require_acyclic(
                flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
                entry: DescriptorPosition,
            ) -> Result<(), &'static str> {
                let mut incoming = flow
                    .keys()
                    .map(|position| (*position, 0usize))
                    .collect::<BTreeMap<_, _>>();
                for step in flow.values() {
                    for edge in &step.successors {
                        *incoming
                            .get_mut(&edge.target)
                            .ok_or("value identity edge has no target")? += 1;
                    }
                }
                if incoming.get(&entry) != Some(&0) {
                    return Err("value identity entry is cyclic or absent");
                }
                let mut pending = vec![entry];
                let mut count = 0;
                while let Some(position) = pending.pop() {
                    count += 1;
                    for edge in &flow[&position].successors {
                        let remaining = incoming
                            .get_mut(&edge.target)
                            .ok_or("value identity edge has no target")?;
                        *remaining -= 1;
                        if *remaining == 0 {
                            pending.push(edge.target);
                        }
                    }
                }
                if count != flow.len() {
                    return Err("value identity crosses cyclic or unavailable incoming flow");
                }
                Ok(())
            }
        }
        use flow::{location_type, require_acyclic};

        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum ValueIdentitySeed {
            Parameter(usize),
            Result(OperationId),
        }

        type QueryResult<T> = Result<T, &'static str>;
        type IdentityFlow = BTreeMap<DescriptorPosition, DescriptorFlowStep>;

        pub struct ValueIdentityAnalysis<'a> {
            module: &'a TerminalModule,
            body: &'a TerminalMachine,
            machine: &'a DescriptorMachine,
            operations: BTreeMap<DescriptorPosition, &'a Operation>,
            positions: IdTable<OperationId, Option<DescriptorPosition>>,
            terminal_positions: BTreeMap<InvocationSite, DescriptorPosition>,
            incoming: QueryResult<payload_reachability::IncomingFlow>,
            analysis: DescriptorAnalysis<'a>,
            cuts: BTreeMap<DescriptorPosition, QueryResult<payload_reachability::Reachability>>,
            flows: BTreeMap<DescriptorPosition, QueryResult<IdentityFlow>>,
            actuals:
                BTreeMap<(DescriptorPosition, StructuralArgument), QueryResult<DescriptorLocation>>,
            values: BTreeMap<(ValueIdentitySeed, DescriptorPosition), BTreeSet<DescriptorLocation>>,
            results:
                BTreeMap<(ValueIdentitySeed, InvocationSite, StructuralArgument), QueryResult<()>>,
        }

        impl<'a> ValueIdentityAnalysis<'a> {
            pub fn new(
                module: &'a TerminalModule,
                reference_graph: &'a [DescriptorMachine],
                identity: MachineId,
            ) -> QueryResult<Self> {
                let body = exactly_one(module.machines.iter(), |body| body.id == identity)
                    .ok_or("value identity machine is absent or ambiguous")?;
                let machine = exactly_one(reference_graph.iter(), |body| body.id == identity)
                    .ok_or("value identity reference graph is absent or ambiguous")?;
                let mut operations = BTreeMap::new();
                let mut position_rows = Vec::new();
                let mut terminal_positions = BTreeMap::new();
                for block in &body.blocks {
                    for (index, operation) in block.operations.iter().enumerate() {
                        let position = (
                            block.id,
                            u32::try_from(index).map_err(|_| "operation index overflow")?,
                        );
                        if operations.insert(position, operation).is_some() {
                            return Err("value identity operation is ambiguous");
                        }
                        position_rows.push((operation.id, position));
                    }
                    let site = match block.terminator {
                        Terminator::InvokeNoNormal { edge, .. } => {
                            Some(InvocationSite::InvokeNoNormal(edge))
                        }
                        Terminator::ExternalCompletion { edge, .. } => {
                            Some(InvocationSite::ExternalCompletion(edge))
                        }
                        _ => None,
                    };
                    if let Some(site) = site {
                        let position = (
                            block.id,
                            u32::try_from(block.operations.len())
                                .map_err(|_| "operation index overflow")?,
                        );
                        if terminal_positions.insert(site, position).is_some() {
                            return Err("value identity terminal invocation is ambiguous");
                        }
                    }
                }
                position_rows.sort_unstable_by_key(|(identity, _)| *identity);
                if position_rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                    return Err("value identity operation is ambiguous");
                }
                let positions = if let Some((first, last)) =
                    position_rows.first().zip(position_rows.last())
                {
                    let first = first.0.get();
                    let length = last
                        .0
                        .get()
                        .checked_sub(first)
                        .and_then(|distance| distance.checked_add(1))
                        .and_then(|length| usize::try_from(length).ok())
                        .ok_or("value identity operation span cannot be indexed")?;
                    let mut slots = Vec::new();
                    slots
                        .try_reserve_exact(length)
                        .map_err(|_| "value identity operation span cannot be allocated")?;
                    slots.resize(length, None);
                    for (operation, position) in position_rows {
                        let offset = usize::try_from(operation.get() - first)
                            .map_err(|_| "value identity operation cannot be indexed")?;
                        slots[offset] = Some(position);
                    }
                    IdTable::from_rows(slots.into_iter().enumerate().map(|(offset, position)| {
                        let identity = OperationId::new(first + offset as u64)
                            .expect("validated nonzero operation interval");
                        (identity, position)
                    }))
                    .map_err(|_| "value identity operation interval is inconsistent")?
                } else {
                    IdTable::default()
                };
                Ok(Self {
                    module,
                    body,
                    machine,
                    operations,
                    positions,
                    terminal_positions,
                    incoming: payload_reachability::incoming_flow(&machine.flow)
                        .ok_or("value identity has incomplete incoming control flow"),
                    analysis: DescriptorAnalysis::with_reference_slots(
                        &module.structural_types,
                        reference_graph,
                    ),
                    cuts: BTreeMap::new(),
                    flows: BTreeMap::new(),
                    actuals: BTreeMap::new(),
                    values: BTreeMap::new(),
                    results: BTreeMap::new(),
                })
            }

            pub fn require(
                &mut self,
                seed: ValueIdentitySeed,
                observation: OperationId,
                actual: &StructuralArgument,
            ) -> QueryResult<()> {
                self.require_at_invocation(seed, InvocationSite::Operation(observation), actual)
            }

            pub fn require_at_invocation(
                &mut self,
                seed: ValueIdentitySeed,
                observation: InvocationSite,
                actual: &StructuralArgument,
            ) -> QueryResult<()> {
                let key = (seed, observation, actual.clone());
                if let Some(result) = self.results.get(&key) {
                    return *result;
                }
                let result = self.query(seed, observation, actual);
                self.results.insert(key, result);
                result
            }

            fn operation_position(&self, operation: OperationId) -> Option<DescriptorPosition> {
                self.positions.get(operation).copied().flatten()
            }

            fn observation_position(&self, site: InvocationSite) -> Option<DescriptorPosition> {
                match site {
                    InvocationSite::Operation(operation) => self.operation_position(operation),
                    InvocationSite::ExternalCompletion(_) | InvocationSite::InvokeNoNormal(_) => {
                        self.terminal_positions.get(&site).copied()
                    }
                    InvocationSite::Cleanup { .. } => None,
                }
            }

            fn query(
                &mut self,
                seed: ValueIdentitySeed,
                invocation: InvocationSite,
                actual: &StructuralArgument,
            ) -> QueryResult<()> {
                // Several cleanup actions share one terminator coordinate.
                // This identity walk has no action-prefix state, so the
                // pre-terminator value cannot prove an unchanged-value claim.
                if matches!(invocation, InvocationSite::Cleanup { .. }) {
                    return Err("unchanged-value proof at an ordered cleanup action is not served");
                }
                let observation = self
                    .observation_position(invocation)
                    .ok_or("value identity observation is absent")?;
                let (establishment, value, at_entry) = match seed {
                    ValueIdentitySeed::Parameter(index) => {
                        let formal = self
                            .body
                            .structural_parameters
                            .get(index)
                            .ok_or("value identity structural formal is absent")?;
                        (
                            self.machine.entry,
                            DescriptorLocation::whole(formal.place),
                            true,
                        )
                    }
                    ValueIdentitySeed::Result(operation) => {
                        let position = self
                            .operation_position(operation)
                            .ok_or("value identity result producer is absent")?;
                        let result = self.operations[&position]
                            .result
                            .as_structural()
                            .ok_or("value identity seed has no structural result")?;
                        if position == observation {
                            return Err(
                                "value identity result has not completed before its observation",
                            );
                        }
                        (position, DescriptorLocation::whole(result.place), false)
                    }
                };
                self.prepare_cut(observation);
                let relevant = self.cuts[&observation].as_ref().map_err(|reason| *reason)?;
                if !relevant.contains(establishment) || !relevant.contains(self.machine.entry) {
                    return Err("value identity seed does not precede this observation");
                }
                let actual_key = (observation, actual.clone());
                if !self.actuals.contains_key(&actual_key) {
                    let observed = self.normalize_actual(observation, actual);
                    self.actuals.insert(actual_key.clone(), observed);
                }
                let observed = self.actuals[&actual_key]
                    .as_ref()
                    .map_err(|reason| *reason)?;
                if location_type(self.module, self.machine, &value)?
                    != location_type(self.module, self.machine, observed)?
                {
                    return Err("value identity seed and actual have different retained types");
                }
                if !self.flows.contains_key(&observation) {
                    let adapted = flow::adapt(
                        self.module,
                        self.machine,
                        &self.operations,
                        &mut self.analysis,
                        self.cuts[&observation].as_ref().map_err(|reason| *reason)?,
                        observation,
                    );
                    self.flows.insert(observation, adapted);
                }
                let flow = self.flows[&observation]
                    .as_ref()
                    .map_err(|reason| *reason)?;
                let known = self.values.entry((seed, observation)).or_insert_with(|| {
                    value_locations(flow, establishment, &value, at_entry)
                        .remove(&observation)
                        .unwrap_or_default()
                });
                if !known.contains(observed) {
                    return Err(
                        "actual value is not the specified unchanged value; transformations and ordinary call returns supply no identity",
                    );
                }
                Ok(())
            }

            fn normalize_actual(
                &mut self,
                observation: DescriptorPosition,
                actual: &StructuralArgument,
            ) -> QueryResult<DescriptorLocation> {
                let observed = descriptor_location(
                    &self.module.structural_types,
                    actual.place,
                    *self
                        .machine
                        .place_types
                        .get(&actual.place)
                        .ok_or("value identity actual type is absent")?,
                    &actual.path,
                    None,
                )
                .ok_or("value identity actual has an unresolved projection")?;
                self.analysis
                    .location(self.machine.id, observation, &observed)
                    .ok_or("value identity actual has ambiguous or unavailable backing")
            }

            fn prepare_cut(&mut self, observation: DescriptorPosition) {
                if self.cuts.contains_key(&observation) {
                    return;
                }
                let cut = (|| {
                    let incoming = self.incoming.as_ref().map_err(|reason| *reason)?;
                    let relevant = payload_reachability::predecessors_with_incoming(
                        self.machine,
                        incoming,
                        observation,
                    )
                    .ok_or("value identity has incomplete incoming control flow")?;
                    let incoming = self
                        .machine
                        .flow
                        .iter()
                        .filter(|(position, _)| relevant.contains(**position))
                        .map(|(position, step)| {
                            (
                                *position,
                                DescriptorFlowStep {
                                    successors: step
                                        .successors
                                        .iter()
                                        .enumerate()
                                        .filter(|(ordinal, _)| {
                                            relevant.includes_edge(*position, *ordinal)
                                        })
                                        .map(|(_, edge)| edge.clone())
                                        .collect(),
                                    ..Default::default()
                                },
                            )
                        })
                        .collect();
                    require_acyclic(&incoming, self.machine.entry)?;
                    Ok(relevant)
                })();
                self.cuts.insert(observation, cut);
            }
        }

        pub fn require_unchanged_value(
            module: &TerminalModule,
            reference_graph: &[DescriptorMachine],
            identity: MachineId,
            seed: ValueIdentitySeed,
            observation: OperationId,
            actual: &StructuralArgument,
        ) -> Result<(), &'static str> {
            ValueIdentityAnalysis::new(module, reference_graph, identity)?.require(
                seed,
                observation,
                actual,
            )
        }
    }
    pub use value_identity::{ValueIdentityAnalysis, ValueIdentitySeed, require_unchanged_value};
    pub(crate) mod scalar_contracts {
        use crate::runtime_structural_path_tip;
        use crate::substitution;
        use crate::substitution::substitute_proposition_values;
        use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
        use std::collections::BTreeMap;

        pub(crate) fn scalar_conjuncts(propositions: &[Proposition]) -> Vec<Proposition> {
            let mut pending = propositions.iter().rev().collect::<Vec<_>>();
            let mut result = Vec::new();
            while let Some(proposition) = pending.pop() {
                if let Proposition::Conjunction(children) = proposition {
                    pending.extend(children.iter().rev());
                } else if proposition.visit_value_ids(|_| {}) {
                    result.push(proposition.clone());
                }
            }
            result
        }

        pub fn instantiate_payload_scalar_guarantees(
            contract: &terminal_psi::MachineContract,
            bindings: &[(ValueId, ScalarTerm)],
        ) -> Vec<Proposition> {
            let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
            if substitutions.len() != bindings.len() {
                return Vec::new();
            }
            let guarantees = contract
                .ensures
                .iter()
                .map(|clause| clause.proposition.clone())
                .collect::<Vec<_>>();
            scalar_conjuncts(&guarantees)
                .into_iter()
                .filter_map(|proposition| {
                    let mut bound = true;
                    proposition
                        .visit_value_ids(|value| bound &= substitutions.contains_key(&value));
                    bound.then(|| substitute_proposition_values(&proposition, &substitutions))
                })
                .collect()
        }

        pub(crate) struct TerminalPostconditions {
            pub scalar: Vec<Proposition>,
            pub stored: Vec<(super::DescriptorLocation, ScalarTerm)>,
        }

        pub(crate) fn terminal_postconditions(
            module: &terminal_psi::TerminalModule,
            machine: &terminal_psi::TerminalMachine,
            operation: &terminal_psi::Operation,
            scalar_expressions: &super::payload_predicates::TerminalScalarExpressions<'_>,
        ) -> Option<TerminalPostconditions> {
            use terminal_psi::{
                OperationKind, OperationResult, StructuralAccess, TerminalMachineResult,
            };
            let (callee, arguments, structural_arguments): (_, &[_], &[_]) = match &operation.kind {
                OperationKind::Call {
                    callee, arguments, ..
                } => (*callee, arguments, &[]),
                OperationKind::CallUnit {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    callee,
                    arguments,
                    structural_arguments,
                    ..
                } => (*callee, arguments, structural_arguments),
                OperationKind::CallStructural {
                    callee,
                    structural_arguments,
                    ..
                } => (*callee, &[], structural_arguments),
                _ => return None,
            };
            let mut matching = module
                .machines
                .iter()
                .filter(|machine| machine.id == callee);
            let callee = matching.next()?;
            if matching.next().is_some()
                || arguments.len() != callee.parameters.len()
                || structural_arguments.len() != callee.structural_parameters.len()
                || !callee.contract.erased_scalar_formals.is_empty()
                || !callee.contract.erased_proof_formals.is_empty()
            {
                return None;
            }
            let types = machine
                .parameters
                .iter()
                .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
                .chain(
                    machine
                        .operations()
                        .filter_map(|operation| operation.result.scalar_ref()),
                )
                .collect::<Vec<_>>();
            let mut bindings = Vec::new();
            for (formal, actual) in callee.parameters.iter().zip(arguments) {
                if !types
                    .iter()
                    .any(|value| value.id == *actual && value.scalar_type == formal.scalar_type)
                    || types
                        .iter()
                        .any(|value| value.id == *actual && value.scalar_type != formal.scalar_type)
                {
                    return None;
                }
                bindings.push((formal.id, scalar_expressions.expression(*actual)?));
            }
            let mut storage = Vec::new();
            for (formal, actual) in callee
                .structural_parameters
                .iter()
                .zip(structural_arguments)
            {
                let root_type = machine.structural_place_type(actual.place)?;
                let selected = runtime_structural_path_tip(
                    module.structural_types.iter(),
                    root_type,
                    &actual.path,
                );
                if actual.access != formal.access
                    || (!selected.is_some_and(|selected| {
                        selected.id == formal.structural_type
                            || crate::calls::const_array_argument_matches(
                                module,
                                machine,
                                callee,
                                arguments,
                                selected.id,
                                formal.structural_type,
                            )
                    }) && crate::borrowed_primitive_projection_declaration(
                        module.structural_types.iter(),
                        root_type,
                        &actual.path,
                        Some(formal.structural_type),
                    ) != Some(formal.structural_type))
                {
                    return None;
                }
                if actual.access == StructuralAccess::Owned {
                    continue;
                }
                if let Some(location) =
                    crate::descriptors::descriptor_lifetimes::descriptor_location(
                        &module.structural_types,
                        actual.place,
                        root_type,
                        &actual.path,
                        None,
                    )
                {
                    storage.push((formal.place, formal.structural_type, location));
                }
            }
            match (&callee.result, &operation.result) {
                (TerminalMachineResult::Unit, OperationResult::Unit) => {}
                (TerminalMachineResult::Scalar(signature), OperationResult::Scalar(result))
                    if signature.scalar_type == result.scalar_type
                        && !arguments.contains(&result.id) =>
                {
                    bindings.push((
                        signature.id,
                        ScalarTerm::value(result.id, result.scalar_type),
                    ));
                }
                (
                    TerminalMachineResult::Structural(signature),
                    OperationResult::Structural(result),
                ) if signature.structural_type == result.structural_type
                    && !structural_arguments
                        .iter()
                        .any(|actual| actual.place == result.place) =>
                {
                    storage.push((
                        signature.place,
                        signature.structural_type,
                        super::DescriptorLocation::whole(result.place),
                    ));
                }
                _ => return None,
            }
            let substitutions = bindings.iter().cloned().collect::<BTreeMap<_, _>>();
            if substitutions.len() != bindings.len() {
                return None;
            }
            let mut postconditions = TerminalPostconditions {
                scalar: instantiate_payload_scalar_guarantees(&callee.contract, &bindings),
                stored: Vec::new(),
            };
            let mut pending = callee
                .contract
                .ensures
                .iter()
                .map(|clause| &clause.proposition)
                .collect::<Vec<_>>();
            while let Some(proposition) = pending.pop() {
                if let Proposition::Conjunction(children) = proposition {
                    pending.extend(children);
                    continue;
                }
                let finite = proof_admission::lower_integer_math_relation(proposition);
                let Proposition::Equal(left, right) = finite.as_ref().unwrap_or(proposition) else {
                    continue;
                };
                for (subject, value) in [(left, right), (right, left)] {
                    let (root, path) = match subject {
                        ScalarTerm::IntegerField { root, path, .. }
                        | ScalarTerm::BooleanField {
                            version: semantic_vocabulary::ContentPlaceVersion::Current,
                            root,
                            path,
                        } => (*root, path),
                        _ => continue,
                    };
                    if subject.scalar_type() != value.scalar_type() {
                        continue;
                    }
                    let mut bound = true;
                    let immutable = Proposition::Equal(value.clone(), value.clone())
                        .visit_value_ids(|value| bound &= substitutions.contains_key(&value));
                    if !immutable || !bound {
                        continue;
                    }
                    let Some((_, carrier, destination)) =
                        storage.iter().find(|(formal, _, _)| *formal == root)
                    else {
                        continue;
                    };
                    let Some(selected) =
                        crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                            &module.structural_types,
                            root,
                            *carrier,
                            path,
                        )
                    else {
                        continue;
                    };
                    if super::payload_predicates::observation_type(
                        &module.structural_types,
                        *carrier,
                        &selected.path,
                    ) != Some(value.scalar_type())
                    {
                        continue;
                    }
                    let mut destination = destination.clone();
                    destination.path.extend(selected.path);
                    postconditions.stored.push((
                        destination,
                        substitution::substitute_scalar_term_values(value, &substitutions),
                    ));
                }
            }
            Some(postconditions)
        }

        pub fn case_payload_scalar_types(
            types: &[terminal_psi::StructuralTypeDeclaration],
            root: semantic_vocabulary::StructuralTypeId,
            path: &[terminal_psi::StructuralPathSegment],
            case: semantic_vocabulary::StructuralCaseId,
            fields: &[semantic_vocabulary::StructuralFieldId],
        ) -> Option<Vec<semantic_vocabulary::ScalarType>> {
            let carrier = runtime_structural_path_tip(types.iter(), root, path)?;
            terminal_psi::find_structural_case(carrier.shape.sum_cases()?, case)?;
            let mut selected = path.to_vec();
            selected.push(terminal_psi::StructuralPathSegment::Case(case));
            fields
                .iter()
                .map(|field| {
                    let declaration =
                        crate::projected_field_declaration(types.iter(), root, &selected, *field)?;
                    (!declaration.relevance.is_erased()).then_some(())?;
                    declaration.field_type.scalar_type()
                })
                .collect()
        }
    }
    pub use contents::{
        DescriptorContentsOperand, DescriptorContentsQuery, DescriptorContentsStep,
        DescriptorContentsTransfer, DescriptorPayloadTarget, DescriptorStoreFlow,
        ElementPayloadMemberships, PayloadMembershipContract, descriptor_contents_query,
        descriptor_contents_transfers, descriptor_leaf_store, descriptor_scalar_arguments,
        element_payload_memberships, element_payload_memberships_in, entry_selector,
        payload_memberships, reference_backing_memberships, terminal_payload_contract,
    };

    pub(crate) use contents::{
        contents_membership_path, descriptor_storage_use_source, inline_selected_index,
    };
    pub use payload_history::{
        PayloadMembershipHistory, current_payload_contract, current_selected_payload_contract,
    };
    pub(crate) use payload_predicates::terminal_scalar_snapshots;
    pub use payload_predicates::{
        PayloadEntryScalarIdentity, PayloadScalarObservation, TerminalScalarExpressions,
        scalar_payload_expression,
    };
    pub use payload_versions::{
        ProducedPayloadVersion, ScalarCaptureAnalysis, ScalarCaptureFailure,
        projected_stored_sources_in, require_completed_scalar_capture,
        require_unchanged_entry_storage, require_unchanged_observed_storage,
        require_unchanged_produced_payload, require_unchanged_produced_payload_in,
        unchanged_payload_producer, unchanged_stored_projection_sources_in,
        unchanged_stored_scalar_sources, unchanged_stored_sources,
    };
    pub(crate) use scalar_captures::scalar_storage_writes;
    pub use scalar_captures::{
        CapturedScalarOperand, ResidentValueCapture, ResidentValueCaptures, ResidentValueTransfer,
        TerminalScalarCaptures, terminal_resident_value_transfers,
    };
    pub use scalar_contracts::{case_payload_scalar_types, instantiate_payload_scalar_guarantees};
    pub(crate) mod owned_shape {
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::StructuralTypeId;
        use std::collections::BTreeSet;
        use terminal_psi::ByteSequenceCarrier;
        use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

        pub fn plain_owned_shape<'a>(
            types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
        ) -> bool {
            owned_shape(types, root, false)
        }

        pub fn owned_shared_byte_descriptor_shape<'a>(
            types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
        ) -> bool {
            owned_shape(types, root, true)
        }

        fn owned_shape<'a>(
            types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
            root: StructuralTypeId,
            shared_byte_slots: bool,
        ) -> bool {
            fn visit<'a>(
                types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
                root: StructuralTypeId,
                active: &mut Vec<StructuralTypeId>,
                complete: &mut BTreeSet<StructuralTypeId>,
                shared_byte_slots: bool,
            ) -> bool {
                if complete.contains(&root) {
                    return true;
                }
                if active.contains(&root) {
                    return false;
                }
                let Some(declaration) =
                    exactly_one(types.clone(), |declaration| declaration.id == root)
                else {
                    return false;
                };
                active.push(root);
                let mut field_is_owned = |field: &terminal_psi::StructuralFieldDeclaration| {
                    if field.relevance.is_erased() {
                        return matches!(field.field_type, StructuralFieldType::Erased { .. });
                    }
                    match &field.field_type {
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::Atomic(_)
                        | StructuralFieldType::AtomicBoundedInteger(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::RefinedScalar(_)
                        | StructuralFieldType::IeeeFloat(_) => true,
                        StructuralFieldType::Structural(child) => {
                            visit(types.clone(), *child, active, complete, shared_byte_slots)
                        }
                        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            ..
                        })
                        | StructuralFieldType::PredicateByteSequence {
                            carrier: ByteSequenceCarrier::BoundedOwned { .. },
                            ..
                        } => true,
                        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        })
                        | StructuralFieldType::PredicateByteSequence {
                            carrier:
                                ByteSequenceCarrier::BorrowedView {
                                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                                },
                            ..
                        } => shared_byte_slots,
                        StructuralFieldType::ByteSequence(_)
                        | StructuralFieldType::PredicateByteSequence { .. }
                        | StructuralFieldType::Erased { .. } => false,
                    }
                };
                let supported = match &declaration.shape {
                    // Moving opaque contents exposes no nested references.
                    // Their claim and boundary contracts remain separate.
                    StructuralTypeShape::Opaque => true,
                    StructuralTypeShape::PrimitiveScalar(_) => true,
                    StructuralTypeShape::AtomicCell(_) => true,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        ..
                    }) => true,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                    }) => shared_byte_slots,
                    StructuralTypeShape::ByteSequence(_) => false,
                    StructuralTypeShape::ElementView { .. } => false,
                    StructuralTypeShape::Reference { .. } => false,
                    StructuralTypeShape::Record { fields, .. } => {
                        fields.iter().all(&mut field_is_owned)
                    }
                    StructuralTypeShape::Sum { cases } => cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .all(&mut field_is_owned),
                    StructuralTypeShape::Mixed { fields, cases } => fields
                        .iter()
                        .chain(cases.iter().flat_map(|case| &case.fields))
                        .all(&mut field_is_owned),
                    StructuralTypeShape::FixedArray { element, .. } => {
                        visit(types.clone(), *element, active, complete, shared_byte_slots)
                    }
                };
                active.pop();
                if supported {
                    complete.insert(root);
                }
                supported
            }
            visit(
                types,
                root,
                &mut Vec::new(),
                &mut BTreeSet::new(),
                shared_byte_slots,
            )
        }
    }
    pub use compatibility::{DescriptorCompatibilityFailure, DescriptorCompatibilityRequirement};
    pub use owned_shape::{owned_shared_byte_descriptor_shape, plain_owned_shape};
    pub(crate) mod invocation {
        use super::*;
        use crate::descriptors::descriptor_lifetimes::descriptor_location;
        use terminal_psi::BoundaryMachineResult;
        use terminal_psi::TerminalDynamicReferenceOrigin;

        pub struct DescriptorMachineSignature<'a> {
            pub machine: MachineId,
            pub structural_parameters: &'a [terminal_psi::StructuralParameterDeclaration],
            pub result: Option<&'a terminal_psi::StructuralResultDeclaration>,
        }

        pub fn machine_invocation(
            owner: DescriptorInvocationOwner,
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            callee: &TerminalMachine,
            actuals: &[StructuralArgument],
            result: Option<&terminal_psi::StructuralOperationResult>,
            actual_root_types: impl IntoIterator<Item = Option<StructuralTypeId>>,
        ) -> Option<DescriptorInvocation> {
            machine_invocation_for_signature(
                owner,
                types,
                slot_kind,
                DescriptorMachineSignature {
                    machine: callee.id,
                    structural_parameters: &callee.structural_parameters,
                    result: callee.result.structural(),
                },
                actuals,
                result,
                actual_root_types,
            )
        }

        pub fn machine_invocation_for_signature(
            owner: DescriptorInvocationOwner,
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            callee: DescriptorMachineSignature<'_>,
            actuals: &[StructuralArgument],
            result: Option<&terminal_psi::StructuralOperationResult>,
            actual_root_types: impl IntoIterator<Item = Option<StructuralTypeId>>,
        ) -> Option<DescriptorInvocation> {
            if callee.structural_parameters.len() != actuals.len() {
                return None;
            }
            let mut arguments = Vec::with_capacity(actuals.len());
            let mut argument_contents = Vec::with_capacity(actuals.len());
            let mut actual_root_types = actual_root_types.into_iter();
            for (actual, formal) in actuals.iter().zip(callee.structural_parameters) {
                if actual.access != formal.access {
                    return None;
                }
                let root = actual_root_types.next()??;
                arguments.push(descriptor_location(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
                argument_contents.push(descriptor_contents_query(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
            }
            if actual_root_types.next().is_some() {
                return None;
            }
            let result = result.filter(|result| {
                slot_kind.tracks_returned_value(types, result.place, result.structural_type)
            });
            if let Some(result) = result {
                let expected = callee.result?;
                if result.structural_type != expected.structural_type
                    || result.multiplicity != expected.multiplicity
                    || result.qualifications != expected.qualifications
                    || !expected
                        .projected_qualifications
                        .iter()
                        .all(|row| result.projected_qualifications.contains(row))
                {
                    return None;
                }
            }
            Some(DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner,
                target: DescriptorInvocationTarget::Machine(callee.machine),
                arguments,
                argument_contents,
                result: result.map(|result| DescriptorLocation::whole(result.place)),
            })
        }

        impl DescriptorMachine {
            // A call that never returns can still read or write its arguments.
            // Both call forms use the shared signature checks and retain the
            // complete parameter list and supplied arguments at the pre-call cut.
            pub(crate) fn direct_invocation(
                &self,
                module: &TerminalModule,
                machines: &[&TerminalMachine],
                slot_kind: DescriptorSlotKind,
                invocation: terminal_psi::ModuleInvocation<'_>,
                result: Option<&terminal_psi::StructuralOperationResult>,
                selector: impl Fn(ValueId) -> Option<ScalarTerm>,
            ) -> Option<DescriptorInvocation> {
                let inputs = invocation.inputs;
                let callee = terminal_graph_machine(machines, inputs.callee)?;
                if callee.parameters.len() != inputs.arguments.len() {
                    return None;
                }
                let owner = match invocation.site {
                    semantic_vocabulary::InvocationSite::Operation(operation) => {
                        DescriptorInvocationOwner::Operation(operation)
                    }
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => {
                        DescriptorInvocationOwner::InvokeNoNormal(edge)
                    }
                    _ => return None,
                };
                let mut call = machine_invocation(
                    owner,
                    &module.structural_types,
                    slot_kind,
                    callee,
                    inputs.structural_arguments,
                    result,
                    inputs
                        .structural_arguments
                        .iter()
                        .map(|actual| self.place_types.get(&actual.place).copied()),
                )?;
                call.scalar_arguments = descriptor_scalar_arguments(
                    callee
                        .parameters
                        .iter()
                        .map(|formal| (formal.id, formal.scalar_type)),
                    inputs.arguments,
                    selector,
                );
                Some(call)
            }
        }

        pub fn public_invocation(
            operation: semantic_vocabulary::OperationId,
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            slot: &impl terminal_psi::ReferenceResultShape,
            realizations: Vec<MachineId>,
            actuals: &[terminal_psi::StructuralArgument],
            result: Option<&terminal_psi::StructuralOperationResult>,
            place_types: &BTreeMap<PlaceId, StructuralTypeId>,
        ) -> Option<DescriptorInvocation> {
            if actuals.len() != slot.structural_parameter_count() {
                return None;
            }
            let mut arguments = Vec::with_capacity(actuals.len());
            let mut argument_contents = Vec::with_capacity(actuals.len());
            let mut invalidates_contents = false;
            for (index, actual) in actuals.iter().enumerate() {
                let (_, access) = slot.structural_parameter(index)?;
                if actual.access != access {
                    return None;
                }
                let root = *place_types.get(&actual.place)?;
                arguments.push(descriptor_location(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
                argument_contents.push(descriptor_contents_query(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
                invalidates_contents |= slot_kind.may_write_indirect_contents(types, root, actual);
            }
            let mut results = Vec::new();
            if let Some(result) = result {
                let declared_result = slot.result();
                let BoundaryMachineResult::Structural(expected) = declared_result.as_ref() else {
                    return None;
                };
                if result.structural_type != expected.structural_type
                    || result.multiplicity != expected.multiplicity
                    || result.qualifications != expected.qualifications
                    || result.projected_qualifications != expected.projected_qualifications
                {
                    return None;
                }
                results = public_result_slots(types, slot_kind, slot, result.place)?;
            } else if matches!(slot.result().as_ref(), BoundaryMachineResult::Structural(_)) {
                return None;
            }
            Some(DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(operation),
                target: DescriptorInvocationTarget::Public {
                    effects_machine: None,
                    realizations,
                    extent: semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                    results,
                    invalidates_contents,
                },
                arguments,
                argument_contents,
                result: result.map(|result| {
                    crate::descriptors::descriptor_lifetimes::DescriptorLocation::whole(
                        result.place,
                    )
                }),
            })
        }

        pub fn public_result_slots(
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            slot: &impl terminal_psi::ReferenceResultShape,
            place: PlaceId,
        ) -> Option<Vec<DescriptorResultSlot>> {
            let declared_result = slot.result();
            let BoundaryMachineResult::Structural(result) = declared_result.as_ref() else {
                return Some(Vec::new());
            };
            let mut results = Vec::new();
            let leaves = crate::borrowed_result_leaves(
                types.iter(),
                result.structural_type,
                result.multiplicity,
                slot.maximum_reference_leaves(),
            )
            .ok()?;
            for leaf in leaves {
                let selected_kind = matches!(
                    (&leaf.kind, slot_kind),
                    (
                        crate::BorrowedResultLeafKind::View,
                        DescriptorSlotKind::Views | DescriptorSlotKind::BorrowedStorage
                    ) | (
                        crate::BorrowedResultLeafKind::Reference(_),
                        DescriptorSlotKind::References
                            | DescriptorSlotKind::BorrowedStorage
                            | DescriptorSlotKind::AccessOrigins,
                    )
                );
                if !selected_kind {
                    continue;
                }
                let path = leaf.path;
                let leaf = descriptor_location(types, place, result.structural_type, &path, None)?;
                let mut origins = Vec::new();
                for region in slot.reference_regions(&path) {
                    let region = region.ok()?;
                    let destination = descriptor_location(
                        types,
                        place,
                        result.structural_type,
                        &region.path,
                        None,
                    )?;
                    if destination.path != leaf.path {
                        continue;
                    }
                    let index = match region.origin {
                        TerminalDynamicReferenceOrigin::Parameter { argument_index } => {
                            argument_index
                        }
                        TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                            let mut projection = Projection::new(&region.source_path);
                            projection.forget_address();
                            origins.push(DescriptorStoredOrigin::Invocation {
                                carrier_type,
                                projection,
                            });
                            continue;
                        }
                    };
                    let parameter = usize::try_from(index).ok()?;
                    let (formal, _) = slot.structural_parameter(parameter)?;
                    origins.push(public_input_origin(
                        types,
                        formal,
                        parameter,
                        place,
                        &region.source_path,
                    )?);
                }
                if origins.is_empty() {
                    return None;
                }
                results.push(DescriptorResultSlot {
                    path: leaf.path,
                    origins,
                });
            }
            Some(results)
        }

        fn public_input_origin(
            types: &[StructuralTypeDeclaration],
            formal: StructuralTypeId,
            parameter: usize,
            scratch_place: PlaceId,
            source_path: &[StructuralPathSegment],
        ) -> Option<DescriptorStoredOrigin> {
            let path = crate::reference_source_storage_path(types.iter(), formal, source_path);
            let mut supplying = None;
            for position in 0..=path.len() {
                let prefix = &path[..position];
                let shape = crate::runtime_structural_path_tip(types.iter(), formal, prefix);
                let stored_view = !prefix.is_empty()
                    && (shape
                        .as_ref()
                        .is_some_and(|row| is_descriptor(types.iter(), row.id))
                        || crate::runtime_structural_field(types.iter(), formal, prefix)
                            .and_then(|field| field.field_type.byte_sequence_carrier())
                            .is_some_and(|carrier| carrier.is_borrowed()));
                let pointer_crossing = shape
                    .as_ref()
                    .is_some_and(|row| matches!(row.shape, StructuralTypeShape::Reference { .. }))
                    && path.get(position) == Some(&StructuralPathSegment::Referent);
                if stored_view || pointer_crossing {
                    supplying = Some((position, position + usize::from(pointer_crossing)));
                }
            }
            let suffix = supplying.map_or(path, |(_, suffix)| &path[suffix..]);
            let mut projection = Projection::new(suffix);
            projection.forget_address();
            match supplying {
                Some((prefix, _)) => {
                    let selected =
                        descriptor_location(types, scratch_place, formal, &path[..prefix], None)?;
                    Some(DescriptorStoredOrigin::IncomingContents {
                        position: parameter,
                        steps: selected
                            .path
                            .into_iter()
                            .map(DescriptorContentsStep::Address)
                            .collect(),
                        projection,
                    })
                }
                None => Some(DescriptorStoredOrigin::Parameter {
                    position: parameter,
                    projection,
                }),
            }
        }

        pub fn writable_descriptor_slots(
            types: &[StructuralTypeDeclaration],
            place_types: &BTreeMap<PlaceId, semantic_vocabulary::StructuralTypeId>,
            actual: &terminal_psi::StructuralArgument,
            slot_kind: DescriptorSlotKind,
        ) -> Option<Vec<DescriptorLocation>> {
            let ty = *place_types.get(&actual.place)?;
            let selected = descriptor_location(types, actual.place, ty, &actual.path, None);
            Some(
                slot_kind
                    .slots(types, actual.place, ty)?
                    .into_iter()
                    .filter(|slot| {
                        !slot.path.is_empty()
                            && selected
                                .as_ref()
                                .is_none_or(|selected| selected.contains(slot))
                    })
                    .collect(),
            )
        }

        pub fn selected_stored_effects_row(
            catalog: &terminal_psi::TerminalDynamicDispatchCatalog,
            table: u32,
            interface: u32,
            slot: u32,
        ) -> Option<semantic_vocabulary::MachineId> {
            let selected = catalog.table(table)?;
            let declaration = catalog.interface(interface)?;
            let signature = declaration.slots.get(usize::try_from(slot).ok()?)?;
            if selected.id != table
                || selected.interface != interface
                || declaration.id != interface
                || signature.slot != slot
                || !signature.family_tuple.is_empty()
                || selected.rows.len() != declaration.slots.len()
            {
                return None;
            }
            catalog.row(table, slot)
        }

        pub fn public_realizations(
            catalog: &terminal_psi::TerminalDynamicDispatchCatalog,
            interface: u32,
            slot: u32,
        ) -> Option<Vec<MachineId>> {
            catalog
                .interface(interface)?
                .slots
                .get(usize::try_from(slot).ok()?)?;
            let mut realizations = Vec::new();
            for table in catalog
                .tables
                .iter()
                .filter(|table| table.interface == interface)
            {
                if let Some(machine) = *table.rows.get(usize::try_from(slot).ok()?)?
                    && !realizations.contains(&machine)
                {
                    realizations.push(machine);
                }
            }
            Some(realizations)
        }
    }
    pub use invocation::selected_stored_effects_row;
    pub(crate) mod provider_invocation {
        use super::*;
        use crate::descriptors::descriptor_lifetimes::descriptor_location;
        use terminal_psi::BoundaryReferenceDestination;

        pub fn provider_outputs_cover_writable_slots(
            types: &[StructuralTypeDeclaration],
            declaration: &terminal_psi::BoundaryMachineDeclaration,
        ) -> bool {
            declaration
                .structural_parameters
                .iter()
                .enumerate()
                .all(|(position, parameter)| {
                    let mut slots = Vec::new();
                    for kind in [DescriptorSlotKind::References, DescriptorSlotKind::Views] {
                        let Some(writable) = kind.writable_slots(
                            types,
                            parameter.place,
                            parameter.structural_type,
                            parameter.access,
                        ) else {
                            return false;
                        };
                        slots.extend(writable);
                    }
                    let outputs = declaration
                        .reference_outputs
                        .iter()
                        .filter(|output| {
                            output.destination
                                == BoundaryReferenceDestination::Parameter(position as u32)
                        })
                        .collect::<Vec<_>>();
                    slots.len() == outputs.len()
                        && slots.iter().all(|slot| {
                            outputs
                                .iter()
                                .filter(|output| {
                                    descriptor_location(
                                        types,
                                        parameter.place,
                                        parameter.structural_type,
                                        &output.path,
                                        None,
                                    )
                                    .as_ref()
                                        == Some(slot)
                                })
                                .count()
                                == 1
                        })
                })
        }

        pub fn provider_invocation(
            operation: semantic_vocabulary::OperationId,
            types: &[StructuralTypeDeclaration],
            slot_kind: DescriptorSlotKind,
            declaration: &terminal_psi::BoundaryMachineDeclaration,
            actuals: &[StructuralArgument],
            result: Option<&terminal_psi::StructuralOperationResult>,
            place_types: &BTreeMap<PlaceId, StructuralTypeId>,
        ) -> Option<DescriptorInvocation> {
            if actuals.len() != declaration.structural_parameters.len()
                || slot_kind == DescriptorSlotKind::AccessOrigins
            {
                return None;
            }
            let mut arguments = Vec::with_capacity(actuals.len());
            let mut argument_contents = Vec::with_capacity(actuals.len());
            let mut invalidates_contents = false;
            for (actual, formal) in actuals.iter().zip(&declaration.structural_parameters) {
                if actual.access != formal.access {
                    return None;
                }
                let root = *place_types.get(&actual.place)?;
                arguments.push(descriptor_location(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
                argument_contents.push(descriptor_contents_query(
                    types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                ));
                invalidates_contents |= slot_kind.may_write_indirect_contents(types, root, actual);
            }
            let mut effects = Vec::new();
            for (output_index, output) in declaration.reference_outputs.iter().enumerate() {
                let (place, root_type) = match output.destination {
                    BoundaryReferenceDestination::Parameter(position) => {
                        let formal = declaration
                            .structural_parameters
                            .get(usize::try_from(position).ok()?)?;
                        (formal.place, formal.structural_type)
                    }
                };
                let location = descriptor_location(types, place, root_type, &output.path, None)?;
                if !slot_kind
                    .owned_slots(types, place, root_type)?
                    .contains(&location)
                {
                    continue;
                }
                let lifetime_dependencies = crate::references::provider_lifetime_debts(
                    types,
                    &declaration.structural_parameters,
                    &output.dependencies,
                )
                .ok()?;
                let origins = vec![DescriptorStoredOrigin::Provider(DescriptorProviderOrigin {
                    boundary: declaration.id,
                    output: DescriptorProviderOutput::Stored(u32::try_from(output_index).ok()?),
                    invocations: Vec::new(),
                    projection: Projection::new(&[]),
                    lifetime_dependencies,
                })];
                match output.destination {
                    BoundaryReferenceDestination::Parameter(position) => {
                        effects.push(DescriptorStoredEffect {
                            parameter: usize::try_from(position).ok()?,
                            steps: location
                                .path
                                .into_iter()
                                .map(DescriptorContentsStep::Address)
                                .collect(),
                            origins,
                        })
                    }
                }
            }
            let results = if !declaration.result_reference_regions.is_empty() {
                super::public_result_slots(types, slot_kind, declaration, result?.place)?
            } else if let Some(result) = result {
                crate::boundary_result_slots(types, declaration, slot_kind, result).ok()?
            } else {
                Vec::new()
            };
            let result = (!results.is_empty()).then(|| {
                crate::descriptor_lifetimes::DescriptorLocation::whole(result.unwrap().place)
            });
            Some(DescriptorInvocation {
                scalar_arguments: Vec::new(),
                owner: DescriptorInvocationOwner::Operation(operation),
                target: DescriptorInvocationTarget::Provider {
                    results,
                    effects,
                    invalidates_contents,
                },
                arguments,
                argument_contents,
                result,
            })
        }
    }
    pub use provider_invocation::{provider_invocation, provider_outputs_cover_writable_slots};
    pub(crate) mod terminal_flow {
        use super::*;
        use crate::descriptors::descriptor_lifetimes::{
            DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, descriptor_location,
            descriptor_location_type,
        };
        use crate::exactly_one::exactly_one;
        use semantic_vocabulary::ScalarTerm;
        use terminal_psi::BoundaryMachineResult;
        use terminal_psi::RecordFieldValue;
        use terminal_psi::element_view_establish_pair_pattern;
        use terminal_psi::find_structural_type;

        enum ScalarArrival<'a> {
            Values(&'a [semantic_vocabulary::ValueId]),
            CasePayload {
                source: PlaceId,
                path: &'a [StructuralPathSegment],
                case: semantic_vocabulary::StructuralCaseId,
                fields: &'a [semantic_vocabulary::StructuralFieldId],
            },
        }

        // Owned access also presents unrestricted copies. Only a call's
        // retained affine/linear formal transfers its source custody.
        fn consume_call(
            arguments: &[StructuralArgument],
            mut parameters: impl Iterator<Item = (StructuralAccess, StructuralMultiplicity)>,
            mut consume: impl FnMut(&StructuralArgument) -> Option<()>,
        ) -> Option<()> {
            for argument in arguments {
                let (access, multiplicity) = parameters.next()?;
                if argument.access == StructuralAccess::Owned
                    && access == StructuralAccess::Owned
                    && multiplicity != StructuralMultiplicity::Unrestricted
                {
                    consume(argument)?;
                }
            }
            parameters.next().is_none().then_some(())
        }

        // Authority follows the value into its destination, but a consumed
        // carrier cannot remain another authorized spelling. Keep these
        // definitions alongside the operation's derivations: replay reads
        // every transported origin from the same pre-operation state.
        fn record_owned_consumption(
            module: &TerminalModule,
            machine: &TerminalMachine,
            operation: &Operation,
            step: &mut DescriptorFlowStep,
        ) -> Option<()> {
            let mut consume = |argument: &StructuralArgument| {
                let consumed = descriptor_location(
                    &module.structural_types,
                    argument.place,
                    machine.structural_place_type(argument.place)?,
                    &argument.path,
                    None,
                )?;
                step.definitions.push(consumed.clone());
                step.consumed.push(consumed);
                Some(())
            };
            let retained = |argument: &StructuralArgument| {
                if argument.access != StructuralAccess::Owned {
                    return Some(false);
                }
                if let Some(source) = crate::structural_argument_source(machine, argument.place, 0)
                {
                    return Some(source.multiplicity != StructuralMultiplicity::Unrestricted);
                }
                match &exactly_one(machine.structural_places.iter(), |place| {
                    place.id == argument.place
                })?
                .kind
                {
                    StructuralPlaceKind::TrivialAffineLocal { .. } => Some(true),
                    StructuralPlaceKind::ByteSequenceLiteral { .. } => Some(false),
                    _ => None,
                }
            };
            match &operation.kind {
                OperationKind::DecomposeStructuralValue { .. } => {
                    // Only the nominal disposition ends. Its contained
                    // descriptors keep their same storage and derivations.
                }
                OperationKind::MoveStructuralValue { source }
                | OperationKind::TransformContent { source, .. } => {
                    if source.access == StructuralAccess::Owned {
                        consume(source)?;
                    }
                }
                OperationKind::StoreStructuralField { value, .. } => {
                    if retained(value)? {
                        consume(value)?;
                    }
                }
                kind => {
                    for argument in kind.constructor_arguments() {
                        if retained(argument)? {
                            consume(argument)?;
                        }
                    }
                }
            }
            if let Some(invocation) = operation.declared_invocation() {
                let parameters = module.invocation_structural_parameters(
                    invocation.callee,
                    invocation.required_contract,
                )?;
                consume_call(
                    invocation.structural_arguments,
                    parameters
                        .iter()
                        .map(|parameter| (parameter.access, parameter.multiplicity)),
                    &mut consume,
                )?;
            } else if let OperationKind::CallDynamic {
                interface,
                slot,
                structural_arguments,
                ..
            } = &operation.kind
            {
                let parameters = &module
                    .dynamic_dispatch
                    .interface(*interface)?
                    .slots
                    .get(usize::try_from(*slot).ok()?)?
                    .structural_parameters;
                consume_call(
                    structural_arguments,
                    parameters
                        .iter()
                        .map(|parameter| (parameter.access, parameter.multiplicity)),
                    &mut consume,
                )?;
            }
            Some(())
        }

        pub(crate) fn residence_rules(
            module: &TerminalModule,
            machine: &TerminalMachine,
        ) -> Option<crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules> {
            let mut rules =
                crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules::default();
            for block in &machine.blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let position = (block.id, u32::try_from(ordinal).ok()?);
                    match &operation.kind {
                        element_view_establish_pair_pattern!(source, ..)
                        | OperationKind::EstablishAccessReference { source, .. }
                        | OperationKind::EstablishReference { source, .. } => {
                            let (source, _, capture) = captured_storage_source(
                                &module.structural_types,
                                source,
                                machine.structural_place_type(source.place)?,
                                position,
                            )?;
                            if capture == CapturedBacking::StorageAddress {
                                rules.captures.insert((
                                    position,
                                    source,
                                    DescriptorLocation::whole(operation.result.structural()?.place),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some(rules)
        }

        pub(crate) fn adapt(
            module: &TerminalModule,
            machines: &[&TerminalMachine],
            machine: &TerminalMachine,
            slot_kind: DescriptorSlotKind,
            selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
            selector_graph: Option<&[DescriptorMachine]>,
        ) -> Result<DescriptorMachine, super::DescriptorGraphError> {
            // Setup failures have no operation. Reuse the same typed error
            // as the reader advances, so a later failure keeps its real site.
            let mut failure = super::DescriptorGraphError::IncompleteMachine {
                machine: machine.id,
                position: None,
                successor: None,
                detail: None,
            };
            adapt_flow(
                module,
                machines,
                machine,
                slot_kind,
                selected_calls,
                selector_graph,
                &mut failure,
            )
            .ok_or(failure)
        }

        fn adapt_flow(
            module: &TerminalModule,
            machines: &[&TerminalMachine],
            machine: &TerminalMachine,
            slot_kind: DescriptorSlotKind,
            selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
            selector_graph: Option<&[DescriptorMachine]>,
            failure: &mut super::DescriptorGraphError,
        ) -> Option<DescriptorMachine> {
            let super::DescriptorGraphError::IncompleteMachine {
                position: failure_position,
                successor: failure_successor,
                detail: failure_detail,
                ..
            } = failure
            else {
                return None;
            };
            let scalar_expressions =
                super::payload_predicates::TerminalScalarExpressions::new(machine);
            let mut scalar_formals = machine
                .parameters
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>();
            scalar_formals.sort_unstable();
            let selector = |value| {
                super::entry_selector(scalar_expressions.expression(value)?, |id| {
                    scalar_formals.binary_search(&id).is_ok()
                })
            };
            let mut graph = DescriptorMachine {
                id: machine.id,
                returns: machine
                    .result
                    .structural()
                    .filter(|result| {
                        slot_kind.tracks_returned_value(
                            &module.structural_types,
                            result.place,
                            result.structural_type,
                        )
                    })
                    .map(|_| Vec::new()),
                places: Vec::new(),
                place_types: BTreeMap::new(),
                entry: (machine.entry, 0),
                flow: BTreeMap::new(),
                case_constructors: BTreeMap::new(),
                case_successors: BTreeMap::new(),
                originations: BTreeMap::new(),
                projections: BTreeMap::new(),
                contents_queries: BTreeMap::new(),
                contents_transfers: BTreeMap::new(),
                payload_invalidations: BTreeMap::new(),
                payload_producers: BTreeMap::new(),
                payload_entry_predicates: Some((
                    semantic_vocabulary::PropositionContext::from_value_types_and_places(
                        machine
                            .parameters
                            .iter()
                            .chain(&machine.contract.erased_scalar_formals)
                            .map(|value| (value.id, value.scalar_type)),
                        machine
                            .structural_places
                            .iter()
                            .map(|place| (place.id, place.kind)),
                    )
                    .ok()?,
                    machine.contract.requires.clone(),
                )),
                scalar_payload_writes: BTreeMap::new(),
                scalar_payload_reads: BTreeMap::new(),
                payload_scalar_definitions: BTreeMap::new(),
                payload_scalar_postconditions: BTreeMap::new(),
                payload_scalar_parameters: BTreeMap::new(),
                payload_edge_predicates: BTreeMap::new(),
                payload_scalar_bindings: BTreeMap::new(),
                unresolved_contents_writes: BTreeSet::new(),
                storage_derivations: BTreeSet::new(),
                calls: BTreeMap::new(),
                normal_exits: Vec::new(),
            };
            for place in &machine.structural_places {
                graph.places.push(DescriptorPlace {
                    producer: None,
                    place: place.id,
                    definition: terminal_definition(module, machine, place.id, place.kind)?,
                });
                if let Some(ty) = machine.structural_place_type(place.id) {
                    graph.place_types.insert(place.id, ty);
                }
            }
            graph.places.sort_unstable_by_key(|row| row.place);
            if graph
                .places
                .windows(2)
                .any(|pair| pair[0].place == pair[1].place)
            {
                return None;
            }
            let location = |root, path: &[StructuralPathSegment], field| {
                descriptor_location(
                    &module.structural_types,
                    root,
                    *graph.place_types.get(&root)?,
                    path,
                    field,
                )
            };
            for block in &machine.blocks {
                graph.payload_scalar_parameters.insert(
                    block.id,
                    block.parameters.iter().map(|value| value.id).collect(),
                );
                for (index, operation) in block.operations.iter().enumerate() {
                    *failure_position = None;
                    *failure_successor = None;
                    *failure_detail = None;
                    let position = (block.id, u32::try_from(index).ok()?);
                    *failure_position = Some(position);
                    if let Some(postconditions) = super::scalar_contracts::terminal_postconditions(
                        module,
                        machine,
                        operation,
                        &scalar_expressions,
                    ) {
                        graph
                            .payload_scalar_postconditions
                            .insert(position, postconditions.scalar);
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .extend(postconditions.stored);
                    }
                    if let Some(result) = operation.result.scalar_ref() {
                        graph
                            .payload_scalar_definitions
                            .insert(position, vec![result.id]);
                    }
                    let scalar_store = match &operation.kind {
                        OperationKind::WriteOnlyPrimitiveStore {
                            destination,
                            path,
                            value,
                            ..
                        } => Some((*destination, path.as_slice(), None, *value)),
                        OperationKind::StructuralScalarFieldStore {
                            destination,
                            path,
                            field,
                            value,
                            ..
                        } => Some((*destination, path.as_slice(), Some(*field), *value)),
                        _ => None,
                    };
                    if let Some((destination, path, field, value)) = scalar_store
                        && let Some(value) = scalar_expressions.expression(value)
                        && let Some(destination) = location(destination, path, field)
                    {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((destination, value));
                    }
                    if let OperationKind::ScalarStructuralField {
                        source,
                        path,
                        field,
                    }
                    | OperationKind::BooleanStructuralField {
                        source,
                        path,
                        field,
                    } = &operation.kind
                    {
                        let mut path = path.clone();
                        path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                            *field,
                        ));
                        if let Some(location) =
                            crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                                &module.structural_types,
                                *source,
                                *graph.place_types.get(source)?,
                                &path,
                            )
                            && let Some(result) = operation.result.scalar_ref()
                        {
                            graph
                                .scalar_payload_reads
                                .entry(position)
                                .or_default()
                                .push((location, ScalarTerm::value(result.id, result.scalar_type)));
                        }
                    }
                    if let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind
                        && let Some(location) = location(*source, path, None)
                        && let Some(result) = operation.result.scalar_ref()
                    {
                        graph
                            .scalar_payload_reads
                            .entry(position)
                            .or_default()
                            .push((location, ScalarTerm::value(result.id, result.scalar_type)));
                    }
                    let mut step = DescriptorFlowStep::default();
                    let is_invocation = matches!(
                        &operation.kind,
                        terminal_psi::call_structural_argument_variants!(_arguments)
                    ) || matches!(&operation.kind, OperationKind::Call { .. });
                    // A scalar sibling does not observe the reference leaves
                    // beside it. Retain the representation's projected uses;
                    // reducing them to roots spuriously extends those loans.
                    for (used, access) in operation.kind.storage_uses() {
                        use terminal_psi::StructuralStorageUse;
                        let (root, selected) = match used {
                            StructuralStorageUse::Place(root) => {
                                (root, Some(DescriptorLocation::whole(root)))
                            }
                            StructuralStorageUse::Projection(projection)
                            | StructuralStorageUse::Element {
                                descriptor: projection,
                                ..
                            } => (
                                projection.root,
                                location(projection.root, projection.path, projection.field),
                            ),
                            StructuralStorageUse::Canonical { root, path } => (
                                root,
                                crate::descriptor_lifetimes::canonical_descriptor_location(
                                    &module.structural_types,
                                    root,
                                    *graph.place_types.get(&root)?,
                                    path,
                                ),
                            ),
                            StructuralStorageUse::ObservedField { root, path, field } => {
                                let mut selected = path.to_vec();
                                selected.push(
                                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                        field,
                                    ),
                                );
                                (
                                    root,
                                    crate::descriptor_lifetimes::canonical_descriptor_location(
                                        &module.structural_types,
                                        root,
                                        *graph.place_types.get(&root)?,
                                        &selected,
                                    ),
                                )
                            }
                            StructuralStorageUse::Type(_) => continue,
                        };
                        if let Some(selected) = selected {
                            let access = access.map(|access| {
                                (
                                    access,
                                    descriptor_storage_use_source(
                                        &module.structural_types,
                                        graph.place_types.get(&root).copied(),
                                        &operation.kind,
                                        used,
                                        |value| scalar_expressions.expression(value),
                                    ),
                                )
                            });
                            step.inputs.push((Some(selected), access));
                        } else {
                            // An unresolved selection can retain authority,
                            // never establish disjointness or an early end.
                            step.may_escape = true;
                            let selected = DescriptorLocation::whole(root);
                            let access = access.map(|access| {
                                (
                                    access,
                                    descriptor_storage_use_source(
                                        &module.structural_types,
                                        graph.place_types.get(&root).copied(),
                                        &operation.kind,
                                        used,
                                        |value| scalar_expressions.expression(value),
                                    ),
                                )
                            });
                            step.inputs.push((Some(selected), access));
                        }
                    }
                    let result = operation.result.structural();
                    if let Some(result) = result {
                        graph.payload_producers.insert(position, result.place);
                        let mut rows = machine
                            .structural_places
                            .iter()
                            .filter(|row| row.id == result.place);
                        if rows.next()?.kind
                            != (StructuralPlaceKind::OperationResult {
                                producer: operation.id,
                                structural_type: result.structural_type,
                            })
                            || rows.next().is_some()
                        {
                            return None;
                        }
                        let index = graph
                            .places
                            .binary_search_by_key(&result.place, |row| row.place)
                            .ok()?;
                        if graph.places[index].producer.replace(position).is_some() {
                            return None;
                        }
                        step.definitions
                            .push(DescriptorLocation::whole(result.place));
                        if !is_descriptor(module.structural_types.iter(), result.structural_type)
                            && !slot_kind.is_slot(&module.structural_types, result.structural_type)
                        {
                            graph.originations.entry(position).or_default().push((
                                DescriptorLocation::whole(result.place),
                                DescriptorCallOrigin::Argument {
                                    source: result.place,
                                    projection: Projection::new(&[]),
                                },
                            ));
                        }
                    }
                    let value_query = |root, path: &[StructuralPathSegment], field| {
                        descriptor_contents_query(
                            &module.structural_types,
                            root,
                            *graph.place_types.get(&root)?,
                            path,
                            field,
                        )
                    };
                    let payload_query = |root, path: &[StructuralPathSegment], field| {
                        let mut query = value_query(root, path, field)?;
                        if let Some(index) = path.iter().position(|segment| {
                            matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
                        }) {
                            let collection = crate::runtime_structural_path_tip(
                                module.structural_types.iter(),
                                *graph.place_types.get(&root)?,
                                &path[..index],
                            )?;
                            if matches!(collection.shape, StructuralTypeShape::FixedArray { .. }) {
                                // A later reference/view step names backing
                                // outside this array. Preserve that full query
                                // rather than turning it into an array write.
                                if query
                                    .inline_steps(
                                        &module.structural_types,
                                        *graph.place_types.get(&root)?,
                                    )
                                    .is_none_or(|inline| inline[index..].contains(&false))
                                {
                                    return Some(query);
                                }
                                // Any element may be written. Keep the array
                                // region rather than enumerating its extent;
                                // the complete destination was checked above.
                                return descriptor_contents_query(
                                    &module.structural_types,
                                    root,
                                    *graph.place_types.get(&root)?,
                                    &path[..index],
                                    None,
                                );
                            }
                        }
                        if field.is_none() {
                            let selected = crate::runtime_structural_path_tip(
                                module.structural_types.iter(),
                                *graph.place_types.get(&root)?,
                                path,
                            )?;
                            match selected.shape {
                                StructuralTypeShape::ElementView { element, .. } => {
                                    query
                                        .steps
                                        .push(DescriptorContentsStep::any_element(element));
                                }
                                StructuralTypeShape::ByteSequence(_)
                                    if is_descriptor(
                                        module.structural_types.iter(),
                                        selected.id,
                                    ) =>
                                {
                                    return None;
                                }
                                _ => {}
                            }
                        }
                        Some(query)
                    };
                    let mut payload_writes = Vec::new();
                    match &operation.kind {
                        OperationKind::DecomposeStructuralValue { subject } => {
                            if operation.result != terminal_psi::OperationResult::Unit {
                                return None;
                            }
                            crate::nominal_cleanup::decomposition(module, machine.id, subject)
                                .ok()?;
                            // Partitioning ownership does not write the value
                            // or any backing reached through its descriptors.
                        }
                        OperationKind::ActivateTaskRegister { gdt, scratch, .. }
                        | OperationKind::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                            for source in [gdt, scratch] {
                                payload_writes.push(payload_query(
                                    source.place,
                                    &source.path,
                                    None,
                                ));
                            }
                        }
                        OperationKind::GlobalDescriptorTable {
                            operation: terminal_psi::GlobalDescriptorTableOperation::Store,
                            source,
                            ..
                        } => {
                            payload_writes.push(payload_query(source.place, &source.path, None));
                        }
                        OperationKind::WriteOnlyPrimitiveStore {
                            destination, path, ..
                        } => {
                            payload_writes.push(payload_query(*destination, path, None));
                        }
                        // A move retires the selected value, including a
                        // view descriptor. It does not write the backing the
                        // descriptor names. Explicit source projections still
                        // retain any actual referent or element selection.
                        OperationKind::MoveStructuralValue { source } => {
                            payload_writes.push(value_query(source.place, &source.path, None));
                        }
                        OperationKind::MoveStructuralField {
                            source,
                            path,
                            field,
                        } => {
                            payload_writes.push(value_query(*source, path, *field));
                        }
                        OperationKind::StructuralScalarFieldStore {
                            destination,
                            path,
                            field,
                            ..
                        } => {
                            payload_writes.push(payload_query(*destination, path, Some(*field)));
                        }
                        OperationKind::StructuralByteSequenceFieldByteStore {
                            destination,
                            path,
                            field,
                            ..
                        }
                        | OperationKind::StructuralByteSequenceFieldStore {
                            destination,
                            path,
                            field,
                            ..
                        } => {
                            payload_writes.push(payload_query(*destination, path, Some(*field)));
                        }
                        OperationKind::StructuralByteViewFieldStore {
                            destination,
                            path,
                            field,
                            ..
                        }
                        | OperationKind::StoreStructuralField {
                            destination,
                            path,
                            field,
                            ..
                        } => {
                            payload_writes.push(payload_query(*destination, path, *field));
                        }
                        OperationKind::ByteSequenceWrite { .. }
                        | OperationKind::ByteSequenceStore { .. } => {
                            payload_writes.push(None);
                        }
                        OperationKind::ElementViewWrite {
                            destination,
                            index,
                            obligation,
                            path,
                            ..
                        } => {
                            let mut selected = destination.path.clone();
                            selected.push(StructuralPathSegment::RuntimeIndex {
                                index: *index,
                                obligation: *obligation,
                            });
                            selected.extend_from_slice(path);
                            payload_writes.push(payload_query(destination.place, &selected, None));
                        }
                        terminal_psi::call_structural_argument_variants!(arguments) => {
                            for argument in arguments {
                                let selected = crate::runtime_structural_path_tip(
                                    module.structural_types.iter(),
                                    *graph.place_types.get(&argument.place)?,
                                    &argument.path,
                                );
                                let mutation = selected
                                    .map(|selected| {
                                        crate::shared_mutation(
                                            module.structural_types.iter(),
                                            selected.id,
                                            &[],
                                        )
                                    })
                                    .unwrap_or(crate::SharedMutation::Unbounded);
                                if mutation == crate::SharedMutation::Unbounded {
                                    payload_writes.push(None);
                                } else if argument.access != StructuralAccess::SharedBorrow
                                    || mutation != crate::SharedMutation::None
                                {
                                    payload_writes.push(payload_query(
                                        argument.place,
                                        &argument.path,
                                        None,
                                    ));
                                }
                            }
                        }
                        OperationKind::AtomicAccess {
                            place, path, event, ..
                        } if event.modifies_resident() => {
                            payload_writes.push(payload_query(*place, path, None));
                        }
                        OperationKind::AtomicAccess { .. }
                        | OperationKind::AtomicFence { .. }
                        | OperationKind::StorageAddress { .. }
                        | OperationKind::StructuralLeafCopy { .. }
                        | OperationKind::StructuralCaseLeafCopy { .. }
                        | OperationKind::EstablishStructuralLocal { .. }
                        | OperationKind::EstablishZeroedStructural { .. } => {}
                        kind if kind.declared_invocation().is_some() => {}
                        kind => {
                            let tag = crate::OperationSemanticTag::for_operation(kind);
                            let scalar = crate::operation_semantic_row(kind)
                                .ok()?
                                .goal_free_scalar_leaf()
                                .is_some()
                                || crate::ProofBearingScalarSemanticRow::ALL
                                    .iter()
                                    .any(|row| row.tag() == tag);
                            let structural =
                                crate::StructuralEffectSemanticRow::ALL.iter().any(|row| {
                                    row.tag() == tag
                                        && row.schema().external_effect()
                                            == crate::StructuralEffectExternalEffect::None
                                });
                            if !scalar && !structural {
                                // A semantic family not represented by this
                                // storage flow cannot publish an empty frame.
                                payload_writes.push(None);
                            }
                        }
                    }
                    if !payload_writes.is_empty() {
                        graph.payload_invalidations.insert(position, payload_writes);
                    }
                    let direct_call = match &operation.kind {
                        kind if terminal_psi::direct_call_parts(kind).is_some() => {
                            let call = terminal_psi::direct_call_parts(kind)?;
                            Some((call.callee, call.structural_arguments))
                        }
                        OperationKind::BoundaryCall {
                            boundary,
                            structural_arguments,
                            ..
                        } => {
                            let mut selections =
                                selected_calls.iter().filter(|(head, _)| head == boundary);
                            match (selections.next(), selections.next()) {
                                (Some((_, callee)), None) => {
                                    Some((*callee, structural_arguments.as_slice()))
                                }
                                (None, None) => None,
                                _ => return None,
                            }
                        }
                        _ => None,
                    };
                    match &operation.kind {
                        OperationKind::DecomposeStructuralValue { subject } => {
                            // Observe every contained descriptor through the
                            // event; no value is consumed or newly derived.
                            step.replace_value_inputs([location(
                                subject.place,
                                &subject.path,
                                None,
                            )?]);
                        }
                        OperationKind::ReleaseReference { source, path } => {
                            let source = location(*source, path, None)?;
                            step.replace_value_inputs([source.clone()]);
                            step.definitions.push(source);
                        }
                        OperationKind::ByteSequenceLength { source }
                        | OperationKind::ElementViewLength { source } => {
                            let query = descriptor_contents_query(
                                &module.structural_types,
                                source.place,
                                *graph.place_types.get(&source.place)?,
                                &source.path,
                                None,
                            )?;
                            step.replace_value_inputs([query.observation()]);
                        }
                        OperationKind::EstablishNativeCodeLayout { .. }
                            if slot_kind.tracks_views() =>
                        {
                            let result = result?;
                            let destination =
                                native_code_layout_regions(&module.structural_types, result)?;
                            step.definitions.push(destination.clone());
                            graph.originations.entry(position).or_default().push((
                                destination,
                                DescriptorCallOrigin::Literal {
                                    machine: machine.id,
                                    place: result.place,
                                },
                            ));
                        }
                        OperationKind::EstablishZeroedStructural {} => {
                            let result = result?;
                            graph.case_constructors.insert(
                                position,
                                zeroed_case_predicates(
                                    &module.structural_types,
                                    result.place,
                                    result.structural_type,
                                ),
                            );
                            for slot in slot_kind
                                .inventory(
                                    &module.structural_types,
                                    result.place,
                                    result.structural_type,
                                )?
                                .slots
                            {
                                graph
                                    .originations
                                    .entry(position)
                                    .or_default()
                                    .push((slot, DescriptorCallOrigin::Empty));
                            }
                        }
                        OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
                            graph.payload_producers.insert(position, *destination);
                            let destination = DescriptorLocation::whole(*destination);
                            step.definitions.push(destination.clone());
                            graph.originations.entry(position).or_default().push((
                                destination,
                                DescriptorCallOrigin::Literal {
                                    machine: machine.id,
                                    place: *destination_root(&operation.kind)?,
                                },
                            ));
                        }
                        OperationKind::EstablishAccessReference { .. }
                            if slot_kind == DescriptorSlotKind::AccessOrigins =>
                        {
                            let result = result?;
                            let StructuralTypeShape::Reference {
                                referent: carrier, ..
                            } = find_structural_type(
                                &module.structural_types,
                                result.structural_type,
                            )?
                            .shape
                            else {
                                return None;
                            };
                            graph.originations.entry(position).or_default().push((
                                DescriptorLocation::whole(result.place),
                                DescriptorCallOrigin::AccessFormation {
                                    machine: machine.id,
                                    operation: operation.id,
                                    place: result.place,
                                    carrier,
                                    projection: Projection::new(&[]),
                                },
                            ));
                        }
                        element_view_establish_pair_pattern!(source, ..)
                        | OperationKind::EstablishAccessReference { source, .. }
                        | OperationKind::EstablishReference { source, .. } => {
                            let destination = DescriptorLocation::whole(result?.place);
                            let (source_location, mut backing_projection, capture) =
                                captured_storage_source(
                                    &module.structural_types,
                                    source,
                                    *graph.place_types.get(&source.place)?,
                                    position,
                                )?;
                            if let OperationKind::EstablishReference {
                                storage: Some(storage),
                                ..
                            } = &operation.kind
                            {
                                let StructuralTypeShape::Reference { referent, .. } =
                                    find_structural_type(
                                        &module.structural_types,
                                        result?.structural_type,
                                    )?
                                    .shape
                                else {
                                    return None;
                                };
                                // The loan keeps the source address and its
                                // new presentation together. Dropping this
                                // step would compare the record to its bytes
                                // as though they were different allocations.
                                let view =
                                    Projection::new(&[StructuralPathSegment::RepresentationView {
                                        structural_type: referent,
                                        stored_scalar: storage.stored_scalar,
                                    }]);
                                backing_projection = Some(match backing_projection {
                                    Some(prefix) => prefix.then(&view),
                                    None => view,
                                });
                            }
                            step.derivations
                                .push((source_location.clone(), destination.clone()));
                            // Inline collections lend their current storage
                            // address, just as references do. A view of another
                            // descriptor instead retains that descriptor's
                            // payload origin, never the descriptor's home.
                            let captures_storage = match &operation.kind {
                                element_view_establish_pair_pattern!(source, element, ..) => {
                                    crate::runtime_structural_path_tip(
                                        module.structural_types.iter(),
                                        *graph.place_types.get(&source.place)?,
                                        &source.path,
                                    )
                                    .and_then(|collection| {
                                        crate::collection_backing(
                                            module.structural_types.iter(),
                                            collection.id,
                                            *element,
                                        )
                                    })
                                    .is_some_and(|backing| {
                                        backing.storage == crate::CollectionStorage::Inline
                                    })
                                }
                                _ => true,
                            };
                            if capture == CapturedBacking::StorageAddress && captures_storage {
                                graph.storage_derivations.insert((
                                    position,
                                    source_location.clone(),
                                    destination.clone(),
                                ));
                            }
                            let preserves_coordinates = match &operation.kind {
                                element_view_establish_pair_pattern!(
                                    source,
                                    element,
                                    region,
                                    ..
                                ) => crate::runtime_structural_path_tip(
                                    module.structural_types.iter(),
                                    *graph.place_types.get(&source.place)?,
                                    &source.path,
                                )
                                .is_some_and(|collection| {
                                    crate::collection_view_preserves_coordinates(
                                        module.structural_types.iter(),
                                        collection.id,
                                        *element,
                                        *region,
                                    )
                                }),
                                _ => true,
                            };
                            if !preserves_coordinates {
                                backing_projection
                                    .get_or_insert_with(|| Projection::new(&[]))
                                    .forget_address();
                            }
                            if let Some(projection) = backing_projection {
                                graph.projections.insert(
                                    (position, source_location, destination.clone()),
                                    projection,
                                );
                            }
                            if let Some(origins) = graph.originations.get_mut(&position) {
                                origins.retain(|(location, _)| location != &destination);
                            }
                        }
                        OperationKind::ByteSequenceSubslice {
                            source, start, end, ..
                        }
                        | OperationKind::ElementViewSubslice {
                            source, start, end, ..
                        } => {
                            let query = descriptor_contents_query(
                                &module.structural_types,
                                source.place,
                                *graph.place_types.get(&source.place)?,
                                &source.path,
                                None,
                            )?;
                            let source = query.observation();
                            step.replace_value_inputs([source.clone()]);
                            let destination = DescriptorLocation::whole(result?.place);
                            step.derivations.push((source.clone(), destination.clone()));
                            graph
                                .contents_queries
                                .insert((position, source.clone(), destination.clone()), query);
                            graph.projections.insert(
                                (position, source, destination),
                                Projection::window(*start, *end, position),
                            );
                        }
                        terminal_psi::structural_leaf_copy_pair_pattern!()
                        | OperationKind::MoveStructuralValue { .. }
                        | OperationKind::TransformContent { .. } => {
                            let destination = DescriptorLocation::whole(result?.place);
                            let source = match &operation.kind {
                                OperationKind::StructuralLeafCopy { source, path } => {
                                    location(*source, path, None)
                                }
                                OperationKind::MoveStructuralValue { source }
                                | OperationKind::TransformContent { source, .. } => {
                                    location(source.place, &source.path, None)
                                }
                                OperationKind::StructuralCaseLeafCopy { source, path } => {
                                    crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                                        &module.structural_types,
                                        *source,
                                        *graph.place_types.get(source)?,
                                        path,
                                    )
                                }
                                _ => unreachable!("selected structural value transport"),
                            };
                            if let Some(source) = source {
                                step.replace_value_inputs([source.clone()]);
                                step.derivations.push((source, destination.clone()));
                            } else {
                                step.may_escape = true;
                            }
                            if (is_descriptor(
                                module.structural_types.iter(),
                                result?.structural_type,
                            ) || module.structural_types.iter().any(|ty| {
                                ty.id == result.unwrap().structural_type
                                    && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                            })) && let Some(origins) = graph.originations.get_mut(&position)
                            {
                                origins.retain(|(location, _)| location != &destination);
                            }
                        }
                        OperationKind::StructuralByteSequenceFieldLength {
                            source,
                            path,
                            field,
                        }
                        | OperationKind::StructuralByteSequenceFieldRead {
                            source,
                            path,
                            field,
                            ..
                        } => {
                            if let Some(source) = location(*source, path, Some(*field)) {
                                step.replace_value_inputs([source]);
                            } else {
                                step.may_escape = true;
                            }
                        }
                        OperationKind::MoveStructuralField {
                            source,
                            path,
                            field,
                            ..
                        } => {
                            let result = result?;
                            graph.contents_transfers.insert(
                                position,
                                descriptor_contents_transfers(
                                    &module.structural_types,
                                    slot_kind,
                                    DescriptorContentsOperand {
                                        root: *source,
                                        root_type: *graph.place_types.get(source)?,
                                        path,
                                        field: *field,
                                    },
                                    DescriptorContentsOperand {
                                        root: result.place,
                                        root_type: result.structural_type,
                                        path: &[],
                                        field: None,
                                    },
                                    result.structural_type,
                                    true,
                                )?,
                            );
                            if let Some(source) = location(*source, path, *field) {
                                let destination = DescriptorLocation::whole(result.place);
                                step.replace_value_inputs([source.clone()]);
                                step.definitions.push(source.clone());
                                step.derivations.push((source, destination));
                            } else {
                                step.may_escape = true;
                            }
                        }
                        OperationKind::StoreStructuralField {
                            destination,
                            path,
                            field,
                            value,
                            ..
                        } => {
                            let value_type = crate::runtime_structural_path_tip(
                                module.structural_types.iter(),
                                *graph.place_types.get(&value.place)?,
                                &value.path,
                            )?
                            .id;
                            graph.contents_transfers.insert(
                                position,
                                descriptor_contents_transfers(
                                    &module.structural_types,
                                    slot_kind,
                                    DescriptorContentsOperand {
                                        root: value.place,
                                        root_type: *graph.place_types.get(&value.place)?,
                                        path: &value.path,
                                        field: None,
                                    },
                                    DescriptorContentsOperand {
                                        root: *destination,
                                        root_type: *graph.place_types.get(destination)?,
                                        path,
                                        field: *field,
                                    },
                                    value_type,
                                    false,
                                )?,
                            );
                            if let (Some(destination), Some(source)) = (
                                location(*destination, path, *field),
                                location(value.place, &value.path, None),
                            ) {
                                step.definitions.push(destination.clone());
                                step.derivations.push((source, destination.clone()));
                                if destination.path.is_empty()
                                    && !is_descriptor(
                                        module.structural_types.iter(),
                                        *graph.place_types.get(&destination.root)?,
                                    )
                                    && !slot_kind.is_slot(
                                        &module.structural_types,
                                        *graph.place_types.get(&destination.root)?,
                                    )
                                {
                                    graph.originations.entry(position).or_default().push((
                                        destination.clone(),
                                        DescriptorCallOrigin::Argument {
                                            source: destination.root,
                                            projection: Projection::new(&[]),
                                        },
                                    ));
                                }
                            } else {
                                step.definitions.extend(slot_kind.overwritten_slots(
                                    &module.structural_types,
                                    *destination,
                                    *graph.place_types.get(destination)?,
                                    path,
                                    *field,
                                )?);
                                step.may_escape = true;
                            }
                        }
                        OperationKind::StructuralByteViewFieldStore {
                            destination,
                            path,
                            field,
                            source,
                            ..
                        } => {
                            let root_type = *graph.place_types.get(destination)?;
                            super::descriptor_leaf_store(
                                super::DescriptorStoreFlow {
                                    position,
                                    transfers: &mut graph.contents_transfers,
                                    unresolved: &mut graph.unresolved_contents_writes,
                                },
                                &mut step,
                                &module.structural_types,
                                slot_kind,
                                super::DescriptorContentsOperand {
                                    root: *destination,
                                    root_type,
                                    path,
                                    field: *field,
                                },
                                *source,
                                selector,
                            )?;
                        }
                        OperationKind::EstablishPrimitiveLocal { value, .. } => {
                            if let Some(value) = scalar_expressions.expression(*value) {
                                graph
                                    .scalar_payload_writes
                                    .entry(position)
                                    .or_default()
                                    .push((location(result?.place, &[], None)?, value));
                            }
                        }
                        OperationKind::EstablishScalarArray { elements } => {
                            let result = result?;
                            for (ordinal, value) in elements.iter().enumerate() {
                                let path = crate::scalar_array_leaf_path(
                                    module.structural_types.iter(),
                                    result.structural_type,
                                    u64::try_from(ordinal).ok()?,
                                )?;
                                if let Some(value) = scalar_expressions.expression(*value) {
                                    graph
                                        .scalar_payload_writes
                                        .entry(position)
                                        .or_default()
                                        .push((location(result.place, &path, None)?, value));
                                }
                            }
                        }
                        OperationKind::EstablishStructuralArray { elements } => {
                            let result = result?;
                            let (element, stored) =
                                match module.structural_type(result.structural_type)?.shape {
                                    StructuralTypeShape::ElementView {
                                        element,
                                        access: None | Some(StructuralAccess::SharedBorrow),
                                    } => (element, true),
                                    StructuralTypeShape::FixedArray {
                                        element,
                                        length: terminal_psi::ArrayExtent::Literal(length),
                                    } if usize::try_from(length).ok() == Some(elements.len()) => {
                                        (element, false)
                                    }
                                    _ => return None,
                                };
                            if stored {
                                // These ordinals belong to this constructor's
                                // real operand roster. They are not unchecked
                                // source indexes under a view descriptor.
                                graph.originations.entry(position).or_default().push((
                                    DescriptorLocation::whole(result.place),
                                    DescriptorCallOrigin::Argument {
                                        source: result.place,
                                        projection: Projection::new(&[]),
                                    },
                                ));
                            }
                            for (index, source) in elements.iter().enumerate() {
                                if source.access != StructuralAccess::Owned
                                    || source.place == result.place
                                    || crate::runtime_structural_path_tip(
                                        module.structural_types.iter(),
                                        *graph.place_types.get(&source.place)?,
                                        &source.path,
                                    )?
                                    .id != element
                                {
                                    return None;
                                }
                                let index = u64::try_from(index).ok()?;
                                let destination = if stored {
                                    DescriptorLocation {
                                        root: result.place,
                                        path: vec![DescriptorPathSegment::FixedIndex(index)],
                                    }
                                } else {
                                    location(
                                        result.place,
                                        &[StructuralPathSegment::FixedIndex(index)],
                                        None,
                                    )?
                                };
                                let source = location(source.place, &source.path, None)?;
                                step.derivations.push((source, destination));
                            }
                        }
                        OperationKind::ElementViewRead {
                            source,
                            index,
                            obligation,
                            path,
                            ..
                        } => {
                            if let Some(result) = result {
                                let mut selected = source.path.clone();
                                selected.push(StructuralPathSegment::RuntimeIndex {
                                    index: *index,
                                    obligation: *obligation,
                                });
                                selected.extend_from_slice(path);
                                graph.contents_transfers.insert(
                                    position,
                                    descriptor_contents_transfers(
                                        &module.structural_types,
                                        slot_kind,
                                        DescriptorContentsOperand {
                                            root: source.place,
                                            root_type: *graph.place_types.get(&source.place)?,
                                            path: &selected,
                                            field: None,
                                        },
                                        DescriptorContentsOperand {
                                            root: result.place,
                                            root_type: result.structural_type,
                                            path: &[],
                                            field: None,
                                        },
                                        result.structural_type,
                                        false,
                                    )?,
                                );
                            } else if operation.result.scalar_ref().is_none() {
                                return None;
                            }
                        }
                        OperationKind::EstablishRecord { fields }
                        | OperationKind::EstablishStructuralCase { fields, .. } => {
                            if let OperationKind::EstablishStructuralCase { result_case, .. } =
                                &operation.kind
                            {
                                graph.case_constructors.insert(
                                    position,
                                    vec![DescriptorCasePredicate {
                                        subject: DescriptorLocation::whole(result?.place),
                                        case: *result_case,
                                    }],
                                );
                                let result_type = result?.structural_type;
                                let shape = &module.structural_type(result_type)?.shape;
                                let cases = match shape {
                                    StructuralTypeShape::Sum { cases }
                                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                                    _ => return None,
                                };
                                terminal_psi::find_structural_case(cases, *result_case)?;
                                for slot in slot_kind
                                    .inventory(
                                        &module.structural_types,
                                        result?.place,
                                        result?.structural_type,
                                    )?
                                    .slots
                                {
                                    if matches!(slot.path.first(), Some(crate::descriptors::descriptor_lifetimes::DescriptorPathSegment::Case(case)) if case != result_case)
                                    {
                                        graph
                                            .originations
                                            .entry(position)
                                            .or_default()
                                            .push((slot, DescriptorCallOrigin::Empty));
                                    }
                                }
                            }
                            for (field_ordinal, field) in fields.iter().enumerate() {
                                let destination = match &operation.kind {
                                    OperationKind::EstablishStructuralCase {
                                        result_case, ..
                                    } => {
                                        let shape =
                                            &module.structural_type(result?.structural_type)?.shape;
                                        let path = terminal_psi::structural_case_initializer_path(
                                            shape,
                                            *result_case,
                                            field_ordinal,
                                            field.field,
                                        )?;
                                        location(result?.place, &path, None)?
                                    }
                                    _ => location(result?.place, &[], Some(field.field))?,
                                };
                                if let RecordFieldValue::Scalar { value, .. } = &field.value
                                    && let Some(value) = scalar_expressions.expression(*value)
                                {
                                    graph
                                        .scalar_payload_writes
                                        .entry(position)
                                        .or_default()
                                        .push((destination.clone(), value));
                                }
                                if let Some(source) = field.value.as_structural() {
                                    if let Some(source) = location(source.place, &source.path, None)
                                    {
                                        step.derivations.push((source, destination));
                                    } else {
                                        step.may_escape = true;
                                    }
                                }
                            }
                        }
                        OperationKind::CallMachine {
                            contract,
                            structural_arguments,
                            ..
                        } => {
                            let contract = module.callables.contract(*contract)?;
                            // A public callable supplies result origins and access ceilings.
                            // Its unknown body supplies no stored-content summary. Invalidate
                            // only writable argument paths, retaining unrelated storage.
                            let call = public_invocation(
                                operation.id,
                                &module.structural_types,
                                slot_kind,
                                contract,
                                Vec::new(),
                                structural_arguments,
                                result,
                                &graph.place_types,
                            )?;
                            for actual in structural_arguments
                                .iter()
                                .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                            {
                                step.definitions.extend(writable_descriptor_slots(
                                    &module.structural_types,
                                    &graph.place_types,
                                    actual,
                                    slot_kind,
                                )?);
                            }
                            if matches!(
                                call.target,
                                DescriptorInvocationTarget::Public {
                                    invalidates_contents: true,
                                    ..
                                }
                            ) {
                                graph.unresolved_contents_writes.insert(position);
                                step.may_escape = true;
                            }
                            insert_invocation(&mut graph.calls, position, call)?;
                        }
                        OperationKind::CallDynamic {
                            selector,
                            interface,
                            slot,
                            structural_arguments,
                            ..
                        } => {
                            let effects_machine = selected_effects_machine(
                                module,
                                machine,
                                selector_graph,
                                *selector,
                                *interface,
                                *slot,
                            );
                            let realizations =
                                public_realizations(&module.dynamic_dispatch, *interface, *slot)?;
                            let slot = module
                                .dynamic_dispatch
                                .interface(*interface)?
                                .slots
                                .get(usize::try_from(*slot).ok()?)?;
                            let mut call = public_invocation(
                                operation.id,
                                &module.structural_types,
                                slot_kind,
                                slot,
                                realizations.clone(),
                                structural_arguments,
                                result,
                                &graph.place_types,
                            )?;
                            let complete_contents =
                                call.argument_contents.iter().all(Option::is_some);
                            let effects_machine = effects_machine.filter(|_| complete_contents);
                            if let DescriptorInvocationTarget::Public {
                                effects_machine: target,
                                ..
                            } = &mut call.target
                            {
                                *target = effects_machine;
                            }
                            let has_stored_effects = complete_contents
                                && (effects_machine.is_some() || !realizations.is_empty());
                            if !has_stored_effects {
                                for actual in structural_arguments.iter().filter(|actual| {
                                    actual.access != StructuralAccess::SharedBorrow
                                }) {
                                    step.definitions.extend(writable_descriptor_slots(
                                        &module.structural_types,
                                        &graph.place_types,
                                        actual,
                                        slot_kind,
                                    )?);
                                }
                            }
                            if matches!(
                                call.target,
                                DescriptorInvocationTarget::Public {
                                    invalidates_contents: true,
                                    ..
                                }
                            ) && !has_stored_effects
                            {
                                graph.unresolved_contents_writes.insert(position);
                                step.may_escape = true;
                            }
                            insert_invocation(&mut graph.calls, position, call)?;
                        }
                        _ if direct_call.is_some() => {
                            let (callee, structural_arguments) = direct_call?;
                            let returns_reference = result.is_some_and(|result| {
                                terminal_psi::structural_type_contains_reference_by(
                                    |id| module.structural_type(id),
                                    result.structural_type,
                                )
                            });
                            // A selected call reads its public requirement;
                            // a rigid call reaches the bodiless requirement
                            // itself. Both read one declared lifetime account,
                            // and only a selected body supplies stored effects.
                            let public = if returns_reference {
                                crate::public_reference_result(
                                    module,
                                    machine.id,
                                    callee,
                                    operation.required_contract,
                                )
                                .inspect_err(|detail| *failure_detail = Some(*detail))
                                .ok()?
                            } else {
                                crate::requirement_reference_result(module, callee)
                                    .inspect_err(|detail| *failure_detail = Some(*detail))
                                    .ok()?
                            };
                            let implemented = operation.required_contract.is_some();
                            let public_call = match &public {
                                Some(public) => Some((
                                    public_invocation(
                                        operation.id,
                                        &module.structural_types,
                                        slot_kind,
                                        public,
                                        if implemented {
                                            vec![callee]
                                        } else {
                                            Vec::new()
                                        },
                                        structural_arguments,
                                        result,
                                        &graph.place_types,
                                    )?,
                                    implemented,
                                )),
                                None => None,
                            };
                            if let Some((mut call, implemented)) = public_call {
                                let complete = call.arguments.iter().all(Option::is_some)
                                    && call.argument_contents.iter().all(Option::is_some);
                                if !complete {
                                    mark_opaque_call_occupancy(
                                        &graph.place_types,
                                        &mut graph.unresolved_contents_writes,
                                        &mut step,
                                        slot_kind,
                                        &module.structural_types,
                                        position,
                                        structural_arguments,
                                    )?;
                                    if matches!(
                                        call.target,
                                        DescriptorInvocationTarget::Public {
                                            invalidates_contents: true,
                                            ..
                                        }
                                    ) {
                                        graph.unresolved_contents_writes.insert(position);
                                        step.may_escape = true;
                                    }
                                }
                                if let DescriptorInvocationTarget::Public {
                                    effects_machine, ..
                                } = &mut call.target
                                {
                                    *effects_machine = (complete && implemented).then_some(callee);
                                }
                                insert_invocation(&mut graph.calls, position, call)?;
                            } else {
                                let inputs = operation.module_invocation()?;
                                if inputs.callee != callee
                                    || inputs.structural_arguments != structural_arguments
                                {
                                    return None;
                                }
                                let call = graph.direct_invocation(
                                    module,
                                    machines,
                                    slot_kind,
                                    terminal_psi::ModuleInvocation {
                                        site: semantic_vocabulary::InvocationSite::Operation(
                                            operation.id,
                                        ),
                                        static_reach_binding: operation.static_reach_binding,
                                        inputs,
                                    },
                                    result,
                                    selector,
                                )?;
                                if call.arguments.iter().any(Option::is_none) {
                                    mark_opaque_call_occupancy(
                                        &graph.place_types,
                                        &mut graph.unresolved_contents_writes,
                                        &mut step,
                                        slot_kind,
                                        &module.structural_types,
                                        position,
                                        structural_arguments,
                                    )?;
                                }
                                insert_invocation(&mut graph.calls, position, call)?;
                            }
                        }
                        OperationKind::BoundaryCall {
                            boundary,
                            structural_arguments,
                            ..
                        } => {
                            if slot_kind == DescriptorSlotKind::AccessOrigins
                                && let Some(plan) =
                                    module.access_lifecycle_plans.iter().find(|plan| {
                                        plan.boundary == *boundary
                                        && plan.operation
                                            == terminal_psi::TerminalAccessLifecycleOperation::View
                                    })
                            {
                                let crate::AccessResultContract::View { ready, carrier, .. } =
                                    crate::access_result_contract(module, plan).ok()?
                                else {
                                    return None;
                                };
                                let result = result?;
                                let shape = &module.structural_type(result.structural_type)?.shape;
                                let path = terminal_psi::structural_case_initializer_path(
                                    shape,
                                    ready.id,
                                    0,
                                    ready.fields.first()?.id,
                                )?;
                                graph.originations.entry(position).or_default().push((
                                    location(result.place, &path, None)?,
                                    DescriptorCallOrigin::AccessFormation {
                                        machine: machine.id,
                                        operation: operation.id,
                                        place: result.place,
                                        carrier,
                                        projection: Projection::new(&[]),
                                    },
                                ));
                            }
                            let preserves_access_token = slot_kind == DescriptorSlotKind::AccessOrigins
                                && module.access_view_plans.iter().any(|plan| {
                                    module
                                        .access_view_plans
                                        .iter()
                                        .filter(|other| other.carrier == plan.carrier)
                                        .count()
                                        == 1
                                        && crate::access_ram_plan_correspondence(
                                            &module.structural_types,
                                            &module.access_view_plans,
                                            plan,
                                            plan.schema,
                                        )
                                        .is_ok()
                                        && plan.fields.iter().any(|field| {
                                            field.operations.iter().any(|accessor| {
                                                accessor.boundary == *boundary
                                                    && matches!(
                                                        accessor.kind,
                                                        terminal_psi::TerminalAccessorKind::Read
                                                            | terminal_psi::TerminalAccessorKind::Write
                                                    )
                                            })
                                        })
                                });
                            if !preserves_access_token
                                && !boundary_call_preserves_descriptor_storage(
                                    module, machine, operation,
                                )
                            {
                                mark_opaque_call_occupancy(
                                    &graph.place_types,
                                    &mut graph.unresolved_contents_writes,
                                    &mut step,
                                    slot_kind,
                                    &module.structural_types,
                                    position,
                                    structural_arguments,
                                )?;
                            }
                            let mut declarations = module
                                .boundary_machines
                                .iter()
                                .filter(|declaration| declaration.id == *boundary);
                            if let (Some(declaration), None) =
                                (declarations.next(), declarations.next())
                                && slot_kind != DescriptorSlotKind::AccessOrigins
                            {
                                insert_invocation(
                                    &mut graph.calls,
                                    position,
                                    provider_invocation(
                                        operation.id,
                                        &module.structural_types,
                                        slot_kind,
                                        declaration,
                                        structural_arguments,
                                        result,
                                        &graph.place_types,
                                    )?,
                                )?;
                            }
                        }
                        _ => {}
                    }
                    // Record unresolved invocations only after attempting exact
                    // construction; a later cohort cannot overwrite unknown effects.
                    if is_invocation && !graph.calls.contains_key(&position) {
                        graph.calls.insert(position, None);
                    }
                    record_owned_consumption(module, machine, operation, &mut step)?;
                    // Borrowing a carrier slot makes a child resource, not
                    // another spelling of the parent's permission. Payload
                    // and residence still follow the captured address.
                    if !matches!(
                        operation.kind,
                        OperationKind::EstablishReference { .. }
                            | OperationKind::EstablishAccessReference { .. }
                            | OperationKind::EstablishElementView { .. }
                            | OperationKind::EstablishMutableElementView { .. }
                    ) {
                        step.authority_derivations = step.derivations.clone();
                    }
                    step.successors.push(DescriptorFlowEdge {
                        edge: None,
                        effects: Vec::new(),
                        definitions: Vec::new(),
                        target: (block.id, position.1 + 1),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    });
                    if graph.flow.insert(position, step).is_some() {
                        return None;
                    }
                }
                *failure_position = None;
                *failure_successor = None;
                *failure_detail = None;
                let position = (block.id, u32::try_from(block.operations.len()).ok()?);
                *failure_position = Some(position);
                let mut step = DescriptorFlowStep::default();
                let mut edge = |identity,
                                target,
                                scalar_arrival: ScalarArrival<'_>,
                                arguments: &[StructuralArgument]|
                 -> Option<()> {
                    *failure_successor = Some((identity, target));
                    let result = (|| -> Result<(), &'static str> {
                        let block = exactly_one(machine.blocks.iter(), |block| block.id == target)
                            .ok_or("descriptor arrival has no unique target block")?;
                        if arguments.len() != block.structural_parameters.len() {
                            return Err(
                                "descriptor arrival structural arity differs from target parameters",
                            );
                        }
                        let scalar_bindings = match scalar_arrival {
                            ScalarArrival::Values(arguments) => {
                                if arguments.len() != block.parameters.len() {
                                    return Err(
                                        "descriptor arrival scalar arity differs from target parameters",
                                    );
                                }
                                block.parameters.iter().zip(arguments).map(|(parameter, actual)| {
                                    let value = scalar_expressions.expression(*actual)
                                        .ok_or("descriptor arrival scalar operand has no current typed expression")?;
                                    if value.scalar_type() != parameter.scalar_type {
                                        return Err("descriptor arrival scalar type differs from target parameter");
                                    }
                                    Ok((parameter.id, value))
                                }).collect::<Result<Vec<_>, _>>()?
                            }
                            ScalarArrival::CasePayload {
                                source,
                                path,
                                case,
                                fields,
                            } => {
                                if fields.len() != block.parameters.len() {
                                    return Err(
                                        "descriptor arrival case payload arity differs from target parameters",
                                    );
                                }
                                let root = *graph.place_types.get(&source).ok_or(
                                    "descriptor arrival case payload has no declared source type",
                                )?;
                                let captured = super::scalar_contracts::case_payload_scalar_types(
                                    &module.structural_types, root, path, case, fields,
                                ).ok_or("descriptor arrival case payload fields have no exact scalar types")?;
                                if captured
                                    .iter()
                                    .zip(&block.parameters)
                                    .any(|(actual, parameter)| *actual != parameter.scalar_type)
                                {
                                    return Err(
                                        "descriptor arrival case payload type differs from target parameter",
                                    );
                                }
                                Vec::new()
                            }
                        };
                        graph
                            .payload_scalar_bindings
                            .entry(position)
                            .or_default()
                            .push(scalar_bindings);
                        let parameters = block
                            .structural_parameters
                            .iter()
                            .map(|parameter| DescriptorLocation::whole(parameter.place))
                            .collect::<Vec<_>>();
                        let mut bindings = Vec::new();
                        let mut definitions = Vec::new();
                        for (argument, parameter) in
                            arguments.iter().zip(&block.structural_parameters)
                        {
                            if argument.access != parameter.access {
                                return Err(
                                    "descriptor arrival access differs from target parameter",
                                );
                            }
                            let source = if let Some(source) =
                                location(argument.place, &argument.path, None)
                            {
                                DescriptorFlowSource::Location(source)
                            } else {
                                step.may_escape = true;
                                DescriptorFlowSource::from_query(
                                    argument.place,
                                    descriptor_contents_query(
                                        &module.structural_types, argument.place,
                                        *graph.place_types.get(&argument.place)
                                            .ok_or("descriptor arrival structural source has no declared type")?,
                                        &argument.path, None,
                                    ),
                                )
                            };
                            step.inputs.push((Some(source.observation()), None));
                            if parameter.access == StructuralAccess::Owned
                                && parameter.multiplicity != StructuralMultiplicity::Unrestricted
                            {
                                definitions.push(source.observation());
                            }
                            let mut binding = DescriptorFlowBinding::new(
                                source,
                                DescriptorLocation::whole(parameter.place),
                                argument.access,
                                parameter.access,
                            )
                            .ok_or("descriptor arrival has no valid source-to-parameter binding")?;
                            if binding.source.exact_location().is_none()
                                && let Some(index) = argument
                                    .path
                                    .iter()
                                    .position(|step| step.runtime_index().is_some())
                            {
                                let descriptor = location(argument.place, &argument.path[..index], None)
                                    .ok_or("descriptor arrival indexed source has no exact descriptor prefix")?;
                                binding.captured_storage = Some((
                                    descriptor,
                                    Projection::new(&argument.path[index..]).at(position),
                                ));
                            }
                            binding.source_access = crate::structural_projection_authority(
                                machine, &module.structural_types, argument,
                            ).ok_or("descriptor arrival structural source has no exact projection authority")?;
                            bindings.push(binding);
                        }
                        step.successors.push(DescriptorFlowEdge {
                            edge: None,
                            effects: Vec::new(),
                            definitions,
                            target: (target, 0),
                            parameters,
                            bindings,
                        });
                        Ok(())
                    })();
                    if let Err(detail) = result {
                        *failure_detail = Some(detail);
                        return None;
                    }
                    *failure_successor = None;
                    Some(())
                };
                match &block.terminator {
                    Terminator::Jump {
                        edge: identity,
                        target,
                        arguments,
                        structural_arguments,
                        ..
                    } => edge(
                        *identity,
                        *target,
                        ScalarArrival::Values(arguments),
                        structural_arguments,
                    )?,
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        for successor in [when_true, when_false] {
                            edge(
                                successor.edge,
                                successor.target,
                                ScalarArrival::Values(&successor.arguments),
                                &successor.structural_arguments,
                            )?;
                        }
                    }
                    Terminator::StructuralCase {
                        source,
                        path,
                        cases,
                    } => {
                        for case in cases {
                            edge(
                                case.edge,
                                case.target,
                                ScalarArrival::CasePayload {
                                    source: *source,
                                    path,
                                    case: case.case,
                                    fields: &case.payload_fields,
                                },
                                &case.structural_arguments,
                            )?;
                        }
                    }
                    Terminator::ReturnStructural { source, .. } => {
                        if let Some(returns) = &mut graph.returns {
                            *failure_detail =
                                Some("descriptor structural return source has no declared type");
                            let actual = machine.structural_place_type(*source)?;
                            *failure_detail = Some(
                                "descriptor structural return has no declared structural result",
                            );
                            let expected = machine.result.structural()?.structural_type;
                            if actual != expected {
                                *failure_detail = Some(
                                    "descriptor structural return source type differs from declared result",
                                );
                                return None;
                            }
                            returns.push((position, DescriptorLocation::whole(*source)));
                        }
                        step.inputs
                            .push((Some(DescriptorLocation::whole(*source)), None));
                        step.may_escape = true;
                        graph.normal_exits.push(position);
                    }
                    Terminator::Crash { .. } | Terminator::EliminateEmpty { .. } => {}
                    Terminator::ExternalCompletion {
                        structural_arguments,
                        ..
                    }
                    | Terminator::InvokeNoNormal {
                        structural_arguments,
                        ..
                    } => {
                        let call = block
                            .terminator
                            .module_invocation()
                            .filter(|inputs| inputs.required_contract.is_none())
                            .and_then(|inputs| {
                                graph.direct_invocation(
                                    module,
                                    machines,
                                    slot_kind,
                                    terminal_psi::ModuleInvocation {
                                        site: semantic_vocabulary::InvocationSite::InvokeNoNormal(
                                            block.terminator.edge(),
                                        ),
                                        static_reach_binding: match block.terminator {
                                            Terminator::InvokeNoNormal {
                                                static_reach_binding,
                                                ..
                                            } => static_reach_binding,
                                            _ => None,
                                        },
                                        inputs,
                                    },
                                    None,
                                    selector,
                                )
                            });
                        if call
                            .as_ref()
                            .is_none_or(|call| !call.arguments.iter().all(Option::is_some))
                        {
                            // An absent result never proves that a call writes
                            // nothing. Unknown and public calls keep the same
                            // conservative invalidation until their owner serves them.
                            graph.unresolved_contents_writes.insert(position);
                        }
                        match call {
                            Some(call) => {
                                *failure_detail = Some(
                                    "descriptor terminal invocation conflicts with an existing call at this site",
                                );
                                insert_invocation(&mut graph.calls, position, call)?;
                            }
                            None => {
                                graph.calls.insert(position, None);
                            }
                        }
                        for argument in structural_arguments {
                            if let Some(source) = location(argument.place, &argument.path, None) {
                                step.inputs.push((Some(source), None));
                            } else {
                                step.inputs
                                    .push((Some(DescriptorLocation::whole(argument.place)), None));
                            }
                        }
                        step.may_escape = true;
                    }
                    _ => {
                        graph.normal_exits.push(position);
                    }
                }
                for (edge, actions) in block.terminator.cleanup_rosters() {
                    for (action_ordinal, action) in actions.iter().enumerate() {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) =
                            action
                        {
                            let callee = crate::nominal_cleanup::owned_target(
                                module.structural_types.iter(),
                                module.machines.iter(),
                                cleanup,
                            )
                            .inspect_err(|reason| *failure_detail = Some(*reason))
                            .ok()?;
                            *failure_detail = Some(
                                "descriptor terminal cleanup subject has no declared root type",
                            );
                            let root = *graph.place_types.get(&cleanup.subject.place)?;
                            *failure_detail = Some(
                                "descriptor terminal cleanup subject has no exact projected type",
                            );
                            if crate::runtime_structural_path_tip(
                                module.structural_types.iter(),
                                root,
                                &cleanup.subject.path,
                            )?
                            .id != cleanup.structural_type
                            {
                                *failure_detail = Some(
                                    "descriptor terminal cleanup subject type differs from its declared carrier",
                                );
                                return None;
                            }
                            *failure_detail =
                                Some("descriptor terminal cleanup action ordinal is out of range");
                            let action_ordinal = u32::try_from(action_ordinal).ok()?;
                            *failure_detail = Some(
                                "descriptor terminal cleanup has no exact invocation correspondence",
                            );
                            let call = machine_invocation(
                                DescriptorInvocationOwner::Cleanup {
                                    edge,
                                    action_ordinal,
                                },
                                &module.structural_types,
                                slot_kind,
                                callee,
                                std::slice::from_ref(&cleanup.subject),
                                None,
                                std::iter::once(Some(root)),
                            )?;
                            // A known cleanup requires the same exact actual
                            // correspondence as a source call. An opaque frame
                            // cannot replace its retained executable effects.
                            if call.arguments.iter().any(Option::is_none)
                                || call.argument_contents.iter().any(Option::is_none)
                            {
                                *failure_detail = Some(
                                    "descriptor terminal cleanup has unresolved actual backing or contents",
                                );
                                return None;
                            }
                            step.inputs.extend(
                                call.arguments
                                    .iter()
                                    .flatten()
                                    .cloned()
                                    .map(|input| (Some(input), None)),
                            );
                            *failure_detail = Some(
                                "descriptor terminal cleanup conflicts with an existing call at this site",
                            );
                            insert_invocation(&mut graph.calls, position, call)?;
                        }
                    }
                }
                // Selected-edge disposal is a real carrier disposition.
                // Keep its definitions apart from the destination roster:
                // every binding reads the old frontier before both are killed.
                let selected = block.terminator.successor_edges();
                if selected.len() != step.successors.len() {
                    *failure_detail = Some(
                        "descriptor terminator successor count differs from reconstructed arrivals",
                    );
                    return None;
                }
                for ((identity, target), edge) in selected.into_iter().zip(&mut step.successors) {
                    *failure_successor = Some((identity, target));
                    if edge.target != (target, 0) {
                        *failure_detail = Some(
                            "descriptor terminator successor target differs from reconstructed arrival",
                        );
                        return None;
                    }
                    edge.edge = Some(identity);
                    *failure_detail =
                        Some("descriptor terminator successor has no exact cleanup roster");
                    let (_, cleanup) = block
                        .terminator
                        .cleanup_rosters()
                        .find(|(at, _)| *at == identity)?;
                    for action in cleanup {
                        *failure_detail =
                            Some("descriptor terminator cleanup has no exact disposal location");
                        edge.definitions
                            .push(location(action.place(), action.path(), None)?);
                    }
                    edge.definitions.sort();
                    edge.definitions.dedup();
                    *failure_successor = None;
                }
                if let Terminator::StructuralCase {
                    source,
                    path,
                    cases,
                } = &block.terminator
                {
                    *failure_detail =
                        Some("descriptor case terminator has no exact subject location");
                    let subject = location(*source, path, None)?;
                    graph.case_successors.insert(
                        position,
                        cases
                            .iter()
                            .map(|case| {
                                (
                                    DescriptorCasePredicate {
                                        subject: subject.clone(),
                                        case: case.case,
                                    },
                                    true,
                                )
                            })
                            .collect(),
                    );
                }
                if let Terminator::Conditional { condition, .. } = &block.terminator
                    && let Some(term) = scalar_expressions.expression(*condition)
                {
                    graph.payload_edge_predicates.insert(
                        position,
                        [true, false]
                            .into_iter()
                            .map(|polarity| {
                                Some(semantic_vocabulary::Proposition::Equal(
                                    ScalarTerm::Boolean(polarity),
                                    term.clone(),
                                ))
                            })
                            .collect(),
                    );
                }
                if graph.normal_exits.contains(&position) {
                    for parameter in &machine.structural_parameters {
                        if matches!(
                            parameter.access,
                            terminal_psi::exclusive_borrow_pair_pattern!()
                        ) {
                            *failure_detail = Some(
                                "descriptor normal exit has no complete borrowed parameter slots",
                            );
                            step.inputs.extend(
                                slot_kind
                                    .slots(
                                        &module.structural_types,
                                        parameter.place,
                                        parameter.structural_type,
                                    )?
                                    .into_iter()
                                    .map(|input| (Some(input), None)),
                            );
                        }
                    }
                }
                if graph.flow.insert(position, step).is_some() {
                    *failure_detail =
                        Some("descriptor terminator repeats an existing storage-flow position");
                    return None;
                }
            }
            Some(graph)
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum CapturedBacking {
            StorageAddress,
            DescriptorPayload,
            AccessInterpretation,
            ByteWindow,
        }

        fn captured_storage_source(
            types: &[StructuralTypeDeclaration],
            source: &StructuralArgument,
            root_type: StructuralTypeId,
            position: DescriptorPosition,
        ) -> Option<(DescriptorLocation, Option<Projection>, CapturedBacking)> {
            let prefix_length = source
                .path
                .iter()
                .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
                .unwrap_or(source.path.len());
            let location = descriptor_location(
                types,
                source.place,
                root_type,
                &source.path[..prefix_length],
                None,
            )?;
            if prefix_length == source.path.len() {
                return Some((location, None, CapturedBacking::StorageAddress));
            }
            let prefix_type = descriptor_location_type(types, root_type, &location.path)?;
            let suffix = &source.path[prefix_length..];
            let prefix_shape = &find_structural_type(types, prefix_type)?.shape;
            let capture = if matches!(prefix_shape, StructuralTypeShape::ByteSequence(_)) {
                CapturedBacking::ByteWindow
            } else if plain_owned_shape(types.iter(), prefix_type) {
                CapturedBacking::StorageAddress
            } else if is_descriptor(types.iter(), prefix_type) {
                CapturedBacking::DescriptorPayload
            } else if crate::access::access_view_plans::is_access_projection_shape(
                types,
                prefix_type,
            ) {
                CapturedBacking::AccessInterpretation
            } else {
                return None;
            };
            if matches!(capture, CapturedBacking::ByteWindow)
                && suffix.iter().any(|segment| {
                    !matches!(
                        segment,
                        StructuralPathSegment::RuntimeIndex { .. }
                            | StructuralPathSegment::FixedIndex(_)
                    )
                })
            {
                return None;
            }
            if crate::runtime_structural_path_tip(types.iter(), prefix_type, suffix).is_none()
                && crate::primitive_projection_type(types.iter(), prefix_type, suffix).is_none()
            {
                return None;
            }
            Some((
                location,
                Some(Projection::new(suffix).at(position)),
                capture,
            ))
        }
        fn destination_root(kind: &OperationKind) -> Option<&PlaceId> {
            match kind {
                OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
                    Some(destination)
                }
                _ => None,
            }
        }

        fn mark_opaque_call_occupancy(
            place_types: &BTreeMap<PlaceId, StructuralTypeId>,
            unresolved_contents_writes: &mut BTreeSet<DescriptorPosition>,
            step: &mut DescriptorFlowStep,
            slot_kind: DescriptorSlotKind,
            types: &[StructuralTypeDeclaration],
            position: DescriptorPosition,
            actuals: &[StructuralArgument],
        ) -> Option<()> {
            step.may_escape = true;
            if actuals.iter().any(|actual| {
                place_types.get(&actual.place).is_none_or(|root_type| {
                    slot_kind.may_write_indirect_contents(types, *root_type, actual)
                })
            }) {
                unresolved_contents_writes.insert(position);
            }
            for actual in actuals
                .iter()
                .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
            {
                step.definitions.extend(slot_kind.slots(
                    types,
                    actual.place,
                    *place_types.get(&actual.place)?,
                )?);
            }
            Some(())
        }

        fn selected_effects_machine(
            module: &TerminalModule,
            machine: &TerminalMachine,
            selector_graph: Option<&[DescriptorMachine]>,
            selector: semantic_vocabulary::ValueId,
            interface: u32,
            slot: u32,
        ) -> Option<semantic_vocabulary::MachineId> {
            #[derive(Clone, PartialEq, Eq)]
            struct SelectedTable(u32);
            let table = crate::unchanged_scalar_identity(selector, |value| {
                let mut definitions = machine.operations().filter(|operation| {
                    operation
                        .result
                        .scalar_ref()
                        .is_some_and(|result| result.id == value)
                });
                if let Some(operation) = definitions.next() {
                    if definitions.next().is_some() {
                        return None;
                    }
                    if let OperationKind::SelectImplementation { table } = operation.kind {
                        return Some(crate::UnchangedScalarOrigin::Known(SelectedTable(table)));
                    }
                    let selector_machine =
                        selector_graph?.iter().find(|row| row.id == machine.id)?;
                    let sources = super::unchanged_stored_scalar_sources(
                        &module.structural_types,
                        selector_graph?,
                        machine.id,
                        value,
                        |(block, index)| {
                            let Some(operation) = machine
                                .blocks
                                .iter()
                                .find(|row| row.id == block)
                                .and_then(|block| block.operations.get(index as usize))
                            else {
                                return Vec::new();
                            };
                            super::scalar_storage_writes(
                                module,
                                |root| selector_machine.place_types.get(&root).copied(),
                                operation,
                            )
                            .and_then(Result::ok)
                            .unwrap_or_default()
                        },
                    )?;
                    return Some(crate::UnchangedScalarOrigin::Incoming(sources));
                }
                let mut owners = machine.blocks.iter().filter_map(|block| {
                    block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == value)
                        .map(|ordinal| (block.id, ordinal))
                });
                let (owner, ordinal) = owners.next()?;
                if owners.next().is_some() || owner == machine.entry {
                    return None;
                }
                let mut incoming = Vec::new();
                for block in &machine.blocks {
                    if crate::control_graph::reaches_by_case(block, owner) {
                        return None;
                    }
                    for binding in crate::control_graph::forwarded_bindings(block) {
                        if binding.target == owner {
                            incoming.push(*binding.arguments?.get(ordinal)?);
                        }
                    }
                }
                Some(crate::UnchangedScalarOrigin::Incoming(incoming))
            })?
            .0;
            let target = super::selected_stored_effects_row(
                &module.dynamic_dispatch,
                table,
                interface,
                slot,
            )?;
            let signature = module
                .dynamic_dispatch
                .interface(interface)?
                .slots
                .get(usize::try_from(slot).ok()?)?;
            let mut bodies = module.machines.iter().filter(|body| body.id == target);
            let body = bodies.next()?;
            if bodies.next().is_some()
                || matches!(signature.result, BoundaryMachineResult::Structural(_))
                || matches!(&signature.result, BoundaryMachineResult::Scalar(result)
                    if result.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO)
                || !signature.result.matches_machine_result(&body.result)
                || body.parameters.len() != signature.scalar_parameters.len()
                || !body
                    .parameters
                    .iter()
                    .zip(&signature.scalar_parameters)
                    .all(|(actual, expected)| actual.scalar_type == *expected)
                || body.structural_parameters.len() != signature.structural_parameters.len()
                || !body
                    .structural_parameters
                    .iter()
                    .zip(&signature.structural_parameters)
                    .all(|(actual, expected)| {
                        actual.position == expected.position
                            && actual.structural_type == expected.structural_type
                            && actual.access == expected.access
                            && actual.multiplicity == expected.multiplicity
                    })
            {
                return None;
            }
            Some(target)
        }
    }
    pub use invocation::{
        DescriptorMachineSignature, machine_invocation, machine_invocation_for_signature,
        public_invocation, public_realizations, public_result_slots, writable_descriptor_slots,
    };
    pub use replay::{
        DescriptorAnalysis, DescriptorSlotInventory, DescriptorSlotKind,
        DescriptorSourceObservation, DescriptorSourceObservations, DescriptorStoredCallEffect,
        DescriptorStoredEffect, DescriptorStoredOrigin, descriptor_slot_inventory,
        descriptor_slots, reconstruct_source_origins, reconstruct_stored_effects,
    };

    #[derive(Clone)]
    pub enum DescriptorResultOrigin {
        Invocation {
            carrier_type: StructuralTypeId,
            projection: Projection,
        },
        Empty,
        Parameter {
            position: usize,
            projection: Projection,
        },
        Literal {
            machine: MachineId,
            place: PlaceId,
        },
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum DescriptorCallOrigin {
        Invocation {
            carrier_type: StructuralTypeId,
            projection: Projection,
        },
        AccessFormation {
            machine: MachineId,
            operation: semantic_vocabulary::OperationId,
            place: PlaceId,
            carrier: StructuralTypeId,
            projection: Projection,
        },
        Empty,
        Argument {
            source: PlaceId,
            projection: Projection,
        },
        Literal {
            machine: MachineId,
            place: PlaceId,
        },
    }

    pub fn native_code_layout_regions(
        types: &[StructuralTypeDeclaration],
        result: &terminal_psi::StructuralOperationResult,
    ) -> Option<DescriptorLocation> {
        let StructuralTypeShape::Record { fields, .. } =
            &terminal_psi::find_structural_type(types, result.structural_type)?.shape
        else {
            return None;
        };
        if fields.len() != 2 {
            return None;
        }
        let terminal_psi::StructuralFieldType::Structural(view) = fields[1].field_type else {
            return None;
        };
        let StructuralTypeShape::ElementView { access, .. } =
            terminal_psi::find_structural_type(types, view)?.shape
        else {
            return None;
        };
        if access.is_some_and(|access| access != StructuralAccess::SharedBorrow) {
            return None;
        }
        Some(DescriptorLocation {
            root: result.place,
            path: vec![DescriptorPathSegment::Field(fields[1].id)],
        })
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum DescriptorSourceOrigin {
        Provider(DescriptorProviderOrigin<DescriptorSourceOrigin>),
        Backing(DescriptorCallOrigin),
        IncomingSlot(DescriptorLocation),
        IncomingContents {
            query: DescriptorContentsQuery,
            projection: Projection,
        },
    }

    impl DescriptorSourceOrigin {
        pub fn captured_entry_slot(&self) -> Option<std::borrow::Cow<'_, DescriptorLocation>> {
            match self {
                Self::IncomingSlot(slot) => Some(std::borrow::Cow::Borrowed(slot)),
                Self::IncomingContents { query, projection }
                    if *projection == Projection::new(&[]) =>
                {
                    Some(std::borrow::Cow::Owned(query.exact_location()?))
                }
                _ => None,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum DescriptorProviderOutput {
        /// An exact row in the boundary's `reference_outputs` table.
        Stored(u32),
        /// A canonical borrowed leaf in the declared result frontier.
        Result(Vec<StructuralPathSegment>),
    }

    impl DescriptorProviderOutput {
        pub fn stored_output(&self) -> Option<u32> {
            match self {
                Self::Stored(index) => Some(*index),
                Self::Result(_) => None,
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DescriptorProviderOrigin<Dependency> {
        pub boundary: semantic_vocabulary::BoundaryMachineId,
        pub output: DescriptorProviderOutput,
        pub invocations: Vec<DescriptorInvocationSite>,
        pub projection: Projection,
        pub lifetime_dependencies: Vec<DescriptorLifetimeDebt<Dependency>>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DescriptorLifetimeDebt<Origin> {
        pub origin: Origin,
        pub access: StructuralAccess,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DescriptorInvocationSite {
        pub machine: MachineId,
        pub owner: DescriptorInvocationOwner,
    }

    pub use terminal_psi::InvocationOwner as DescriptorInvocationOwner;

    pub fn is_descriptor<'a>(
        types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
    ) -> bool {
        let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
        let Some(declaration) = declarations.next() else {
            return false;
        };
        declarations.next().is_none() && declaration.shape.is_view_descriptor()
    }

    pub fn descriptor_value_access<'a>(
        types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> Option<StructuralAccess> {
        let declaration = exactly_one(types.into_iter(), |item| item.id == structural_type)?;
        if multiplicity == StructuralMultiplicity::Linear {
            return None;
        }
        let access = match declaration.shape {
            StructuralTypeShape::ElementView { access, .. }
            | StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView { access }
                | ByteSequenceCarrier::BoundedView { access, .. },
            ) => access,
            _ => return None,
        };
        let access = access.unwrap_or(if multiplicity == StructuralMultiplicity::Affine {
            StructuralAccess::MutableBorrow
        } else {
            StructuralAccess::SharedBorrow
        });
        match access {
            StructuralAccess::SharedBorrow => Some(access),
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                if multiplicity == StructuralMultiplicity::Affine =>
            {
                Some(access)
            }
            _ => None,
        }
    }

    pub fn is_shared_descriptor<'a>(
        types: impl IntoIterator<Item = &'a StructuralTypeDeclaration>,
        structural_type: StructuralTypeId,
    ) -> bool {
        let mut declarations = types.into_iter().filter(|item| item.id == structural_type);
        let Some(declaration) = declarations.next() else {
            return false;
        };
        declarations.next().is_none()
            && (declaration.shape.is_shared_readable_byte_view()
                || matches!(
                    declaration.shape,
                    StructuralTypeShape::ElementView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                        ..
                    }
                ))
    }

    pub fn result_access<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Option<StructuralAccess> {
        descriptor_value_access(types, result.structural_type, result.multiplicity)
            .filter(|access| matches!(access, terminal_psi::reading_borrow_pair_pattern!()))
    }

    pub fn result_origins(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Option<Vec<DescriptorResultOrigin>> {
        let result = machine.result.structural()?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
        {
            return None;
        }
        reconstruct_result_origins(
            &module.structural_types,
            &terminal_graph(module),
            machine.id,
        )
    }

    pub fn return_origins(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> Option<Vec<DescriptorResultOrigin>> {
        let result = machine.result.structural()?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
        {
            return None;
        }
        let graph = terminal_graph(module);
        let row = descriptor_machine(&graph, machine.id)?;
        let mut origins = Vec::new();
        for (position, location) in row
            .returns
            .as_ref()?
            .iter()
            .filter(|(_, location)| location.root == source && location.path.is_empty())
        {
            origins.extend(replay::return_origins(
                &module.structural_types,
                &graph,
                machine.id,
                *position,
                location,
            )?);
        }
        (!origins.is_empty()).then_some(origins)
    }

    pub fn reconstruct_return_slots_external(
        types: &[StructuralTypeDeclaration],
        machines: &[DescriptorMachine],
        identity: MachineId,
        position: DescriptorPosition,
        source: PlaceId,
    ) -> bool {
        let Some(machine) = descriptor_machine(machines, identity) else {
            return false;
        };
        if !machine.returns.as_ref().is_some_and(|returns| {
            returns.iter().any(|(returned_position, location)| {
                *returned_position == position && *location == DescriptorLocation::whole(source)
            })
        }) {
            return false;
        }
        replay::return_slots_have_external_backing(types, machines, identity, position, source)
            .is_some()
    }

    pub fn return_slots_have_external_backing(
        module: &TerminalModule,
        machine: &TerminalMachine,
        source: PlaceId,
    ) -> bool {
        let graph = terminal_graph(module);
        let Some(row) = descriptor_machine(&graph, machine.id) else {
            return false;
        };
        let Some(returns) = &row.returns else {
            return false;
        };
        let mut found = false;
        for (position, location) in returns
            .iter()
            .filter(|(_, location)| location.root == source && location.path.is_empty())
        {
            found = true;
            if replay::return_slots_have_external_backing(
                &module.structural_types,
                &graph,
                machine.id,
                *position,
                location.root,
            )
            .is_none()
            {
                return false;
            }
        }
        found
    }

    pub fn call_origins(
        module: &TerminalModule,
        caller: &TerminalMachine,
        operation: &Operation,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        let graph = terminal_graph(module);
        call_origins_in(
            &mut DescriptorAnalysis::new(&module.structural_types, &graph),
            module,
            caller,
            operation,
        )
    }

    pub fn call_origins_in(
        analysis: &mut DescriptorAnalysis<'_>,
        module: &TerminalModule,
        caller: &TerminalMachine,
        operation: &Operation,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        if !matches!(
            operation.kind,
            terminal_psi::call_structural_pair_pattern!(..) | OperationKind::CallDynamic { .. }
        ) {
            return None;
        }
        let result = operation.result.structural()?;
        if result.multiplicity != StructuralMultiplicity::Unrestricted
            || !is_shared_descriptor(module.structural_types.iter(), result.structural_type)
        {
            return None;
        }

        analysis.call_origins(caller.id, result.place)
    }

    pub struct DescriptorMachine {
        pub id: MachineId,
        pub returns: Option<Vec<(DescriptorPosition, DescriptorLocation)>>,
        /// Sorted by place identity with no duplicate declarations.
        pub places: Vec<DescriptorPlace>,
        pub place_types: BTreeMap<PlaceId, StructuralTypeId>,
        pub entry: DescriptorPosition,
        pub flow: BTreeMap<DescriptorPosition, DescriptorFlowStep>,
        pub case_constructors: BTreeMap<DescriptorPosition, Vec<DescriptorCasePredicate>>,
        pub case_successors: BTreeMap<DescriptorPosition, Vec<(DescriptorCasePredicate, bool)>>,
        pub originations:
            BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, DescriptorCallOrigin)>>,
        pub projections:
            BTreeMap<(DescriptorPosition, DescriptorLocation, DescriptorLocation), Projection>,
        pub contents_queries: BTreeMap<
            (DescriptorPosition, DescriptorLocation, DescriptorLocation),
            DescriptorContentsQuery,
        >,
        pub contents_transfers: BTreeMap<DescriptorPosition, Vec<DescriptorContentsTransfer>>,
        pub payload_producers: BTreeMap<DescriptorPosition, PlaceId>,
        pub payload_entry_predicates: Option<(PropositionContext, Vec<Proposition>)>,
        pub payload_scalar_definitions: BTreeMap<DescriptorPosition, Vec<ValueId>>,
        pub payload_scalar_postconditions: BTreeMap<DescriptorPosition, Vec<Proposition>>,
        pub payload_scalar_parameters: BTreeMap<semantic_vocabulary::BlockId, Vec<ValueId>>,
        pub scalar_payload_reads:
            BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, ScalarTerm)>>,
        pub payload_edge_predicates: BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
        pub payload_scalar_bindings: BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
        pub scalar_payload_writes:
            BTreeMap<DescriptorPosition, Vec<(DescriptorLocation, ScalarTerm)>>,
        pub payload_invalidations:
            BTreeMap<DescriptorPosition, Vec<Option<DescriptorContentsQuery>>>,
        pub unresolved_contents_writes: BTreeSet<DescriptorPosition>,
        pub storage_derivations:
            BTreeSet<(DescriptorPosition, DescriptorLocation, DescriptorLocation)>,
        /// Missing is not a call; None is a real invocation whose effects
        /// cannot be represented. Known rows retain their exact ordered owners.
        pub calls: BTreeMap<DescriptorPosition, Option<Vec<DescriptorInvocation>>>,
        pub normal_exits: Vec<DescriptorPosition>,
    }

    pub fn insert_invocation(
        calls: &mut BTreeMap<DescriptorPosition, Option<Vec<DescriptorInvocation>>>,
        position: DescriptorPosition,
        call: DescriptorInvocation,
    ) -> Option<()> {
        let calls = calls
            .entry(position)
            .or_insert_with(|| Some(Vec::new()))
            .as_mut()?;
        if !calls.is_empty()
            && (matches!(
                call.owner,
                DescriptorInvocationOwner::Operation(_)
                    | DescriptorInvocationOwner::InvokeNoNormal(_)
            ) || matches!(
                calls[0].owner,
                DescriptorInvocationOwner::Operation(_)
                    | DescriptorInvocationOwner::InvokeNoNormal(_)
            ))
        {
            return None;
        }
        let insertion = calls
            .binary_search_by_key(&call.owner, |existing| existing.owner)
            .err()?;
        calls.insert(insertion, call);
        Some(())
    }

    impl DescriptorMachine {
        pub fn calls_at(&self, position: DescriptorPosition) -> &[DescriptorInvocation] {
            self.calls
                .get(&position)
                .and_then(Option::as_ref)
                .map_or(&[], Vec::as_slice)
        }

        /// One source call, including a call with no normal return. Cleanup
        /// actions have their own ordered owners and are queried separately.
        pub fn direct_call(&self, position: DescriptorPosition) -> Option<&DescriptorInvocation> {
            let [call] = self.calls_at(position) else {
                return None;
            };
            matches!(
                call.owner,
                DescriptorInvocationOwner::Operation(_)
                    | DescriptorInvocationOwner::InvokeNoNormal(_)
            )
            .then_some(call)
        }

        pub fn invocation(
            &self,
            position: DescriptorPosition,
            owner: DescriptorInvocationOwner,
        ) -> Option<&DescriptorInvocation> {
            // Prepared replay validates the entire owner roster once before
            // publishing any state. Query its sorted rows without rewalking
            // every action, and retain local duplicate refusal for raw users.
            let calls = self.calls_at(position);
            let index = calls.binary_search_by_key(&owner, |call| call.owner).ok()?;
            if index
                .checked_sub(1)
                .and_then(|previous| calls.get(previous))
                .is_some_and(|call| call.owner == owner)
                || calls.get(index + 1).is_some_and(|call| call.owner == owner)
            {
                return None;
            }
            Some(&calls[index])
        }

        pub fn insert_call(
            &mut self,
            position: DescriptorPosition,
            call: DescriptorInvocation,
        ) -> Option<()> {
            insert_invocation(&mut self.calls, position, call)
        }

        pub(crate) fn validate_invocation_owners(&self) -> Option<()> {
            for (&position, calls) in &self.calls {
                let step = self.flow.get(&position)?;
                let Some(calls) = calls else {
                    continue;
                };
                if calls.is_empty() || !calls.windows(2).all(|rows| rows[0].owner < rows[1].owner) {
                    return None;
                }
                if matches!(
                    calls[0].owner,
                    DescriptorInvocationOwner::Operation(_)
                        | DescriptorInvocationOwner::InvokeNoNormal(_)
                ) {
                    if calls.len() != 1 {
                        return None;
                    }
                    continue;
                }
                let mut exit_edge = None;
                let mut successor_edges = BTreeSet::new();
                for edge in &step.successors {
                    if let Some(edge) = edge.edge
                        && !successor_edges.insert(edge)
                    {
                        return None;
                    }
                }
                for call in calls {
                    let DescriptorInvocationOwner::Cleanup { edge, .. } = call.owner else {
                        return None;
                    };
                    if self.normal_exits.contains(&position) && step.successors.is_empty() {
                        if exit_edge.is_some_and(|selected| selected != edge) {
                            return None;
                        }
                        exit_edge = Some(edge);
                    } else if !successor_edges.contains(&edge) {
                        return None;
                    }
                }
            }
            Some(())
        }

        pub fn known_invocation(
            &self,
            position: &DescriptorPosition,
        ) -> Option<&DescriptorInvocation> {
            self.direct_call(*position)
        }

        pub fn known_invocations(
            &self,
        ) -> impl Iterator<Item = (&DescriptorPosition, &DescriptorInvocation)> {
            self.calls.iter().flat_map(|(position, calls)| {
                calls.iter().flatten().map(move |call| (position, call))
            })
        }

        pub(crate) fn producer(&self, root: PlaceId) -> Option<DescriptorPosition> {
            let index = self
                .places
                .binary_search_by_key(&root, |row| row.place)
                .ok()?;
            if index
                .checked_sub(1)
                .and_then(|index| self.places.get(index))
                .is_some_and(|row| row.place == root)
                || self
                    .places
                    .get(index + 1)
                    .is_some_and(|row| row.place == root)
            {
                return None;
            }
            self.places[index].producer
        }

        pub(crate) fn place_definition(&self, place: PlaceId) -> Option<&DescriptorDefinition> {
            let index = self
                .places
                .binary_search_by_key(&place, |row| row.place)
                .ok()?;
            Some(&self.places[index].definition)
        }

        pub(crate) fn sequence_construction<'graph>(
            &'graph self,
            types: &[StructuralTypeDeclaration],
            root: PlaceId,
            completed: Option<(MachineId, DescriptorPosition)>,
        ) -> Option<(
            DescriptorPosition,
            StructuralTypeId,
            &'graph DescriptorFlowStep,
        )> {
            let ty = *self.place_types.get(&root)?;
            let StructuralTypeShape::ElementView {
                element,
                access: None | Some(StructuralAccess::SharedBorrow),
            } = terminal_psi::find_structural_type(types, ty)?.shape
            else {
                return None;
            };
            // The outer sequence read authenticates the constructor once.
            // Its inner ordinal reads already carry that completed cut, so
            // they can borrow the same roster by its retained coordinate.
            let captured = match completed {
                Some((owner, (block, index))) => {
                    if owner != self.id {
                        return None;
                    }
                    let position = (block, index.checked_sub(1)?);
                    (self.producer(root) == Some(position)).then_some(position)
                }
                None => None,
            };
            let position = if let Some(position) = captured {
                position
            } else {
                self.producer(root)?
            };
            let origin = DescriptorLocation::whole(root);
            let origins = self.originations.get(&position)?;
            if !origins.iter().any(|(location, value)| {
                *location == origin
                    && matches!(value,
                    DescriptorCallOrigin::Argument { source, projection }
                    if *source == root && *projection == Projection::new(&[]))
            }) {
                return None;
            }
            let step = self.flow.get(&position)?;
            if step.inputs.iter().any(|(input, _)| input.is_none()) {
                return None;
            }
            // The adapter forms this ordered roster once from the actual
            // constructor operands. Reads borrow it rather than rebuilding or
            // rechecking every source element at every ordinal.
            Some((position, element, step))
        }

        fn resolve_location(
            &self,
            types: &[StructuralTypeDeclaration],
            location: &DescriptorLocation,
            completed: Option<(MachineId, DescriptorPosition)>,
        ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
            descriptor_lifetimes::locations::resolved_path(
                types,
                *self.place_types.get(&location.root)?,
                &location.path,
                |prefix, ty, index| {
                    if !prefix.is_empty() || self.place_types.get(&location.root) != Some(&ty) {
                        return None;
                    }
                    let (_, element, step) =
                        self.sequence_construction(types, location.root, completed)?;
                    let (_, destination) = step.derivations.get(usize::try_from(index).ok()?)?;
                    (destination.root == location.root
                        && destination.path == [DescriptorPathSegment::FixedIndex(index)])
                    .then_some(element)
                },
            )
        }

        pub(crate) fn location_capture(
            &self,
            types: &[StructuralTypeDeclaration],
            location: &DescriptorLocation,
            completed: Option<(MachineId, DescriptorPosition)>,
        ) -> Option<(MachineId, DescriptorPosition)> {
            if !matches!(
                location.path.first(),
                Some(DescriptorPathSegment::FixedIndex(_))
            ) {
                return None;
            }
            self.resolve_location(types, location, completed)?;
            let (producer, _, _) = self.sequence_construction(types, location.root, completed)?;
            Some((self.id, (producer.0, producer.1.checked_add(1)?)))
        }

        pub(crate) fn location_type(
            &self,
            types: &[StructuralTypeDeclaration],
            location: &DescriptorLocation,
            completed: Option<(MachineId, DescriptorPosition)>,
        ) -> Option<StructuralTypeId> {
            self.resolve_location(types, location, completed)?.1
        }

        pub(crate) fn location_path(
            &self,
            types: &[StructuralTypeDeclaration],
            location: &DescriptorLocation,
            completed: Option<(MachineId, DescriptorPosition)>,
        ) -> Option<Vec<StructuralPathSegment>> {
            Some(self.resolve_location(types, location, completed)?.0)
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct DescriptorCasePredicate {
        pub subject: DescriptorLocation,
        pub case: semantic_vocabulary::StructuralCaseId,
    }

    pub fn zeroed_case_predicates(
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Vec<DescriptorCasePredicate> {
        let Some(leaves) = crate::zeroed_structural_leaves(types.iter(), structural_type) else {
            return Vec::new();
        };
        let mut predicates = Vec::new();
        for leaf in leaves {
            let crate::ZeroedStructuralLeaf::SumSelection { path, case, .. } = leaf else {
                continue;
            };
            let Some(subject) = descriptor_lifetimes::descriptor_location(
                types,
                root,
                structural_type,
                &path,
                None,
            ) else {
                return Vec::new();
            };
            predicates.push(DescriptorCasePredicate { subject, case });
        }
        predicates
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum FlowFeasibility {
        ProvedImpossible,
        Possible,
        Unknown,
    }

    pub fn descriptor_machine(
        graph: &[DescriptorMachine],
        id: MachineId,
    ) -> Option<&DescriptorMachine> {
        graph.iter().find(|current| current.id == id)
    }

    #[derive(Clone, Copy)]
    pub struct DescriptorActualInputs<'a> {
        pub scalar_arguments: &'a [(ValueId, semantic_vocabulary::ScalarTerm)],
        pub arguments: &'a [Option<DescriptorLocation>],
        pub argument_contents: &'a [Option<DescriptorContentsQuery>],
    }

    pub struct DescriptorInvocation {
        pub scalar_arguments: Vec<(ValueId, semantic_vocabulary::ScalarTerm)>,
        pub owner: DescriptorInvocationOwner,
        pub target: DescriptorInvocationTarget,
        pub arguments: Vec<Option<DescriptorLocation>>,
        pub argument_contents: Vec<Option<DescriptorContentsQuery>>,
        pub result: Option<DescriptorLocation>,
    }

    pub enum DescriptorInvocationTarget {
        Machine(MachineId),
        Provider {
            results: Vec<DescriptorResultSlot>,
            effects: Vec<DescriptorStoredEffect>,
            invalidates_contents: bool,
        },
        Public {
            effects_machine: Option<MachineId>,
            realizations: Vec<MachineId>,
            extent: semantic_vocabulary::ReferenceOriginExtent,
            results: Vec<DescriptorResultSlot>,
            invalidates_contents: bool,
        },
    }

    impl DescriptorInvocation {
        pub fn actual_inputs(&self) -> DescriptorActualInputs<'_> {
            DescriptorActualInputs {
                scalar_arguments: &self.scalar_arguments,
                arguments: &self.arguments,
                argument_contents: &self.argument_contents,
            }
        }

        pub fn stored_effects_callee(&self) -> Option<MachineId> {
            match self.target {
                DescriptorInvocationTarget::Public {
                    effects_machine, ..
                } => effects_machine,
                _ => self.callee(),
            }
        }
        pub fn callee(&self) -> Option<MachineId> {
            match self.target {
                DescriptorInvocationTarget::Machine(machine) => Some(machine),
                DescriptorInvocationTarget::Public { .. }
                | DescriptorInvocationTarget::Provider { .. } => None,
            }
        }
    }

    #[derive(Clone)]
    pub struct DescriptorResultSlot {
        pub path: Vec<DescriptorPathSegment>,
        pub origins: Vec<DescriptorStoredOrigin>,
    }

    impl DescriptorResultSlot {
        pub(crate) fn element_path(
            types: &[StructuralTypeDeclaration],
            account: (StructuralTypeId, &[Self]),
            path: &[DescriptorPathSegment],
        ) -> Option<(Vec<StructuralPathSegment>, Option<StructuralTypeId>)> {
            descriptor_lifetimes::locations::resolved_path(
                types,
                account.0,
                path,
                |prefix, view, ordinal| {
                    let StructuralTypeShape::ElementView { element, .. } =
                        terminal_psi::find_structural_type(types, view)?.shape
                    else {
                        return None;
                    };
                    let origins = Self::matching(account.1, prefix)?.origins_at(prefix)?;
                    crate::descriptor_replay::DescriptorStoredOrigin::element_at(
                        &origins, element, ordinal,
                    )?;
                    Some(element)
                },
            )
        }

        pub(crate) fn canonicalize(slots: &mut [Self]) -> Option<()> {
            slots.sort_unstable_by(|left, right| left.path.cmp(&right.path));
            if slots.windows(2).any(|rows| rows[0].path == rows[1].path) {
                return None;
            }
            for slot in slots {
                for origin in &mut slot.origins {
                    if let DescriptorStoredOrigin::Carried { elements, .. } = origin {
                        for slots in elements {
                            Self::canonicalize(slots)?;
                        }
                    }
                }
            }
            Some(())
        }

        pub(crate) fn matching<'slots>(
            slots: &'slots [Self],
            path: &[DescriptorPathSegment],
        ) -> Option<&'slots Self> {
            // Machine result accounts are ordered once by their owner. Query
            // the longest recorded prefix instead of rescanning every sibling.
            for length in (0..=path.len()).rev() {
                if let Ok(index) =
                    slots.binary_search_by(|slot| slot.path.as_slice().cmp(&path[..length]))
                {
                    return slots.get(index);
                }
            }
            None
        }

        pub(crate) fn origins_at(
            &self,
            path: &[DescriptorPathSegment],
        ) -> Option<Vec<&DescriptorStoredOrigin>> {
            let suffix = path.strip_prefix(self.path.as_slice())?;
            if suffix.is_empty() {
                return Some(self.origins.iter().collect());
            }
            let (DescriptorPathSegment::FixedIndex(ordinal), suffix) = suffix.split_first()? else {
                return None;
            };
            let mut origins = Vec::new();
            for origin in &self.origins {
                let DescriptorStoredOrigin::Carried { elements, .. } = origin else {
                    return None;
                };
                let slots = elements.get(usize::try_from(*ordinal).ok()?)?;
                let slot = Self::matching(slots, suffix)?;
                origins.extend(slot.origins_at(suffix)?);
            }
            (!origins.is_empty()).then_some(origins)
        }

        pub(crate) fn merge(&mut self, other: Self) -> Option<()> {
            if self.path != other.path {
                return None;
            }
            self.origins.extend(other.origins);
            Some(())
        }

        pub(crate) fn visit<'a, Error>(
            &'a self,
            containing_type: StructuralTypeId,
            prefix: &mut Vec<DescriptorPathSegment>,
            visit: &mut impl FnMut(
                &[DescriptorPathSegment],
                StructuralTypeId,
                &'a Self,
            ) -> Result<(), Error>,
        ) -> Result<(), Error> {
            let length = prefix.len();
            prefix.extend_from_slice(&self.path);
            let result = (|| {
                visit(prefix, containing_type, self)?;
                for origin in &self.origins {
                    if let DescriptorStoredOrigin::Carried { element, elements } = origin {
                        for (ordinal, slots) in elements.iter().enumerate() {
                            prefix.push(DescriptorPathSegment::FixedIndex(ordinal as u64));
                            for slot in slots {
                                slot.visit(*element, prefix, visit)?;
                            }
                            prefix.pop();
                        }
                    }
                }
                Ok(())
            })();
            prefix.truncate(length);
            result
        }
    }

    pub struct DescriptorPlace {
        /// The canonical declaration's actual producer, retained once while
        /// adapting its instruction. Roster queries need not rescan the body.
        pub producer: Option<DescriptorPosition>,
        pub place: PlaceId,
        pub definition: DescriptorDefinition,
    }

    pub enum DescriptorDefinition {
        Parameter {
            position: usize,
            access: StructuralAccess,
            structural_type: StructuralTypeId,
        },
        Arrival {
            access: StructuralAccess,
        },
        Local,
        Literal,
    }

    impl DescriptorDefinition {
        pub(crate) fn has_independent_storage(&self) -> bool {
            match self {
                Self::Parameter { access, .. } | Self::Arrival { access } => {
                    *access == StructuralAccess::Owned
                }
                Self::Local | Self::Literal => true,
            }
        }
    }

    pub fn reconstruct_result_origins(
        types: &[StructuralTypeDeclaration],
        machines: &[DescriptorMachine],
        machine: MachineId,
    ) -> Option<Vec<DescriptorResultOrigin>> {
        replay::result_origins(types, machines, machine)
    }

    pub fn reconstruct_call_origins(
        types: &[StructuralTypeDeclaration],
        machines: &[DescriptorMachine],
        caller: MachineId,
        result: PlaceId,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        replay::call_origins(types, machines, caller, result)
    }

    pub(crate) fn external_backing(
        types: &[StructuralTypeDeclaration],
        root: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> bool {
        (0..=path.len()).any(|length| {
            let Some(declaration) =
                crate::runtime_structural_path_tip(types.iter(), root, &path[..length])
            else {
                return false;
            };
            if matches!(
                declaration.shape,
                StructuralTypeShape::Reference {
                    access: terminal_psi::reading_borrow_pair_pattern!(),
                    ..
                } | StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(
                        ByteSequenceCarrier::BorrowedView {
                            access: None | Some(terminal_psi::reading_borrow_pair_pattern!())
                        } | ByteSequenceCarrier::BoundedView {
                            access: None | Some(terminal_psi::reading_borrow_pair_pattern!()),
                            ..
                        }
                    )
            ) {
                return true;
            }
            let Some(StructuralPathSegment::Field(identity)) = path.get(length) else {
                return false;
            };
            let (terminal_psi::record_or_mixed_shape_pattern!(fields)) = &declaration.shape else {
                return false;
            };
            terminal_psi::live_field_by_identity(fields, *identity)
                .and_then(|field| field.field_type.byte_sequence_carrier())
                .is_some_and(|carrier| {
                    matches!(
                        carrier,
                        ByteSequenceCarrier::BorrowedView {
                            access: None | Some(terminal_psi::reading_borrow_pair_pattern!())
                        } | ByteSequenceCarrier::BoundedView {
                            access: None | Some(terminal_psi::reading_borrow_pair_pattern!()),
                            ..
                        }
                    )
                })
        })
    }

    pub fn aggregate_descriptor_slots(
        types: &[StructuralTypeDeclaration],
        root: PlaceId,
        structural_type: StructuralTypeId,
    ) -> Option<Vec<DescriptorLocation>> {
        let declaration = exactly_one(types.iter(), |declaration| {
            declaration.id == structural_type
        })?;
        if matches!(
            declaration.shape,
            StructuralTypeShape::Record { .. }
                | terminal_psi::sum_or_mixed_shape_pattern!()
                | StructuralTypeShape::FixedArray { .. }
        ) {
            DescriptorSlotKind::Views.owned_slots(types, root, structural_type)
        } else {
            Some(Vec::new())
        }
    }

    pub fn terminal_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
        terminal_graph_for(module, DescriptorSlotKind::Views, &[]).unwrap_or_default()
    }

    pub fn terminal_graph_from(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Vec<DescriptorMachine> {
        terminal_graph_closure(module, roots, DescriptorSlotKind::Views, &[]).unwrap_or_default()
    }

    pub fn terminal_borrowed_storage_graph_from(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = MachineId>,
        selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    ) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
        terminal_graph_closure(
            module,
            roots,
            DescriptorSlotKind::BorrowedStorage,
            selected_calls,
        )
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum DescriptorGraphError {
        MissingOrAmbiguousMachine(MachineId),
        IncompleteMachine {
            machine: MachineId,
            position: Option<DescriptorPosition>,
            successor: Option<(semantic_vocabulary::EdgeId, semantic_vocabulary::BlockId)>,
            detail: Option<&'static str>,
        },
    }

    pub const INEXACT_REFERENCE_BACKING: &str =
        "reference use without exact live backing correspondence";

    impl DescriptorGraphError {
        /// Report a location only when the operation ID is unique and its
        /// instruction matches. Other failures retain only the machine.
        pub fn at_operation(
            module: &TerminalModule,
            machine: MachineId,
            operation: &Operation,
            detail: &'static str,
        ) -> Self {
            let position = exactly_one(module.machines.iter(), |row| row.id == machine)
                .and_then(|machine| {
                    exactly_one(
                        machine.blocks.iter().flat_map(|block| {
                            block
                                .operations
                                .iter()
                                .enumerate()
                                .filter_map(move |(index, row)| {
                                    (row.id == operation.id).then_some((block.id, index, row))
                                })
                        }),
                        |_| true,
                    )
                    .and_then(|(block, index, row)| (row == operation).then_some((block, index)))
                })
                .and_then(|(block, index)| u32::try_from(index).ok().map(|index| (block, index)));
            Self::IncompleteMachine {
                machine,
                position,
                successor: None,
                detail: Some(detail),
            }
        }

        pub const fn detail(self) -> Option<&'static str> {
            match self {
                Self::IncompleteMachine { detail, .. } => detail,
                Self::MissingOrAmbiguousMachine(_) => None,
            }
        }

        pub const fn machine(self) -> MachineId {
            match self {
                Self::MissingOrAmbiguousMachine(machine)
                | Self::IncompleteMachine { machine, .. } => machine,
            }
        }

        /// Resolve only an actual operation in the failed machine. Terminator
        /// failures retain their block and edge without becoming operations.
        pub fn operation(
            self,
            module: &TerminalModule,
        ) -> Option<semantic_vocabulary::OperationId> {
            self.operation_row(module).map(|operation| operation.id)
        }

        fn operation_row(self, module: &TerminalModule) -> Option<&Operation> {
            let Self::IncompleteMachine {
                machine,
                position: Some((block, index)),
                ..
            } = self
            else {
                return None;
            };
            let machine = exactly_one(module.machines.iter(), |row| row.id == machine)?;
            let block = exactly_one(machine.blocks.iter(), |row| row.id == block)?;
            block.operations.get(usize::try_from(index).ok()?)
        }

        /// A dependency may fail while another machine requests its closure.
        /// Keep that dependency's actual declaration in the diagnostic rather
        /// than blaming the first requested root or erasing the graph error.
        pub fn in_module(self, module: &TerminalModule) -> String {
            let name = exactly_one(module.machines.iter(), |machine| {
                machine.id == self.machine()
            })
            .and_then(|machine| machine.declaration.as_ref())
            .and_then(|declaration| module.identities.text(declaration.callable));
            let mut detail = self.to_string();
            if let Some(operation) = self.operation_row(module) {
                detail.push_str(&format!(
                    "; checking {} ({:?})",
                    crate::OperationSemanticTag::for_operation(&operation.kind).name(),
                    operation.id
                ));
                if let Some(call) = operation.declared_invocation() {
                    detail.push_str(&format!("; invocation target {:?}", call.callee));
                    if let Some(required) = call.required_contract {
                        detail.push_str(&format!("; required contract {required:?}"));
                    }
                    if let terminal_psi::TargetMachineCallable::Machine(callee) = call.callee
                        && let Some(name) =
                            exactly_one(module.machines.iter(), |row| row.id == callee)
                                .and_then(|callee| callee.declaration.as_ref())
                                .and_then(|declaration| {
                                    module.identities.text(declaration.callable)
                                })
                    {
                        detail.push_str("; callee body ");
                        detail.push_str(name);
                    }
                }
            }
            if let Self::IncompleteMachine {
                position: Some((source, index)),
                ..
            } = self
                && let Some(machine) =
                    exactly_one(module.machines.iter(), |row| row.id == self.machine())
                && let Some(block) = exactly_one(machine.blocks.iter(), |row| row.id == source)
                && usize::try_from(index).ok() == Some(block.operations.len())
            {
                let kind = match &block.terminator {
                    Terminator::EliminateEmpty { .. } => "EliminateEmpty",
                    Terminator::InvokeNoNormal { .. } => "InvokeNoNormal",
                    Terminator::ExternalCompletion { .. } => "ExternalCompletion",
                    Terminator::Jump { .. } => "Jump",
                    Terminator::Conditional { .. } => "Conditional",
                    Terminator::StructuralCase { .. } => "StructuralCase",
                    Terminator::Return { .. } => "Return",
                    Terminator::ReturnUnit { .. } => "ReturnUnit",
                    Terminator::ReturnStructural { .. } => "ReturnStructural",
                    Terminator::Crash { .. } => "Crash",
                };
                detail.push_str(&format!("; terminator {kind}"));
                if let Terminator::ReturnStructural { source, .. } = block.terminator {
                    detail.push_str(&format!(
                        "; returned source {source:?} type {:?}; declared structural result {:?}",
                        machine.structural_place_type(source),
                        machine.result.structural(),
                    ));
                }
            }
            if let Self::IncompleteMachine {
                position: Some((source, _)),
                successor: Some((identity, target)),
                ..
            } = self
                && let Some(machine) =
                    exactly_one(module.machines.iter(), |row| row.id == self.machine())
            {
                if let Some(block) = exactly_one(machine.blocks.iter(), |row| row.id == source) {
                    let mut scalar_arguments = None;
                    let structural_arguments = match &block.terminator {
                        Terminator::Jump { edge, arguments, structural_arguments, .. }
                            if *edge == identity => {
                            scalar_arguments = Some(arguments.as_slice());
                            Some(structural_arguments.as_slice())
                        }
                        Terminator::Conditional { when_true, when_false, .. } => {
                            [when_true, when_false].into_iter()
                                .find(|row| row.edge == identity)
                                .map(|row| {
                                    scalar_arguments = Some(row.arguments.as_slice());
                                    row.structural_arguments.as_slice()
                                })
                        }
                        Terminator::StructuralCase { source, path, cases } => {
                            cases.iter().find(|row| row.edge == identity).map(|row| {
                                detail.push_str(&format!(
                                    "; case payload actuals {:?} from {source:?} via {path:?} case {:?}",
                                    row.payload_fields, row.case,
                                ));
                                row.structural_arguments.as_slice()
                            })
                        }
                        _ => None,
                    };
                    if let Some(arguments) = scalar_arguments {
                        let types = payload_predicates::TerminalScalarExpressions::new(machine);
                        let actual = arguments
                            .iter()
                            .map(|value| (*value, types.scalar_type(*value)))
                            .collect::<Vec<_>>();
                        detail.push_str(&format!("; scalar actuals {actual:?}"));
                    }
                    if let Some(arguments) = structural_arguments {
                        let actual = arguments
                            .iter()
                            .map(|argument| {
                                let root = machine.structural_place_type(argument.place);
                                let selected = root
                                    .and_then(|root| {
                                        crate::runtime_structural_path_tip(
                                            module.structural_types.iter(),
                                            root,
                                            &argument.path,
                                        )
                                    })
                                    .map(|row| row.id);
                                (argument, selected)
                            })
                            .collect::<Vec<_>>();
                        detail.push_str(&format!("; structural actuals {actual:?}"));
                    }
                }
                if let Some(target) = exactly_one(machine.blocks.iter(), |row| row.id == target) {
                    detail.push_str(&format!(
                        "; target scalar parameters {:?}; target structural parameters {:?}",
                        target.parameters, target.structural_parameters,
                    ));
                }
            }
            if let Some(name) = name {
                detail.push_str("; declaration ");
                detail.push_str(name);
            }
            detail
        }
    }

    impl std::fmt::Display for DescriptorGraphError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingOrAmbiguousMachine(machine) => write!(
                    formatter,
                    "descriptor storage closure has no unique machine {machine:?}"
                ),
                Self::IncompleteMachine {
                    machine,
                    position,
                    successor,
                    detail,
                } => {
                    let detail =
                        detail.unwrap_or("descriptor storage flow cannot be reconstructed");
                    write!(formatter, "{detail} for machine {machine:?}")?;
                    if let Some((block, index)) = position {
                        write!(formatter, " at block {block:?}, position {index}")?;
                    }
                    if let Some((edge, target)) = successor {
                        write!(formatter, "; successor {edge:?} to block {target:?}")?;
                    }
                    Ok(())
                }
            }
        }
    }

    impl std::error::Error for DescriptorGraphError {}

    pub(crate) fn terminal_graph_closure(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = MachineId>,
        kind: DescriptorSlotKind,
        selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    ) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
        terminal_graph_rows(module, roots, kind, selected_calls)
            .into_iter()
            .collect()
    }

    // Saved modules can use sparse identities. Sort declarations once;
    // do not reserve storage up to the largest identity.
    fn terminal_machine_roster(
        module: &TerminalModule,
    ) -> Result<Vec<&TerminalMachine>, DescriptorGraphError> {
        let mut machines = module.machines.iter().collect::<Vec<_>>();
        machines.sort_unstable_by_key(|machine| machine.id);
        if let Some(pair) = machines.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(DescriptorGraphError::MissingOrAmbiguousMachine(pair[1].id));
        }
        Ok(machines)
    }

    fn terminal_graph_machine<'a>(
        machines: &[&'a TerminalMachine],
        identity: MachineId,
    ) -> Option<&'a TerminalMachine> {
        let position = machines
            .binary_search_by_key(&identity, |machine| machine.id)
            .ok()?;
        Some(machines[position])
    }

    // An unsupported body must not erase useful analyses of independent
    // bodies. Keep its failure in the same roster: complete-closure consumers
    // still reject it, while optional summaries retain their unknown ceiling.
    pub(crate) fn terminal_graph_rows(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = MachineId>,
        kind: DescriptorSlotKind,
        selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    ) -> Vec<Result<DescriptorMachine, DescriptorGraphError>> {
        let machines = match terminal_machine_roster(module) {
            Ok(machines) => machines,
            Err(error) => return vec![Err(error)],
        };
        let mut pending: Vec<_> = roots.into_iter().collect();
        let mut reached = BTreeMap::new();
        while let Some(identity) = pending.pop() {
            if reached.contains_key(&identity) {
                continue;
            }
            let Some(body) = terminal_graph_machine(&machines, identity) else {
                reached.insert(
                    identity,
                    Err(DescriptorGraphError::MissingOrAmbiguousMachine(identity)),
                );
                continue;
            };
            let graph =
                match terminal_flow::adapt(module, &machines, body, kind, selected_calls, None) {
                    Ok(graph) => graph,
                    Err(error) => {
                        reached.insert(identity, Err(error));
                        continue;
                    }
                };
            for (_, call) in graph.known_invocations() {
                match &call.target {
                    DescriptorInvocationTarget::Machine(callee) => pending.push(*callee),
                    DescriptorInvocationTarget::Public {
                        effects_machine,
                        realizations,
                        ..
                    } => {
                        pending.extend(effects_machine);
                        pending.extend(realizations);
                    }
                    DescriptorInvocationTarget::Provider { .. } => {}
                }
            }
            reached.insert(identity, Ok(graph));
        }
        let mut rows: Vec<_> = module
            .machines
            .iter()
            .filter_map(|body| reached.remove(&body.id))
            .collect();
        // Missing call targets have no module row, but their failure must
        // remain visible to both complete and optional consumers.
        rows.extend(reached.into_values());
        if !rows
            .iter()
            .filter_map(|row| row.as_ref().ok())
            .any(|graph| {
                terminal_graph_machine(&machines, graph.id).is_some_and(|body| {
                    body.operations().any(|operation| {
                        matches!(operation.kind, OperationKind::CallDynamic { .. })
                    })
                })
            })
        {
            return rows;
        }
        let mut initial = Vec::with_capacity(rows.len());
        let mut failures = Vec::new();
        for row in rows {
            match row {
                Ok(graph) => initial.push(graph),
                Err(error) => failures.push(Err(error)),
            }
        }
        let mut rows = Vec::with_capacity(initial.len() + failures.len());
        for graph in &initial {
            let identity = graph.id;
            let Some(body) = terminal_graph_machine(&machines, identity) else {
                rows.push(Err(DescriptorGraphError::MissingOrAmbiguousMachine(
                    identity,
                )));
                continue;
            };
            rows.push(terminal_flow::adapt(
                module,
                &machines,
                body,
                kind,
                selected_calls,
                Some(&initial),
            ));
        }
        rows.extend(failures);
        rows
    }

    pub fn terminal_reference_graph_from(
        module: &TerminalModule,
        roots: impl IntoIterator<Item = MachineId>,
    ) -> Result<Vec<DescriptorMachine>, DescriptorGraphError> {
        terminal_graph_closure(module, roots, DescriptorSlotKind::References, &[])
    }

    pub fn terminal_reference_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
        terminal_graph_for(module, DescriptorSlotKind::References, &[]).unwrap_or_default()
    }

    pub fn terminal_borrowed_storage_graph(module: &TerminalModule) -> Vec<DescriptorMachine> {
        terminal_graph_for(module, DescriptorSlotKind::BorrowedStorage, &[]).unwrap_or_default()
    }

    pub fn terminal_residence_rules(
        module: &TerminalModule,
        machine: &terminal_psi::TerminalMachine,
    ) -> Option<descriptor_lifetimes::DescriptorResidenceRules> {
        terminal_flow::residence_rules(module, machine)
    }

    pub fn terminal_access_origin_graph(
        module: &TerminalModule,
        selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    ) -> Option<Vec<DescriptorMachine>> {
        terminal_graph_for(module, DescriptorSlotKind::AccessOrigins, selected_calls)
    }

    pub(crate) fn terminal_graph_for(
        module: &TerminalModule,
        kind: DescriptorSlotKind,
        selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    ) -> Option<Vec<DescriptorMachine>> {
        let machines = terminal_machine_roster(module).ok()?;
        let initial = module
            .machines
            .iter()
            .map(|machine| {
                terminal_flow::adapt(module, &machines, machine, kind, selected_calls, None).ok()
            })
            .collect::<Option<Vec<_>>>()?;
        if !module.machines.iter().any(|machine| {
            machine
                .operations()
                .any(|operation| matches!(operation.kind, OperationKind::CallDynamic { .. }))
        }) {
            return Some(initial);
        }
        module
            .machines
            .iter()
            .map(|machine| {
                terminal_flow::adapt(
                    module,
                    &machines,
                    machine,
                    kind,
                    selected_calls,
                    Some(&initial),
                )
                .ok()
            })
            .collect()
    }

    pub(crate) fn terminal_definition(
        _module: &TerminalModule,
        machine: &TerminalMachine,
        place: PlaceId,
        kind: StructuralPlaceKind,
    ) -> Option<DescriptorDefinition> {
        match kind {
            StructuralPlaceKind::Parameter { position } => {
                let (position, parameter) = exactly_one(
                    machine.structural_parameters.iter().enumerate(),
                    |(_, parameter)| parameter.place == place && parameter.position == position,
                )?;
                Some(DescriptorDefinition::Parameter {
                    position,
                    access: parameter.access,
                    structural_type: parameter.structural_type,
                })
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => {
                let mut establishments = machine.operations().filter(|operation| matches!(operation.kind, OperationKind::EstablishByteSequenceLiteral { destination, .. } if destination == place));
                establishments.next()?;
                establishments
                    .next()
                    .is_none()
                    .then_some(DescriptorDefinition::Literal)
            }
            StructuralPlaceKind::BlockParameter { block, position } => {
                let mut matching = machine
                    .blocks
                    .iter()
                    .filter(|candidate| candidate.id == block);
                let block = matching.next()?;
                if matching.next().is_some()
                    || block.structural_parameters.get(position as usize)?.place != place
                {
                    return None;
                }
                Some(DescriptorDefinition::Arrival {
                    access: block.structural_parameters.get(position as usize)?.access,
                })
            }
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let mut matching = machine
                    .operations()
                    .filter(|operation| operation.id == producer);
                let operation = matching.next()?;
                let result = operation.result.structural()?;
                if matching.next().is_some()
                    || result.place != place
                    || result.structural_type != structural_type
                {
                    return None;
                }
                Some(DescriptorDefinition::Local)
            }
            _ => Some(DescriptorDefinition::Local),
        }
    }

    pub fn source_origins(
        module: &TerminalModule,
        machine: &TerminalMachine,
        position: DescriptorPosition,
        location: &DescriptorLocation,
    ) -> Option<Vec<DescriptorCallOrigin>> {
        reconstruct_source_origins(
            &module.structural_types,
            &terminal_graph(module),
            machine.id,
            position,
            location,
        )
    }
    pub fn stored_effects(
        module: &TerminalModule,
        machine: &TerminalMachine,
    ) -> Option<Vec<DescriptorStoredEffect>> {
        reconstruct_stored_effects(
            &module.structural_types,
            &terminal_graph(module),
            machine.id,
        )
    }
    pub fn call_stored_effects(
        module: &TerminalModule,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Option<Vec<DescriptorStoredCallEffect>> {
        let graph = terminal_graph(module);
        call_stored_effects_in(
            &mut DescriptorAnalysis::new(&module.structural_types, &graph),
            machine,
            operation,
        )
    }

    pub fn call_stored_effects_in(
        analysis: &mut DescriptorAnalysis<'_>,
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Option<Vec<DescriptorStoredCallEffect>> {
        analysis.call_stored_effects(machine.id, operation_position(machine, operation)?)
    }
    pub(crate) fn operation_position(
        machine: &TerminalMachine,
        operation: &Operation,
    ) -> Option<DescriptorPosition> {
        let mut positions = machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .filter_map(move |(index, candidate)| {
                    (candidate.id == operation.id && candidate == operation)
                        .then(|| Some((block.id, u32::try_from(index).ok()?)))
                        .flatten()
                })
        });
        let position = positions.next()?;
        positions.next().is_none().then_some(position)
    }
}
