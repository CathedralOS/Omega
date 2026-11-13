/*
Scalar, record, and slice recast layout and footprint validation
(`value_custody::recasts`).
*/
pub(crate) mod qualification_views {
    use typed_trees::types::{
        DomainConstraint, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) fn qualification_view_compatible(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        target: TypeReferenceHandle,
    ) -> Option<bool> {
        let (source_carrier, source_domains) = declared_qualifications(program, source)?;
        let (target_carrier, target_domains) = declared_qualifications(program, target)?;
        if program.normalized_type_identity(source_carrier)
            != program.normalized_type_identity(target_carrier)
        {
            return None;
        }
        Some(domains_imply(program, &source_domains, &target_domains))
    }

    pub(crate) fn lender_qualifications_implied(
        program: &dyn crate::CheckingInput,
        view: TypeReferenceHandle,
        lender: TypeReferenceHandle,
    ) -> Option<bool> {
        let (_, view_domains) = declared_qualifications(program, view)?;
        let (_, lender_domains) = declared_qualifications(program, lender)?;
        Some(domains_imply(program, &view_domains, &lender_domains))
    }

    fn domains_imply(
        program: &dyn crate::CheckingInput,
        source_domains: &[&DomainConstraint],
        target_domains: &[&DomainConstraint],
    ) -> bool {
        target_domains.iter().all(|target| {
            crate::field_domains::domain_is_vacuous(program, target.symbol, &mut Vec::new())
                || source_domains.iter().any(|source| {
                    (source.symbol == target.symbol
                        && source.semantic_id.is_valid()
                        && source.semantic_id == target.semantic_id)
                        || (source.arguments.is_empty()
                            && target.arguments.is_empty()
                            && crate::field_domains::domain_membership_implies(
                                program,
                                source.symbol,
                                target.symbol,
                            ))
                })
        })
    }

    pub(crate) fn qualification_carrier(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        declared_qualifications(program, reference)
            .map(|(carrier, _)| carrier)
            .unwrap_or(reference)
    }

    fn declared_qualifications(
        program: &dyn crate::CheckingInput,
        mut reference: TypeReferenceHandle,
    ) -> Option<(TypeReferenceHandle, Vec<&DomainConstraint>)> {
        let mut domains = Vec::new();
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            for constraint in program.type_reference_table.constraints(*constraints) {
                let TypeConstraintNode::Domain(domain) = constraint else {
                    return None;
                };
                if !domain.symbol.is_valid() || !domain.semantic_id.is_valid() {
                    return None;
                }
                domains.push(domain);
            }
            reference = *base_type;
        }
        Some((reference, domains))
    }
}
pub(crate) mod raw_byte_region {
    use diagnostics::Diagnostic;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{
        FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
    };

    pub(crate) enum ByteRegionExtent {
        Fixed(i64),
        Descriptor,
    }

    pub(crate) struct IndexedByteRegion {
        start: ExpressionHandle,
        element: TypeReferenceHandle,
        extent: ByteRegionExtent,
    }

    fn indexed_byte_region(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> Option<IndexedByteRegion> {
        let indexed = program.expression_table.indexed_node(source)?;
        let collection_type = crate::value_custody::places::declared_place_type(
            program,
            machine,
            Some(state),
            indexed.collection,
        )?;
        let (element, extent) = match program.type_reference_table.type_reference(collection_type) {
            TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            } => (
                *element_type,
                ByteRegionExtent::Fixed(i64::try_from(*length).ok()?),
            ),
            TypeReferenceNode::Slice { element_type } => {
                (*element_type, ByteRegionExtent::Descriptor)
            }
            _ => return None,
        };
        (super::exact_scalar_representation_type(program, element) == Some(PrimitiveType::U8))
            .then_some(IndexedByteRegion {
                start: indexed.index,
                element,
                extent,
            })
    }

    pub(crate) fn is_indexed_byte_region(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> bool {
        indexed_byte_region(program, machine, state, source).is_some()
    }

    pub(crate) fn byte_region_allows_arbitrary_writes(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> bool {
        let Some(region) = indexed_byte_region(program, machine, state, source) else {
            return false;
        };
        let raw = super::scalar_representation::MutableScalarRepresentationFacts {
            atomic: false,
            domains: Vec::new(),
            values: super::scalar_representation::ScalarRepresentationSet::ExactBitPatterns(
                super::scalar_representation::full_scalar_bit_patterns(PrimitiveType::U8),
            ),
        };
        super::scalar_representation::mutable_scalar_representation_facts(program, region.element)
            .is_some_and(|facts| {
                super::scalar_representation::scalar_representation_facts_imply(
                    program, &raw, &facts,
                )
            })
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SliceRecastByteRegion {
        pub collection: ExpressionHandle,
        pub source: ExpressionHandle,
        pub start: ExpressionHandle,
        pub extent: u64,
        pub element: TypeReferenceHandle,
        pub access: language_core::ReferenceAccess,
    }

    pub fn slice_recast_byte_region(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<SliceRecastByteRegion> {
        let authored = super::slice_recast_source(program, expression)?;
        let source = authored.source;
        let indexed = program.expression_table.indexed_node(source)?;
        let collection_type = crate::value_custody::places::declared_place_type(
            program,
            machine,
            Some(state),
            indexed.collection,
        )?;
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
            ..
        } = program.type_reference_table.type_reference(collection_type)
        else {
            return None;
        };
        if super::exact_scalar_representation_type(program, *element_type)
            != Some(PrimitiveType::U8)
        {
            return None;
        }
        Some(SliceRecastByteRegion {
            collection: indexed.collection,
            source,
            start: indexed.index,
            extent: u64::try_from(*length).ok()?,
            element: authored.element,
            access: authored.access,
        })
    }

    pub(crate) fn exact_interior_byte_region_offset(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> Option<i64> {
        let indexed = program.expression_table.indexed_node(source)?;
        if let Some(literal) = program.expression_table.integer_node(indexed.index) {
            return literal.value_i64().filter(|offset| *offset >= 0);
        }
        let type_reference = crate::value_custody::places::declared_place_type_raw(
            program,
            machine,
            Some(state),
            indexed.index,
        )?;
        let interval = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
            program,
            type_reference,
        )?;
        let (Some(low), Some(high)) = (interval.low(), interval.high()) else {
            return None;
        };
        (low == high && low >= 0).then_some(low)
    }
}

pub(crate) mod recast_judgments {
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};

    pub(crate) fn strip_mutable(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(inner) => strip_mutable(program, inner.target),
            _ => expression,
        }
    }
}
pub(crate) mod record_eligibility {
    use crate::value_custody::places::unique_data_definition_by_symbol;
    use crate::value_custody::recasts::representation_types::exact_primitive_type;
    use language_semantics::const_value::CanonicalConstValue;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    const MAX_RECAST_RECORD_GRAPH_NODES: usize = 4096;
    const MAX_RECAST_RECORD_GRAPH_EDGES: usize = 16384;
    pub(crate) const MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH: usize = 2;

    pub(crate) fn closed_fact_free_record_type_is_eligible(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
    ) -> bool {
        enum Work {
            Enter(TypeReferenceHandle),
            Exit(typed_trees::type_identity::TypeIdentityId),
        }
        let mut pending = vec![Work::Enter(root)];
        let mut states = Vec::<u8>::new();
        let mut nodes = 0usize;
        let mut edges = 0usize;
        while let Some(work) = pending.pop() {
            let reference = match work {
                Work::Exit(identity) => {
                    states[identity.storage_index().expect("entered identity")] = 2;
                    continue;
                }
                Work::Enter(reference) => reference,
            };
            if !program
                .type_reference_table
                .contains_type_reference(reference)
            {
                return false;
            }
            if matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Constrained { .. }
            ) {
                return false;
            }
            let identity = program.normalized_type_identity(reference);
            if !identity.is_valid() {
                return false;
            }
            let Some(index) = identity.storage_index() else {
                return false;
            };
            if states.len() <= index {
                states.resize(index + 1, 0);
            }
            match states[index] {
                2 => continue,
                1 => return false,
                _ => {}
            }
            nodes += 1;
            if nodes > MAX_RECAST_RECORD_GRAPH_NODES {
                return false;
            }
            states[index] = 1;
            pending.push(Work::Exit(identity));
            if let Some(storage) = program.layout_storage_arguments(reference) {
                pending.push(Work::Enter(storage.semantic_type));
                edges += 1;
            } else if let Some(primitive) = exact_primitive_type(program, reference) {
                if primitive == PrimitiveType::Bool || primitive.scalar_byte_size().is_none() {
                    return false;
                }
            } else {
                match program.type_reference_table.type_reference(reference) {
                    TypeReferenceNode::FixedArray {
                        element_type,
                        length: typed_trees::types::FixedArrayLength::Literal(_),
                    } => {
                        pending.push(Work::Enter(*element_type));
                        edges += 1;
                    }
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                        if program
                            .type_reference_table
                            .type_reference(reference)
                            .as_named_symbol()
                            .is_some_and(|symbol| {
                                program.symbols.builtin_type_atom(symbol).is_some()
                            })
                        {
                            return false;
                        }
                        let Some(data) = program.closed_data_definition(reference) else {
                            return false;
                        };
                        let members = program.data_members(data);
                        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                            || data.quotient.is_some()
                            || !data.requires_facts.is_empty()
                            || data.zero_gated
                            || typed_trees::data::DataDefinition::shape_kind_from_members(members)
                                != typed_trees::data::DataShapeKind::Record
                        {
                            return false;
                        }
                        for member in members.iter().rev() {
                            let Some(field) = member.as_field() else {
                                return false;
                            };
                            if field.relevance.is_erased() {
                                return false;
                            }
                            pending.push(Work::Enter(field.type_reference));
                            edges += 1;
                        }
                    }
                    _ => return false,
                }
            }
            if edges > MAX_RECAST_RECORD_GRAPH_EDGES {
                return false;
            }
        }
        true
    }

    pub(crate) fn phantom_lifetime_record_symbol_shape(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<symbols::SymbolHandle> {
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        if !base_symbol.is_valid()
            || lifetime_arguments.is_empty()
            || !program
                .type_reference_table
                .type_reference_handles(*arguments)
                .is_empty()
        {
            return None;
        }

        let instance = unique_data_definition_by_symbol(program, *base_symbol)?;
        let origin = instance.generic_instance?;
        let TypeReferenceNode::Generic {
            base_symbol: origin_base_symbol,
            lifetime_arguments: origin_lifetime_arguments,
            ..
        } = program.type_reference_table.type_reference(origin)
        else {
            return None;
        };
        let base = unique_data_definition_by_symbol(program, *origin_base_symbol)?;
        if !origin_lifetime_arguments.is_empty()
            || instance.lifetime_parameters.is_empty()
            || instance.lifetime_parameters.len() != lifetime_arguments.len()
            || instance.lifetime_parameters != base.lifetime_parameters
            || !program.data_type_parameters(instance).is_empty()
            || !closed_record_generic_origin_is_eligible(program, instance)
            || instance.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || instance.quotient.is_some()
            || !instance.requires_facts.is_empty()
            || instance.zero_gated
        {
            return None;
        }

        let members = program.data_members(instance);
        if typed_trees::data::DataDefinition::shape_kind_from_members(members)
            != typed_trees::data::DataShapeKind::Record
        {
            return None;
        }
        Some(instance.symbol)
    }

    pub(crate) fn direct_phantom_lifetime_record_symbol(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<symbols::SymbolHandle> {
        let symbol = phantom_lifetime_record_symbol_shape(program, type_reference)?;
        closed_fact_free_record_type_is_eligible(program, type_reference).then_some(symbol)
    }

    pub(crate) fn direct_record_view_type_is_fact_free(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> bool {
        closed_fact_free_record_type_is_eligible(program, type_reference)
    }

    fn closed_record_generic_origin_is_eligible(
        program: &dyn crate::CheckingInput,
        instance: &typed_trees::data::DataDefinition,
    ) -> bool {
        let Some(origin) = instance.generic_instance else {
            return true;
        };
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = program.type_reference_table.type_reference(origin)
        else {
            return false;
        };
        if !base_symbol.is_valid()
            || *base_symbol == instance.symbol
            || !lifetime_arguments.is_empty()
        {
            return false;
        }
        let Some(base) = unique_data_definition_by_symbol(program, *base_symbol) else {
            return false;
        };
        let parameters = program.data_type_parameters(base);
        let arguments = program
            .type_reference_table
            .type_reference_handles(*arguments);
        base.supply_mode == language_semantics::DataSupplyMode::CheckedShape
            && base.generic_instance.is_none()
            && base.quotient.is_none()
            && typed_trees::data::DataDefinition::shape_kind_from_members(
                program.data_members(base),
            ) == typed_trees::data::DataShapeKind::Record
            && !parameters.is_empty()
            && parameters.len() == arguments.len()
            && parameters
                .iter()
                .zip(arguments)
                .all(|(parameter, argument)| match parameter.kind {
                    typed_trees::data::TypeParameterKind::Type => {
                        closed_raw_generic_type_argument_is_eligible(program, *argument)
                    }
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                        closed_raw_generic_const_argument_is_eligible(
                            program,
                            type_reference,
                            *argument,
                        )
                    }
                    typed_trees::data::TypeParameterKind::Machine { .. }
                    | typed_trees::data::TypeParameterKind::Implementation
                    | typed_trees::data::TypeParameterKind::Proposition { .. } => false,
                })
    }

    fn closed_raw_generic_const_argument_is_eligible(
        program: &dyn crate::CheckingInput,
        parameter_type: TypeReferenceHandle,
        argument: TypeReferenceHandle,
    ) -> bool {
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(argument)
        else {
            return false;
        };
        if symbol.is_valid() {
            return false;
        }
        if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
            return crate::value_custody::type_references::validate_exact_typed_structured_const_argument(
            program,
            parameter_type,
            &value,
        )
        .is_ok();
        }
        let Some(primitive) = exact_primitive_type(program, parameter_type)
            .filter(|primitive| primitive.accepts_integer_literal())
        else {
            return false;
        };
        let Ok(value) = name.as_str().parse::<i128>() else {
            return false;
        };
        name.as_str() == value.to_string()
            && crate::value_custody::type_references::const_integer_value_fits_primitive(
                primitive, value,
            )
    }

    fn closed_raw_generic_type_argument_is_eligible(
        program: &dyn crate::CheckingInput,
        argument: TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(argument) {
            TypeReferenceNode::Named { symbol, .. } => {
                exact_primitive_type(program, argument).is_some()
                    || (symbol.is_valid()
                        && program.data_definitions().iter().any(|data| {
                            data.symbol == *symbol
                                && data.lifetime_parameters.is_empty()
                                && program.data_type_parameters(data).is_empty()
                        }))
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length: typed_trees::types::FixedArrayLength::Literal(length),
            } => {
                *length > 0 && closed_raw_generic_type_argument_is_eligible(program, *element_type)
            }
            _ => false,
        }
    }
}
pub(crate) mod record_representation {
    use super::scalar_representation::{
        MutableScalarRepresentationFacts, mutable_scalar_representation_facts,
        narrow_by_field_representation_bounds, scalar_representation_facts_imply,
    };
    use std::collections::HashSet;
    use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) mod initialized_storage {
        use super::{MutableRecordLeaf, MutableRecordRepresentation};
        use std::ops::Range;

        pub(crate) struct InitializedStorageBits {
            ranges: Vec<Range<usize>>,
            limit: usize,
        }

        impl InitializedStorageBits {
            pub(crate) fn covers_entire_extent(&self) -> bool {
                self.limit == 0
                    || matches!(self.ranges.as_slice(), [range] if range.start == 0 && range.end == self.limit)
            }

            pub(crate) fn prepare(source: &MutableRecordRepresentation) -> Option<Self> {
                let limit = source.size.checked_mul(8)?;
                let count = source.leaves.iter().try_fold(0usize, |count, leaf| {
                    count.checked_add(if leaf.byte_projection {
                        1
                    } else {
                        leaf.bit_fragments.len()
                    })
                })?;
                let mut ranges = Vec::<Range<usize>>::new();
                ranges.try_reserve_exact(count).ok()?;
                for leaf in &source.leaves {
                    if leaf.facts.atomic {
                        continue;
                    }
                    visit_leaf_ranges(leaf, limit, |range| {
                        ranges.push(range);
                        Some(())
                    })?;
                }

                ranges.sort_unstable_by_key(|range| (range.start, range.end));
                let mut retained = 0usize;
                for position in 0..ranges.len() {
                    if retained > 0 && ranges[position].start <= ranges[retained - 1].end {
                        ranges[retained - 1].end =
                            ranges[retained - 1].end.max(ranges[position].end);
                    } else {
                        ranges.swap(retained, position);
                        retained += 1;
                    }
                }
                ranges.truncate(retained);
                Some(Self { ranges, limit })
            }

            pub(crate) fn contains_leaf(&self, leaf: &MutableRecordLeaf) -> bool {
                visit_leaf_ranges(leaf, self.limit, |range| {
                    let next = self
                        .ranges
                        .partition_point(|source| source.start <= range.start);
                    next.checked_sub(1)
                        .filter(|&previous| range.end <= self.ranges[previous].end)
                        .map(|_| ())
                })
                .is_some()
            }
        }

        fn visit_leaf_ranges(
            leaf: &MutableRecordLeaf,
            limit: usize,
            mut visit: impl FnMut(Range<usize>) -> Option<()>,
        ) -> Option<()> {
            if leaf.byte_projection {
                let start = leaf.offset.checked_mul(8)?;
                let end = leaf.offset.checked_add(leaf.size)?.checked_mul(8)?;
                if end > limit {
                    return None;
                }
                if start < end {
                    visit(start..end)?;
                }
            } else {
                if leaf.bit_fragments.is_empty() {
                    return None;
                }
                for fragment in &leaf.bit_fragments {
                    let offset = leaf
                        .offset
                        .checked_add(fragment.container_byte_offset)?
                        .checked_mul(8)?;
                    let width = usize::from(fragment.container_width_bits);
                    let first = usize::from(fragment.destination_lsb);
                    let last = first.checked_add(usize::from(fragment.width))?;
                    if fragment.width == 0 || last > width || offset.checked_add(width)? > limit {
                        return None;
                    }
                    visit(offset.checked_add(first)?..offset.checked_add(last)?)?;
                }
            }
            Some(())
        }
    }

    pub(crate) type SymbolIdentity = (symbols::SymbolHandle, Option<TypeReferenceHandle>);

    const MAX_RECAST_REPRESENTATION_DEPTH: usize = 256;
    const MAX_RECAST_REPRESENTATION_WORK: usize = 16384;

    pub(crate) struct RepresentationBudget {
        depth: usize,
        work: usize,
        lifetime_shell_depth: usize,
        pending_predicate_owner: Option<SymbolIdentity>,
    }

    impl RepresentationBudget {
        fn enter(&mut self) -> Option<()> {
            self.depth = self.depth.checked_add(1)?;
            (self.depth <= MAX_RECAST_REPRESENTATION_DEPTH).then_some(())
        }

        fn leave(&mut self) {
            self.depth -= 1;
        }

        fn consume(&mut self, amount: usize) -> Option<()> {
            self.work = self.work.checked_add(amount)?;
            (self.work <= MAX_RECAST_REPRESENTATION_WORK).then_some(())
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct MutableRecordRepresentation {
        pub(crate) size: usize,
        pub(crate) align: usize,
        leaves: Vec<MutableRecordLeaf>,
    }

    impl MutableRecordRepresentation {
        pub(crate) fn permits_shared_mutation(&self) -> bool {
            self.leaves.iter().any(|leaf| leaf.facts.atomic)
        }
    }

    pub(crate) fn type_has_atomic_storage(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
    ) -> bool {
        let mut pending = vec![root];
        let mut visited = Vec::new();
        while let Some(reference) = pending.pop() {
            let Some(index) = program.normalized_type_identity(reference).storage_index() else {
                return false;
            };
            if visited.len() <= index {
                visited.resize(index + 1, false);
            }
            if std::mem::replace(&mut visited[index], true) {
                continue;
            }
            if let Some(storage) = program.layout_storage_arguments(reference) {
                pending.push(storage.semantic_type);
                continue;
            }
            if super::representation_types::scalar_is_atomic(program, reference) == Some(true) {
                return true;
            }
            match program.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Constrained { base_type, .. }
                | TypeReferenceNode::FixedArray {
                    element_type: base_type,
                    ..
                } => pending.push(*base_type),
                TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                    if let Some(data) = program.closed_data_definition(reference) {
                        pending.extend(
                            program
                                .live_data_fields(program.data_members(data))
                                .map(|field| field.type_reference),
                        );
                    }
                }
                _ => {}
            }
        }
        false
    }

    #[derive(Debug, Clone)]
    pub(crate) struct MutableRecordLeaf {
        offset: usize,
        size: usize,
        facts: MutableScalarRepresentationFacts,
        path: Vec<RepresentationSegment>,
        primitive: typed_trees::types::PrimitiveType,
        stored_integer: Option<(u16, layout_plans::IntegerInterpretation)>,
        byte_projection: bool,
        bit_fragments: Vec<typed_trees::PlanLaidBitFragment>,
        arrays: Vec<NormalizedArrayStep>,
    }

    fn expand_replicated_leaves(
        element: &MutableRecordRepresentation,
        count: usize,
        stride: usize,
    ) -> Option<Vec<MutableRecordLeaf>> {
        let mut leaves = Vec::new();
        leaves
            .try_reserve_exact(element.leaves.len().checked_mul(count)?)
            .ok()?;
        for index in 0..count {
            let base = stride.checked_mul(index)?;
            for leaf in &element.leaves {
                let mut projected = leaf.clone();
                projected.arrays.push(NormalizedArrayStep {
                    suffix_length: projected.path.len(),
                    stride,
                    extent: count,
                });
                projected
                    .path
                    .insert(0, RepresentationSegment::Element(index));
                projected.offset = base.checked_add(leaf.offset)?;
                leaves.push(projected);
            }
        }
        Some(leaves)
    }

    #[derive(Debug, Clone)]
    pub(crate) struct NormalizedArrayStep {
        suffix_length: usize,
        stride: usize,
        extent: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum RepresentationSegment {
        Field(symbols::SymbolHandle),
        Element(usize),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RecastScalarProjection {
        pub offset: usize,
        pub footprint: usize,
        pub primitive: typed_trees::types::PrimitiveType,
        pub stored_integer: Option<semantic_vocabulary::IntegerType>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RecastRuntimeIndexGeometry {
        pub path_index: usize,
        pub stride: usize,
        pub extent: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RecastScalarGeometry {
        pub projection: RecastScalarProjection,
        pub indices: Vec<RecastRuntimeIndexGeometry>,
    }

    #[derive(Debug, Clone)]
    pub struct RecastScalarLayout {
        leaves: std::sync::Arc<[RecastScalarLayoutLeaf]>,
    }

    #[derive(Debug)]
    pub(crate) struct RecastScalarLayoutLeaf {
        path: Box<[checked_trees::CheckedUnitStructuralPathSegment]>,
        projection: RecastScalarProjection,
        arrays: Box<[NormalizedArrayStep]>,
    }

    impl RecastScalarLayout {
        pub fn scalar_path_geometry(
            &self,
            path: &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Option<RecastScalarGeometry> {
            use checked_trees::CheckedUnitStructuralPathSegment as Segment;
            if path.len() > MAX_RECAST_REPRESENTATION_DEPTH {
                return None;
            }
            let mut matching = self.leaves.iter().filter(|leaf| {
                leaf.path.len() == path.len()
                    && leaf
                        .path
                        .iter()
                        .zip(path)
                        .all(|(fixed, requested)| match requested {
                            Segment::RuntimeIndex(_) => matches!(fixed, Segment::FixedIndex(0)),
                            _ => fixed == requested,
                        })
            });
            let leaf = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            let mut indices = Vec::new();
            for (path_index, segment) in path.iter().enumerate() {
                if !matches!(segment, Segment::RuntimeIndex(_)) {
                    continue;
                }
                let suffix_length = path.len().checked_sub(path_index.checked_add(1)?)?;
                let mut arrays = leaf
                    .arrays
                    .iter()
                    .filter(|array| array.suffix_length == suffix_length);
                let array = arrays.next()?;
                if arrays.next().is_some() || array.extent == 0 {
                    return None;
                }
                indices.try_reserve(1).ok()?;
                indices.push(RecastRuntimeIndexGeometry {
                    path_index,
                    stride: array.stride,
                    extent: array.extent,
                });
            }
            Some(RecastScalarGeometry {
                projection: leaf.projection,
                indices,
            })
        }
    }

    pub fn recast_scalar_layout(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
    ) -> Option<RecastScalarLayout> {
        let root = program
            .type_reference_table
            .unconstrained_type_reference(root);
        let representation = match pending_record_fields(program, root) {
            Some((_, fields)) => fields,
            None => type_representation(program, root)?,
        };
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(representation.leaves.len()).ok()?;
        for leaf in &representation.leaves {
            if !leaf.byte_projection {
                continue;
            }
            let path = checked_projection_path(program, root, &leaf.path)?;
            leaves.push(RecastScalarLayoutLeaf {
                path: path.into_boxed_slice(),
                projection: scalar_leaf_projection(representation.size, leaf)?,
                arrays: leaf.arrays.clone().into_boxed_slice(),
            });
        }
        Some(RecastScalarLayout {
            leaves: leaves.into(),
        })
    }

    fn unique_record_field(
        program: &dyn crate::CheckingInput,
        carrier: TypeReferenceHandle,
        select: impl Fn(&typed_trees::data::DataField) -> bool,
    ) -> Option<&typed_trees::data::DataField> {
        let owner = program.closed_data_field_definition(carrier)?;
        let mut fields = program
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if !field.relevance.is_erased() && select(field) =>
                {
                    Some(field)
                }
                _ => None,
            });
        let selected = fields.next()?;
        fields.next().is_none().then_some(selected)
    }

    fn fixed_array_element(
        program: &dyn crate::CheckingInput,
        carrier: TypeReferenceHandle,
        index: usize,
    ) -> Option<TypeReferenceHandle> {
        let TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(carrier)
        else {
            return None;
        };
        (index < *length).then_some(*element_type)
    }

    fn checked_projection_path(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        path: &[RepresentationSegment],
    ) -> Option<Vec<checked_trees::CheckedUnitStructuralPathSegment>> {
        use checked_trees::CheckedUnitStructuralPathSegment as CheckedSegment;
        let mut carrier = root;
        let mut checked_path = Vec::new();
        checked_path.try_reserve_exact(path.len()).ok()?;
        for segment in path {
            match segment {
                RepresentationSegment::Field(symbol) => {
                    let selected =
                        unique_record_field(program, carrier, |field| field.symbol == *symbol)?;
                    checked_path.push(CheckedSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            selected.identity,
                            selected.name.clone(),
                        ),
                    ));
                    carrier = selected.type_reference;
                }
                RepresentationSegment::Element(index) => {
                    carrier = fixed_array_element(program, carrier, *index)?;
                    checked_path.push(CheckedSegment::FixedIndex(u64::try_from(*index).ok()?));
                }
            }
        }
        (recast_projection_path(program, root, &checked_path)? == path).then_some(checked_path)
    }

    pub fn recast_scalar_projection(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
        field: symbols::SymbolHandle,
    ) -> Option<RecastScalarProjection> {
        let mut path = recast_projection_path(program, root, carrier_path)?;
        path.push(RepresentationSegment::Field(field));
        normalized_scalar_projection(program, root, &path)
    }

    pub fn recast_scalar_path_projection(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        scalar_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<RecastScalarProjection> {
        let path = recast_projection_path(program, root, scalar_path)?;
        normalized_scalar_projection(program, root, &path)
    }

    pub fn recast_scalar_path_geometry(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        scalar_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<RecastScalarGeometry> {
        recast_scalar_layout(program, root)?.scalar_path_geometry(scalar_path)
    }

    fn recast_projection_path(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        carrier_path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> Option<Vec<RepresentationSegment>> {
        let mut carrier = root;
        let mut path = Vec::new();
        for segment in carrier_path {
            match segment {
                checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                    let selected = unique_record_field(program, carrier, |field| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ) == *identity
                    })?;
                    path.push(RepresentationSegment::Field(selected.symbol));
                    carrier = selected.type_reference;
                }
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    let index = usize::try_from(*index).ok()?;
                    carrier = fixed_array_element(program, carrier, index)?;
                    path.push(RepresentationSegment::Element(index));
                }
                _ => return None,
            }
        }
        Some(path)
    }

    fn normalized_scalar_projection(
        program: &dyn crate::CheckingInput,
        root: TypeReferenceHandle,
        path: &[RepresentationSegment],
    ) -> Option<RecastScalarProjection> {
        let representation = field_geometry_representation(program, root)?;
        let leaf = representation
            .leaves
            .iter()
            .find(|leaf| leaf.path == path)?;
        scalar_leaf_projection(representation.size, leaf)
    }

    fn scalar_leaf_projection(
        footprint: usize,
        leaf: &MutableRecordLeaf,
    ) -> Option<RecastScalarProjection> {
        if !leaf.byte_projection {
            return None;
        }
        Some(RecastScalarProjection {
            offset: leaf.offset,
            footprint,
            primitive: leaf.primitive,
            stored_integer: match leaf.stored_integer {
                Some((width, interpretation)) => {
                    let sign = match interpretation {
                        layout_plans::IntegerInterpretation::Signed => {
                            semantic_vocabulary::IntegerSign::Signed
                        }
                        layout_plans::IntegerInterpretation::Unsigned => {
                            semantic_vocabulary::IntegerSign::Unsigned
                        }
                    };
                    Some(semantic_vocabulary::IntegerType::new(sign, width).ok()?)
                }
                None => None,
            },
        })
    }

    pub(crate) fn unrestricted_scalar_read_projection(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<RecastScalarProjection> {
        let representation = type_representation(program, reference)?;
        let [leaf] = representation.leaves.as_slice() else {
            return None;
        };
        (leaf.path.is_empty()
            && leaf.offset == 0
            && leaf.size == representation.size
            && leaf.byte_projection
            && leaf_accepts_all_bits(leaf))
        .then_some(())?;
        scalar_leaf_projection(representation.size, leaf)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RecastLeafSegment {
        Field(symbols::SymbolHandle),
        Element(usize),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RecastRecordSourceLeaf {
        pub path: Vec<RecastLeafSegment>,
        pub primitive: typed_trees::types::PrimitiveType,
        pub view_primitive: typed_trees::types::PrimitiveType,
        pub covers_source: bool,
        pub preserves_scalar_value: bool,
        pub preserves_primitive_bits: bool,
    }

    pub fn recast_record_source_leaf(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        target: TypeReferenceHandle,
        target_field: symbols::SymbolHandle,
    ) -> Option<RecastRecordSourceLeaf> {
        recast_scalar_source_leaf(
            program,
            source,
            target,
            &[facts::PlaceSegment::Field {
                symbol: target_field,
            }],
        )
    }

    pub fn recast_scalar_source_leaf(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        target: TypeReferenceHandle,
        target_path: &[facts::PlaceSegment],
    ) -> Option<RecastRecordSourceLeaf> {
        common_scalar_source_leaf(program, source, target, target_path, |alternative| {
            type_representation(alternative, source)
        })
    }

    pub(crate) fn recast_scalar_source_leaf_in_array_region(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        start: usize,
        end: usize,
        target: TypeReferenceHandle,
        target_path: &[facts::PlaceSegment],
    ) -> Option<RecastRecordSourceLeaf> {
        common_scalar_source_leaf(program, source, target, target_path, |alternative| {
            array_region_representation(alternative, source, start, end)
        })
    }

    fn common_scalar_source_leaf(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        target: TypeReferenceHandle,
        target_path: &[facts::PlaceSegment],
        source_inventory: impl Fn(&dyn crate::CheckingInput) -> Option<MutableRecordRepresentation>,
    ) -> Option<RecastRecordSourceLeaf> {
        let targets = crate::layout_checking_targets(program, [source, target]).ok()?;
        let mut common = None;
        for target_guard in targets {
            let alternative = crate::LayoutCheckingContext::new(program, target_guard);
            let projected = scalar_source_leaf(
                &source_inventory(&alternative)?,
                &field_geometry_representation(&alternative, target)?,
                target_path,
            )?;
            if common.as_ref().is_some_and(|prior| prior != &projected) {
                return None;
            }
            common = Some(projected);
        }
        common
    }

    fn scalar_source_leaf(
        source_representation: &MutableRecordRepresentation,
        target_representation: &MutableRecordRepresentation,
        target_path: &[facts::PlaceSegment],
    ) -> Option<RecastRecordSourceLeaf> {
        let target_path = target_path
            .iter()
            .map(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => {
                    Some(RepresentationSegment::Field(*symbol))
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    Some(RepresentationSegment::Element(*index))
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        if source_representation.size != target_representation.size {
            return None;
        }
        let target_leaf = target_representation
            .leaves
            .iter()
            .find(|leaf| leaf.path == target_path)?;
        let mut matching = source_representation.leaves.iter().filter(|leaf| {
            (leaf.offset == target_leaf.offset && leaf.size == target_leaf.size)
                || (leaf.byte_projection
                    && target_leaf.byte_projection
                    && leaf.offset <= target_leaf.offset
                    && leaf.offset.checked_add(leaf.size).is_some_and(|end| {
                        target_leaf
                            .offset
                            .checked_add(target_leaf.size)
                            .is_some_and(|target_end| target_end <= end)
                    }))
        });
        let source_leaf = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        let same_extent =
            source_leaf.offset == target_leaf.offset && source_leaf.size == target_leaf.size;
        Some(RecastRecordSourceLeaf {
            path: source_leaf
                .path
                .iter()
                .map(|segment| match segment {
                    RepresentationSegment::Field(symbol) => RecastLeafSegment::Field(*symbol),
                    RepresentationSegment::Element(index) => RecastLeafSegment::Element(*index),
                })
                .collect(),
            primitive: source_leaf.primitive,
            view_primitive: target_leaf.primitive,
            covers_source: same_extent,
            preserves_primitive_bits: same_extent
                && primitive_bit_representation(source_leaf)
                && primitive_bit_representation(target_leaf),
            preserves_scalar_value: same_extent
                && source_leaf.primitive == target_leaf.primitive
                && source_leaf.byte_projection
                && target_leaf.byte_projection
                && stored_integer_interpretation(source_leaf)
                    == stored_integer_interpretation(target_leaf),
        })
    }

    fn stored_integer_interpretation(
        leaf: &MutableRecordLeaf,
    ) -> Option<(u16, layout_plans::IntegerInterpretation)> {
        leaf.stored_integer
            .or_else(|| primitive_integer_interpretation(leaf.primitive))
    }

    fn primitive_integer_interpretation(
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<(u16, layout_plans::IntegerInterpretation)> {
        let (sign, width) = primitive.integer_sign_bits()?;
        Some((
            width,
            match sign {
                semantic_vocabulary::IntegerSign::Signed => {
                    layout_plans::IntegerInterpretation::Signed
                }
                semantic_vocabulary::IntegerSign::Unsigned => {
                    layout_plans::IntegerInterpretation::Unsigned
                }
            },
        ))
    }

    fn primitive_bit_representation(leaf: &MutableRecordLeaf) -> bool {
        leaf.byte_projection
            && leaf.primitive.scalar_byte_size() == Some(leaf.size)
            && if leaf.primitive.is_fixed_integer() {
                stored_integer_interpretation(leaf)
                    == primitive_integer_interpretation(leaf.primitive)
            } else {
                matches!(
                    leaf.primitive,
                    typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64
                ) && leaf.stored_integer.is_none()
            }
    }

    pub(crate) fn type_representation(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<MutableRecordRepresentation> {
        normalized_representation(program, type_reference, None)
    }

    pub(crate) fn pending_record_fields(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<(
        &typed_trees::data::DataDefinition,
        MutableRecordRepresentation,
    )> {
        let definition = pending_record_definition(program, reference)?;
        let owner = (definition.symbol, definition.generic_instance);
        Some((
            definition,
            normalized_representation(program, reference, Some(owner))?,
        ))
    }

    pub(crate) fn pending_record_definition(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<&typed_trees::data::DataDefinition> {
        let reference = crate::value_custody::places::assignment_value_type(program, reference);
        if matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Constrained { .. }
        ) {
            return None;
        }
        let semantic = program
            .layout_storage_arguments(reference)
            .map_or(reference, |storage| storage.semantic_type);
        if matches!(
            program.type_reference_table.type_reference(semantic),
            TypeReferenceNode::Constrained { .. }
        ) {
            return None;
        }
        let definition = program.closed_data_field_definition(semantic)?;
        (!definition.requires_facts.is_empty()
            && program.type_multiplicity(semantic)
                == language_semantics::Multiplicity::Unrestricted
            && program
                .proof_facts
                .span_or_empty(definition.requires_facts)
                .iter()
                .all(|fact| fact.as_expression().is_some()))
        .then_some(definition)
    }

    fn field_geometry_representation(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<MutableRecordRepresentation> {
        pending_record_fields(program, reference)
            .map(|(_, fields)| fields)
            .or_else(|| type_representation(program, reference))
    }

    pub(crate) fn source_supports_pending_fields(
        program: &dyn crate::CheckingInput,
        source: &MutableRecordRepresentation,
        fields: &MutableRecordRepresentation,
    ) -> bool {
        compatible_backing_geometry(source, fields)
            && source.leaves.iter().all(leaf_accepts_all_bits)
            && initialized_storage::InitializedStorageBits::prepare(source)
                .is_some_and(|coverage| coverage.covers_entire_extent())
            && representation_values_imply(program, source, fields)
            && !fields.permits_shared_mutation()
    }

    pub(crate) fn fields_cover_source_extent(fields: &MutableRecordRepresentation) -> bool {
        initialized_storage::InitializedStorageBits::prepare(fields)
            .is_some_and(|coverage| coverage.covers_entire_extent())
    }

    fn normalized_representation(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        pending_predicate_owner: Option<SymbolIdentity>,
    ) -> Option<MutableRecordRepresentation> {
        let type_reference =
            crate::value_custody::places::assignment_value_type(program, type_reference);
        let mut visiting = HashSet::new();
        visiting.try_reserve(256).ok()?;
        let mut budget = RepresentationBudget {
            depth: 0,
            work: 0,
            lifetime_shell_depth: 0,
            pending_predicate_owner,
        };
        let phantom_lifetime_shell =
        crate::value_custody::recasts::record_eligibility::direct_phantom_lifetime_record_symbol(
            program,
            type_reference,
        );
        let mut representation = if let Some(symbol) = phantom_lifetime_shell {
            budget.lifetime_shell_depth = 1;
            mutable_record_representation_inner(program, symbol, &mut visiting, &mut budget, true)?
        } else {
            mutable_record_type_representation(
                program,
                type_reference,
                &mut visiting,
                &mut budget,
                false,
            )?
        };
        if phantom_lifetime_shell.is_some() && representation.size == 0 {
            return None;
        }
        representation
            .leaves
            .sort_by_key(|leaf| (leaf.offset, leaf.size));
        for leaf in &mut representation.leaves {
            if let Some((width, interpretation)) = leaf.stored_integer {
                super::scalar_representation::encode_stored_integer_facts(
                    &mut leaf.facts,
                    leaf.primitive,
                    width,
                    interpretation,
                )?;
            }
        }
        Some(representation)
    }

    pub(crate) fn array_region_representation(
        program: &dyn crate::CheckingInput,
        array: TypeReferenceHandle,
        start: usize,
        end: usize,
    ) -> Option<MutableRecordRepresentation> {
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(extent),
            ..
        } = program.type_reference_table.type_reference(array)
        else {
            return None;
        };
        if start > end || end > *extent {
            return None;
        }
        let mut representation = type_representation(program, array)?;
        let stride = if *extent == 0 {
            0
        } else {
            representation.size.checked_div(*extent)?
        };
        let offset = start.checked_mul(stride)?;
        let limit = end.checked_mul(stride)?;
        if representation.align == 0 || !offset.is_multiple_of(representation.align) {
            return None;
        }
        for leaf in &representation.leaves {
            let leaf_end = leaf.offset.checked_add(leaf.size)?;
            if leaf.offset < limit
                && offset < leaf_end
                && (leaf.offset < offset || leaf_end > limit)
            {
                return None;
            }
        }
        representation.leaves.retain_mut(|leaf| {
            if leaf.offset < offset || leaf.offset >= limit {
                return false;
            }
            leaf.offset -= offset;
            true
        });
        representation.size = limit.checked_sub(offset)?;
        Some(representation)
    }

    fn mutable_record_representation_inner(
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        visiting: &mut HashSet<SymbolIdentity>,
        budget: &mut RepresentationBudget,
        allow_lifetime_shell: bool,
    ) -> Option<MutableRecordRepresentation> {
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, symbol)?;
        mutable_record_representation_for_data(
            program,
            data,
            visiting,
            budget,
            allow_lifetime_shell,
        )
    }

    fn mutable_record_representation_for_data(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        visiting: &mut HashSet<SymbolIdentity>,
        budget: &mut RepresentationBudget,
        allow_lifetime_shell: bool,
    ) -> Option<MutableRecordRepresentation> {
        budget.enter()?;
        let representation = mutable_record_representation_inner_body(
            program,
            data,
            visiting,
            budget,
            allow_lifetime_shell,
        );
        budget.leave();
        representation
    }

    fn mutable_record_representation_inner_body(
        program: &dyn crate::CheckingInput,
        data: &typed_trees::data::DataDefinition,
        visiting: &mut HashSet<SymbolIdentity>,
        budget: &mut RepresentationBudget,
        allow_lifetime_shell: bool,
    ) -> Option<MutableRecordRepresentation> {
        if !data.symbol.is_valid() {
            return None;
        }
        let symbol_identity = (data.symbol, data.generic_instance);
        if !visiting.insert(symbol_identity) {
            return None;
        }
        let layout = program.plan_laid_layout_for_data(data);
        if layout.is_none()
            && data
                .generic_instance
                .is_some_and(|reference| program.layout_storage_arguments(reference).is_some())
        {
            visiting.remove(&symbol_identity);
            return None;
        }
        let semantic_data = match layout {
            Some(plan) => program.closed_data_field_definition(plan.semantic_type)?,
            None => data,
        };
        let predicate_pending = budget.pending_predicate_owner
            == Some((semantic_data.symbol, semantic_data.generic_instance));
        let requires_bounds = if predicate_pending {
            Some(Vec::new())
        } else {
            crate::proof_contracts::default_domains::data_requires_field_representation_bounds(
                program,
                semantic_data,
            )
        };
        let Some(requires_bounds) = requires_bounds else {
            visiting.remove(&symbol_identity);
            return None;
        };
        if semantic_data.zero_gated && requires_bounds.is_empty() && !predicate_pending {
            visiting.remove(&symbol_identity);
            return None;
        }

        let members = program.data_members(data);
        let mut fields = Vec::new();
        let mut field_types = Vec::new();
        let mut field_symbols = Vec::new();
        fields.try_reserve_exact(members.len()).ok()?;
        field_types.try_reserve_exact(members.len()).ok()?;
        field_symbols.try_reserve_exact(members.len()).ok()?;
        for member in members {
            let Some(field) = member.as_field() else {
                visiting.remove(&symbol_identity);
                return None;
            };
            if field.relevance.is_erased() {
                continue;
            }
            let Some(mut representation) = mutable_record_type_representation(
                program,
                field.type_reference,
                visiting,
                budget,
                allow_lifetime_shell,
            ) else {
                visiting.remove(&symbol_identity);
                return None;
            };
            let semantic_field = match layout {
                Some(plan) => {
                    let position = plan
                        .field_symbols
                        .iter()
                        .position(|symbol| *symbol == field.symbol)?;
                    *plan.schema_field_symbols.get(position)?
                }
                None => field.symbol,
            };
            let field_bounds = requires_bounds
                .iter()
                .filter(|bound| bound.field() == semantic_field)
                .copied()
                .collect::<Vec<_>>();
            if !field_bounds.is_empty() {
                let narrowed = match representation.leaves.as_mut_slice() {
                    [leaf] if leaf.path.is_empty() => narrow_by_field_representation_bounds(
                        program,
                        semantic_data,
                        &mut leaf.facts,
                        leaf.primitive,
                        &field_bounds,
                    ),
                    _ => None,
                };
                if narrowed.is_none() {
                    visiting.remove(&symbol_identity);
                    return None;
                }
            }
            fields.push(representation);
            field_types.push(field.type_reference);
            field_symbols.push(field.symbol);
        }

        let (size, align, offsets) = if let Some(plan) = program.plan_laid_layout_for_data(data) {
            if plan.field_symbols != field_symbols {
                return None;
            }
            let plan =
                plan.geometry_for_target(program.typed_program(), program.layout_target())?;
            if plan.offsets.len() != fields.len() {
                visiting.remove(&symbol_identity);
                return None;
            }
            for integer_field in &plan.integer_fields {
                let field = fields.get_mut(integer_field.field_index)?;
                if field.leaves.len() != 1
                    || integer_field.stored_width_bits == 0
                    || integer_field.stored_width_bits % 8 != 0
                {
                    visiting.remove(&symbol_identity);
                    return None;
                }
                let stored_size = usize::from(integer_field.stored_width_bits / 8);
                field.size = stored_size;
                field.align = field.align.min(stored_size.max(1));
                field.leaves[0].size = stored_size;
                field.leaves[0].stored_integer = Some((
                    integer_field.stored_width_bits,
                    integer_field.interpretation,
                ));
            }
            for bit_field in &plan.bit_fields {
                let field = fields.get_mut(bit_field.field_index)?;
                let [leaf] = field.leaves.as_mut_slice() else {
                    return None;
                };
                leaf.byte_projection = false;
                leaf.bit_fragments = bit_field
                    .fragments
                    .iter()
                    .map(|fragment| {
                        let mut fragment = *fragment;
                        fragment.container_byte_offset = fragment
                            .container_byte_offset
                            .checked_sub(plan.offsets[bit_field.field_index])?;
                        Some(fragment)
                    })
                    .collect::<Option<Vec<_>>>()?;
                leaf.bit_fragments.sort_by_key(|fragment| {
                    (
                        fragment.container_byte_offset,
                        fragment.container_width_bits,
                        fragment.destination_lsb,
                        fragment.source_lsb,
                        fragment.width,
                    )
                });
                let stored_size =
                    leaf.bit_fragments
                        .iter()
                        .try_fold(0usize, |extent, fragment| {
                            (fragment.container_width_bits % 8 == 0).then_some(())?;
                            let end = fragment
                                .container_byte_offset
                                .checked_add(usize::from(fragment.container_width_bits / 8))?;
                            Some(extent.max(end))
                        })?;
                field.size = stored_size;
                leaf.size = stored_size;
            }
            for repeated_field in &plan.repeated_fields {
                let field_type = *field_types.get(repeated_field.field_index)?;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(element_count),
                } = program.type_reference_table.type_reference(field_type)
                else {
                    visiting.remove(&symbol_identity);
                    return None;
                };
                let element = mutable_record_type_representation(
                    program,
                    *element_type,
                    visiting,
                    budget,
                    false,
                )?;
                let repeated =
                    repeat_representation(&element, *element_count, repeated_field.element_stride)?;
                *fields.get_mut(repeated_field.field_index)? = repeated;
            }
            if fields.iter().zip(&plan.offsets).any(|(field, offset)| {
                offset
                    .checked_add(field.size)
                    .is_none_or(|end| end > plan.size)
            }) {
                visiting.remove(&symbol_identity);
                return None;
            }
            let mut offsets = Vec::new();
            offsets.try_reserve_exact(plan.offsets.len()).ok()?;
            offsets.extend_from_slice(&plan.offsets);
            (plan.size, plan.align, offsets)
        } else {
            let mut offsets = Vec::new();
            offsets.try_reserve_exact(fields.len()).ok()?;
            let mut offset = 0usize;
            let mut max_align = 1usize;
            for field in &fields {
                offset = checked_align_up(offset, field.align)?;
                offsets.push(offset);
                offset = offset.checked_add(field.size)?;
                max_align = max_align.max(field.align);
            }
            (checked_align_up(offset, max_align)?, max_align, offsets)
        };

        let leaf_count = fields
            .iter()
            .try_fold(0usize, |count, field| count.checked_add(field.leaves.len()))?;
        budget.consume(leaf_count)?;
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(leaf_count).ok()?;
        for ((field, field_offset), symbol) in fields.into_iter().zip(offsets).zip(field_symbols) {
            for mut leaf in field.leaves {
                leaf.offset = leaf.offset.checked_add(field_offset)?;
                leaf.path.insert(0, RepresentationSegment::Field(symbol));
                leaves.push(leaf);
            }
        }
        visiting.remove(&symbol_identity);
        Some(MutableRecordRepresentation {
            size,
            align,
            leaves,
        })
    }

    fn mutable_record_type_representation(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        visiting: &mut HashSet<SymbolIdentity>,
        budget: &mut RepresentationBudget,
        allow_lifetime_shell: bool,
    ) -> Option<MutableRecordRepresentation> {
        budget.enter()?;
        let representation = mutable_record_type_representation_body(
            program,
            type_reference,
            visiting,
            budget,
            allow_lifetime_shell,
        );
        budget.leave();
        representation
    }

    fn mutable_record_type_representation_body(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
        visiting: &mut HashSet<SymbolIdentity>,
        budget: &mut RepresentationBudget,
        allow_lifetime_shell: bool,
    ) -> Option<MutableRecordRepresentation> {
        if let Some(primitive) = super::exact_scalar_representation_type(program, type_reference) {
            let size = primitive.scalar_byte_size()?;
            budget.consume(1)?;
            let mut leaves = Vec::new();
            leaves.try_reserve_exact(1).ok()?;
            leaves.push(MutableRecordLeaf {
                offset: 0,
                size,
                facts: mutable_scalar_representation_facts(program, type_reference)?,
                path: Vec::new(),
                primitive,
                stored_integer: None,
                byte_projection: true,
                bit_fragments: Vec::new(),
                arrays: Vec::new(),
            });
            return Some(MutableRecordRepresentation {
                size,
                align: size,
                leaves,
            });
        }
        match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let element = mutable_record_type_representation(
                program,
                *element_type,
                visiting,
                budget,
                false,
            )?;
            let size = element.size.checked_mul(*length)?;
            let leaf_count = element.leaves.len().checked_mul(*length)?;
            budget.consume(leaf_count)?;
            let leaves = expand_replicated_leaves(&element, *length, element.size)?;
            Some(MutableRecordRepresentation {
                size,
                align: element.align,
                leaves,
            })
        }
        TypeReferenceNode::Named { symbol, .. } => mutable_record_representation_inner(
            program,
            *symbol,
            visiting,
            budget,
            allow_lifetime_shell,
        ),
        TypeReferenceNode::Generic { .. } if program.closed_data_definition(type_reference).is_some() => {
            mutable_record_representation_for_data(program,
                program.closed_data_definition(type_reference)?, visiting, budget, allow_lifetime_shell)
        }
        TypeReferenceNode::Generic { .. }
            if allow_lifetime_shell
                && budget.lifetime_shell_depth > 0
                && budget.lifetime_shell_depth < crate::value_custody::recasts::record_eligibility::MAX_RECAST_PHANTOM_LIFETIME_SHELL_DEPTH =>
        {
            let symbol = crate::value_custody::recasts::record_eligibility::phantom_lifetime_record_symbol_shape(program, type_reference)?;
            budget.lifetime_shell_depth += 1;
            let representation = mutable_record_representation_inner(
                program,
                symbol,
                visiting,
                budget,
                allow_lifetime_shell,
            );
            budget.lifetime_shell_depth -= 1;
            representation.filter(|representation| representation.size > 0)
        }
        TypeReferenceNode::Constrained { .. } | TypeReferenceNode::Reference { .. } => None,
        _ => None,
    }
    }

    fn checked_align_up(value: usize, alignment: usize) -> Option<usize> {
        if alignment == 0 {
            return None;
        }
        let remainder = value % alignment;
        if remainder == 0 {
            Some(value)
        } else {
            value.checked_add(alignment.checked_sub(remainder)?)
        }
    }

    fn compatible_backing_geometry(
        source: &MutableRecordRepresentation,
        target: &MutableRecordRepresentation,
    ) -> bool {
        source.size == target.size && target.align != 0 && source.align.is_multiple_of(target.align)
    }

    fn leaf_accepts_all_bits(leaf: &MutableRecordLeaf) -> bool {
        if leaf.facts.atomic || !leaf.facts.domains.is_empty() {
            return false;
        }
        let bits = if leaf.byte_projection {
            leaf.size.checked_mul(8)
        } else {
            leaf.bit_fragments
                .iter()
                .try_fold(0usize, |width, fragment| {
                    (fragment.width != 0).then_some(())?;
                    Some(
                        width.max(
                            usize::from(fragment.source_lsb)
                                .checked_add(usize::from(fragment.width))?,
                        ),
                    )
                })
        };
        let Some(bits @ 1..=64) = bits else {
            return false;
        };
        let maximum = u64::MAX >> (64 - bits);
        matches!(&leaf.facts.values,
        super::scalar_representation::ScalarRepresentationSet::ExactBitPatterns(patterns)
        if patterns.as_slice() == [(0, maximum)])
    }

    fn representation_values_imply(
        program: &dyn crate::CheckingInput,
        source: &MutableRecordRepresentation,
        target: &MutableRecordRepresentation,
    ) -> bool {
        let initialized = initialized_storage::InitializedStorageBits::prepare(source);
        target.leaves.iter().all(|target| {
            let first = source
                .leaves
                .partition_point(|source| source.offset < target.offset);
            let end = source
                .leaves
                .partition_point(|source| source.offset <= target.offset);
            if source.leaves[first..end].iter().any(|source_leaf| {
                source_leaf.size == target.size
                    && source_leaf.byte_projection == target.byte_projection
                    && source_leaf.bit_fragments == target.bit_fragments
                    && scalar_representation_facts_imply(program, &source_leaf.facts, &target.facts)
            }) {
                return true;
            }
            if !leaf_accepts_all_bits(target) {
                return false;
            }
            initialized
                .as_ref()
                .is_some_and(|coverage| coverage.contains_leaf(target))
        })
    }

    pub(crate) fn mutable_record_representations_equivalent(
        program: &dyn crate::CheckingInput,
        source: &MutableRecordRepresentation,
        target: &MutableRecordRepresentation,
    ) -> bool {
        compatible_backing_geometry(source, target)
            && representation_values_imply(program, source, target)
            && representation_values_imply(program, target, source)
    }

    pub(crate) fn recast_representations_equivalent(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        target: TypeReferenceHandle,
        stored_scalar: Option<semantic_vocabulary::IntegerType>,
    ) -> bool {
        let Some(mut source) = type_representation(program, source) else {
            return false;
        };
        let Some(target) = type_representation(program, target) else {
            return false;
        };
        if let Some(stored) = stored_scalar {
            let [leaf] = source.leaves.as_mut_slice() else {
                return false;
            };
            if !leaf.path.is_empty() || stored.bits() == 0 || stored.bits() % 8 != 0 {
                return false;
            }
            let interpretation = match stored.sign() {
                semantic_vocabulary::IntegerSign::Signed => {
                    layout_plans::IntegerInterpretation::Signed
                }
                semantic_vocabulary::IntegerSign::Unsigned => {
                    layout_plans::IntegerInterpretation::Unsigned
                }
            };
            if super::scalar_representation::encode_stored_integer_facts(
                &mut leaf.facts,
                leaf.primitive,
                stored.bits(),
                interpretation,
            )
            .is_none()
            {
                return false;
            }
            let size = usize::from(stored.bits() / 8);
            source.size = size;
            source.align = source.align.min(size);
            leaf.size = size;
        }
        mutable_record_representations_equivalent(program, &source, &target)
    }

    pub(crate) fn repeat_representation(
        element: &MutableRecordRepresentation,
        count: usize,
        stride: usize,
    ) -> Option<MutableRecordRepresentation> {
        if count > 1 && stride < element.size {
            return None;
        }
        let size = if count == 0 {
            0
        } else {
            stride
                .checked_mul(count.checked_sub(1)?)?
                .checked_add(element.size)?
        };
        let leaves = expand_replicated_leaves(element, count, stride)?;
        Some(MutableRecordRepresentation {
            size,
            align: element.align,
            leaves,
        })
    }

    pub(crate) fn representation_is_exactly_tiled(
        representation: &MutableRecordRepresentation,
    ) -> bool {
        let mut cursor = 0usize;
        for leaf in &representation.leaves {
            if leaf.offset != cursor || leaf.size == 0 {
                return false;
            }
            let Some(next) = cursor.checked_add(leaf.size) else {
                return false;
            };
            cursor = next;
        }
        cursor == representation.size
    }

    pub(crate) fn record_representation_implies(
        program: &dyn crate::CheckingInput,
        source: &MutableRecordRepresentation,
        target: &MutableRecordRepresentation,
    ) -> bool {
        compatible_backing_geometry(source, target)
            && representation_values_imply(program, source, target)
    }

    #[cfg(test)]
    mod representation_implication_tests {
        use super::super::scalar_representation::ScalarRepresentationSet;
        use super::*;
        use typed_trees::types::PrimitiveType;

        fn leaf(offset: usize, size: usize, values: (u64, u64)) -> MutableRecordLeaf {
            MutableRecordLeaf {
                offset,
                size,
                facts: MutableScalarRepresentationFacts {
                    atomic: false,
                    domains: Vec::new(),
                    values: ScalarRepresentationSet::ExactBitPatterns(vec![values]),
                },
                path: Vec::new(),
                primitive: PrimitiveType::U64,
                stored_integer: None,
                byte_projection: true,
                bit_fragments: Vec::new(),
                arrays: Vec::new(),
            }
        }

        fn bytes(count: usize) -> MutableRecordRepresentation {
            MutableRecordRepresentation {
                size: count,
                align: 1,
                leaves: (0..count).map(|offset| leaf(offset, 1, (0, 255))).collect(),
            }
        }

        #[test]
        fn established_words_allow_byte_observation_but_not_arbitrary_restoration() {
            let program = typed_trees::TypedTrees::default();
            let mut source = MutableRecordRepresentation {
                size: 16,
                align: 8,
                leaves: vec![leaf(0, 8, (0, u64::MAX)), leaf(8, 8, (42, 42))],
            };
            assert!(record_representation_implies(&program, &source, &bytes(16)));
            assert!(!mutable_record_representations_equivalent(
                &program,
                &source,
                &bytes(16)
            ));
            source.leaves[1].facts.values =
                ScalarRepresentationSet::ExactBitPatterns(vec![(0, u64::MAX)]);
            assert!(mutable_record_representations_equivalent(
                &program,
                &source,
                &bytes(16)
            ));
            assert!(!record_representation_implies(
                &program,
                &bytes(16),
                &source
            ));
        }

        #[test]
        fn regrouping_initialized_storage_preserves_alignment_and_validity() {
            let program = typed_trees::TypedTrees::default();
            let source = MutableRecordRepresentation {
                size: 4,
                align: 4,
                leaves: vec![leaf(0, 2, (0, 65535)), leaf(2, 2, (0, 65535))],
            };
            let mut target = MutableRecordRepresentation {
                size: 4,
                align: 4,
                leaves: vec![leaf(0, 4, (0, u64::from(u32::MAX)))],
            };
            assert!(mutable_record_representations_equivalent(
                &program, &source, &target
            ));
            target.leaves[0].facts.values = ScalarRepresentationSet::ExactBitPatterns(vec![(0, 1)]);
            assert!(!record_representation_implies(&program, &source, &target));
        }

        #[test]
        fn equal_logical_facts_require_the_same_fragment_encoding() {
            let program = typed_trees::TypedTrees::default();
            let mut source = MutableRecordRepresentation {
                size: 8,
                align: 8,
                leaves: vec![leaf(0, 8, (1, 1))],
            };
            source.leaves[0].byte_projection = false;
            source.leaves[0].bit_fragments = vec![
                typed_trees::PlanLaidBitFragment {
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 0,
                    source_lsb: 0,
                    width: 32,
                },
                typed_trees::PlanLaidBitFragment {
                    container_byte_offset: 0,
                    container_width_bits: 64,
                    destination_lsb: 32,
                    source_lsb: 32,
                    width: 32,
                },
            ];
            let mut target = source.clone();
            target.align = 1;
            assert!(record_representation_implies(&program, &source, &target));
            target.leaves[0].bit_fragments[0].source_lsb = 32;
            target.leaves[0].bit_fragments[1].source_lsb = 0;
            assert!(!record_representation_implies(&program, &source, &target));
        }

        #[test]
        fn padding_atomic_protocol_and_fragmented_encodings_are_not_raw_observations() {
            let program = typed_trees::TypedTrees::default();
            let padded = MutableRecordRepresentation {
                size: 3,
                align: 1,
                leaves: vec![leaf(0, 1, (0, 255)), leaf(2, 1, (0, 255))],
            };
            assert!(!record_representation_implies(&program, &padded, &bytes(3)));
            let mut source = MutableRecordRepresentation {
                size: 8,
                align: 8,
                leaves: vec![leaf(0, 8, (0, u64::MAX))],
            };
            source.leaves[0].facts.atomic = true;
            assert!(!record_representation_implies(&program, &source, &bytes(8)));
            source.leaves[0].facts.atomic = false;
            source.leaves[0].byte_projection = false;
            assert!(!record_representation_implies(&program, &source, &bytes(8)));
        }
    }
}
pub(crate) mod slice_views {
    use typed_trees::{
        expression::ExpressionHandle,
        types::{TypeReferenceHandle, TypeReferenceNode},
    };

    #[derive(Debug, Clone, Copy)]
    pub struct SliceRecastSource {
        pub source: ExpressionHandle,
        pub element: TypeReferenceHandle,
        pub access: language_core::ReferenceAccess,
    }

    pub fn slice_recast_source(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<SliceRecastSource> {
        let formation = super::reference_formation_source(program, expression)?;
        if !formation.view_type.is_valid() {
            return None;
        }
        let TypeReferenceNode::Slice { element_type } = program
            .type_reference_table
            .type_reference(formation.view_type)
        else {
            return None;
        };
        Some(SliceRecastSource {
            source: formation.source,
            element: *element_type,
            access: formation.access,
        })
    }
}
pub(crate) mod source_regions {
    use super::record_representation::{
        MutableRecordRepresentation, array_region_representation, type_representation,
    };
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    pub(crate) fn source_layout_root(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        if let Some(operation) = crate::value_custody::places::subslice_expression(program, source)
        {
            let indexed = program.expression_table.indexed_node(operation)?;
            crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                indexed.collection,
            )
        } else {
            crate::value_custody::places::declared_place_type_raw(
                program,
                machine,
                Some(state),
                source,
            )
        }
    }

    pub(crate) fn source_representation(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        source: ExpressionHandle,
    ) -> Result<MutableRecordRepresentation, &'static str> {
        let places = crate::value_custody::places::declared_place_type_raw;
        let Some(operation) = crate::value_custody::places::subslice_expression(program, source)
        else {
            let reference = places(program, machine, Some(state), source)
                .ok_or("recast source has no declaration-owned storage type")?;
            return type_representation(program, reference)
                .ok_or("recast source has no fixed representation inventory");
        };
        if !crate::value_custody::places::has_builtin_subslice_meaning(
            program,
            machine,
            Some(state),
            operation,
        ) {
            return Err("recast source range has no builtin storage correspondence");
        }
        let indexed = program.expression_table.indexed_node(operation).unwrap();
        let range = program.expression_table.range_node(indexed.index).unwrap();
        let collection = places(program, machine, Some(state), indexed.collection)
            .ok_or("recast source window has no declaration-owned collection")?;
        let collection = crate::value_custody::places::assignment_value_type(program, collection);
        if matches!(
            program.type_reference_table.type_reference(collection),
            TypeReferenceNode::Constrained { .. }
        ) {
            return Err("recast source window must retain its collection's qualifications");
        }
        let TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(extent),
            ..
        } = program.type_reference_table.type_reference(collection)
        else {
            return Err("recast source window needs a proved fixed collection extent");
        };
        let coordinate = |expression| {
            crate::value_custody::places::window_coordinate(program, expression)
                .ok_or("recast source window endpoint is not statically exact")
        };
        let start = if range.start.is_valid() {
            coordinate(range.start)?
        } else {
            0
        };
        let end = if range.end.is_valid() {
            let end = coordinate(range.end)?;
            if range.end_inclusive {
                end.checked_add(1)
                    .ok_or("recast source window endpoint overflows")?
            } else {
                end
            }
        } else if range.end_inclusive {
            return Err("an inclusive recast source window needs an end coordinate");
        } else {
            *extent
        };
        if start > end || end > *extent {
            return Err("recast source window lies outside its declared collection extent");
        }
        array_region_representation(program, collection, start, end)
            .ok_or("recast source window has no bounded fixed representation inventory")
    }
}
use language_core::ReferenceAccess;
pub(crate) use record_representation::recast_representations_equivalent;
pub(crate) use record_representation::recast_scalar_source_leaf_in_array_region;
pub use record_representation::{
    RecastLeafSegment, RecastRecordSourceLeaf, RecastRuntimeIndexGeometry, RecastScalarGeometry,
    RecastScalarLayout, RecastScalarProjection, recast_record_source_leaf, recast_scalar_layout,
    recast_scalar_path_geometry, recast_scalar_path_projection, recast_scalar_projection,
    recast_scalar_source_leaf,
};
use typed_trees::expression::AccessBorrowBacking;
use typed_trees::typed_trees::AccessExecutionDeclaration;
pub(crate) mod representation_types {
    use symbols::BuiltinTypeAtom;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    pub(crate) fn scalar_is_atomic(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<bool> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        exact_scalar_representation_type(program, reference)?;
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        Some(program.symbols.builtin_type_atom(*symbol)?.is_atomic())
    }

    pub(crate) fn exact_primitive_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        let TypeReferenceNode::Named { symbol, name } =
            program.type_reference_table.type_reference(type_reference)
        else {
            return None;
        };
        let atom = program.symbols.builtin_type_atom(*symbol)?;
        let primitive = match atom {
            BuiltinTypeAtom::Bool => PrimitiveType::Bool,
            BuiltinTypeAtom::I8 => PrimitiveType::I8,
            BuiltinTypeAtom::I16 => PrimitiveType::I16,
            BuiltinTypeAtom::I32 => PrimitiveType::I32,
            BuiltinTypeAtom::I64 => PrimitiveType::I64,
            BuiltinTypeAtom::U8 => PrimitiveType::U8,
            BuiltinTypeAtom::U16 => PrimitiveType::U16,
            BuiltinTypeAtom::U32 => PrimitiveType::U32,
            BuiltinTypeAtom::U64 => PrimitiveType::U64,
            BuiltinTypeAtom::Address => PrimitiveType::Addr,
            BuiltinTypeAtom::F32 => PrimitiveType::F32,
            BuiltinTypeAtom::F64 => PrimitiveType::F64,
            _ => return None,
        };
        (name.as_str() == atom.symbol_name()).then_some(primitive)
    }

    pub(crate) fn exact_scalar_representation_type(
        program: &dyn crate::CheckingInput,
        type_reference: TypeReferenceHandle,
    ) -> Option<PrimitiveType> {
        let (symbol, name) = match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                return exact_scalar_representation_type(program, *base_type);
            }
            TypeReferenceNode::Named { symbol, name } => (symbol, name),
            _ => return None,
        };
        let atom = program.symbols.builtin_type_atom(*symbol)?;
        let primitive = match atom {
            BuiltinTypeAtom::Bool | BuiltinTypeAtom::AtomicBool => PrimitiveType::Bool,
            BuiltinTypeAtom::I8 => PrimitiveType::I8,
            BuiltinTypeAtom::I16 => PrimitiveType::I16,
            BuiltinTypeAtom::I32 => PrimitiveType::I32,
            BuiltinTypeAtom::I64 => PrimitiveType::I64,
            BuiltinTypeAtom::U8 => PrimitiveType::U8,
            BuiltinTypeAtom::U16 => PrimitiveType::U16,
            BuiltinTypeAtom::U32 | BuiltinTypeAtom::AtomicU32 => PrimitiveType::U32,
            BuiltinTypeAtom::U64 | BuiltinTypeAtom::AtomicU64 => PrimitiveType::U64,
            BuiltinTypeAtom::Address => PrimitiveType::Addr,
            BuiltinTypeAtom::F32 => PrimitiveType::F32,
            BuiltinTypeAtom::F64 => PrimitiveType::F64,
            _ => return None,
        };
        (name.as_str() == atom.symbol_name()).then_some(primitive)
    }
}
pub(crate) mod scalar_representation {
    use symbols::SymbolHandle;
    use typed_trees::types::TypeConstraintNode;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct MutableScalarRepresentationFacts {
        pub(crate) atomic: bool,
        pub(crate) domains: Vec<SymbolHandle>,
        pub(crate) values: ScalarRepresentationSet,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum ScalarRepresentationSet {
        ExactBitPatterns(Vec<(u64, u64)>),
        FloatInterval {
            primitive: PrimitiveType,
            minimum: FloatEndpoint,
            maximum: FloatEndpoint,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct FloatEndpoint(u64);

    impl FloatEndpoint {
        const NEGATIVE_INFINITY: Self = Self(f64::NEG_INFINITY.to_bits());
        const POSITIVE_INFINITY: Self = Self(f64::INFINITY.to_bits());

        fn new(value: f64) -> Option<Self> {
            if value.is_nan() {
                return None;
            }
            Some(Self(if value == 0.0 { 0.0f64 } else { value }.to_bits()))
        }

        fn from_integer(primitive: PrimitiveType, value: i64) -> Option<Self> {
            const EXACT_IN_F64: i64 = 1 << 53;
            if !(-EXACT_IN_F64..=EXACT_IN_F64).contains(&value) {
                return None;
            }
            let widened = value as f64;
            if primitive == PrimitiveType::F32 && f64::from(widened as f32) != widened {
                return None;
            }
            Self::new(widened)
        }

        fn value(self) -> f64 {
            f64::from_bits(self.0)
        }

        fn maximum(self, other: Self) -> Self {
            if other.value() > self.value() {
                other
            } else {
                self
            }
        }

        fn minimum(self, other: Self) -> Self {
            if other.value() < self.value() {
                other
            } else {
                self
            }
        }
    }

    pub(crate) fn mutable_scalar_representation_facts(
        program: &dyn crate::CheckingInput,
        mut type_reference: TypeReferenceHandle,
    ) -> Option<MutableScalarRepresentationFacts> {
        let mut domains = Vec::new();
        loop {
            match program.type_reference_table.type_reference(type_reference) {
                TypeReferenceNode::Reference { referee, .. } => {
                    type_reference = *referee;
                }
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    for constraint in program.type_reference_table.constraints(*constraints) {
                        match constraint {
                            TypeConstraintNode::Domain(domain) if domain.symbol.is_valid() => {
                                if !domains.contains(&domain.symbol) {
                                    domains.push(domain.symbol);
                                }
                            }
                            TypeConstraintNode::ArithmeticDomain(_) => {}

                            TypeConstraintNode::Domain(_) | TypeConstraintNode::Named(_) => {
                                return None;
                            }
                        }
                    }
                    type_reference = *base_type;
                }
                TypeReferenceNode::Named { symbol, name }
                    if PrimitiveType::from_name(name.as_str()).is_some() =>
                {
                    let primitive = PrimitiveType::from_name(name.as_str())?;
                    let values = match primitive {
                        PrimitiveType::Bool => {
                            ScalarRepresentationSet::ExactBitPatterns(vec![(0, 1)])
                        }
                        _ => ScalarRepresentationSet::ExactBitPatterns(full_scalar_bit_patterns(
                            primitive,
                        )),
                    };
                    let atomic = program
                        .symbols
                        .builtin_type_atom(*symbol)
                        .is_some_and(|atom| atom.is_atomic());
                    return Some(MutableScalarRepresentationFacts {
                        atomic,
                        domains,
                        values,
                    });
                }
                _ => return None,
            }
        }
    }

    pub(crate) fn encode_stored_integer_facts(
        facts: &mut MutableScalarRepresentationFacts,
        primitive: PrimitiveType,
        width: u16,
        interpretation: layout_plans::IntegerInterpretation,
    ) -> Option<()> {
        let carrier_width = u16::try_from(primitive.scalar_byte_size()?.checked_mul(8)?).ok()?;
        if !primitive.accepts_integer_literal() || width == 0 || width > carrier_width {
            return None;
        }
        let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values else {
            return None;
        };
        if width == 64 {
            return Some(());
        }
        let mask = (1u64 << width) - 1;
        let (minimum, maximum) = match interpretation {
            layout_plans::IntegerInterpretation::Signed => {
                let half = 1i64 << (width - 1);
                (-half, half - 1)
            }
            layout_plans::IntegerInterpretation::Unsigned => (0, i64::try_from(mask).ok()?),
        };
        let admitted =
            integer_range_bit_patterns(primitive, (i128::from(minimum), i128::from(maximum)))?;
        let fitting = intersect_bit_pattern_intervals(std::mem::take(values), &admitted);
        *values = normalize_bit_pattern_intervals(
            fitting
                .into_iter()
                .map(|(lower, upper)| (lower & mask, upper & mask))
                .collect(),
        );
        Some(())
    }

    pub(crate) fn full_scalar_bit_patterns(primitive: PrimitiveType) -> Vec<(u64, u64)> {
        let bit_count = primitive
            .scalar_byte_size()
            .expect("scalar primitive must have a byte size")
            * 8;
        let maximum = if bit_count == 64 {
            u64::MAX
        } else {
            (1u64 << bit_count) - 1
        };
        vec![(0, maximum)]
    }

    fn integer_range_bit_patterns(
        primitive: PrimitiveType,
        (minimum, maximum): (i128, i128),
    ) -> Option<Vec<(u64, u64)>> {
        if minimum > maximum || !primitive.accepts_integer_literal() {
            return None;
        }
        let bit_count = primitive.scalar_byte_size()? * 8;
        let (primitive_minimum, primitive_maximum) = primitive_integer_extent(primitive)?;
        if minimum < primitive_minimum || maximum > primitive_maximum {
            return None;
        }
        let mask = if bit_count == 64 {
            u64::MAX
        } else {
            (1u64 << bit_count) - 1
        };
        let signed = primitive.is_signed_integer();
        if signed {
            let bits = |value: i128| (value as u64) & mask;
            return Some(normalize_bit_pattern_intervals(
                if maximum < 0 || minimum >= 0 {
                    vec![(bits(minimum), bits(maximum))]
                } else {
                    vec![(0, bits(maximum)), (bits(minimum), mask)]
                },
            ));
        }

        Some(vec![(
            u64::try_from(minimum).ok()?,
            u64::try_from(maximum).ok()?,
        )])
    }

    pub(crate) fn narrow_to_integer_interval(
        facts: &mut MutableScalarRepresentationFacts,
        primitive: PrimitiveType,
        minimum: Option<i64>,
        maximum: Option<i64>,
    ) {
        let (Some(minimum), Some(maximum)) = (minimum, maximum) else {
            return;
        };
        if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            if let (Some(minimum), Some(maximum)) = (
                FloatEndpoint::from_integer(primitive, minimum),
                FloatEndpoint::from_integer(primitive, maximum),
            ) {
                narrow_to_float_interval(facts, primitive, minimum, maximum);
            }
            return;
        }
        if let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values
            && let Some(interval) =
                integer_range_bit_patterns(primitive, (i128::from(minimum), i128::from(maximum)))
        {
            *values = intersect_bit_pattern_intervals(std::mem::take(values), &interval);
        }
    }

    fn narrow_to_float_interval(
        facts: &mut MutableScalarRepresentationFacts,
        primitive: PrimitiveType,
        minimum: FloatEndpoint,
        maximum: FloatEndpoint,
    ) {
        match &mut facts.values {
            ScalarRepresentationSet::ExactBitPatterns(_) => {
                facts.values = ScalarRepresentationSet::FloatInterval {
                    primitive,
                    minimum,
                    maximum,
                };
            }
            ScalarRepresentationSet::FloatInterval {
                minimum: facts_minimum,
                maximum: facts_maximum,
                ..
            } => {
                *facts_minimum = facts_minimum.maximum(minimum);
                *facts_maximum = facts_maximum.minimum(maximum);
            }
        }
    }

    pub(crate) fn narrow_by_field_representation_bounds(
        program: &dyn crate::CheckingInput,
        definition: &typed_trees::data::DataDefinition,
        facts: &mut MutableScalarRepresentationFacts,
        primitive: PrimitiveType,
        bounds: &[crate::proof_contracts::default_domains::FieldRepresentationBound],
    ) -> Option<()> {
        use crate::proof_contracts::default_domains::FieldRepresentationBound;
        use typed_trees::expression::BinaryOperator;
        if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
            let mut minimum = FloatEndpoint::NEGATIVE_INFINITY;
            let mut maximum = FloatEndpoint::POSITIVE_INFINITY;
            for bound in bounds {
                let FieldRepresentationBound::FloatLiteral(bound) = bound else {
                    return None;
                };
                let meaning =
                    crate::proof_contracts::default_domains::float_field_literal_bound_meaning(
                        program,
                        definition,
                        bound.conjunct,
                    )?;
                let value = FloatEndpoint::new(meaning.to_f64())?;
                match bound.operator {
                    BinaryOperator::LessOrEqual => maximum = maximum.minimum(value),
                    BinaryOperator::GreaterOrEqual => minimum = minimum.maximum(value),
                    BinaryOperator::Equal => {
                        minimum = minimum.maximum(value);
                        maximum = maximum.minimum(value);
                    }
                    _ => return None,
                }
            }
            if minimum.value() > maximum.value() {
                return None;
            }
            narrow_to_float_interval(facts, primitive, minimum, maximum);
            return Some(());
        }
        if !primitive.accepts_integer_literal() {
            return None;
        }
        let ScalarRepresentationSet::ExactBitPatterns(values) = &mut facts.values else {
            return None;
        };
        for bound in bounds {
            let FieldRepresentationBound::Integer { interval, .. } = bound else {
                return None;
            };
            let (carrier_minimum, carrier_maximum) = primitive_integer_extent(primitive)?;
            let minimum = interval
                .low
                .map(i128::from)
                .unwrap_or(carrier_minimum)
                .max(carrier_minimum);
            let maximum = interval
                .high
                .map(i128::from)
                .unwrap_or(carrier_maximum)
                .min(carrier_maximum);
            let interval = integer_range_bit_patterns(primitive, (minimum, maximum))?;
            *values = intersect_bit_pattern_intervals(std::mem::take(values), &interval);
        }
        if values.is_empty() {
            return None;
        }
        *values = normalize_bit_pattern_intervals(std::mem::take(values));
        Some(())
    }

    fn primitive_integer_extent(primitive: PrimitiveType) -> Option<(i128, i128)> {
        let bit_count = primitive.scalar_byte_size()? * 8;
        if !primitive.accepts_integer_literal() || !(1..=64).contains(&bit_count) {
            return None;
        }
        if primitive.is_signed_integer() {
            let half = 1i128 << (bit_count - 1);
            return Some((-half, half - 1));
        }
        Some((0, (1i128 << bit_count) - 1))
    }

    fn intersect_bit_pattern_intervals(
        source: Vec<(u64, u64)>,
        interval: &[(u64, u64)],
    ) -> Vec<(u64, u64)> {
        let mut intersected = Vec::new();
        for (low, high) in source {
            for &(interval_low, interval_high) in interval {
                let low = low.max(interval_low);
                let high = high.min(interval_high);
                if low <= high {
                    intersected.push((low, high));
                }
            }
        }
        intersected
    }

    fn normalize_bit_pattern_intervals(mut intervals: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
        intervals.sort_unstable_by_key(|&(low, high)| (low, high));
        let mut normalized: Vec<(u64, u64)> = Vec::with_capacity(intervals.len());
        for (low, high) in intervals {
            if let Some((_, previous_high)) = normalized.last_mut()
                && low <= previous_high.saturating_add(1)
            {
                *previous_high = (*previous_high).max(high);
            } else {
                normalized.push((low, high));
            }
        }
        normalized
    }

    pub(crate) fn mutable_scalar_representation_facts_equivalent(
        program: &dyn crate::CheckingInput,
        source: &MutableScalarRepresentationFacts,
        target: &MutableScalarRepresentationFacts,
    ) -> bool {
        if source.values != target.values {
            return false;
        }
        let implies = |sources: &[SymbolHandle], targets: &[SymbolHandle]| {
            targets.iter().all(|target| {
                sources.iter().any(|source| {
                    typed_trees::domain::declared_domain_implies(program, *source, *target)
                })
            })
        };
        implies(&source.domains, &target.domains) && implies(&target.domains, &source.domains)
    }

    pub(crate) fn scalar_representation_facts_imply(
        program: &dyn crate::CheckingInput,
        source: &MutableScalarRepresentationFacts,
        target: &MutableScalarRepresentationFacts,
    ) -> bool {
        if source.atomic != target.atomic {
            return false;
        }
        if target.atomic {
            return mutable_scalar_representation_facts_equivalent(program, source, target);
        }
        let domains_imply = target.domains.iter().all(|target| {
            source.domains.iter().any(|source| {
                typed_trees::domain::declared_domain_implies(program, *source, *target)
            })
        });
        domains_imply && scalar_representation_set_implies(&source.values, &target.values)
    }

    fn scalar_representation_set_implies(
        source: &ScalarRepresentationSet,
        target: &ScalarRepresentationSet,
    ) -> bool {
        match (source, target) {
            (
                ScalarRepresentationSet::ExactBitPatterns(source),
                ScalarRepresentationSet::ExactBitPatterns(target),
            ) => source.iter().all(|(source_low, source_high)| {
                target.iter().any(|(target_low, target_high)| {
                    target_low <= source_low && source_high <= target_high
                })
            }),
            (
                ScalarRepresentationSet::FloatInterval {
                    primitive: source_primitive,
                    minimum: source_minimum,
                    maximum: source_maximum,
                },
                ScalarRepresentationSet::FloatInterval {
                    primitive: target_primitive,
                    minimum: target_minimum,
                    maximum: target_maximum,
                },
            ) => {
                source_primitive == target_primitive
                    && target_minimum.value() <= source_minimum.value()
                    && source_maximum.value() <= target_maximum.value()
            }
            (
                ScalarRepresentationSet::FloatInterval { primitive, .. },
                ScalarRepresentationSet::ExactBitPatterns(target),
            ) => target == &full_scalar_bit_patterns(*primitive),
            _ => false,
        }
    }
}
pub(crate) mod storage_interpretation {
    use checked_trees::CheckedReferenceStorage;
    use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

    pub fn reference_view_storage(
        program: &dyn crate::CheckingInput,
        source: TypeReferenceHandle,
        referent: TypeReferenceHandle,
    ) -> Option<CheckedReferenceStorage> {
        let source_identity = crate::stored_structural_type_identity(program, source, &[], &[]);
        let referent_identity = crate::stored_structural_type_identity(program, referent, &[], &[]);
        if source_identity.is_valid() && source_identity == referent_identity {
            return Some(CheckedReferenceStorage::Declared);
        }
        let referent = program
            .type_reference_table
            .unconstrained_type_reference(referent);
        let stored_scalar = if let Some(primitive) = program.primitive_type_reference(referent) {
            let geometry = super::recast_scalar_path_projection(program, referent, &[])?;
            Some(match geometry.stored_integer {
                Some(integer) => ScalarType::Integer(integer),
                None => primitive_storage_scalar(primitive)?,
            })
        } else {
            for target in crate::layout_checking_targets(program, [referent]).ok()? {
                let context = crate::LayoutCheckingContext::new(program, target);
                super::recast_scalar_layout(&context, referent)?;
            }
            None
        };
        Some(CheckedReferenceStorage::View { stored_scalar })
    }

    fn primitive_storage_scalar(primitive: PrimitiveType) -> Option<ScalarType> {
        match primitive {
            PrimitiveType::Bool => Some(ScalarType::Boolean),
            PrimitiveType::F32 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
            PrimitiveType::F64 => Some(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
            primitive if primitive.is_fixed_integer() => {
                let sign = if primitive.is_signed_integer() {
                    IntegerSign::Signed
                } else {
                    IntegerSign::Unsigned
                };
                let bits = u16::try_from(primitive.scalar_byte_size()?.checked_mul(8)?).ok()?;
                IntegerType::new(sign, bits).ok().map(ScalarType::Integer)
            }
            _ => None,
        }
    }
}
pub use storage_interpretation::reference_view_storage;

pub(crate) use record_representation::unrestricted_scalar_read_projection;
pub(crate) use representation_types::{exact_primitive_type, exact_scalar_representation_type};

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub struct ReferenceFormationSource {
    pub source: ExpressionHandle,
    pub access: ReferenceAccess,
    pub view_type: TypeReferenceHandle,
    pub interpretation: ReferenceFormationInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceFormationInterpretation {
    Declared,
    RepresentationView,
    Access,
    ResidentAccess {
        extent: symbols::SymbolHandle,
        granted: symbols::SymbolHandle,
        resident: symbols::SymbolHandle,
    },
}

pub fn access_borrow_has_empty_write_frame(
    program: &dyn crate::CheckingInput,
    target: symbols::SymbolHandle,
) -> bool {
    let machine = program.machine_by_symbol(target).or_else(|| {
        crate::machine_calls::calls::machine_state_by_symbol(program, target)
            .map(|(machine, _)| machine)
    });
    machine.is_some_and(|machine| {
        program.access_borrow_factory(machine.symbol).is_some()
            || AccessExecutionDeclaration::authenticate(program, machine.symbol).is_ok_and(
                |declaration| {
                    declaration.is_some_and(|declaration| {
                        declaration.operation()
                            == typed_trees::typed_trees::AccessLifecycleOperation::View
                    })
                },
            )
    })
}

impl ReferenceFormationInterpretation {
    pub fn is_access_view(self) -> bool {
        matches!(self, Self::Access | Self::ResidentAccess { .. })
    }

    pub fn access_storage(
        self,
        program: &dyn crate::CheckingInput,
        endpoint: TypeReferenceHandle,
        plan: &typed_trees::typed_trees::AccessViewPlan,
    ) -> Option<checked_trees::CheckedReferenceStorage> {
        use checked_trees::CheckedReferenceStorage;
        let endpoint_carrier = program.type_reference_table.peeled_type_reference(endpoint);
        let TypeReferenceNode::Named { symbol, .. } = program
            .type_reference_table
            .type_reference(endpoint_carrier)
        else {
            return None;
        };
        match self {
            Self::Access => {
                (*symbol == plan.schema_symbol).then_some(CheckedReferenceStorage::Access)
            }
            Self::ResidentAccess {
                extent,
                granted,
                resident,
            } if *symbol == extent => {
                let owner = crate::value_custody::resident_content::resident_owner_qualifications(
                    program, endpoint,
                )?;
                if owner.extent != extent
                    || owner.granted_symbol != granted
                    || owner.resident_symbol != resident
                {
                    return None;
                }
                let exact_argument = |argument, expected| {
                    matches!(
                        program.type_reference_table.type_reference(argument),
                        TypeReferenceNode::Named { symbol, .. } if *symbol == expected
                    )
                };
                if !exact_argument(owner.policy, plan.policy_symbol)
                    || !exact_argument(owner.schema, plan.schema_symbol)
                {
                    return None;
                }
                Some(CheckedReferenceStorage::ResidentAccess {
                    extent: owner.extent,
                    granted: owner.granted,
                    resident: owner.resident,
                })
            }
            _ => None,
        }
    }
}

pub fn reference_formation_source(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ReferenceFormationSource> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let Some(cast) = program.expression_table.cast_node(expression)
        && cast.form == language_core::CastForm::Value
        && cast.semantic_domain.is_empty()
        && let Some(target) = crate::reference_shell(program, cast.target_type)
    {
        let mut formation = reference_formation_source(program, cast.value)?;
        formation.access = target.access;
        if formation.view_type.is_valid() {
            formation.view_type = cast.target_type;
        }
        return Some(formation);
    }
    if let Some(call) = program.expression_table.call_node(expression) {
        let selected = program.machine_by_symbol(call.target_symbol).or_else(|| {
            crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                .map(|(machine, _)| machine)
        })?;
        let request = program.access_borrow_factory(selected.symbol)?;
        let result = crate::machine_calls::calls::call_result_type(program, expression)?;
        let shell = crate::reference_shell(program, result)?;
        if shell.access != request.access {
            return None;
        }
        let referee = program
            .type_reference_table
            .peeled_type_reference(shell.referee);
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(referee)
        else {
            return None;
        };
        let mut plans = program
            .access_view_plans
            .iter()
            .filter(|plan| plan.data_symbol == *symbol);
        let (Some(_), None) = (plans.next(), plans.next()) else {
            return None;
        };
        let [argument] = program.expression_table.expression_handles(call.arguments) else {
            return None;
        };
        let source = match program.expression_table.expression(*argument) {
            ExpressionNode::Borrow(borrow) => {
                if !matches!(
                    (borrow.access, request.access),
                    (ReferenceAccess::Mutable, _)
                        | (ReferenceAccess::Shared, ReferenceAccess::Shared)
                ) {
                    return None;
                }
                borrow.target
            }
            _ => *argument,
        };
        return Some(ReferenceFormationSource {
            source,
            access: request.access,
            view_type: shell.referee,
            interpretation: match request.backing {
                AccessBorrowBacking::Ordinary => ReferenceFormationInterpretation::Access,
                AccessBorrowBacking::Resident {
                    extent,
                    granted,
                    resident,
                } => ReferenceFormationInterpretation::ResidentAccess {
                    extent,
                    granted,
                    resident,
                },
            },
        });
    }
    let (operand, borrowed_access) = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => (borrow.target, Some(borrow.access)),
        _ => (expression, None),
    };
    if let Some(cast) = program.expression_table.cast_node(operand) {
        let access = cast.form.recast_access()?;
        if borrowed_access.is_some_and(|borrowed| borrowed != access) {
            return None;
        }
        return Some(ReferenceFormationSource {
            source: strip_mutable(program, cast.value),
            access,
            view_type: cast.target_type,
            interpretation: ReferenceFormationInterpretation::RepresentationView,
        });
    }
    Some(ReferenceFormationSource {
        source: operand,
        access: borrowed_access?,
        view_type: TypeReferenceHandle::default(),
        interpretation: ReferenceFormationInterpretation::Declared,
    })
}

use crate::value_custody::recasts::recast_judgments::strip_mutable;

pub use raw_byte_region::{SliceRecastByteRegion, slice_recast_byte_region};
pub use slice_views::{SliceRecastSource, slice_recast_source};
