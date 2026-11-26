//! Prepare the issued body for image composition. Its section bytes, symbols,
//! and relocations arrive together from object construction. Semantic,
//! transport and stack evidence refer to that object's function extents.

mod current_image_selections;
pub(crate) mod image_data;

mod attribution {
    use super::{Error, host};
    use crate::order::strictly_increasing;
    #[cfg(test)]
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use machine_code::{
        FunctionFragment, FunctionFragmentBranchEvidence,
        FunctionFragmentControlProvenance as Control, SemanticCodeAttribution, SemanticCodeSite,
    };
    use selected_instructions::SelectedSuccessorRole;

    fn semantic_edge(
        successor: &machine_code::FunctionFragmentSuccessorProvenance,
    ) -> Result<semantic_vocabulary::EdgeId, Error> {
        successor
            .psi_edge
            .ok_or(Error::Mismatch("semantic successor lost its authored edge"))
    }

    fn implementation_successor(
        successor: &machine_code::FunctionFragmentSuccessorProvenance,
        provenance: &selected_instructions::SelectedInstructionProvenance,
    ) -> bool {
        if !successor.work.is_empty() || !successor.bindings.is_empty() {
            return false;
        }
        match successor.role {
            SelectedSuccessorRole::CaseDispatchContinuation => successor.psi_edge.is_some(),
            SelectedSuccessorRole::OperationContinuation { operation } => {
                successor.psi_edge.is_none()
                    && provenance.operations.contains(&operation)
                    && provenance.edges.is_empty()
                    && provenance.work.is_empty()
            }
            _ => false,
        }
    }

    #[cfg(test)]
    fn build_ordinal_index(
        source: &AbstractFunction,
    ) -> std::collections::BTreeMap<SemanticCodeSite, Option<usize>> {
        let mut index = std::collections::BTreeMap::new();
        let mut record = |site: SemanticCodeSite, ordinal: usize| {
            index
                .entry(site)
                .and_modify(|existing| *existing = None)
                .or_insert(Some(ordinal));
        };
        for (ordinal, operation) in source.operations().enumerate() {
            if let Some(psi_operation) = operation.psi_operation() {
                record(SemanticCodeSite::Operation(psi_operation), ordinal);
            } else {
                match operation {
                    abstract_operations::return_or_return_unit_pattern!(psi_edge, ..)
                    | AbstractOperation::InvokeNoNormal { psi_edge, .. }
                    | AbstractOperation::ExternalCompletion { psi_edge, .. }
                    | AbstractOperation::EliminateEmpty { psi_edge, .. }
                    | AbstractOperation::Crash { psi_edge, .. }
                    | AbstractOperation::ReturnStructural { psi_edge, .. }
                    | AbstractOperation::Jump { psi_edge, .. } => {
                        record(SemanticCodeSite::Edge(*psi_edge), ordinal);
                    }
                    AbstractOperation::Conditional {
                        when_true,
                        when_false,
                        ..
                    } => {
                        record(SemanticCodeSite::Edge(when_true.psi_edge), ordinal);
                        if when_false.psi_edge != when_true.psi_edge {
                            record(SemanticCodeSite::Edge(when_false.psi_edge), ordinal);
                        }
                    }
                    AbstractOperation::StructuralCase { cases, .. } => {
                        let mut seen = Vec::new();
                        for case in cases {
                            if !seen.contains(&case.psi_edge) {
                                seen.push(case.psi_edge);
                                record(SemanticCodeSite::Edge(case.psi_edge), ordinal);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        index
    }

    #[cfg(test)]
    fn test_publication(
        source: &AbstractFunction,
    ) -> Result<target_vocabulary::NativePublicationFacts, Error> {
        let sites = build_ordinal_index(source)
            .into_iter()
            .map(|(site, ordinal)| {
                let ordinal = ordinal.ok_or(Error::Mismatch(
                    "fragment semantic site has no unique authored ordinal",
                ))?;
                Ok((site, u32::try_from(ordinal).map_err(|_| Error::Overflow)?))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        target_vocabulary::NativePublicationFacts::seal(
            vec![target_vocabulary::NativeFunctionPublication {
                call_targets: Vec::new(),
                machine: source.machine,
                operation_count: u32::try_from(source.operations().count())
                    .map_err(|_| Error::Overflow)?,
                sites,
                byte_literals: Vec::new(),
            }],
            Default::default(),
        )
        .map_err(Error::Mismatch)
    }

    #[cfg(test)]
    fn produce(
        fragment: &FunctionFragment,
        source: &AbstractFunction,
    ) -> Result<Vec<SemanticCodeAttribution>, Error> {
        let publication = test_publication(source)?;
        produce_current(fragment, publication.function(source.machine).unwrap())
    }

    pub(super) fn ordinal(
        source: &target_vocabulary::NativeFunctionPublication,
        site: SemanticCodeSite,
    ) -> Result<usize, Error> {
        let ordinal = source.ordinal(site).ok_or(Error::Mismatch(
            "fragment semantic site has no unique authored ordinal",
        ))?;
        usize::try_from(ordinal).map_err(|_| Error::Overflow)
    }

    pub(super) fn produce_current(
        fragment: &FunctionFragment,
        source: &target_vocabulary::NativeFunctionPublication,
    ) -> Result<Vec<SemanticCodeAttribution>, Error> {
        produce_with_ordinals(fragment, |site| ordinal(source, site))
    }

    fn produce_with_ordinals(
        fragment: &FunctionFragment,
        mut ordinal: impl FnMut(SemanticCodeSite) -> Result<usize, Error>,
    ) -> Result<Vec<SemanticCodeAttribution>, Error> {
        let mut rows = Vec::new();
        for instruction in fragment.instructions() {
            let offset = host(instruction.offset)?;
            let length = instruction.bytes.len();
            let mut push = |site, code_offset, byte_count| -> Result<(), Error> {
                rows.push(SemanticCodeAttribution {
                    site,
                    operation_ordinal: ordinal(site)?,
                    code_offset,
                    byte_count,
                });
                Ok(())
            };
            for operation in &instruction.provenance.operations {
                push(SemanticCodeSite::Operation(*operation), offset, length)?;
            }
            match &instruction.control {
                Control::OperationTrap { operation } => {
                    if instruction.provenance.operations.as_slice() != [*operation]
                        || !instruction.provenance.edges.is_empty()
                        || !instruction.provenance.work.is_empty()
                    {
                        return Err(Error::Mismatch(
                            "operation trap changed its owning occurrence",
                        ));
                    }
                }
                Control::EliminateEmpty { psi_edge } | Control::Crash { psi_edge, .. } => {
                    push(SemanticCodeSite::Edge(*psi_edge), offset, length)?
                }
                Control::HostedExitProcess { completion_edge } => {
                    push(SemanticCodeSite::Edge(*completion_edge), offset, length)?;
                }
                Control::Return { psi_return_edge } => {
                    push(SemanticCodeSite::Edge(*psi_return_edge), offset, length)?
                }
                Control::Jump { successor }
                    if successor.role == SelectedSuccessorRole::Semantic =>
                {
                    push(
                        SemanticCodeSite::Edge(semantic_edge(successor)?),
                        offset,
                        length,
                    )?
                }
                Control::Jump { successor } => {
                    if matches!(
                        successor.role,
                        SelectedSuccessorRole::OperationContinuation { .. }
                    ) && !implementation_successor(successor, &instruction.provenance)
                    {
                        return Err(Error::Mismatch(
                            "operation jump changed its owning occurrence",
                        ));
                    }
                }
                Control::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => {
                    if [when_taken, when_fallthrough].iter().any(|successor| {
                        successor.role != SelectedSuccessorRole::Semantic
                            && !implementation_successor(successor, &instruction.provenance)
                    }) {
                        return Err(Error::Mismatch(
                            "conditional successor has nonsemantic attribution role",
                        ));
                    }
                    if when_taken.role == SelectedSuccessorRole::Semantic {
                        push(
                            SemanticCodeSite::Edge(semantic_edge(when_taken)?),
                            offset,
                            length,
                        )?;
                    }
                    let Some(branch) = instruction.branch.as_deref() else {
                        return Err(Error::Mismatch("conditional has no decoded branch"));
                    };
                    let FunctionFragmentBranchEvidence::Conditional(branch) = branch else {
                        return Err(Error::Mismatch("conditional has jump evidence"));
                    };
                    if when_fallthrough.role == SelectedSuccessorRole::Semantic {
                        push(
                            SemanticCodeSite::Edge(semantic_edge(when_fallthrough)?),
                            host(branch.when_fallthrough_offset)?,
                            0,
                        )?;
                    }
                }
                Control::InternalCall { owner, .. } => {
                    if let Some(edge) = owner.edge() {
                        push(SemanticCodeSite::Edge(edge), offset, length)?;
                    }
                }
                Control::NormalizedForeignCall {
                    invocation_site, ..
                } => {
                    if let semantic_vocabulary::InvocationSite::ExternalCompletion(edge) =
                        invocation_site
                    {
                        push(SemanticCodeSite::Edge(*edge), offset, length)?;
                    }
                }
                Control::None => {
                    for edge in &instruction.provenance.edges {
                        push(SemanticCodeSite::Edge(*edge), offset, length)?;
                    }
                }
            }
        }
        rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset, row.byte_count));
        rows.dedup();
        let mut joined: Vec<SemanticCodeAttribution> = Vec::new();
        for row in rows {
            if let Some(previous) = joined
                .last_mut()
                .filter(|previous| previous.site == row.site)
            {
                let end = previous
                    .code_offset
                    .checked_add(previous.byte_count)
                    .ok_or(Error::Overflow)?;
                if row.code_offset > end {
                    joined.push(row);
                    continue;
                }
                if row.code_offset < end {
                    return Err(Error::Mismatch("semantic physical intervals overlap"));
                }
                previous.byte_count = previous
                    .byte_count
                    .checked_add(row.byte_count)
                    .ok_or(Error::Overflow)?;
            } else {
                joined.push(row);
            }
        }
        Ok(joined)
    }

    #[cfg(test)]
    fn validate(
        fragment: &FunctionFragment,
        source: &AbstractFunction,
        rows: &[SemanticCodeAttribution],
    ) -> Result<(), Error> {
        let publication = test_publication(source)?;
        validate_current(
            fragment,
            publication.function(source.machine).unwrap(),
            rows,
        )
    }

    pub(super) fn validate_current(
        fragment: &FunctionFragment,
        source: &target_vocabulary::NativeFunctionPublication,
        rows: &[SemanticCodeAttribution],
    ) -> Result<(), Error> {
        validate_with_ordinals(fragment, rows, |site| ordinal(source, site))
    }

    fn validate_with_ordinals(
        fragment: &FunctionFragment,
        rows: &[SemanticCodeAttribution],
        mut ordinal: impl FnMut(SemanticCodeSite) -> Result<usize, Error>,
    ) -> Result<(), Error> {
        let physical = PhysicalSpans::prepare(fragment);
        let attributed = AttributionRows::new(rows);
        let mut previous_ends = std::collections::BTreeMap::new();
        for row in rows {
            if ordinal(row.site)? != row.operation_ordinal
                || previous_ends
                    .get(&row.site)
                    .is_some_and(|end| *end >= row.code_offset)
                || !match &physical {
                    Some(physical) => physical.interval_is_exact(row)?,
                    None => interval_is_exact(fragment, row)?,
                }
            {
                return Err(Error::Mismatch(
                    "object semantic interval differs from its physical spans",
                ));
            }
            // Only accepted rows enter this prefix; their interval ends are finite.
            previous_ends.insert(
                row.site,
                row.code_offset
                    .checked_add(row.byte_count)
                    .ok_or(Error::Overflow)?,
            );
        }
        if !strictly_increasing(rows, |row| (row.operation_ordinal, row.code_offset)) {
            return Err(Error::Mismatch(
                "object semantic attribution order is not canonical",
            ));
        }
        for instruction in fragment.instructions() {
            let present = |site, offset, length| {
                attributed.at(site).iter().any(|row| {
                    row.code_offset <= offset
                        && offset.checked_add(length).is_some_and(|end| {
                            row.code_offset
                                .checked_add(row.byte_count)
                                .is_some_and(|row_end| end <= row_end)
                        })
                })
            };
            for operation in &instruction.provenance.operations {
                if !present(
                    SemanticCodeSite::Operation(*operation),
                    host(instruction.offset)?,
                    instruction.bytes.len(),
                ) {
                    return Err(Error::Mismatch("object omits an attributed operation"));
                }
            }
            let edges: Vec<_> = match &instruction.control {
                Control::OperationTrap { operation } => {
                    if instruction.provenance.operations.as_slice() != [*operation]
                        || !instruction.provenance.edges.is_empty()
                        || !instruction.provenance.work.is_empty()
                    {
                        return Err(Error::Mismatch(
                            "operation trap changed its owning occurrence",
                        ));
                    }
                    Vec::new()
                }
                Control::EliminateEmpty { psi_edge } | Control::Crash { psi_edge, .. } => {
                    vec![*psi_edge]
                }
                Control::HostedExitProcess { completion_edge } => vec![*completion_edge],
                Control::Return { psi_return_edge } => vec![*psi_return_edge],
                Control::Jump { successor } => {
                    if successor.role == SelectedSuccessorRole::Semantic {
                        vec![semantic_edge(successor)?]
                    } else {
                        if matches!(
                            successor.role,
                            SelectedSuccessorRole::OperationContinuation { .. }
                        ) && !implementation_successor(successor, &instruction.provenance)
                        {
                            return Err(Error::Mismatch(
                                "operation jump changed its owning occurrence",
                            ));
                        }
                        Vec::new()
                    }
                }
                Control::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => {
                    if [when_taken, when_fallthrough].iter().any(|successor| {
                        successor.role != SelectedSuccessorRole::Semantic
                            && !implementation_successor(successor, &instruction.provenance)
                    }) {
                        return Err(Error::Mismatch(
                            "conditional successor has nonsemantic attribution role",
                        ));
                    }
                    [when_taken.as_ref(), when_fallthrough.as_ref()]
                        .into_iter()
                        .filter(|successor| successor.role == SelectedSuccessorRole::Semantic)
                        .map(semantic_edge)
                        .collect::<Result<Vec<_>, _>>()?
                }
                Control::InternalCall { owner, .. } => owner.edge().into_iter().collect(),
                Control::NormalizedForeignCall {
                    invocation_site, ..
                } => match invocation_site {
                    semantic_vocabulary::InvocationSite::InvokeNoNormal(_)
                    | semantic_vocabulary::InvocationSite::Cleanup { .. } => {
                        return Err(Error::Mismatch(
                            "foreign invocation names a module invocation edge",
                        ));
                    }
                    semantic_vocabulary::InvocationSite::Operation(_) => Vec::new(),
                    semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => {
                        vec![*edge]
                    }
                },
                Control::None => instruction.provenance.edges.clone(),
            };
            let instruction_offset = host(instruction.offset)?;
            let instruction_end = instruction_offset
                .checked_add(instruction.bytes.len())
                .ok_or(Error::Overflow)?;
            for edge in edges {
                if let Control::ConditionalBranch {
                    when_fallthrough, ..
                } = &instruction.control
                    && when_fallthrough.role == SelectedSuccessorRole::Semantic
                    && when_fallthrough.psi_edge == Some(edge)
                    && let Some(FunctionFragmentBranchEvidence::Conditional(branch)) =
                        instruction.branch.as_deref()
                    && present(
                        SemanticCodeSite::Edge(edge),
                        host(branch.when_fallthrough_offset)?,
                        0,
                    )
                {
                    continue;
                }
                if !attributed
                    .at(SemanticCodeSite::Edge(edge))
                    .iter()
                    .any(|row| {
                        supports(instruction, row.site, row.code_offset, row.byte_count)
                            || (supports(
                                instruction,
                                row.site,
                                instruction_offset,
                                instruction.bytes.len(),
                            ) && row.code_offset <= instruction_offset
                                && row
                                    .code_offset
                                    .checked_add(row.byte_count)
                                    .is_some_and(|end| instruction_end <= end))
                    })
                {
                    return Err(Error::Mismatch("object omits an attributed control edge"));
                }
            }
        }
        Ok(())
    }

    struct AttributionRows<'inputs> {
        rows: Vec<&'inputs SemanticCodeAttribution>,
    }

    impl<'inputs> AttributionRows<'inputs> {
        fn new(rows: &'inputs [SemanticCodeAttribution]) -> Self {
            let mut rows: Vec<_> = rows.iter().collect();
            rows.sort_by_key(|row| row.site);
            Self { rows }
        }

        fn at(&self, site: SemanticCodeSite) -> &[&'inputs SemanticCodeAttribution] {
            let first = self.rows.partition_point(|row| row.site < site);
            let count = self.rows[first..].partition_point(|row| row.site == site);
            &self.rows[first..first + count]
        }
    }

    struct PhysicalSpan<'inputs> {
        instruction: &'inputs machine_code::FunctionFragmentInstructionSpan,
        start: usize,
        end: usize,
    }

    struct PhysicalSpans<'inputs> {
        spans: Vec<PhysicalSpan<'inputs>>,
        possible_sites: Vec<(SemanticCodeSite, usize)>,
    }

    // Canonical physical spans let interval checks visit only overlapping bytes.
    // Other input shapes retain the original full scan and its error order.
    impl<'inputs> PhysicalSpans<'inputs> {
        fn prepare(fragment: &'inputs FunctionFragment) -> Option<Self> {
            let mut spans = Vec::new();
            let mut possible_sites = Vec::new();
            let mut previous_end = 0;
            for instruction in fragment.instructions() {
                let start = host(instruction.offset).ok()?;
                let end = start.checked_add(instruction.bytes.len())?;
                if start < previous_end {
                    return None;
                }
                previous_end = end;
                let position = spans.len();
                let mut record = |site| possible_sites.push((site, position));
                for operation in &instruction.provenance.operations {
                    record(SemanticCodeSite::Operation(*operation));
                }
                for edge in &instruction.provenance.edges {
                    record(SemanticCodeSite::Edge(*edge));
                }
                match &instruction.control {
                    Control::EliminateEmpty { psi_edge } | Control::Crash { psi_edge, .. } => {
                        record(SemanticCodeSite::Edge(*psi_edge))
                    }
                    Control::HostedExitProcess { completion_edge } => {
                        record(SemanticCodeSite::Edge(*completion_edge));
                    }
                    Control::Return { psi_return_edge } => {
                        record(SemanticCodeSite::Edge(*psi_return_edge));
                    }
                    Control::Jump { successor } => {
                        if let Some(edge) = successor.psi_edge {
                            record(SemanticCodeSite::Edge(edge));
                        }
                    }
                    Control::ConditionalBranch {
                        when_taken,
                        when_fallthrough,
                        ..
                    } => {
                        for successor in [when_taken, when_fallthrough] {
                            if let Some(edge) = successor.psi_edge {
                                record(SemanticCodeSite::Edge(edge));
                            }
                        }
                    }
                    Control::InternalCall { owner, .. } => {
                        if let Some(edge) = owner.edge() {
                            record(SemanticCodeSite::Edge(edge));
                        }
                    }
                    Control::NormalizedForeignCall {
                        invocation_site, ..
                    } => {
                        if let semantic_vocabulary::InvocationSite::ExternalCompletion(edge) =
                            invocation_site
                        {
                            record(SemanticCodeSite::Edge(*edge));
                        }
                    }
                    Control::OperationTrap { .. } | Control::None => {}
                }
                spans.push(PhysicalSpan {
                    instruction,
                    start,
                    end,
                });
            }
            possible_sites.sort_by_key(|(site, _)| *site);
            Some(Self {
                spans,
                possible_sites,
            })
        }

        fn interval_is_exact(&self, row: &SemanticCodeAttribution) -> Result<bool, Error> {
            if row.byte_count == 0 {
                let first = self
                    .possible_sites
                    .partition_point(|(site, _)| *site < row.site);
                return Ok(self.possible_sites[first..]
                    .iter()
                    .take_while(|(site, _)| *site == row.site)
                    .any(|(_, position)| {
                        supports(
                            self.spans[*position].instruction,
                            row.site,
                            row.code_offset,
                            0,
                        )
                    }));
            }
            let end = row
                .code_offset
                .checked_add(row.byte_count)
                .ok_or(Error::Overflow)?;
            let range = overlap_range(&self.spans, row.code_offset, end, |span| {
                (span.start, span.end)
            });
            let mut minimum = usize::MAX;
            let mut maximum = 0;
            let mut covered = 0usize;
            let mut found = false;
            for span in &self.spans[range] {
                if !supports(
                    span.instruction,
                    row.site,
                    span.start,
                    span.instruction.bytes.len(),
                ) {
                    return Ok(false);
                }
                found = true;
                minimum = minimum.min(span.start);
                maximum = maximum.max(span.end);
                covered = covered
                    .checked_add(span.instruction.bytes.len())
                    .ok_or(Error::Overflow)?;
            }
            Ok(found && minimum == row.code_offset && maximum == end && covered == row.byte_count)
        }
    }

    fn overlap_range<T>(
        spans: &[T],
        start: usize,
        end: usize,
        bounds: impl Fn(&T) -> (usize, usize),
    ) -> std::ops::Range<usize> {
        let first = spans.partition_point(|span| bounds(span).1 <= start);
        let last = spans.partition_point(|span| bounds(span).0 < end);
        first..last
    }

    fn supports(
        instruction: &machine_code::FunctionFragmentInstructionSpan,
        site: SemanticCodeSite,
        offset: usize,
        length: usize,
    ) -> bool {
        let ordinary = u64::try_from(offset).ok() == Some(instruction.offset)
            && length == instruction.bytes.len();
        match site {
            SemanticCodeSite::Operation(operation) => {
                ordinary && instruction.provenance.operations.contains(&operation)
            }
            SemanticCodeSite::Edge(edge) => match &instruction.control {
                Control::OperationTrap { .. } => false,
                Control::EliminateEmpty { psi_edge } | Control::Crash { psi_edge, .. } => {
                    ordinary && edge == *psi_edge
                }
                Control::HostedExitProcess { completion_edge } => {
                    edge == *completion_edge && ordinary
                }
                Control::Return { psi_return_edge } => ordinary && edge == *psi_return_edge,
                Control::Jump { successor } => {
                    successor.role == SelectedSuccessorRole::Semantic
                        && ordinary
                        && Some(edge) == successor.psi_edge
                }
                Control::ConditionalBranch {
                    when_taken,
                    when_fallthrough,
                    ..
                } => {
                    (when_taken.role == SelectedSuccessorRole::Semantic
                        && ordinary
                        && Some(edge) == when_taken.psi_edge)
                        || (when_fallthrough.role == SelectedSuccessorRole::Semantic
                            && Some(edge) == when_fallthrough.psi_edge
                            && length == 0
                            && matches!(
                                instruction.branch.as_deref(),
                                Some(FunctionFragmentBranchEvidence::Conditional(branch)) if u64::try_from(offset).ok() == Some(branch.when_fallthrough_offset)
                            ))
                }
                Control::None => ordinary && instruction.provenance.edges.contains(&edge),
                Control::InternalCall { owner, .. } => ordinary && owner.edge() == Some(edge),
                Control::NormalizedForeignCall {
                    invocation_site, ..
                } => {
                    ordinary
                        && *invocation_site
                            == semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
                }
            },
        }
    }
    fn interval_is_exact(
        fragment: &FunctionFragment,
        row: &SemanticCodeAttribution,
    ) -> Result<bool, Error> {
        if row.byte_count == 0
            && fragment
                .instructions()
                .any(|instruction| supports(instruction, row.site, row.code_offset, 0))
        {
            return Ok(true);
        }
        let end = row
            .code_offset
            .checked_add(row.byte_count)
            .ok_or(Error::Overflow)?;
        let mut minimum = usize::MAX;
        let mut maximum = 0usize;
        let mut covered = 0usize;
        let mut found = false;
        for instruction in fragment.instructions() {
            let start = host(instruction.offset)?;
            let instruction_end = start
                .checked_add(instruction.bytes.len())
                .ok_or(Error::Overflow)?;
            if supports(instruction, row.site, start, instruction.bytes.len()) {
                if (instruction_end <= row.code_offset || start >= end)
                    && (!instruction.bytes.is_empty() || start != row.code_offset || end != start)
                {
                    continue;
                }
                found = true;
                minimum = minimum.min(start);
                maximum = maximum.max(instruction_end);
                covered = covered
                    .checked_add(instruction.bytes.len())
                    .ok_or(Error::Overflow)?;
            } else if (start < end && instruction_end > row.code_offset)
                || (instruction.bytes.is_empty() && start > row.code_offset && start < end)
            {
                return Ok(false);
            }
        }
        Ok(found && minimum == row.code_offset && maximum == end && covered == row.byte_count)
    }

    #[cfg(test)]
    mod interval_ranges {
        use super::overlap_range;

        #[test]
        fn ordered_interval_ranges_match_direct_membership() {
            let spans = [
                (0usize, 0usize),
                (0, 2),
                (2, 2),
                (2, 2),
                (4, 7),
                (7, 7),
                (9, 10),
            ];
            for start in 0..=11 {
                for end in start + 1..=12 {
                    let expected: Vec<_> = spans
                        .iter()
                        .enumerate()
                        .filter(|(_, (span_start, span_end))| {
                            *span_end > start && *span_start < end
                        })
                        .map(|(position, _)| position)
                        .collect();
                    let actual: Vec<_> = overlap_range(&spans, start, end, |span| *span).collect();
                    assert_eq!(actual, expected, "range {start}..{end}");
                }
            }
            assert_eq!(
                overlap_range(&[] as &[(usize, usize)], 1, 2, |span| *span),
                0..0
            );
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            AbstractFunction, Control, Error, FunctionFragment, SelectedSuccessorRole,
            SemanticCodeAttribution, SemanticCodeSite, ordinal as current_ordinal, produce,
            test_publication, validate,
        };
        use abstract_operations::{
            AbstractFunctionResult, AbstractOperation, IntegerArithmetic,
            IntegerArithmeticOperator, IntegerOverflowPolicy,
        };
        use machine_code::{FunctionFragmentBlockSpan, FunctionFragmentInstructionSpan};
        use selected_instructions::{
            MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
        };
        use semantic_vocabulary::{
            BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
            ScalarType, ValueId,
        };

        fn ordinal(source: &AbstractFunction, site: SemanticCodeSite) -> Result<usize, Error> {
            let publication = test_publication(source)?;
            current_ordinal(publication.function(source.machine).unwrap(), site)
        }

        fn assert_requires_unique_operation_ordinal(
            source: &mut AbstractFunction,
            operation: OperationId,
        ) {
            let publication = test_publication(source).unwrap();
            let current = publication.function(source.machine).unwrap();
            assert_eq!(
                current_ordinal(current, SemanticCodeSite::Operation(operation)).unwrap(),
                1
            );
            assert!(
                current_ordinal(
                    current,
                    SemanticCodeSite::Operation(OperationId::new(99).unwrap())
                )
                .is_err()
            );
            let repeated = source.blocks[0].operations[1].clone();
            source.blocks[0].operations.push(repeated);
            assert!(test_publication(source).is_err());
        }

        #[test]
        fn boolean_equality_attribution_requires_a_unique_authored_operation_ordinal() {
            let (_, mut source) = fixture();
            let operation = OperationId::new(2).unwrap();
            source.blocks[0].operations[1] = AbstractOperation::BooleanEqual {
                psi_operation: operation,
                result: ValueId::new(3).unwrap(),
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            };
            assert_requires_unique_operation_ordinal(&mut source, operation);
        }

        #[test]
        fn bitwise_and_attribution_requires_a_unique_authored_operation_ordinal() {
            let (_, mut source) = fixture();
            let operation = OperationId::new(2).unwrap();
            source.blocks[0].operations[1] = AbstractOperation::IntegerBitwiseAnd {
                psi_operation: operation,
                result: ValueId::new(3).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            };
            assert_requires_unique_operation_ordinal(&mut source, operation);
        }

        #[test]
        fn wrapping_add_attribution_requires_its_exact_complete_interval_and_ordinal() {
            let (fragment, mut source) = fixture();
            let operation = OperationId::new(2).unwrap();
            source.blocks[0].operations[1] = AbstractOperation::IntegerArithmetic {
                psi_operation: operation,
                result: ValueId::new(3).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                arithmetic: IntegerArithmetic::new(
                    IntegerOverflowPolicy::Wrapping,
                    IntegerArithmeticOperator::Add,
                    None,
                )
                .expect("admitted integer arithmetic"),
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(1).unwrap(),
            };
            let rows = produce(&fragment, &source).unwrap();
            validate(&fragment, &source, &rows).unwrap();
            let position = rows
                .iter()
                .position(|row| row.site == SemanticCodeSite::Operation(operation))
                .unwrap();
            assert_eq!(
                (
                    rows[position].operation_ordinal,
                    rows[position].code_offset,
                    rows[position].byte_count
                ),
                (1, 5, 2)
            );
            for corruption in 0..4 {
                let mut changed = rows.clone();
                match corruption {
                    0 => {
                        changed.remove(position);
                    }
                    1 => changed[position].byte_count -= 1,
                    2 => {
                        changed[position].code_offset -= 1;
                        changed[position].byte_count += 1;
                    }
                    3 => changed[position].operation_ordinal = 0,
                    _ => unreachable!(),
                }
                assert!(
                    validate(&fragment, &source, &changed).is_err(),
                    "wrapping add attribution corruption {corruption}"
                );
            }
            let repeated = source.blocks[0].operations[1].clone();
            source.blocks[0].operations.push(repeated);
            assert!(ordinal(&source, SemanticCodeSite::Operation(operation)).is_err());
        }

        #[test]
        fn wrapping_remainder_attribution_requires_a_unique_authored_operation_ordinal() {
            let (_, mut source) = fixture();
            let operation = OperationId::new(2).unwrap();
            source.blocks[0].operations[1] = AbstractOperation::IntegerArithmetic {
                psi_operation: operation,
                result: ValueId::new(3).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                arithmetic: IntegerArithmetic::new(
                    IntegerOverflowPolicy::Wrapping,
                    IntegerArithmeticOperator::Remainder,
                    Some(semantic_vocabulary::ObligationId::new(1).unwrap()),
                )
                .expect("admitted integer arithmetic"),
                left: ValueId::new(1).unwrap(),
                right: ValueId::new(2).unwrap(),
            };
            assert_requires_unique_operation_ordinal(&mut source, operation);
        }

        #[test]
        fn legalized_scalar_family_attribution_requires_unique_authored_ordinals() {
            let operation = OperationId::new(2).unwrap();
            let result = ValueId::new(3).unwrap();
            let left = ValueId::new(1).unwrap();
            let right = ValueId::new(2).unwrap();
            let obligation = semantic_vocabulary::ObligationId::new(1).unwrap();
            let signed64 = IntegerType::new(IntegerSign::Signed, 64).unwrap();
            let unsigned64 = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let integer_arithmetic =
                |scalar_type, policy, operator, obligation| AbstractOperation::IntegerArithmetic {
                    psi_operation: operation,
                    result,
                    scalar_type,
                    arithmetic: IntegerArithmetic::new(policy, operator, obligation)
                        .expect("admitted integer arithmetic"),
                    left,
                    right,
                };
            for (name, candidate) in [
                (
                    "bitwise or",
                    AbstractOperation::IntegerBitwiseOr {
                        psi_operation: operation,
                        result,
                        scalar_type: unsigned64,
                        left,
                        right,
                    },
                ),
                (
                    "bitwise not",
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: operation,
                        result,
                        scalar_type: unsigned64,
                        operand: left,
                    },
                ),
                (
                    "wrapping subtract",
                    integer_arithmetic(
                        signed64,
                        IntegerOverflowPolicy::Wrapping,
                        IntegerArithmeticOperator::Subtract,
                        None,
                    ),
                ),
                (
                    "wrapping multiply",
                    integer_arithmetic(
                        unsigned64,
                        IntegerOverflowPolicy::Wrapping,
                        IntegerArithmeticOperator::Multiply,
                        None,
                    ),
                ),
                (
                    "wrapping divide",
                    integer_arithmetic(
                        signed64,
                        IntegerOverflowPolicy::Wrapping,
                        IntegerArithmeticOperator::Divide,
                        Some(obligation),
                    ),
                ),
                (
                    "saturating remainder",
                    integer_arithmetic(
                        signed64,
                        IntegerOverflowPolicy::Saturating,
                        IntegerArithmeticOperator::Remainder,
                        Some(obligation),
                    ),
                ),
                (
                    "exact multiply",
                    integer_arithmetic(
                        unsigned64,
                        IntegerOverflowPolicy::Exact,
                        IntegerArithmeticOperator::Multiply,
                        Some(obligation),
                    ),
                ),
                (
                    "exact remainder",
                    integer_arithmetic(
                        unsigned64,
                        IntegerOverflowPolicy::Exact,
                        IntegerArithmeticOperator::Remainder,
                        Some(obligation),
                    ),
                ),
            ] {
                let (_, mut source) = fixture();
                source.blocks[0].operations[1] = candidate;
                assert_eq!(
                    ordinal(&source, SemanticCodeSite::Operation(operation)).unwrap(),
                    1,
                    "{name}"
                );
                assert!(
                    ordinal(
                        &source,
                        SemanticCodeSite::Operation(OperationId::new(99).unwrap())
                    )
                    .is_err(),
                    "{name}"
                );
                let repeated = source.blocks[0].operations[1].clone();
                source.blocks[0].operations.push(repeated);
                assert!(
                    ordinal(&source, SemanticCodeSite::Operation(operation)).is_err(),
                    "{name}"
                );
            }
        }

        fn fixture() -> (FunctionFragment, AbstractFunction) {
            let first = OperationId::new(1).unwrap();
            let second = OperationId::new(2).unwrap();
            let edge = EdgeId::new(3).unwrap();
            let scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            let source = AbstractFunction {
                machine: MachineId::new(1).unwrap(),
                attachment: None,
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                block_entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                blocks: vec![abstract_operations::AbstractBlock {
                    block: BlockId::new(1).unwrap(),
                    structural_parameters: Vec::new(),
                    parameters: Vec::new(),
                    operations: vec![
                        AbstractOperation::IntegerConstant {
                            psi_operation: first,
                            result: ValueId::new(1).unwrap(),
                            scalar_type,
                            value: IntegerValue::Unsigned(1),
                        },
                        AbstractOperation::IntegerConstant {
                            psi_operation: second,
                            result: ValueId::new(2).unwrap(),
                            scalar_type,
                            value: IntegerValue::Unsigned(2),
                        },
                        AbstractOperation::ReturnUnit {
                            psi_edge: edge,
                            cleanup_actions: Vec::new(),
                        },
                    ],
                }],
            };
            let mut instructions = Vec::new();
            for (index, (offset, length, operation)) in [
                (0, 2, Some(first)),
                (2, 3, Some(first)),
                (5, 2, Some(second)),
                (7, 1, None),
            ]
            .into_iter()
            .enumerate()
            {
                instructions.push(FunctionFragmentInstructionSpan {
                    instruction: SelectedInstructionId(index as u32),
                    alternative: MachineAlternativeKey {
                        family: MachineAlternativeFamily::MaterializeI64,
                        variant: 0,
                    },
                    offset,
                    bytes: vec![0x90; length],
                    branch: None,
                    internal_machine_fixup: None,
                    normalized_foreign_call_fixup: None,
                    image_address_fixup: None,
                    provenance: selected_instructions::SelectedInstructionProvenance {
                        operations: operation.into_iter().collect(),
                        ..Default::default()
                    },
                    control: if operation.is_some() {
                        Control::None
                    } else {
                        Control::Return {
                            psi_return_edge: edge,
                        }
                    },
                });
            }
            let fragment = FunctionFragment {
                machine: source.machine,
                attachment: None,
                provenance: Default::default(),
                byte_count: 8,
                bytes: vec![0x90; 8],
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(0),
                    offset: 0,
                    byte_count: 8,
                    instructions,
                }],
            };
            (fragment, source)
        }

        #[test]
        fn case_edges_share_the_authored_case_ordinal_without_fabricated_operations() {
            let (_, mut source) = fixture();
            source.blocks[0].operations[2] = AbstractOperation::StructuralCase {
                psi_block: source.blocks[0].block,
                source: semantic_vocabulary::PlaceId::new(1).unwrap(),
                path: Vec::new(),
                cases: [10, 11]
                    .into_iter()
                    .map(
                        |identity| abstract_operations::AbstractStructuralCaseSuccessor {
                            psi_edge: EdgeId::new(identity).unwrap(),
                            target: BlockId::new(identity).unwrap(),
                            case: semantic_vocabulary::StructuralCaseId::new(identity).unwrap(),
                            payloads: Vec::new(),
                            structural_bindings: Vec::new(),
                            cleanup_actions: Vec::new(),
                        },
                    )
                    .collect(),
            };
            for identity in [10, 11] {
                let edge = EdgeId::new(identity).unwrap();
                assert_eq!(ordinal(&source, SemanticCodeSite::Edge(edge)).unwrap(), 2);
                assert!(
                    ordinal(
                        &source,
                        SemanticCodeSite::Operation(OperationId::new(identity).unwrap())
                    )
                    .is_err()
                );
            }
            assert!(ordinal(&source, SemanticCodeSite::Edge(EdgeId::new(99).unwrap())).is_err());
            let repeated = source.blocks[0].operations[2].clone();
            source.blocks[0].operations.push(repeated);
            assert!(ordinal(&source, SemanticCodeSite::Edge(EdgeId::new(10).unwrap())).is_err());
        }

        #[test]
        fn process_exit_completion_edge_owns_its_terminal_span() {
            let (mut fragment, source) = fixture();
            let terminal = fragment.blocks[0].instructions.last_mut().unwrap();
            let Control::Return { psi_return_edge } = &terminal.control else {
                panic!("return fixture");
            };
            let psi_return_edge = *psi_return_edge;
            terminal.control = Control::HostedExitProcess {
                completion_edge: psi_return_edge,
            };
            let start = usize::try_from(terminal.offset).unwrap();
            let byte_count = terminal.bytes.len();
            let rows = produce(&fragment, &source).unwrap();
            validate(&fragment, &source, &rows).unwrap();
            let position = rows
                .iter()
                .position(|row| row.site == SemanticCodeSite::Edge(psi_return_edge))
                .unwrap();
            assert_eq!(rows[position].code_offset, start);
            assert_eq!(rows[position].byte_count, byte_count);
            let mut changed = rows.clone();
            changed[position].byte_count = 0;
            assert!(validate(&fragment, &source, &changed).is_err());
            let mut changed = rows;
            changed[position].code_offset += 1;
            assert!(validate(&fragment, &source, &changed).is_err());
        }

        #[test]
        fn boolean_constant_attribution_requires_its_exact_operation_ordinal() {
            let (fragment, mut source) = fixture();
            source.blocks[0].operations[0] = AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: ValueId::new(1).unwrap(),
                value: true,
            };
            let rows = produce(&fragment, &source).unwrap();
            assert_eq!(rows[0].operation_ordinal, 0);
            validate(&fragment, &source, &rows).unwrap();
            let mut substituted = rows.clone();
            substituted[0].operation_ordinal = 1;
            assert!(validate(&fragment, &source, &substituted).is_err());
            source.blocks[0].operations.swap(0, 1);
            assert!(validate(&fragment, &source, &rows).is_err());
        }

        #[test]
        fn contiguous_spans_coalesce_without_absorbing_neighboring_operations() {
            let (fragment, source) = fixture();
            let rows = produce(&fragment, &source).unwrap();
            assert_eq!(rows.len(), 3);
            assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 5));
            validate(&fragment, &source, &rows).unwrap();

            let mut extended = rows.clone();
            extended[0].byte_count = 7;
            assert!(validate(&fragment, &source, &extended).is_err());
            let mut shifted = rows.clone();
            shifted[1].code_offset -= 1;
            shifted[1].byte_count += 1;
            assert!(validate(&fragment, &source, &shifted).is_err());
        }

        #[test]
        fn ieee_literal_attribution_requires_complete_exact_operation_membership() {
            for value in [
                semantic_vocabulary::IeeeFloatValue::Binary32(0x7fc0_0041),
                semantic_vocabulary::IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            ] {
                let (fragment, mut source) = fixture();
                source.blocks[0].operations[0] = AbstractOperation::IeeeFloatConstant {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: ValueId::new(1).unwrap(),
                    value,
                };
                let rows = produce(&fragment, &source).unwrap();
                validate(&fragment, &source, &rows).unwrap();
                assert_eq!(
                    (
                        rows[0].operation_ordinal,
                        rows[0].code_offset,
                        rows[0].byte_count
                    ),
                    (0, 0, 5)
                );
                let mut omitted = rows.clone();
                omitted.remove(0);
                assert!(validate(&fragment, &source, &omitted).is_err());
                source.blocks[0].operations.swap(0, 1);
                assert!(validate(&fragment, &source, &rows).is_err());
            }
        }

        #[test]
        fn ieee_comparison_attribution_retains_every_span_at_its_exact_ordinal() {
            for format in [
                semantic_vocabulary::IeeeFloatFormat::Binary32,
                semantic_vocabulary::IeeeFloatFormat::Binary64,
            ] {
                let (fragment, mut source) = fixture();
                source.blocks[0].operations[0] = AbstractOperation::IeeeFloatCompare {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: ValueId::new(3).unwrap(),
                    comparison: semantic_vocabulary::IeeeFloatComparisonOperation::Equal,
                    format,
                    left: ValueId::new(1).unwrap(),
                    right: ValueId::new(2).unwrap(),
                };
                let rows = produce(&fragment, &source).unwrap();
                validate(&fragment, &source, &rows).unwrap();
                assert_eq!((rows[0].operation_ordinal, rows[0].byte_count), (0, 5));
                let mut truncated = rows.clone();
                truncated[0].byte_count -= 1;
                assert!(validate(&fragment, &source, &truncated).is_err());
                source.blocks[0].operations.swap(0, 1);
                assert!(validate(&fragment, &source, &rows).is_err());
            }
        }

        #[test]
        fn omitted_operation_and_return_attribution_reject() {
            let (fragment, source) = fixture();
            let rows = produce(&fragment, &source).unwrap();
            for removed in 0..rows.len() {
                let mut omitted = rows.clone();
                omitted.remove(removed);
                assert!(validate(&fragment, &source, &omitted).is_err());
            }
        }

        #[test]
        fn bridge_continuation_does_not_attribute_the_semantic_edge_twice() {
            let (mut fragment, mut source) = fixture();
            let edge = EdgeId::new(4).unwrap();
            source.blocks[0].operations.push(AbstractOperation::Jump {
                structural_bindings: Vec::new(),
                psi_edge: edge,
                target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                cleanup_actions: Vec::new(),
            });
            let successor = machine_code::FunctionFragmentSuccessorProvenance {
                role: SelectedSuccessorRole::Semantic,
                psi_edge: Some(edge),
                block: SelectedBlockId(1),
                source_target: BlockId::new(2).unwrap(),
                bindings: Vec::new(),
                work: Vec::new(),
            };
            let mut jump = fragment.blocks[0].instructions[3].clone();
            jump.instruction = SelectedInstructionId(4);
            jump.offset = 8;
            jump.control = Control::Jump {
                successor: Box::new(successor.clone()),
            };
            fragment.blocks[0].instructions.push(jump.clone());
            jump.instruction = SelectedInstructionId(5);
            jump.offset = 9;
            jump.control = Control::Jump {
                successor: Box::new(machine_code::FunctionFragmentSuccessorProvenance {
                    role: SelectedSuccessorRole::EdgeTransferContinuation,
                    block: SelectedBlockId(2),
                    ..successor
                }),
            };
            fragment.blocks[0].instructions.push(jump);
            fragment.blocks[0].byte_count = 10;
            fragment.byte_count = 10;
            fragment.bytes.resize(10, 0x90);
            let rows = produce(&fragment, &source).unwrap();
            validate(&fragment, &source, &rows).unwrap();
            let row = rows
                .iter()
                .find(|row| row.site == SemanticCodeSite::Edge(edge))
                .unwrap();
            assert_eq!((row.code_offset, row.byte_count), (8, 1));
            let mut forged = rows.clone();
            let row = forged
                .iter_mut()
                .find(|row| row.site == SemanticCodeSite::Edge(edge))
                .unwrap();
            row.byte_count = 2;
            assert!(validate(&fragment, &source, &forged).is_err());
            let row = forged
                .iter_mut()
                .find(|row| row.site == SemanticCodeSite::Edge(edge))
                .unwrap();
            row.code_offset = 9;
            row.byte_count = 1;
            assert!(validate(&fragment, &source, &forged).is_err());
        }

        #[test]
        fn disjoint_same_site_spans_are_not_widened_over_another_operation() {
            let (mut fragment, source) = fixture();
            let instructions = &mut fragment.blocks[0].instructions;
            instructions[1].provenance.operations = vec![OperationId::new(2).unwrap()];
            instructions[2].provenance.operations = vec![OperationId::new(1).unwrap()];
            let rows = produce(&fragment, &source).unwrap();
            validate(&fragment, &source, &rows).unwrap();
            assert_eq!(rows.len(), 4);
            assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 2));
            assert_eq!((rows[1].code_offset, rows[1].byte_count), (5, 2));
            let forged = vec![
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(OperationId::new(1).unwrap()),
                    operation_ordinal: 0,
                    code_offset: 0,
                    byte_count: 7,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(OperationId::new(2).unwrap()),
                    operation_ordinal: 1,
                    code_offset: 2,
                    byte_count: 3,
                },
                SemanticCodeAttribution {
                    site: SemanticCodeSite::Edge(EdgeId::new(3).unwrap()),
                    operation_ordinal: 2,
                    code_offset: 7,
                    byte_count: 1,
                },
            ];
            assert!(validate(&fragment, &source, &forged).is_err());
        }

        #[test]
        fn operation_trap_attributes_only_its_owning_occurrence() {
            let (mut fragment, source) = fixture();
            let expected = produce(&fragment, &source).unwrap();
            let operation = fragment.blocks[0].instructions[0].provenance.operations[0];
            fragment.blocks[0].instructions[0].control = Control::OperationTrap { operation };
            let actual = produce(&fragment, &source).unwrap();
            assert_eq!(actual, expected);
            validate(&fragment, &source, &actual).unwrap();
            fragment.blocks[0].instructions[0].control = Control::OperationTrap {
                operation: OperationId::new(99).unwrap(),
            };
            assert!(produce(&fragment, &source).is_err());
            assert!(validate(&fragment, &source, &actual).is_err());
            fragment.blocks[0].instructions[0].control = Control::OperationTrap { operation };
            fragment.blocks[0].instructions[0]
                .provenance
                .edges
                .push(EdgeId::new(1).unwrap());
            assert!(produce(&fragment, &source).is_err());
            assert!(validate(&fragment, &source, &actual).is_err());
        }

        #[test]
        fn internal_jump_attributes_its_operation_not_a_fabricated_edge() {
            let (mut fragment, source) = fixture();
            let expected = produce(&fragment, &source).unwrap();
            let operation = fragment.blocks[0].instructions[0].provenance.operations[0];
            let successor = machine_code::FunctionFragmentSuccessorProvenance {
                role: SelectedSuccessorRole::OperationContinuation { operation },
                psi_edge: None,
                block: SelectedBlockId(1),
                source_target: BlockId::new(1).unwrap(),
                bindings: Vec::new(),
                work: Vec::new(),
            };
            fragment.blocks[0].instructions[0].control = Control::Jump {
                successor: Box::new(successor.clone()),
            };
            let actual = produce(&fragment, &source).unwrap();
            assert_eq!(actual, expected);
            validate(&fragment, &source, &actual).unwrap();
            for forged in [
                machine_code::FunctionFragmentSuccessorProvenance {
                    psi_edge: EdgeId::new(1),
                    ..successor.clone()
                },
                machine_code::FunctionFragmentSuccessorProvenance {
                    role: SelectedSuccessorRole::OperationContinuation {
                        operation: OperationId::new(99).unwrap(),
                    },
                    ..successor
                },
            ] {
                fragment.blocks[0].instructions[0].control = Control::Jump {
                    successor: Box::new(forged),
                };
                assert!(produce(&fragment, &source).is_err());
                assert!(validate(&fragment, &source, &actual).is_err());
            }
        }

        #[test]
        fn compiler_spill_gaps_remain_unattributed_and_every_site_interval_is_required() {
            let (mut fragment, source) = fixture();
            let instructions = &mut fragment.blocks[0].instructions;
            instructions[1].provenance = Default::default();
            instructions[2].provenance.operations = vec![OperationId::new(1).unwrap()];
            let rows = produce(&fragment, &source).unwrap();
            validate(&fragment, &source, &rows).unwrap();
            assert_eq!(rows.len(), 3);
            assert_eq!((rows[0].code_offset, rows[0].byte_count), (0, 2));
            assert_eq!((rows[1].code_offset, rows[1].byte_count), (5, 2));
            for missing in [0, 1] {
                let mut omitted = rows.clone();
                omitted.remove(missing);
                assert!(validate(&fragment, &source, &omitted).is_err());
            }
            let mut widened = rows.clone();
            widened[0].byte_count = 7;
            widened.remove(1);
            assert!(validate(&fragment, &source, &widened).is_err());
            let mut duplicate = rows.clone();
            duplicate.insert(1, rows[0]);
            assert!(validate(&fragment, &source, &duplicate).is_err());
            let mut reordered = rows.clone();
            reordered.swap(0, 1);
            assert!(validate(&fragment, &source, &reordered).is_err());
            let mut shifted = rows.clone();
            shifted[1].code_offset += 1;
            shifted[1].byte_count -= 1;
            assert!(validate(&fragment, &source, &shifted).is_err());
        }

        #[test]
        fn contiguous_membership_cannot_be_forged_as_nonmaximal_intervals() {
            let (fragment, source) = fixture();
            let mut rows = produce(&fragment, &source).unwrap();
            let mut second = rows[0];
            rows[0].byte_count = 2;
            second.code_offset = 2;
            second.byte_count = 3;
            rows.insert(1, second);
            assert!(validate(&fragment, &source, &rows).is_err());
        }
    }
}

mod byte_literals {
    use super::Error;
    use crate::{ObjectArtifact, object_input::ObjectEmissionInput};
    use object_file::body_object::byte_literals::pool_contents;
    pub(super) use object_file::body_object::byte_literals::{POOL_SYMBOL_NAME, records};
    use object_file::{SectionKind, SymbolKind, SymbolSection};
    pub(super) fn validate(
        source: &ObjectEmissionInput,
        artifact: &ObjectArtifact,
        pool_text_offset: usize,
    ) -> Result<(), Error> {
        let expected = pool_contents(source.publication(), pool_text_offset)?;
        let (pool, (pool_bytes, addresses)) = match (&artifact.byte_literal_pool, expected) {
            (None, None) => {
                if artifact.text_bytes.len() != pool_text_offset {
                    return Err(Error::Mismatch("byte-literal pool bytes without literals"));
                }
                return Ok(());
            }
            (Some(pool), Some(expected)) => (pool, expected),
            _ => return Err(Error::Mismatch("byte-literal pool roster changed")),
        };
        let layout = &artifact.object.layout;
        let symbol = layout
            .symbols
            .is_valid(pool.symbol)
            .then(|| layout.symbols.get(pool.symbol))
            .ok_or(Error::Mismatch("byte-literal pool symbol is missing"))?;
        if pool.text_offset != pool_text_offset
            || pool.byte_count != pool_bytes.len()
            || pool.addresses != addresses
            || artifact.text_bytes.get(pool_text_offset..) != Some(pool_bytes.as_slice())
            || symbol.name != POOL_SYMBOL_NAME
            || symbol.section != SymbolSection::Section(SectionKind::Text)
            || symbol.offset != pool_text_offset
            || symbol.size != pool_bytes.len()
            || symbol.kind != SymbolKind::Object
            || !symbol.import_library.is_empty()
        {
            return Err(Error::Mismatch(
                "byte-literal pool differs from its literals",
            ));
        }
        let expected_records = records(pool, &artifact.functions)?;
        let actual = super::image_addresses::rows_for_symbols(&artifact.relocations, [pool.symbol]);
        if actual != expected_records {
            return Err(Error::Mismatch(
                "byte-literal relocation differs from its placed address",
            ));
        }
        Ok(())
    }
}

mod image_addresses {
    use object_file::{ObjectSymbolHandle, RelocationOrigin, RelocationPlan, RelocationRecord};
    pub(super) fn rows_for_symbols(
        plan: &RelocationPlan,
        symbols: impl IntoIterator<Item = ObjectSymbolHandle>,
    ) -> Vec<RelocationRecord> {
        let symbols = symbols.into_iter().collect::<Vec<_>>();
        plan.records()
            .filter(|(_, record)| {
                symbols.iter().any(|symbol| {
                    record.symbol_handle == *symbol
                        || record.origin
                            == RelocationOrigin::Materialization {
                                object_symbol_handle: *symbol,
                            }
                })
            })
            .map(|(_, record)| record.clone())
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::rows_for_symbols;
        use object_file::{
            ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
            RelocationRecord, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
        };

        fn symbol(object: &mut ObjectPlan, name: &str) -> ObjectSymbolHandle {
            object.layout.symbols.insert(SymbolPlan {
                name: name.into(),
                section: SymbolSection::Section(SectionKind::Text),
                offset: 0,
                size: 0,
                kind: SymbolKind::Object,
                import_library: String::new(),
            })
        }

        fn record(
            origin: RelocationOrigin,
            symbol_handle: ObjectSymbolHandle,
            offset: usize,
        ) -> RelocationRecord {
            RelocationRecord {
                origin,
                section: SectionKind::Text,
                offset,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind: RelocationKind::X86_64Relative32,
            }
        }

        #[test]
        fn owned_rows_join_binding_symbols_and_materialization_origins_in_plan_order() {
            let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 1, 2);
            let first = symbol(&mut object, "first");
            let second = symbol(&mut object, "second");
            let mut plan = RelocationPlan::with_target(object.target);
            plan.push_record(record(
                RelocationOrigin::Materialization {
                    object_symbol_handle: second,
                },
                first,
                0,
            ));
            plan.push_record(record(
                RelocationOrigin::Materialization {
                    object_symbol_handle: first,
                },
                first,
                8,
            ));
            plan.push_record(record(
                RelocationOrigin::Materialization {
                    object_symbol_handle: second,
                },
                second,
                16,
            ));
            let rows = rows_for_symbols(&plan, [first]);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].offset, 0);
            assert_eq!(rows[1].offset, 8);
            let both = rows_for_symbols(&plan, [second, first]);
            assert_eq!(
                both.iter().map(|row| row.offset).collect::<Vec<_>>(),
                [0, 8, 16]
            );
            assert!(rows_for_symbols(&plan, []).is_empty());
        }
    }
}

pub(crate) mod image_residence {
    use super::{Error, image_addresses::rows_for_symbols};
    pub(crate) use object_file::body_object::image_residence::declare_symbol;
    use object_file::body_object::image_residence::observes_residence;
    pub(super) use object_file::body_object::image_residence::records;
    use object_file::{SymbolKind, SymbolSection};
    pub(super) fn validate(
        source: &crate::object_input::ObjectEmissionInput,
        artifact: &crate::ObjectArtifact,
    ) -> Result<(), Error> {
        let addresses = &source.text_section().unresolved_image_addresses;
        let present = addresses.iter().any(observes_residence);
        if present != artifact.retained_entry_storage.is_valid() {
            return Err(Error::Mismatch(
                "image residence observation roster changed",
            ));
        }
        if !present {
            return Ok(());
        }
        let symbol = artifact
            .object
            .layout
            .symbols
            .get(artifact.retained_entry_storage);
        if artifact.profile != Some(target::TargetProfile::UefiX64)
            || symbol.section != SymbolSection::None
            || symbol.offset != 0
            || symbol.size != 0
            || symbol.kind != SymbolKind::Object
            || !symbol.import_library.is_empty()
        {
            return Err(Error::Mismatch(
                "image residence was defined before physical placement",
            ));
        }
        let expected = records(
            addresses,
            artifact.retained_entry_storage,
            &artifact.functions,
        )?;
        let actual = rows_for_symbols(&artifact.relocations, [artifact.retained_entry_storage]);
        if actual != expected {
            return Err(Error::Mismatch("image residence relocations changed"));
        }
        Ok(())
    }
}

pub(crate) mod code_descriptors {
    use super::Error;
    use object_file::{
        ObjectPlan, RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind,
    };

    use object_file::body_object::code_descriptors::place;
    pub(super) use object_file::body_object::code_descriptors::records;
    pub(crate) use object_file::body_object::code_descriptors::{
        ObjectCodeDescriptorPool, address_source, target_entries,
    };
    pub(super) fn validate(
        artifact: &crate::ObjectArtifact,
        selections: Option<&super::current_image_selections::CurrentImageSelections>,
    ) -> Result<(), Error> {
        let catalogs = artifact
            .physical_facts()
            .contracts
            .publication()
            .code_catalogs();
        let Some(pool) = &artifact.code_descriptor_pool else {
            if (!artifact.data_bytes.is_empty() && artifact.native_code_layout_pool.is_none())
                || !catalogs.dynamic_dispatch().tables.is_empty()
                || artifact
                    .image_data
                    .addresses
                    .iter()
                    .any(|address| address_source(address).is_some())
            {
                return Err(Error::Mismatch(
                    "initialized data has no implementation table owner",
                ));
            }
            return Ok(());
        };
        if &pool.catalogs != catalogs
            || pool.addresses
                != artifact
                    .image_data
                    .addresses
                    .iter()
                    .filter(|address| address_source(address).is_some())
                    .copied()
                    .collect::<Vec<_>>()
        {
            return Err(Error::Mismatch(
                "code descriptor inputs differ from admitted source",
            ));
        }
        if !pool.addresses.is_empty() {
            let selections = selections.ok_or(Error::Mismatch(
                "descriptor addresses have no current selection index",
            ))?;
            for address in &pool.addresses {
                let result = match address.machine_descriptor() {
                    Some(descriptor) => Some(semantic_vocabulary::ScalarType::Callable(
                        pool.catalogs
                            .callables()
                            .descriptor(descriptor)
                            .ok_or(Error::Mismatch(
                                "code descriptor has no current callable contract",
                            ))?
                            .contract,
                    )),
                    None => None,
                };
                selections.require(address, result)?;
            }
        }
        let mut expected = ObjectPlan::with_capacity(artifact.target, 1, pool.descriptors.len());
        let (rebuilt, data) = place(&pool.catalogs, &pool.addresses, &mut expected)?;
        let rebuilt = rebuilt.ok_or(Error::Mismatch("implementation table pool disappeared"))?;
        if artifact.data_bytes.get(..data.len()) != Some(data.as_slice())
            || rebuilt.descriptors.len() != pool.descriptors.len()
        {
            return Err(Error::Mismatch("code descriptor bytes changed"));
        }
        let data_section = artifact
            .object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == SectionKind::Data)
            .ok_or(Error::Mismatch(
                "implementation table data section is absent",
            ))?
            .1;
        if data_section.size != artifact.data_bytes.len() || data_section.alignment != 8 {
            return Err(Error::Mismatch(
                "implementation table data geometry changed",
            ));
        }
        for (actual, expected_table) in pool.descriptors.iter().zip(&rebuilt.descriptors) {
            if actual.source != expected_table.source
                || actual.data_offset != expected_table.data_offset
                || actual.byte_count != expected_table.byte_count
                || !artifact.object.layout.symbols.is_valid(actual.symbol)
                || artifact.object.layout.symbols.get(actual.symbol)
                    != expected.layout.symbols.get(expected_table.symbol)
            {
                return Err(Error::Mismatch("code descriptor symbol changed"));
            }
        }
        let actual = super::image_addresses::rows_for_symbols(
            &artifact.relocations,
            pool.descriptors.iter().map(|table| table.symbol),
        );
        if actual != records(pool, &artifact.functions)? {
            return Err(Error::Mismatch("code descriptor relocations changed"));
        }
        Ok(())
    }

    pub(crate) fn validate_image_data_relocations(
        artifact: &crate::ObjectArtifact,
        object: &ObjectPlan,
        relocations: &RelocationPlan,
    ) -> Result<(), Error> {
        let mut expected = Vec::new();
        for namespace in crate::object::private_object_composition::namespaces(artifact) {
            let local = namespace.artifact;
            let image_selections = local.image_data.selections.as_ref();
            let Some(pool) = &local.code_descriptor_pool else {
                if !local.data_bytes.is_empty() && local.native_code_layout_pool.is_none() {
                    return Err(Error::Mismatch(
                        "initialized image data has no implementation table owner",
                    ));
                }
                expected.extend(super::native_code_layouts::image_data_records(
                    &namespace,
                    artifact,
                    object,
                    image_selections,
                )?);
                continue;
            };
            validate(local, image_selections)?;
            for symbol in pool
                .descriptors
                .iter()
                .map(|table| table.symbol)
                .chain(local.functions.iter().map(|function| function.symbol))
            {
                let symbol = namespace.symbol(symbol)?;
                if !object.layout.symbols.is_valid(symbol)
                    || object.layout.symbols.get(symbol)
                        != artifact.object.layout.symbols.get(symbol)
                {
                    return Err(Error::Mismatch(
                        "image changed code descriptor symbol custody",
                    ));
                }
            }
            for mut record in records(pool, &local.functions)? {
                record.symbol_handle = namespace.symbol(record.symbol_handle)?;
                record.origin = RelocationOrigin::Materialization {
                    object_symbol_handle: namespace.symbol(record.origin.symbol_handle())?,
                };
                record.offset = record
                    .offset
                    .checked_add(namespace.data_offset)
                    .ok_or(Error::Overflow)?;
                expected.push(record);
            }
            expected.extend(super::native_code_layouts::image_data_records(
                &namespace,
                artifact,
                object,
                image_selections,
            )?);
        }
        validate_data_relocation_roster(&expected, relocations)
    }

    fn validate_data_relocation_roster(
        expected: &[RelocationRecord],
        actual: &RelocationPlan,
    ) -> Result<(), Error> {
        if !expected
            .iter()
            .filter(|record| record.section == SectionKind::Data)
            .eq(actual
                .records()
                .filter(|(_, record)| record.section == SectionKind::Data)
                .map(|(_, record)| record))
        {
            return Err(Error::Mismatch(
                "initialized image data relocations differ from exact code descriptor cells",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use object_file::body_object::code_descriptors::records_for_symbols;
        use object_file::{RelocationKind, SymbolKind, SymbolPlan, SymbolSection};
        use terminal_psi::TerminalDynamicDispatchCatalog;
        use terminal_psi::{TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable};

        fn catalog(
            rows: Vec<Option<semantic_vocabulary::MachineId>>,
        ) -> TerminalDynamicDispatchCatalog {
            TerminalDynamicDispatchCatalog {
                interfaces: vec![TerminalDynamicInterface {
                    id: 0,
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    slots: (0..rows.len())
                        .map(|slot| TerminalDynamicSlot {
                            slot: slot as u32,
                            declaring_trait_identity: terminal_psi::TerminalIdentityIndex(1),
                            requirement_identity: terminal_psi::TerminalIdentityIndex(slot as u32),
                            family_tuple: Vec::new(),
                            scalar_parameters: Vec::new(),
                            structural_parameters: Vec::new(),
                            entry_claim_paths: Vec::new(),
                            result: terminal_psi::BoundaryMachineResult::Unit,
                            result_claim_paths: Vec::new(),
                            result_reference_regions: Vec::new(),
                        })
                        .collect(),
                }],
                tables: vec![TerminalDynamicTable {
                    id: 0,
                    interface: 0,
                    implementation_identity: terminal_psi::TerminalIdentityIndex(2),
                    rows,
                }],
            }
        }

        #[test]
        fn trap_rows_keep_slot_offsets_and_callable_relocations() {
            let machine = semantic_vocabulary::MachineId::new(1).unwrap();
            let catalog = catalog(vec![None, Some(machine), None, Some(machine)]);
            let catalogs =
                target_vocabulary::NativeCodeCatalogs::seal(catalog, Default::default()).unwrap();
            let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 2, 2);
            let (pool, data) = place(&catalogs, &[], &mut object).unwrap();
            let pool = pool.unwrap();
            assert_eq!(data, vec![0; 32]);
            let symbol = object.layout.symbols.insert(SymbolPlan {
                name: "function".into(),
                section: SymbolSection::Section(SectionKind::Text),
                offset: 0,
                size: 4,
                kind: SymbolKind::Function,
                import_library: String::new(),
            });
            let relocations =
                records_for_symbols(&pool, |selected| (selected == machine).then_some(symbol))
                    .unwrap();
            assert_eq!(
                relocations
                    .iter()
                    .map(|record| record.offset)
                    .collect::<Vec<_>>(),
                [8, 24]
            );
            assert!(
                relocations
                    .iter()
                    .all(|record| record.symbol_handle == symbol
                        && record.kind == RelocationKind::Absolute64
                        && record.section == SectionKind::Data
                        && record.byte_width == 8)
            );
            assert_eq!(
                records_for_symbols(&pool, |selected| (selected == machine).then_some(symbol))
                    .unwrap()
                    .len(),
                2
            );
            let relocation_plan = |rows: &[RelocationRecord]| {
                let mut plan = RelocationPlan::with_target(object.target);
                for row in rows {
                    plan.push_record(row.clone());
                }
                plan
            };
            assert!(
                validate_data_relocation_roster(&relocations, &relocation_plan(&relocations),)
                    .is_ok()
            );
            let mut extra = relocations.clone();
            let mut trap = relocations[0].clone();
            trap.offset = 0;
            extra.push(trap);
            assert!(
                validate_data_relocation_roster(&relocations, &relocation_plan(&extra)).is_err()
            );
            let mut substituted = relocations.clone();
            substituted[0].symbol_handle = pool.descriptors[0].symbol;
            assert!(
                validate_data_relocation_roster(&relocations, &relocation_plan(&substituted),)
                    .is_err()
            );
            assert!(
                validate_data_relocation_roster(&relocations, &relocation_plan(&relocations[..1]),)
                    .is_err()
            );
            assert!(validate_data_relocation_roster(&[], &relocation_plan(&relocations)).is_err());
        }

        #[test]
        fn empty_tables_keep_distinct_nonzero_storage() {
            let mut catalog = catalog(Vec::new());
            catalog.tables.push(TerminalDynamicTable {
                id: 1,
                interface: 0,
                implementation_identity: terminal_psi::TerminalIdentityIndex(3),
                rows: Vec::new(),
            });
            let mut object = ObjectPlan::with_capacity(target::NativeTarget::macos_arm64(), 1, 2);
            let catalogs =
                target_vocabulary::NativeCodeCatalogs::seal(catalog, Default::default()).unwrap();
            let (pool, data) = place(&catalogs, &[], &mut object).unwrap();
            let pool = pool.unwrap();
            assert_eq!(data, vec![0; 16]);
            assert_eq!(
                pool.descriptors
                    .iter()
                    .map(|table| table.data_offset)
                    .collect::<Vec<_>>(),
                [0, 8]
            );
            assert_ne!(pool.descriptors[0].symbol, pool.descriptors[1].symbol);
            assert!(records_for_symbols(&pool, |_| None).unwrap().is_empty());
        }
    }
}

mod imports {
    use super::Error;
    use crate::ObjectArtifact;

    use crate::object_input::ObjectEmissionInput;
    use machine_code::CallbackAddressEncoding;
    use object_file::{
        SymbolKind, SymbolSection, normalized_import_symbol_name, object_function_reference,
    };

    use object_file::body_object::imports::call_for_import_field;
    pub(super) fn validate(
        source: &ObjectEmissionInput,
        artifact: &ObjectArtifact,
        private_address_records: usize,
    ) -> Result<(), Error> {
        let calls = object_file::body_object::imports::readback(
            source.publication(),
            &artifact.object,
            &artifact.relocations,
            &artifact.functions,
        )?;
        let import_calls = calls
            .iter()
            .filter_map(|call| call.binding.import_locator().map(|locator| (call, locator)))
            .collect::<Vec<_>>();
        let fields = &source.text_section().unresolved_normalized_foreign_calls;
        let layout = &artifact.object.layout;
        let expected_records = fields.len()
            + calls
                .iter()
                .flat_map(|call| call.callback_addresses.iter())
                .map(|callback| match callback.encoding {
                    CallbackAddressEncoding::X86_64Relative32 { .. } => 1,
                    CallbackAddressEncoding::Aarch64PageAddress { .. } => 2,
                })
                .sum::<usize>();
        let mut observation_records = super::image_residence::records(
            &source.text_section().unresolved_image_addresses,
            artifact.retained_entry_storage,
            &artifact.functions,
        )?
        .len();
        if let Some(pool) = &artifact.byte_literal_pool {
            observation_records += super::byte_literals::records(pool, &artifact.functions)?.len();
        }
        if let Some(pool) = &artifact.code_descriptor_pool {
            observation_records +=
                super::code_descriptors::records(pool, &artifact.functions)?.len();
        }
        if let Some(pool) = &artifact.native_code_layout_pool {
            observation_records +=
                super::native_code_layouts::records(pool, &artifact.functions)?.len();
        }
        let expected_records = expected_records + observation_records + private_address_records;
        if artifact.foreign_calls != calls
            || artifact.relocations.record_count() != expected_records
            || fields.len() != import_calls.len()
        {
            return Err(Error::Mismatch("fragment import relocation roster changed"));
        }
        let mut imports = layout.normalized_imports.iter();
        let mut symbols = layout.symbols.iter().skip(
            artifact.functions.len()
                + layout.function_symbols.len()
                + usize::from(artifact.byte_literal_pool.is_some())
                + usize::from(artifact.retained_entry_storage.is_valid())
                + artifact
                    .code_descriptor_pool
                    .as_ref()
                    .map_or(0, |pool| pool.descriptors.len())
                + artifact
                    .native_code_layout_pool
                    .as_ref()
                    .map_or(0, |pool| pool.layouts.len()),
        );
        let mut prior_locators = Vec::new();
        for field in fields {
            let (_, locator) = call_for_import_field(&import_calls, field)?;
            if prior_locators.contains(&locator) {
                continue;
            }
            prior_locators.push(locator);
            let (Some(import), Some((symbol_handle, symbol))) = (imports.next(), symbols.next())
            else {
                return Err(Error::Mismatch("fragment import is missing"));
            };
            if import.locator.as_ref() != locator
                || import.symbol != symbol_handle
                || symbol.name != normalized_import_symbol_name(locator)
                || symbol.kind != SymbolKind::Import
                || symbol.section != SymbolSection::None
                || symbol.offset != 0
                || symbol.size != 0
                || !symbol.import_library.is_empty()
                || layout
                    .symbols
                    .iter()
                    .filter(|(_, other)| other.name == symbol.name)
                    .count()
                    != 1
            {
                return Err(Error::Mismatch(
                    "fragment import differs from selected locator",
                ));
            }
        }
        if imports.next().is_some() || symbols.next().is_some() {
            return Err(Error::Mismatch("fragment import roster has extra rows"));
        }
        for (call, _) in &import_calls {
            for callback in &call.callback_addresses {
                let Some((callback_symbol, callback_plan)) =
                    object_function_reference(&artifact.object, callback.target.callback_function)
                else {
                    return Err(Error::Mismatch(
                        "fragment import callback private function is missing",
                    ));
                };
                let mut carriers = artifact
                    .private_functions
                    .iter()
                    .filter(|private| private.identity == callback.target.callback_function);
                match (carriers.next(), carriers.next()) {
                    (Some(private), None)
                        if private.function.symbol == callback_symbol
                            && private.function.text_offset == callback_plan.offset
                            && private.function.byte_count == callback_plan.size => {}
                    (None, None)
                        if callback_plan.section == SymbolSection::None
                            && callback_plan.kind == SymbolKind::Function
                            && callback_plan.offset == 0
                            && callback_plan.size == 0
                            && object_file::private_function_symbol_name(
                                callback.target.callback_function,
                            )
                            .as_deref()
                                == Some(callback_plan.name.as_str()) => {}
                    _ => {
                        return Err(Error::Mismatch(
                            "fragment callback definition or reference differs",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

pub(crate) mod native_code_layouts {
    use super::Error;
    use object_file::{ObjectPlan, RelocationOrigin, RelocationRecord, SectionKind};
    use selected_instructions::SelectedImageAddressReference;

    pub(crate) use object_file::body_object::native_code_layouts::NativeCodeLayoutPool;
    use object_file::body_object::native_code_layouts::place;
    pub(super) use object_file::body_object::native_code_layouts::records;
    pub(super) fn validate(
        artifact: &crate::ObjectArtifact,
        selections: Option<&super::current_image_selections::CurrentImageSelections>,
    ) -> Result<(), Error> {
        let addresses = artifact
            .image_data
            .addresses
            .iter()
            .filter(|row| {
                matches!(
                    row.reference,
                    SelectedImageAddressReference::NativeCodeLayout { .. }
                )
            })
            .copied()
            .collect::<Vec<_>>();
        let prefix = artifact.code_descriptor_pool.as_ref().map_or(0, |pool| {
            pool.descriptors
                .last()
                .map_or(0, |row| row.data_offset + row.byte_count)
        });
        let Some(pool) = &artifact.native_code_layout_pool else {
            if !addresses.is_empty() || artifact.data_bytes.len() != prefix {
                return Err(Error::Mismatch(
                    "native metadata or initialized-data owner is absent",
                ));
            }
            return Ok(());
        };
        if pool.addresses != addresses || pool.data_start != prefix {
            return Err(Error::Mismatch(
                "native metadata selection or prefix differs from admitted source",
            ));
        }
        if !addresses.is_empty() {
            let selections = selections.ok_or(Error::Mismatch(
                "native metadata has no current selection index",
            ))?;
            for address in &addresses {
                selections.require(address, None)?;
            }
        }
        let mut object = ObjectPlan::with_capacity(artifact.target, 1, pool.layouts.len());
        let mut data = artifact
            .data_bytes
            .get(..prefix)
            .ok_or(Error::Mismatch("native metadata prefix is truncated"))?
            .to_vec();
        let rebuilt = place(
            &artifact.image_data.native_layouts,
            &addresses,
            &mut object,
            &mut data,
        )?
        .ok_or(Error::Mismatch("native metadata pool disappeared"))?;
        if data != artifact.data_bytes || rebuilt.layouts.len() != pool.layouts.len() {
            return Err(Error::Mismatch(
                "native metadata bytes or realization roster changed",
            ));
        }
        for (actual, rebuilt) in pool.layouts.iter().zip(&rebuilt.layouts) {
            if actual.layout != rebuilt.layout
                || actual.data_offset != rebuilt.data_offset
                || actual.byte_count != rebuilt.byte_count
                || !artifact.object.layout.symbols.is_valid(actual.symbol)
                || artifact.object.layout.symbols.get(actual.symbol)
                    != object.layout.symbols.get(rebuilt.symbol)
            {
                return Err(Error::Mismatch(
                    "native metadata symbol or final instruction attribution changed",
                ));
            }
        }
        let section = artifact
            .object
            .layout
            .sections
            .iter()
            .find(|(_, section)| section.kind == SectionKind::Data)
            .ok_or(Error::Mismatch("native metadata has no data section"))?
            .1;
        if section.size != data.len() || section.alignment != 8 {
            return Err(Error::Mismatch("native metadata data geometry changed"));
        }
        let actual = super::image_addresses::rows_for_symbols(
            &artifact.relocations,
            pool.layouts.iter().map(|descriptor| descriptor.symbol),
        );
        if actual != records(pool, &artifact.functions)? {
            return Err(Error::Mismatch("native metadata relocation roster changed"));
        }
        Ok(())
    }

    pub(crate) fn image_data_records(
        namespace: &crate::object::private_object_composition::ObjectNamespace<'_>,
        root: &crate::ObjectArtifact,
        object: &ObjectPlan,
        selections: Option<&super::current_image_selections::CurrentImageSelections>,
    ) -> Result<Vec<RelocationRecord>, Error> {
        let local = namespace.artifact;
        let Some(pool) = &local.native_code_layout_pool else {
            return Ok(Vec::new());
        };
        validate(local, selections)?;
        for symbol in pool
            .layouts
            .iter()
            .map(|row| row.symbol)
            .chain(local.functions.iter().map(|function| function.symbol))
        {
            let symbol = namespace.symbol(symbol)?;
            if !object.layout.symbols.is_valid(symbol)
                || object.layout.symbols.get(symbol) != root.object.layout.symbols.get(symbol)
            {
                return Err(Error::Mismatch(
                    "image changed native metadata symbol custody",
                ));
            }
        }
        records(pool, &local.functions)?
            .into_iter()
            .filter(|row| row.section == SectionKind::Data)
            .map(|mut row| {
                row.symbol_handle = namespace.symbol(row.symbol_handle)?;
                row.origin = RelocationOrigin::Materialization {
                    object_symbol_handle: namespace.symbol(row.origin.symbol_handle())?,
                };
                row.offset = row
                    .offset
                    .checked_add(namespace.data_offset)
                    .ok_or(Error::Overflow)?;
                Ok(row)
            })
            .collect()
    }
}

mod port_effects {
    use super::{Error, attribution, host, source, structural::placed_text_offset};
    use crate::ObjectPortEffect;
    use crate::object_input::ObjectEmissionInput;
    use machine_code::{PortEffectRecord, SemanticCodeSite};
    use selected_instructions::{MachineAlternativeFamily, MachineAlternativeKey};

    pub(super) fn produce(container: &ObjectEmissionInput) -> Result<Vec<ObjectPortEffect>, Error> {
        let origins = container
            .emission_facts()
            .physical
            .port_writes
            .as_ref()
            .map_err(|reason| Error::Mismatch(reason))?;
        let mut effects = Vec::new();
        for placed in &container.text_section().functions {
            let fragment = source::fragments(container)
                .function(placed.machine)
                .ok_or(Error::Mismatch(
                    "port event lost its current function fragment",
                ))?;
            append_function_effects(
                origins.iter().filter(|row| row.machine == placed.machine),
                fragment,
                source::publication_function(container, placed.machine)?,
                placed.section_offset,
                &mut effects,
            )?;
        }
        Ok(effects)
    }

    fn append_function_effects<'input>(
        instructions: impl IntoIterator<
            Item = &'input machine_code::function_metadata::PhysicalPortWrite,
        >,
        fragment: &machine_code::FunctionFragment,
        publication: &target_vocabulary::NativeFunctionPublication,
        section_offset: u64,
        effects: &mut Vec<ObjectPortEffect>,
    ) -> Result<(), Error> {
        if fragment.machine != publication.machine {
            return Err(Error::Mismatch("port event has another function owner"));
        }
        let mut spans: Vec<_> = fragment
            .instructions()
            .filter(|span| span.alternative.family == MachineAlternativeFamily::PortWrite)
            .collect();
        spans.sort_unstable_by_key(|span| span.instruction);
        if spans
            .windows(2)
            .any(|rows| rows[0].instruction == rows[1].instruction)
        {
            return Err(Error::Mismatch("port event has two selected spans"));
        }
        let start = effects.len();
        for instruction in instructions {
            if instruction.machine != publication.machine {
                return Err(Error::Mismatch("port event has another function owner"));
            }
            let psi_operation = instruction.operation;
            let index = spans
                .binary_search_by_key(&instruction.instruction, |span| span.instruction)
                .map_err(|_| Error::Mismatch("port event has no exact selected span"))?;
            let span = spans[index];
            if span.alternative
                != (MachineAlternativeKey {
                    family: MachineAlternativeFamily::PortWrite,
                    variant: 0,
                })
                || span.provenance.operations.as_slice() != [psi_operation]
            {
                return Err(Error::Mismatch(
                    "port event differs from its current instruction",
                ));
            }
            let code_offset = host(span.offset)?;
            effects.push(ObjectPortEffect {
                machine: publication.machine,
                text_offset: placed_text_offset(section_offset, code_offset)?,
                effect: PortEffectRecord {
                    psi_operation,
                    service: instruction.service,
                    port: instruction.port,
                    value: instruction.value,
                    operation_ordinal: attribution::ordinal(
                        publication,
                        SemanticCodeSite::Operation(psi_operation),
                    )?,
                    code_offset,
                    byte_count: span.bytes.len(),
                },
            });
        }
        if effects.len() - start != spans.len() {
            return Err(Error::Mismatch("port event has an unowned selected span"));
        }
        // Preserve authored order even when native block placement changes.
        effects[start..].sort_unstable_by_key(|row| row.effect.operation_ordinal);
        if effects[start..]
            .windows(2)
            .any(|rows| rows[0].effect.operation_ordinal == rows[1].effect.operation_ordinal)
        {
            return Err(Error::Mismatch("port event repeats its source occurrence"));
        }
        Ok(())
    }

    #[cfg(test)]
    mod current_event_tests {
        use super::*;
        use machine_code::{
            FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance,
            FunctionFragmentInstructionSpan,
        };
        use selected_instructions::{
            SelectedBlockId, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance,
        };
        use semantic_vocabulary::{MachineId, OperationId, ServiceId};

        fn instruction(
            id: u32,
            operation: u64,
            service: u64,
            port: u16,
            value: u8,
        ) -> SelectedInstruction {
            SelectedInstruction {
                id: SelectedInstructionId(id),
                kind: SelectedInstructionKind::PortWrite {
                    service: ServiceId::new(service).unwrap(),
                    port,
                    value,
                },
                constraint: register_model::RegisterConstraintKey::instruction(0),
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(operation).unwrap()],
                    ..Default::default()
                },
            }
        }

        fn span(instruction: &SelectedInstruction, offset: u64) -> FunctionFragmentInstructionSpan {
            let physical = register_model::validate_physical_register_model(
                target_x86_64::x86_64_physical_register_model(),
            )
            .unwrap();
            let alternative = MachineAlternativeKey {
                family: MachineAlternativeFamily::PortWrite,
                variant: 0,
            };
            let encoded = isa_x86_64::encode_x86_64_selected_form(
                &physical,
                instruction.kind,
                alternative,
                &[],
            )
            .unwrap();
            FunctionFragmentInstructionSpan {
                instruction: instruction.id,
                alternative,
                offset,
                bytes: encoded.bytes().to_vec(),
                branch: None,
                internal_machine_fixup: None,
                normalized_foreign_call_fixup: None,
                image_address_fixup: None,
                provenance: instruction.provenance.clone(),
                control: FunctionFragmentControlProvenance::None,
            }
        }

        #[test]
        fn current_port_events_keep_service_authored_order_and_exact_native_owners() {
            // Model only these current columns; the actual x86 encoder supplies
            // span bytes. This does not execute privileged port instructions.
            let later = instruction(0, 2, 12, 0x2f8, 0x42);
            let earlier = instruction(1, 1, 11, 0x3f8, 0x41);
            let first = span(&later, 0);
            let offset = first.bytes.len() as u64;
            let second = span(&earlier, offset);
            let bytes = [first.bytes.as_slice(), second.bytes.as_slice()].concat();
            let machine = MachineId::new(1).unwrap();
            let fragment = FunctionFragment {
                machine,
                attachment: None,
                provenance: Default::default(),
                byte_count: bytes.len() as u64,
                bytes,
                blocks: vec![FunctionFragmentBlockSpan {
                    block: SelectedBlockId(0),
                    offset: 0,
                    byte_count: offset + second.bytes.len() as u64,
                    instructions: vec![first, second],
                }],
            };
            let publication = target_vocabulary::NativeFunctionPublication {
                machine,
                operation_count: 2,
                sites: vec![
                    (
                        target_vocabulary::publication::PsiProvenance::Operation(
                            OperationId::new(1).unwrap(),
                        ),
                        0,
                    ),
                    (
                        target_vocabulary::publication::PsiProvenance::Operation(
                            OperationId::new(2).unwrap(),
                        ),
                        1,
                    ),
                ],
                byte_literals: Vec::new(),
                call_targets: Vec::new(),
            };
            let instructions = [later, earlier];
            let run = |fragment: &_, publication: &_| {
                let mut rows = Vec::new();
                let origins = frame_planning::physical_facts::port_writes_for_function(
                    machine,
                    &instructions,
                )
                .map_err(Error::Mismatch)?;
                append_function_effects(&origins, fragment, publication, 32, &mut rows)?;
                Ok::<_, Error>(rows)
            };
            let rows = run(&fragment, &publication).unwrap();
            assert_eq!(
                rows.iter()
                    .map(|row| (
                        row.effect.psi_operation.get(),
                        row.effect.service.get(),
                        row.effect.port,
                        row.effect.value,
                        row.effect.operation_ordinal
                    ))
                    .collect::<Vec<_>>(),
                [(1, 11, 0x3f8, 0x41, 0), (2, 12, 0x2f8, 0x42, 1)]
            );
            assert_eq!(rows[0].text_offset, 32 + offset as usize);
            assert_eq!(rows[1].text_offset, 32);
            let mut missing = fragment.clone();
            missing.blocks[0].instructions.pop();
            assert!(run(&missing, &publication).is_err());
            let mut repeated = fragment.clone();
            let duplicate = repeated.blocks[0].instructions[0].clone();
            repeated.blocks[0].instructions.push(duplicate);
            assert!(run(&repeated, &publication).is_err());
            let mut foreign = fragment.clone();
            foreign.blocks[0].instructions[0].provenance.operations[0] =
                OperationId::new(3).unwrap();
            assert!(run(&foreign, &publication).is_err());
            let mut orphan = fragment.clone();
            let mut extra = orphan.blocks[0].instructions[0].clone();
            extra.instruction = SelectedInstructionId(9);
            orphan.blocks[0].instructions.push(extra);
            assert!(run(&orphan, &publication).is_err());
            let mut unbound = publication;
            unbound.sites.clear();
            assert!(run(&fragment, &unbound).is_err());
        }
    }
}

mod private_addresses {
    use super::Error;
    use crate::ObjectArtifact;
    use function_identity::MachineFunctionIdentity;
    use machine_code::PlacedImageAddress;
    use object_file::{ObjectPlan, RelocationRecord, SymbolKind, SymbolSection};

    pub(super) use object_file::body_object::private_addresses::identities;
    use object_file::body_object::private_addresses::records;
    pub(super) fn validate(
        identities: &[MachineFunctionIdentity],
        addresses: &[PlacedImageAddress],
        artifact: &ObjectArtifact,
    ) -> Result<usize, Error> {
        validate_symbols(identities, &artifact.object, artifact.functions.len())?;
        let expected = records(
            addresses,
            &artifact.object,
            &artifact.functions,
            artifact.psi,
        )?;
        validate_relocation_tail(&artifact.relocations, &expected)?;
        Ok(expected.len())
    }

    fn validate_symbols(
        identities: &[MachineFunctionIdentity],
        object: &ObjectPlan,
        ordinary_functions: usize,
    ) -> Result<(), Error> {
        let layout = &object.layout;
        if identities.len() != layout.function_symbols.len() {
            return Err(Error::Mismatch("private function reference roster changed"));
        }
        let mut symbols = layout.symbols.iter().skip(ordinary_functions);
        for (identity, (_, reference)) in identities.iter().zip(layout.function_symbols.iter()) {
            let (handle, symbol) = symbols.next().ok_or(Error::Mismatch(
                "private function reference symbol is absent",
            ))?;
            if reference.identity != *identity
                || reference.symbol != handle
                || object_file::private_function_symbol_name(*identity).as_deref()
                    != Some(symbol.name.as_str())
                || symbol.kind != SymbolKind::Function
                || symbol.section != SymbolSection::None
                || symbol.offset != 0
                || symbol.size != 0
                || !symbol.import_library.is_empty()
            {
                return Err(Error::Mismatch(
                    "private function reference differs from its retained identity",
                ));
            }
        }
        Ok(())
    }

    fn validate_relocation_tail(
        relocations: &object_file::RelocationPlan,
        expected: &[RelocationRecord],
    ) -> Result<(), Error> {
        let start = relocations
            .record_count()
            .checked_sub(expected.len())
            .ok_or(Error::Mismatch("private address relocation is absent"))?;
        if !relocations
            .records()
            .skip(start)
            .map(|(_, row)| row)
            .eq(expected.iter())
        {
            return Err(Error::Mismatch(
                "private address relocations differ from retained observations",
            ));
        }
        Ok(())
    }
}

mod production {
    use super::{Error, attribution, host, source};
    use crate::{
        ObjectArtifact, ObjectCodeAttribution, ObjectFunction, ObjectScalarCallStack,
        ObjectScalarStack, ObjectUnitCallStack, ObjectUnitStack,
    };
    use std::sync::Arc;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ProjectedObject(());

    /// Carry the issued sections and relocations into image composition. The
    /// function and stack evidence describes the object-relative coordinates.
    /// Explicit audits retain their original stage inputs alongside this view.
    pub fn prepare_object_for_image(
        object: object_file::PublishedFunctionObject,
    ) -> Result<ObjectArtifact, Error> {
        let (publication, sections, replay) = object.into_parts();
        let object_file::FunctionObjectSections {
            object,
            relocations,
            text_bytes,
            data_bytes,
            functions: locations,
            foreign_calls,
            retained_entry_storage,
            byte_literal_pool,
            code_descriptor_pool,
            native_code_layout_pool,
        } = sections;
        let emission_input = Arc::new(crate::object_input::ObjectEmissionInput::from_publication(
            publication,
        ));
        let source = emission_input.as_ref();
        let callable_invocations = source::callable_invocations(source)?;
        let profile = super::source::profile(source);
        let text = source.text_section();
        let fragments = source::fragments(source);
        let temporary_stack_bytes = if locations.is_empty() {
            Vec::new()
        } else {
            source::temporary_stack_bytes_by_function(source)?
        };
        let mut functions = Vec::new();
        let mut semantic_code_attribution = Vec::new();
        let mut requires_graph_storage_replay = false;
        for location in locations {
            let machine = location.machine;
            let (attachment, provenance) = source::fragment_metadata(source, machine)?;
            let abstracted = source::function_facts(source, machine)?;
            let abi = source::function_abi(source, machine)?;
            requires_graph_storage_replay |= abstracted.requires_graph_storage_replay
                || super::structural::requires_call_graph_replay(source, machine)?;
            let offset = location.text_offset;
            let frame = source::frame(source, machine)?;
            let frame_bytes = u32::try_from(frame.map_or(0, |frame| frame.frame_size_bytes))
                .map_err(|_| Error::Overflow)?;
            let alignment = frame.map_or(16, |frame| u32::from(frame.abi_stack_alignment_bytes));
            let linkage = match text.target.architecture {
                target::Architecture::X86_64 => 8,
                target::Architecture::Aarch64 => 0,
            };
            let unit = matches!(
                abstracted.result,
                abstract_operations::AbstractFunctionResult::Unit
            );
            let mut unit_call_stacks = Vec::new();
            let mut scalar_call_stacks = Vec::new();
            fragments.function(machine).ok_or(Error::Mismatch(
                "missing current fragment for temporary stack replay",
            ))?;
            let stack_index = source::temporary_stack_index(&temporary_stack_bytes, machine)?;
            let function_temporary_stack = temporary_stack_bytes[stack_index].byte_count;
            let mut local_peak = frame_bytes
                .checked_add(function_temporary_stack)
                .ok_or(Error::Overflow)?;
            for call in source::stack_calls(source, machine, callable_invocations)? {
                let caller_live_bytes = frame_bytes.checked_add(linkage).ok_or(Error::Overflow)?;
                local_peak = local_peak.max(caller_live_bytes);
                let owner = call.owner;
                let text_offset = host(call.section_offset)?;
                if unit {
                    unit_call_stacks.push(ObjectUnitCallStack {
                        owner,
                        target: call.callee,
                        text_offset,
                        active_frame_bytes: frame_bytes,
                        transient_bytes: linkage,
                        caller_live_bytes,
                    });
                } else {
                    scalar_call_stacks.push(ObjectScalarCallStack {
                        owner,
                        target: call.callee,
                        text_offset,
                        caller_live_bytes,
                    });
                }
            }
            functions.push(ObjectFunction {
                location,
                attachment,
                scalar_abi: abi.scalar.clone(),
                mixed_structural_scalar_abi: abi.mixed_structural_scalar.clone(),
                structural_call_scalar_return: None,
                parameter_abi: abi.parameter_abi().map(|(call_plan, parameters, _)| {
                    machine_code::ParameterFunctionAbiRecord {
                        call_plan: call_plan.clone(),
                        parameters: parameters.to_vec(),
                        entry_register_spills: Vec::new(),
                    }
                }),
                provenance: provenance.clone(),
                x86_floating_control: None,
                unit_stack: unit.then_some(ObjectUnitStack {
                    frame_bytes,
                    local_peak_bytes: local_peak,
                    stack_alignment: alignment,
                }),
                scalar_stack: (!unit).then_some(ObjectScalarStack {
                    local_peak_bytes: local_peak,
                    stack_alignment: alignment,
                }),
                unit_call_stacks,
                scalar_call_stacks,
                internal_unit_calls: Vec::new(),
                internal_unit_scalar_calls: Vec::new(),
                installed_provider_unit_scalar_calls: Vec::new(),
                unit_scalar_homes: Vec::new(),
                unit_integer_constants: Vec::new(),
                unit_affine_scalar_records: Vec::new(),
                unit_structural_scalar_field_stores: Vec::new(),
                unit_write_only_primitive_stores: Vec::new(),
                scalar_structural_scalar_field_stores: Vec::new(),
                unit_parameters: Vec::new(),
                unit_parameter_homes: Vec::new(),
                unit_continuations: Vec::new(),
                unit_affine_cleanup: None,
                scalar_affine_cleanup: None,
                scalar_control_affine_cleanups: Vec::new(),
                scalar_structural_parameters: Vec::new(),
                scalar_structural_parameter_homes: Vec::new(),
                structural_return: None,
            });
            let fragment = fragments
                .function(machine)
                .ok_or(Error::Mismatch("missing current fragment"))?;
            let mut rows = attribution::produce_current(
                fragment,
                source::publication_function(source, machine)?,
            )?;
            rows.extend(super::structural::settlement_attributions(source, machine)?);
            rows.sort_by_key(|row| (row.operation_ordinal, row.code_offset, row.byte_count));
            super::structural::populate(
                source,
                functions
                    .last_mut()
                    .ok_or(Error::Mismatch("missing inserted function"))?,
                &rows,
            )?;
            for attribution in rows {
                semantic_code_attribution.push(ObjectCodeAttribution {
                    machine,
                    text_offset: offset
                        .checked_add(attribution.code_offset)
                        .ok_or(Error::Overflow)?,
                    attribution,
                });
            }
        }
        let image_data = super::image_data::ObjectImageData::prepare(source)?;
        let bindings = source.publication().bindings();
        let imports = Arc::clone(&bindings.imports);
        let windows_internet = bindings.windows_internet.clone();
        let mut artifact = ObjectArtifact {
            emission_input: Arc::clone(&emission_input),
            image_data,
            foreign_call_plans: source
                .emission_facts()
                .physical
                .foreign_calls
                .clone()
                .map_err(|error| Error::ForeignCalls(error.into()))?,
            imports,
            windows_internet,
            projected: ProjectedObject(()),
            hosted_entry: None,
            physical_entry: None,
            retained_entry_storage,
            requires_graph_storage_replay,
            fragment_replay: None,
            composition_source: None,
            psi: text.psi,
            target: text.target,
            profile,
            entry: text.semantic_entry,
            object,
            relocations,
            text_bytes,
            data_bytes,
            functions,
            private_functions: Vec::new(),
            semantic_code_attribution,
            port_effects: super::port_effects::produce(source)?,
            boundary_settlements: super::structural::settlements(source)?,
            foreign_calls,
            byte_literal_pool,
            code_descriptor_pool,
            native_code_layout_pool,
        };
        if let Some(replay) = replay {
            artifact
                .attach_issued_replay(replay)
                .map_err(Error::PhysicalEntry)?;
        }
        Ok(artifact)
    }
}

pub(crate) mod replay {
    use std::sync::Arc;

    use object_file::StagedOptimizedRelocationFreeObjectContainer;
    pub use verified_optimization_input::publication::target_replay::RetainedTargetOperations;

    pub use machine_code::layout::PhysicalEncodingReadback;

    /// The preceding object stage supplies every input to this explicit audit.
    ///
    /// ```compile_fail,E0061
    /// fn inject_target(artifact: &mut image_emission::ObjectArtifact, object: std::sync::Arc<object_file::StagedOptimizedRelocationFreeObjectContainer>,
    ///                  target: image_emission::RetainedTargetOperations) {
    ///     let _ = artifact.attach_issued_replay(object, Some(target));
    /// }
    /// ```
    #[derive(Debug, Clone)]
    pub struct FragmentReplay {
        object: Arc<StagedOptimizedRelocationFreeObjectContainer>,
    }

    impl FragmentReplay {
        /// Attach the archive only to the completed object that issued its publication.
        pub fn attach_to(
            self,
            artifact: &mut crate::ObjectArtifact,
        ) -> Result<(), diagnostics::Diagnostic> {
            if !self
                .fragment_source()
                .publication()
                .same_issuance(artifact.publication())
                || self.target_source().is_some_and(|original| {
                    !original.belongs_to(&artifact.publication().emission_facts().semantic)
                })
            {
                return Err(diagnostics::Diagnostic::error(
                    "retained object belongs to another publication",
                ));
            }
            if artifact.fragment_replay.is_some() {
                return Err(diagnostics::Diagnostic::error(
                    "object already has retained replay inputs",
                ));
            }
            artifact.fragment_replay = Some(self);
            Ok(())
        }

        pub fn fragment_source(&self) -> &StagedOptimizedRelocationFreeObjectContainer {
            &self.object
        }

        pub fn original_encoding(&self) -> Option<&PhysicalEncodingReadback> {
            self.object.original_encoding()
        }

        pub fn target_source(&self) -> Option<&RetainedTargetOperations> {
            self.object.original_target()
        }
    }

    impl crate::ObjectArtifact {
        pub fn attach_issued_replay(
            &mut self,
            object: std::sync::Arc<StagedOptimizedRelocationFreeObjectContainer>,
        ) -> Result<(), diagnostics::Diagnostic> {
            FragmentReplay { object }.attach_to(self)
        }

        /// The exact inputs intentionally retained for this body, if requested.
        pub fn fragment_replay(&self) -> Option<&FragmentReplay> {
            self.fragment_replay.as_ref()
        }
    }

    impl PartialEq for FragmentReplay {
        fn eq(&self, other: &Self) -> bool {
            self.object.object() == other.object.object()
                && self.object.container() == other.object.container()
                && self.object.manifest().record() == other.object.manifest().record()
        }
    }

    impl Eq for FragmentReplay {}

    #[cfg(any(test, feature = "test-support"))]
    impl crate::ObjectArtifact {}

    pub(crate) fn validate(
        artifact: &crate::ObjectArtifact,
    ) -> Result<(), diagnostics::Diagnostic> {
        super::validation::validate_available(artifact)
            .map_err(|error| diagnostics::Diagnostic::error(error.to_string()))
    }
}

mod source {
    pub(super) mod retained_source_validation;
    pub(super) fn profile(
        source: &crate::object_input::ObjectEmissionInput,
    ) -> Option<target::TargetProfile> {
        source.emission_facts().physical.contracts.profile()
    }

    pub(crate) fn selected_constant_operand(
        replay: &crate::FragmentReplay,
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        operand: u16,
    ) -> Result<Option<u64>, super::Error> {
        let function = replay
            .original_encoding()
            .ok_or(Error::Mismatch(
                "original physical instructions were not retained (ReplayUnavailable)",
            ))?
            .physical
            .function(machine)
            .ok_or(super::Error::Mismatch(
                "constant operand has no retained function",
            ))?;
        installation_admission::selected::selected_constant_operand(function, instruction, operand)
            .map_err(Into::into)
    }

    use super::Error;
    use crate::object_input::ObjectEmissionInput;
    use abstract_operations::AbstractFunction;
    use machine_code::function_metadata::FunctionTemporaryStack;
    use machine_code::{FunctionFragmentEmissionPlan, FunctionTargetFrameLayout};
    use semantic_vocabulary::MachineId;
    pub(super) mod placed_reference {
        pub(super) mod backing_geometry {
            mod incoming_calls {
                mod source_sites {
                    use abstract_operations::{AbstractOperation, AbstractOperationPlan};
                    use semantic_vocabulary::{
                        BoundaryMachineId, ExternalCompletionKind, InvocationSite, MachineId,
                    };
                    use terminal_psi::StructuralArgument;
                    use terminal_semantics::descriptor_lifetimes::DescriptorPosition;

                    #[derive(Clone, Copy)]
                    pub(super) enum SourceCallTarget {
                        Machine(MachineId),
                        Boundary {
                            boundary: BoundaryMachineId,
                            kind: ExternalCompletionKind,
                        },
                    }
                    pub(super) struct SourceSite<'a> {
                        pub position: Result<DescriptorPosition, &'static str>,
                        pub target: SourceCallTarget,
                        pub arguments: &'a [StructuralArgument],
                        pub cleanup: Option<&'a terminal_psi::OwnedCleanupInvocation>,
                    }
                    struct Row<'a> {
                        caller: MachineId,
                        site: InvocationSite,
                        source: SourceSite<'a>,
                    }
                    pub(super) struct SourceSites<'a> {
                        rows: Vec<Row<'a>>,
                    }
                    impl<'a> SourceSites<'a> {
                        pub(super) fn new(source: &'a AbstractOperationPlan) -> Self {
                            let mut rows = Vec::new();
                            for body in &source.functions {
                                for block in &body.blocks {
                                    for (offset, operation) in block.operations.iter().enumerate() {
                                        for (edge, actions) in operation.cleanup_rosters() {
                                            for (ordinal, action) in actions.iter().enumerate() {
                                                let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) = action else { continue; };
                                                let Ok(action_ordinal) = u32::try_from(ordinal)
                                                else {
                                                    continue;
                                                };
                                                rows.push(Row {
                                                    caller: body.machine,
                                                    site: InvocationSite::Cleanup { edge, action_ordinal },
                                                    source: SourceSite {
                                                        position: u32::try_from(offset).map(|offset| (block.block, offset)).map_err(|_| "cleanup caller position exceeds descriptor range"),
                                                        target: SourceCallTarget::Machine(cleanup.cleanup_machine),
                                                        arguments: std::slice::from_ref(&cleanup.subject),
                                                        cleanup: Some(cleanup),
                                                    },
                                                });
                                            }
                                        }
                                        let (site, target, arguments) = match operation {
                                            AbstractOperation::InvokeNoNormal {
                                                psi_edge,
                                                callee,
                                                structural_arguments,
                                                ..
                                            } => (
                                                InvocationSite::InvokeNoNormal(*psi_edge),
                                                SourceCallTarget::Machine(*callee),
                                                structural_arguments.as_slice(),
                                            ),
                                            AbstractOperation::ExternalCompletion {
                                                psi_edge,
                                                boundary,
                                                kind,
                                                structural_arguments,
                                                ..
                                            } => (
                                                InvocationSite::ExternalCompletion(*psi_edge),
                                                SourceCallTarget::Boundary {
                                                    boundary: *boundary,
                                                    kind: *kind,
                                                },
                                                structural_arguments.as_slice(),
                                            ),
                                            _ => continue,
                                        };
                                        let position = u32::try_from(offset)
                                            .map(|offset| (block.block, offset))
                                            .map_err(|_| "terminal caller position exceeds descriptor range");
                                        rows.push(Row {
                                            caller: body.machine,
                                            site,
                                            source: SourceSite {
                                                position,
                                                target,
                                                arguments,
                                                cleanup: None,
                                            },
                                        });
                                    }
                                }
                            }
                            rows.sort_by_key(|row| (row.caller, row.site));
                            Self { rows }
                        }
                        pub(super) fn get(
                            &self,
                            caller: MachineId,
                            site: InvocationSite,
                        ) -> Result<&SourceSite<'a>, &'static str> {
                            let key = (caller, site);
                            let start = self
                                .rows
                                .partition_point(|row| (row.caller, row.site) < key);
                            let end = self
                                .rows
                                .partition_point(|row| (row.caller, row.site) <= key);
                            if end != start + 1 {
                                return Err("terminal call has no unique abstract snapshot");
                            }
                            Ok(&self.rows[start].source)
                        }
                    }
                    #[cfg(test)]
                    mod tests {
                        use super::super::super::source_operation_position;
                        use super::{SourceCallTarget, SourceSites};
                        use abstract_operations::{
                            AbstractBlock, AbstractFunction, AbstractFunctionResult,
                            AbstractOperation, AbstractOperationPlan,
                        };
                        use semantic_vocabulary::{
                            BlockId, BoundaryMachineId, EdgeId, ExternalCompletionKind,
                            InvocationSite, MachineId, OperationId, PlaceId, ValueId,
                        };
                        use terminal_psi::{
                            SemanticFingerprint, StructuralAccess, StructuralArgument,
                            TerminalPsiIdentity, VocabularyMarker,
                        };

                        fn constant(id: u64) -> AbstractOperation {
                            AbstractOperation::BooleanConstant {
                                psi_operation: OperationId::new(id).unwrap(),
                                result: ValueId::new(id).unwrap(),
                                value: true,
                            }
                        }

                        fn source() -> AbstractOperationPlan {
                            let machine = MachineId::new(1).unwrap();
                            let argument = StructuralArgument {
                                place: PlaceId::new(5).unwrap(),
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            };
                            let block = |id, operations| AbstractBlock {
                                block: BlockId::new(id).unwrap(),
                                parameters: Vec::new(),
                                structural_parameters: Vec::new(),
                                operations,
                            };
                            AbstractOperationPlan {
                                psi: TerminalPsiIdentity {
                                    vocabulary_marker: VocabularyMarker::CURRENT,
                                    program_fingerprint: SemanticFingerprint::from_bytes([1; 32]),
                                },
                                entry: machine,
                                identities: Default::default(),
                                structural_types: Vec::new().into(),
                                boundary_machines: Vec::new(),
                                provider_candidates: Vec::new(),
                                callables: Default::default(),
                                dynamic_dispatch: Default::default(),
                                functions: vec![AbstractFunction {
                                    machine,
                                    attachment: None,
                                    entry: BlockId::new(10).unwrap(),
                                    parameters: Vec::new(),
                                    structural_parameters: Vec::new(),
                                    result: AbstractFunctionResult::Unit,
                                    entry_claims: Vec::new(),
                                    block_entry_claims: Vec::new(),
                                    published_service_ceiling: Vec::new(),
                                    blocks: vec![
                                        block(
                                            10,
                                            vec![
                                                constant(1),
                                                AbstractOperation::InvokeNoNormal {
                                                    psi_edge: EdgeId::new(100).unwrap(),
                                                    callee: MachineId::new(2).unwrap(),
                                                    arguments: Vec::new(),
                                                    structural_arguments: vec![argument.clone()],
                                                    claim_transfers: Vec::new(),
                                                    requirement_obligations: Vec::new(),
                                                    crash_continuations: Vec::new(),
                                                },
                                            ],
                                        ),
                                        block(
                                            20,
                                            vec![
                                                constant(2),
                                                constant(3),
                                                AbstractOperation::ExternalCompletion {
                                                    psi_edge: EdgeId::new(101).unwrap(),
                                                    boundary: BoundaryMachineId::new(3).unwrap(),
                                                    kind: ExternalCompletionKind::ProcessExit,
                                                    arguments: Vec::new(),
                                                    structural_arguments: vec![argument],
                                                    requirement_obligations: Vec::new(),
                                                    frontier_lower_bound: Vec::new(),
                                                },
                                            ],
                                        ),
                                    ],
                                }],
                            }
                        }

                        #[test]
                        fn source_sites_keep_block_local_positions_and_unique_invocations() {
                            let mut source = source();
                            let caller = source.entry;
                            let invoke = InvocationSite::InvokeNoNormal(EdgeId::new(100).unwrap());
                            let completion =
                                InvocationSite::ExternalCompletion(EdgeId::new(101).unwrap());
                            let sites = SourceSites::new(&source);
                            let call = sites.get(caller, invoke).unwrap();
                            assert_eq!(call.position, Ok((BlockId::new(10).unwrap(), 1)));
                            assert!(
                                matches!(call.target, SourceCallTarget::Machine(machine) if machine == MachineId::new(2).unwrap())
                            );
                            let exit = sites.get(caller, completion).unwrap();
                            assert_eq!(exit.position, Ok((BlockId::new(20).unwrap(), 2)));
                            assert!(
                                matches!(exit.target, SourceCallTarget::Boundary { boundary, kind: ExternalCompletionKind::ProcessExit } if boundary == BoundaryMachineId::new(3).unwrap())
                            );
                            assert_eq!(call.arguments, exit.arguments);
                            assert_eq!(call.arguments.len(), 1);
                            assert!(sites.get(MachineId::new(9).unwrap(), invoke).is_err());
                            assert!(
                                sites
                                    .get(
                                        caller,
                                        InvocationSite::ExternalCompletion(
                                            EdgeId::new(100).unwrap()
                                        )
                                    )
                                    .is_err()
                            );

                            let repeated = source.functions[0].blocks[0].operations[1].clone();
                            source.functions[0].blocks[1].operations.push(repeated);
                            let sites = SourceSites::new(&source);
                            assert!(sites.get(caller, invoke).is_err());
                            assert_eq!(
                                sites.get(caller, completion).unwrap().position,
                                Ok((BlockId::new(20).unwrap(), 2))
                            );
                            source.functions[0].blocks[1].operations.pop();
                            source.functions[0].blocks[0].operations.pop();
                            assert!(SourceSites::new(&source).get(caller, invoke).is_err());
                        }

                        #[test]
                        fn cleanup_sites_keep_action_subjects_and_requirements() {
                            use calling_conventions::{ValuePlacement, ValueShape};
                            use semantic_vocabulary::{ObligationId, StructuralTypeId};
                            use target_operations::{
                                TargetStructuralArgument, TargetStructuralArgumentSource,
                            };
                            use terminal_psi::{
                                OwnedCleanupInvocation, TerminalAffineCleanupAction,
                            };
                            let mut source = source();
                            let caller = source.entry;
                            let cleanup = |place, requirement| OwnedCleanupInvocation {
                                subject: StructuralArgument {
                                    place: PlaceId::new(place).unwrap(),
                                    access: StructuralAccess::Owned,
                                    path: Vec::new(),
                                },
                                structural_type: StructuralTypeId::new(1).unwrap(),
                                cleanup_machine: MachineId::new(2).unwrap(),
                                claim_transfers: Vec::new(),
                                requirement_obligations: vec![
                                    ObligationId::new(requirement).unwrap(),
                                ],
                            };
                            let first = cleanup(5, 7);
                            let second = cleanup(6, 8);
                            for (block, edge, actions) in [
                                (
                                    30,
                                    102,
                                    vec![
                                        TerminalAffineCleanupAction::RetireHome(
                                            first.subject.place,
                                        ),
                                        TerminalAffineCleanupAction::InvokeOwned(first.clone()),
                                        TerminalAffineCleanupAction::InvokeOwned(second.clone()),
                                    ],
                                ),
                                (
                                    40,
                                    103,
                                    vec![TerminalAffineCleanupAction::InvokeOwned(first.clone())],
                                ),
                            ] {
                                source.functions[0].blocks.push(AbstractBlock {
                                    block: BlockId::new(block).unwrap(),
                                    parameters: Vec::new(),
                                    structural_parameters: Vec::new(),
                                    operations: vec![AbstractOperation::ReturnUnit {
                                        psi_edge: EdgeId::new(edge).unwrap(),
                                        cleanup_actions: actions,
                                    }],
                                });
                            }
                            {
                                let sites = SourceSites::new(&source);
                                for (edge, action_ordinal, expected) in
                                    [(102, 1, &first), (102, 2, &second), (103, 0, &first)]
                                {
                                    let site = sites
                                        .get(
                                            caller,
                                            InvocationSite::Cleanup {
                                                edge: EdgeId::new(edge).unwrap(),
                                                action_ordinal,
                                            },
                                        )
                                        .unwrap();
                                    assert_eq!(site.cleanup, Some(expected));
                                    assert_eq!(
                                        site.arguments,
                                        std::slice::from_ref(&expected.subject)
                                    );
                                    let placement = ValuePlacement {
                                        shape: ValueShape::integer(8, 8),
                                        locations: Vec::new(),
                                    };
                                    let target = TargetStructuralArgument {
                                        place: expected.subject.place,
                                        access: StructuralAccess::Owned,
                                        path: Vec::new(),
                                        root_structural_type: expected.structural_type,
                                        structural_type: expected.structural_type,
                                        shape: placement.shape,
                                        source_byte_offset: 0,
                                        fixed_array_length: None,
                                        element_stride: None,
                                        indices: Vec::new(),
                                        source: TargetStructuralArgumentSource::Placement(
                                            placement.clone(),
                                        ),
                                        destination: placement,
                                    };
                                    assert!(super::super::cleanup_source_matches(
                                        site,
                                        expected.cleanup_machine,
                                        std::slice::from_ref(&target),
                                        &expected.requirement_obligations
                                    ));
                                    assert!(!super::super::cleanup_source_matches(
                                        site,
                                        expected.cleanup_machine,
                                        std::slice::from_ref(&target),
                                        &[ObligationId::new(99).unwrap()]
                                    ));
                                    let mut changed = target.clone();
                                    changed.place = PlaceId::new(99).unwrap();
                                    assert!(!super::super::cleanup_source_matches(
                                        site,
                                        expected.cleanup_machine,
                                        &[changed],
                                        &expected.requirement_obligations
                                    ));
                                    assert!(!super::super::cleanup_source_matches(
                                        site,
                                        MachineId::new(9).unwrap(),
                                        &[target],
                                        &expected.requirement_obligations
                                    ));
                                }
                                assert!(
                                    sites
                                        .get(
                                            caller,
                                            InvocationSite::Cleanup {
                                                edge: EdgeId::new(102).unwrap(),
                                                action_ordinal: 0
                                            }
                                        )
                                        .is_err()
                                );
                                assert!(
                                    sites
                                        .get(
                                            caller,
                                            InvocationSite::InvokeNoNormal(
                                                EdgeId::new(102).unwrap()
                                            )
                                        )
                                        .is_err()
                                );
                            }
                            let repeated = source.functions[0].blocks[2].operations[0].clone();
                            source.functions[0].blocks[3].operations.push(repeated);
                            assert!(
                                SourceSites::new(&source)
                                    .get(
                                        caller,
                                        InvocationSite::Cleanup {
                                            edge: EdgeId::new(102).unwrap(),
                                            action_ordinal: 1
                                        }
                                    )
                                    .is_err()
                            );
                        }

                        #[test]
                        fn source_operation_positions_reject_missing_and_duplicate_identities() {
                            let mut source = source();
                            let function = &mut source.functions[0];
                            let id = OperationId::new(3).unwrap();
                            assert_eq!(
                                source_operation_position(function, id),
                                Ok((BlockId::new(20).unwrap(), 1))
                            );
                            assert_eq!(
                                source_operation_position(function, OperationId::new(1).unwrap()),
                                Ok((BlockId::new(10).unwrap(), 0))
                            );
                            assert!(
                                source_operation_position(function, OperationId::new(9).unwrap())
                                    .is_err()
                            );
                            function.blocks[0].operations.push(constant(3));
                            assert!(source_operation_position(function, id).is_err());
                            function.blocks[0].operations.pop();
                            function.blocks[1].operations[1] = constant(4);
                            assert!(source_operation_position(function, id).is_err());
                            assert_eq!(
                                source_operation_position(function, OperationId::new(4).unwrap()),
                                Ok((BlockId::new(20).unwrap(), 1))
                            );
                        }
                    }
                }

                use abstract_operations::AbstractOperationPlan;
                use semantic_vocabulary::{InvocationSite, MachineId};
                use source_sites::{SourceCallTarget, SourceSite, SourceSites};
                use target_operations::{
                    CallSiteOwner, TargetControlTerminator, TargetOperationPlan,
                    TargetUnitOperation,
                };
                use terminal_semantics::descriptor_lifetimes::{
                    DescriptorContentsQuery, DescriptorLocation, DescriptorPosition,
                    descriptor_location,
                };
                use terminal_semantics::descriptor_results::{
                    DescriptorActualInputs, DescriptorAnalysis, DescriptorInvocationOwner,
                    DescriptorMachine, DescriptorSourceOrigin, descriptor_contents_query,
                    descriptor_machine,
                };

                pub(super) struct IncomingCalls {
                    rows: Vec<IncomingCall>,
                }
                struct IncomingCall {
                    caller: MachineId,
                    callee: MachineId,
                    site: CallSiteOwner,
                    inputs: Result<Inputs, &'static str>,
                }
                enum Inputs {
                    Invocation {
                        position: DescriptorPosition,
                        owner: DescriptorInvocationOwner,
                    },
                    Entry {
                        position: DescriptorPosition,
                        arguments: Vec<Option<DescriptorLocation>>,
                        contents: Vec<Option<DescriptorContentsQuery>>,
                    },
                }
                impl IncomingCalls {
                    pub(super) fn new(
                        source: &AbstractOperationPlan,
                        targeted: &TargetOperationPlan,
                        graph: &[DescriptorMachine],
                    ) -> Self {
                        let mut rows = Vec::new();
                        let source_sites = SourceSites::new(source);
                        for caller in &targeted.functions {
                            for operation in caller.graph.operations() {
                                match operation {
                                    TargetUnitOperation::Call {
                                        target,
                                        psi_operation,
                                        ..
                                    } => {
                                        let Some(callee) = target.machine() else {
                                            continue;
                                        };
                                        let inputs = invocation(
                                            graph,
                                            caller.machine,
                                            DescriptorInvocationOwner::Operation(*psi_operation),
                                            callee,
                                        );
                                        rows.push(IncomingCall {
                                            caller: caller.machine,
                                            callee,
                                            site: CallSiteOwner::Operation(*psi_operation),
                                            inputs,
                                        });
                                    }
                                    TargetUnitOperation::CleanupCall {
                                        owner,
                                        callee,
                                        arguments,
                                        requirement_obligations,
                                        ..
                                    } => {
                                        let inputs = (|| {
                                            let CallSiteOwner::CleanupAction {
                                                edge,
                                                action_ordinal,
                                            } = *owner
                                            else {
                                                return Err(
                                                    "cleanup target has no edge/action owner",
                                                );
                                            };
                                            let site = source_sites.get(
                                                caller.machine,
                                                InvocationSite::Cleanup {
                                                    edge,
                                                    action_ordinal,
                                                },
                                            )?;
                                            if !cleanup_source_matches(
                                                site,
                                                *callee,
                                                arguments,
                                                requirement_obligations,
                                            ) {
                                                return Err(
                                                    "cleanup target changes its original subject, callee, type or requirement evidence",
                                                );
                                            }
                                            let inputs = invocation(
                                                graph,
                                                caller.machine,
                                                DescriptorInvocationOwner::Cleanup {
                                                    edge,
                                                    action_ordinal,
                                                },
                                                *callee,
                                            )?;
                                            let Inputs::Invocation { position, .. } = &inputs
                                            else {
                                                return Err(
                                                    "cleanup lost its exact invocation observation",
                                                );
                                            };
                                            if *position != site.position? {
                                                return Err(
                                                    "cleanup source and descriptor owner positions disagree",
                                                );
                                            }
                                            Ok(inputs)
                                        })();
                                        rows.push(IncomingCall {
                                            caller: caller.machine,
                                            callee: *callee,
                                            site: *owner,
                                            inputs,
                                        });
                                    }
                                    TargetUnitOperation::InstallStaticInterrupt {
                                        psi_operation,
                                        calls,
                                        ..
                                    } => {
                                        for call in calls {
                                            rows.push(IncomingCall {
                                                caller: caller.machine, callee: call.callee,
                                                site: CallSiteOwner::Installation { operation: *psi_operation, role: call.role },
                                                inputs: Err("installation helper arrival has no reconstructed backing observation"),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            for block in &caller.graph.blocks {
                                match &block.terminator {
                                    TargetControlTerminator::InvokeNoNormal {
                                        psi_edge,
                                        callee,
                                        arguments,
                                        ..
                                    } => {
                                        let inputs = source_sites.get(caller.machine, InvocationSite::InvokeNoNormal(*psi_edge)).and_then(|site| {
                                            if !matches!(site.target, SourceCallTarget::Machine(source_callee) if source_callee == *callee) {
                                                return Err("no-normal source and target callee disagree");
                                            }
                                            if site.arguments.len() != arguments.len() || site.arguments.iter().zip(arguments).any(|(source, target)| source.place != target.place || source.path != target.path || source.access != target.access) {
                                                return Err("no-normal source and target argument transport disagree");
                                            }
                                            entry(source, graph, caller.machine, *callee, site)
                                        });
                                        rows.push(IncomingCall {
                                            caller: caller.machine,
                                            callee: *callee,
                                            site: CallSiteOwner::InvokeNoNormal(*psi_edge),
                                            inputs,
                                        });
                                    }
                                    TargetControlTerminator::ExternalCompletion {
                                        psi_edge,
                                        boundary,
                                        kind,
                                        binding:
                                            target_operations::ExternalCompletionBinding::Checked {
                                                callee,
                                                origin,
                                                ..
                                            },
                                        structural_arguments,
                                        ..
                                    } => {
                                        let inputs = source_sites.get(caller.machine, InvocationSite::ExternalCompletion(*psi_edge)).and_then(|site| {
                                            if !matches!(site.target, SourceCallTarget::Boundary { boundary: source_boundary, kind: source_kind } if source_boundary == *boundary && source_kind == *kind)
                                                || site.arguments != structural_arguments
                                                || !selected_origin(origin, *boundary, *callee)
                                            {
                                                return Err("external completion differs from its selected source binding");
                                            }
                                            entry(source, graph, caller.machine, *callee, site)
                                        });
                                        rows.push(IncomingCall {
                                            caller: caller.machine,
                                            callee: *callee,
                                            site: CallSiteOwner::ExternalCompletion(*psi_edge),
                                            inputs,
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                        rows.sort_by_key(|row| (row.callee, row.caller, row.site));
                        Self { rows }
                    }
                    pub(super) fn range(&self, callee: MachineId) -> std::ops::Range<usize> {
                        self.rows.partition_point(|row| row.callee < callee)
                            ..self.rows.partition_point(|row| row.callee <= callee)
                    }
                    pub(super) fn substitute(
                        &self,
                        index: usize,
                        analysis: &mut DescriptorAnalysis<'_>,
                        origin: &DescriptorSourceOrigin,
                    ) -> Result<(MachineId, Vec<DescriptorSourceOrigin>), &'static str>
                    {
                        let row = self
                            .rows
                            .get(index)
                            .ok_or("incoming backing site is absent")?;
                        let result = match row.inputs.as_ref().map_err(|reason| *reason)? {
                            Inputs::Invocation { position, owner } => analysis
                                .substitute_call_source_at(row.caller, *position, *owner, origin),
                            Inputs::Entry {
                                position,
                                arguments,
                                contents,
                            } => analysis.substitute_input_source(
                                row.caller,
                                *position,
                                row.callee,
                                DescriptorActualInputs {
                                    scalar_arguments: &[],
                                    arguments,
                                    argument_contents: contents,
                                },
                                origin,
                            ),
                        }
                        .ok_or("call argument has no complete backing substitution")?;
                        if result.is_empty() {
                            return Err("call argument has no backing alternatives");
                        }
                        Ok((row.caller, result))
                    }
                }
                fn cleanup_source_matches(
                    site: &SourceSite<'_>,
                    callee: MachineId,
                    arguments: &[target_operations::TargetStructuralArgument],
                    requirements: &[semantic_vocabulary::ObligationId],
                ) -> bool {
                    let Some(cleanup) = site.cleanup else {
                        return false;
                    };
                    matches!(site.target, SourceCallTarget::Machine(machine) if machine == callee)
                        && cleanup.cleanup_machine == callee
                        && cleanup.subject.access == terminal_psi::StructuralAccess::Owned
                        && cleanup.claim_transfers.is_empty()
                        && cleanup.requirement_obligations == requirements
                        && arguments.len() == 1
                        && arguments.iter().all(|actual| {
                            actual.place == cleanup.subject.place
                                && actual.path == cleanup.subject.path
                                && actual.access == cleanup.subject.access
                                && actual.structural_type == cleanup.structural_type
                        })
                }

                fn invocation(
                    graph: &[DescriptorMachine],
                    caller: MachineId,
                    owner: DescriptorInvocationOwner,
                    callee: MachineId,
                ) -> Result<Inputs, &'static str> {
                    let caller = descriptor_machine(graph, caller)
                        .ok_or("selected caller has no reference-origin graph")?;
                    let mut calls = caller
                        .known_invocations()
                        .filter(|(_, call)| call.owner == owner);
                    let (Some((position, call)), None) = (calls.next(), calls.next()) else {
                        return Err(
                            "selected call has no unique reference-origin owner observation",
                        );
                    };
                    if call.callee() != Some(callee) {
                        return Err("selected target differs from the reference-origin call");
                    }
                    if matches!(owner, DescriptorInvocationOwner::Cleanup { .. })
                        && (call.arguments.iter().any(Option::is_none)
                            || call.argument_contents.iter().any(Option::is_none))
                    {
                        return Err(
                            "selected cleanup has incomplete owned argument or contents correspondence",
                        );
                    }
                    Ok(Inputs::Invocation {
                        position: *position,
                        owner,
                    })
                }
                fn selected_origin(
                    origin: &target_operations::NativeCallOrigin,
                    boundary: semantic_vocabulary::BoundaryMachineId,
                    callee: MachineId,
                ) -> bool {
                    match origin {
                        target_operations::NativeCallOrigin::Authored => false,
                        target_operations::NativeCallOrigin::ProgramEntry {
                            boundary: actual,
                            entry,
                            ..
                        } => *actual == boundary && *entry == callee,
                        target_operations::NativeCallOrigin::InstalledProvider {
                            boundary: actual,
                            provider,
                            ..
                        } => {
                            *actual == boundary
                                && provider.boundary == boundary
                                && provider.candidate == callee
                        }
                    }
                }
                fn entry(
                    source: &AbstractOperationPlan,
                    graph: &[DescriptorMachine],
                    caller: MachineId,
                    callee: MachineId,
                    site: &SourceSite<'_>,
                ) -> Result<Inputs, &'static str> {
                    let declaration = source
                        .functions
                        .iter()
                        .find(|body| body.machine == callee)
                        .ok_or("terminal callee has no abstract declaration")?;
                    if site.arguments.len() != declaration.structural_parameters.len() {
                        return Err("terminal source and callee argument arity disagree");
                    }
                    let position = site.position?;
                    let caller = descriptor_machine(graph, caller)
                        .ok_or("terminal caller has no descriptor graph")?;
                    if !caller.flow.contains_key(&position) {
                        return Err("terminal call lost its current descriptor snapshot");
                    }
                    let mut locations = Vec::with_capacity(site.arguments.len());
                    let mut contents = Vec::with_capacity(site.arguments.len());
                    for actual in site.arguments {
                        let root = caller
                            .place_types
                            .get(&actual.place)
                            .copied()
                            .ok_or("terminal actual has no declared structural type")?;
                        locations.push(descriptor_location(
                            source.structural_types.as_slice(),
                            actual.place,
                            root,
                            &actual.path,
                            None,
                        ));
                        contents.push(descriptor_contents_query(
                            source.structural_types.as_slice(),
                            actual.place,
                            root,
                            &actual.path,
                            None,
                        ));
                    }
                    Ok(Inputs::Entry {
                        position,
                        arguments: locations,
                        contents,
                    })
                }
            }

            use abstract_operations::AbstractOperationPlan;
            use semantic_vocabulary::{MachineId, PlaceId};
            use terminal_psi::{AccessRamCorrespondence, StructuralAccess, StructuralArgument};
            use terminal_semantics::descriptor_lifetimes::{
                descriptor_location, fixed_runtime_index_alternatives,
            };
            use terminal_semantics::descriptor_results::{
                DescriptorAnalysis, DescriptorCallOrigin, DescriptorDefinition, DescriptorMachine,
                DescriptorSourceOrigin, descriptor_machine,
            };

            pub(in crate::function_fragments::source) struct BackingGeometry<'a> {
                abstracted: &'a AbstractOperationPlan,
                selected: &'a post_allocation_machine::PostAllocationMachineProgram,
                graph: &'a [DescriptorMachine],
                analysis: DescriptorAnalysis<'a>,
                incoming: incoming_calls::IncomingCalls,
            }

            // Source operations keep their block-local identity through native realization.
            // Duplicate identities cannot supply an arbitrary activation or formation position.
            fn source_operation_position(
                source: &abstract_operations::AbstractFunction,
                operation: semantic_vocabulary::OperationId,
            ) -> Result<terminal_semantics::descriptor_lifetimes::DescriptorPosition, &'static str>
            {
                let mut position = None;
                for block in &source.blocks {
                    for (ordinal, row) in block.operations.iter().enumerate() {
                        if row.psi_operation() != Some(operation) {
                            continue;
                        }
                        let ordinal = u32::try_from(ordinal)
                            .map_err(|_| "source operation position exceeds descriptor range")?;
                        if position.replace((block.block, ordinal)).is_some() {
                            return Err("source operation has no unique pre-operation position");
                        }
                    }
                }
                position.ok_or("source operation has no pre-operation position")
            }

            pub(in crate::function_fragments::source) fn record_selected_formation_calls(
                abstracted: &AbstractOperationPlan,
                targeted: &target_operations::TargetOperationPlan,
                graph: &mut [DescriptorMachine],
            ) -> Option<()> {
                use terminal_semantics::descriptor_results::{
                    DescriptorInvocationOwner, DescriptorMachineSignature, DescriptorSlotKind,
                    machine_invocation_for_signature,
                };
                for function in &targeted.functions {
                    for operation in function.graph.operations() {
                        let target_operations::TargetUnitOperation::Call {
                            psi_operation,
                            target: target_operations::NativeCallTarget::Machine(callee),
                            scalar_arguments: targeted_scalars,
                            ..
                        } = operation
                        else {
                            continue;
                        };
                        let caller = graph.iter_mut().find(|row| row.id == function.machine)?;
                        let owner = DescriptorInvocationOwner::Operation(*psi_operation);
                        if caller
                            .known_invocations()
                            .any(|(_, call)| call.owner == owner)
                        {
                            continue;
                        }
                        let source = abstracted
                            .functions
                            .iter()
                            .find(|row| row.machine == function.machine)?;
                        let position = source_operation_position(source, *psi_operation).ok()?;
                        let mut originals =
                            source.operations().filter_map(|operation| match operation {
                                abstract_operations::AbstractOperation::BoundaryCall {
                                    psi_operation: identity,
                                    arguments,
                                    structural_arguments,
                                    ..
                                } if identity == psi_operation => {
                                    Some((arguments, structural_arguments))
                                }
                                _ => None,
                            });
                        let Some((scalars, actuals)) = originals.next() else {
                            continue;
                        };
                        if originals.next().is_some() {
                            return None;
                        }
                        let mut candidates = abstracted
                            .functions
                            .iter()
                            .filter(|row| row.machine == *callee);
                        let declaration = candidates.next()?;
                        if candidates.next().is_some()
                            || scalars.len() != declaration.parameters.len()
                            || scalars.len() != targeted_scalars.len()
                        {
                            return None;
                        }
                        let mut call = machine_invocation_for_signature(
                            owner,
                            abstracted.structural_types.as_slice(),
                            DescriptorSlotKind::References,
                            DescriptorMachineSignature {
                                machine: declaration.machine,
                                structural_parameters: &declaration.structural_parameters,
                                result: declaration.result.structural(),
                            },
                            actuals,
                            None,
                            actuals
                                .iter()
                                .map(|actual| caller.place_types.get(&actual.place).copied()),
                        )?;
                        for ((formal, actual), targeted) in declaration
                            .parameters
                            .iter()
                            .zip(scalars)
                            .zip(targeted_scalars)
                        {
                            if targeted.source_value() != *actual
                                || targeted.scalar_type() != formal.scalar_type
                            {
                                return None;
                            }
                            call.scalar_arguments.push((
                                formal.value,
                                semantic_vocabulary::ScalarTerm::value(*actual, formal.scalar_type),
                            ));
                        }
                        call.scalar_arguments
                            .sort_unstable_by_key(|(formal, _)| *formal);
                        if call
                            .scalar_arguments
                            .windows(2)
                            .any(|rows| rows[0].0 == rows[1].0)
                            || call.arguments.iter().any(Option::is_none)
                            || call.argument_contents.iter().any(Option::is_none)
                        {
                            return None;
                        }
                        if let Some(row) = caller.calls.get_mut(&position) {
                            if row.is_some() {
                                return None;
                            }
                            // Native selection has resolved this exact boundary invocation
                            // to the checked machine above; source-only graphs retain None.
                            *row = Some(vec![call]);
                        } else {
                            caller.insert_call(position, call)?;
                        }
                    }
                }
                Some(())
            }

            pub(in crate::function_fragments::source) fn graph(
                source: &AbstractOperationPlan,
            ) -> Option<Vec<DescriptorMachine>> {
                optimization_unit_semantics::descriptor_results::abstract_reference_catalog_graph(
                    &source.functions,
                    &source.boundary_machines,
                    source.structural_types.as_slice(),
                    &source.dynamic_dispatch,
                    &source.callables,
                )
            }

            pub(in crate::function_fragments::source) fn completed_graph(
                source: &AbstractOperationPlan,
                targeted: &target_operations::TargetOperationPlan,
            ) -> Option<Vec<DescriptorMachine>> {
                let mut graph = graph(source)?;
                record_selected_formation_calls(source, targeted, &mut graph)?;
                Some(graph)
            }

            impl<'a> BackingGeometry<'a> {
                pub(in crate::function_fragments::source) fn new(
                    abstracted: &'a AbstractOperationPlan,
                    selected: &'a post_allocation_machine::PostAllocationMachineProgram,
                    targeted: &'a target_operations::TargetOperationPlan,
                    graph: &'a [DescriptorMachine],
                ) -> Self {
                    Self {
                        abstracted,
                        selected,
                        graph,
                        incoming: incoming_calls::IncomingCalls::new(abstracted, targeted, graph),
                        analysis: DescriptorAnalysis::with_reference_slots(
                            abstracted.structural_types.as_slice(),
                            graph,
                        ),
                    }
                }

                pub(in crate::function_fragments::source) fn proves(
                    &mut self,
                    machine: MachineId,
                    result: PlaceId,
                    source: &StructuralArgument,
                    required: &abstract_operations::AccessReferenceBacking,
                ) -> Result<(), &'static str> {
                    let required = match required {
                        abstract_operations::AccessReferenceBacking::OrdinaryRam(required) => {
                            required
                        }
                        abstract_operations::AccessReferenceBacking::BorrowedExtent(_) => {
                            return Err("borrowed extent requires a successful View lifecycle");
                        }
                        abstract_operations::AccessReferenceBacking::Resident(resident) => {
                            let mut plans = self
                                .abstracted
                                .structural_types
                                .access_views()
                                .iter()
                                .filter(|plan| plan.carrier == resident.carrier);
                            let (Some(plan), None) = (plans.next(), plans.next()) else {
                                return Err("resident backing has no unique retained placement");
                            };
                            if plan.schema != resident.schema
                                || plan.placement.content_interpretation().commitment()
                                    != resident.placement_commitment
                            {
                                return Err(
                                    "resident backing differs from its checked content interpretation",
                                );
                            }
                            return Ok(());
                        }
                    };
                    let current = descriptor_machine(self.graph, machine)
                        .ok_or("formation machine has no reference-origin graph")?;
                    let mut positions = current
                        .payload_producers
                        .iter()
                        .filter(|(_, place)| **place == result);
                    let (Some((position, _)), None) = (positions.next(), positions.next()) else {
                        return Err("formation has no unique reference-origin position");
                    };
                    let root = *current
                        .place_types
                        .get(&source.place)
                        .ok_or("formation source has no retained declaration")?;
                    let types = self.abstracted.structural_types.as_slice();
                    let locations =
                        fixed_runtime_index_alternatives(types, source.place, root, &source.path)
                            .or_else(|| {
                                descriptor_location(types, source.place, root, &source.path, None)
                                    .map(|location| vec![location])
                            })
                            .ok_or("formation source has no exact storage locations")?;
                    for location in locations {
                        let alternatives = self
                            .analysis
                            .symbolic_source_origins(machine, *position, &location)
                            .ok_or("formation source has no complete current backing origins")?;
                        if alternatives.is_empty() {
                            return Err("formation source has no backing alternatives");
                        }
                        for origin in alternatives {
                            self.check_origin(machine, &origin, required, &mut Vec::new())?;
                        }
                    }
                    Ok(())
                }

                fn check_origin(
                    &mut self,
                    machine: MachineId,
                    origin: &DescriptorSourceOrigin,
                    required: &AccessRamCorrespondence,
                    pending: &mut Vec<(MachineId, DescriptorSourceOrigin)>,
                ) -> Result<(), &'static str> {
                    if pending
                        .iter()
                        .any(|entry| entry == &(machine, origin.clone()))
                    {
                        return Err("backing caller equation is cyclic");
                    }
                    let (source, incoming_contents) = match origin {
                        DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                            source,
                            ..
                        }) => (*source, false),
                        DescriptorSourceOrigin::IncomingSlot(location) => (location.root, true),
                        DescriptorSourceOrigin::IncomingContents { query, .. } => {
                            (query.root, true)
                        }
                        _ => return Err("backing provenance has no allocation geometry guarantee"),
                    };
                    let current = descriptor_machine(self.graph, machine)
                        .ok_or("backing machine is absent from the reference-origin graph")?;
                    let definition = &current
                        .places
                        .iter()
                        .find(|row| row.place == source)
                        .ok_or("backing place is absent from the reference-origin graph")?
                        .definition;
                    if matches!(definition, DescriptorDefinition::Parameter { access, .. }
                        if incoming_contents || *access != StructuralAccess::Owned)
                    {
                        if machine == self.abstracted.entry
                            || self
                                .abstracted
                                .dynamic_dispatch
                                .tables
                                .iter()
                                .any(|table| table.rows.contains(&Some(machine)))
                        {
                            return Err(
                                "entry or dynamic input has no closed physical backing supply",
                            );
                        }
                        pending.push((machine, origin.clone()));
                        let mut incoming = false;
                        for index in self.incoming.range(machine) {
                            let (caller, alternatives) =
                                self.incoming
                                    .substitute(index, &mut self.analysis, origin)?;
                            incoming = true;
                            for alternative in alternatives {
                                self.check_origin(caller, &alternative, required, pending)?;
                            }
                        }
                        pending.pop();
                        return if incoming {
                            Ok(())
                        } else {
                            Err("borrowed formal has no closed incoming callers")
                        };
                    }
                    let DescriptorSourceOrigin::Backing(DescriptorCallOrigin::Argument {
                        projection,
                        ..
                    }) = origin
                    else {
                        return Err("incoming contents did not resolve to an actual backing");
                    };
                    if !projection.exact
                        || projection.indirect
                        || projection.window.is_some()
                        || projection.captured_at.is_some()
                    {
                        return Err("backing projection has no exact physical address");
                    }
                    let root = *current
                        .place_types
                        .get(&source)
                        .ok_or("allocation origin has no retained root declaration")?;
                    let types = self.abstracted.structural_types.as_slice();
                    let (_, tip) = terminal_semantics::canonical_static_projection(
                        types.iter(),
                        root,
                        &projection.path,
                    )
                    .ok_or("allocation projection has no exact schema endpoint")?;
                    let region = terminal_semantics::reference_storage_region(
                        types.iter(),
                        root,
                        &projection.path,
                    )
                    .ok_or("allocation projection has no complete byte region")?;
                    let mut allocations = self
                        .selected
                        .function(machine)
                        .ok_or("allocation origin has no selected function")?
                        .local_storage_slots
                        .iter()
                        .filter(|slot| slot.id.structural_place() == Some(source));
                    let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
                        return Err("backing origin has no unique actual allocation");
                    };
                    let modulus = required.required_base.modulus();
                    if tip.id == required.schema
                        && region.byte_length == required.source_byte_length
                        && u64::from(allocation.alignment) % modulus == 0
                        && region.byte_offset % modulus == required.required_base.residue()
                        && region
                            .byte_offset
                            .checked_add(region.byte_length)
                            .is_some_and(|end| end <= u64::from(allocation.byte_size))
                    {
                        Ok(())
                    } else {
                        Err(
                            "actual backing does not meet the placed schema, extent or base congruence",
                        )
                    }
                }
            }
        }
    }

    mod stack_calls {
        use super::{Error, MachineId};
        use crate::object_input::ObjectEmissionInput;
        use selected_instructions::SelectedCallTarget;
        use target_operations::{CallSiteOwner, NativeCallTarget};

        #[derive(Clone, Copy)]
        pub(in crate::function_fragments) struct CallStackEdge {
            pub owner: CallSiteOwner,
            pub callee: MachineId,
            pub section_offset: u64,
        }

        impl From<&machine_code::PlacedInternalMachineCallResolution> for CallStackEdge {
            fn from(call: &machine_code::PlacedInternalMachineCallResolution) -> Self {
                Self {
                    owner: call.owner,
                    callee: call.callee,
                    section_offset: call.field_section_offset,
                }
            }
        }

        pub(in crate::function_fragments) fn stack_calls(
            source: &ObjectEmissionInput,
            machine: MachineId,
            callable_invocations: &terminal_semantics::callable_values::ReachableCallableInvocations,
        ) -> Result<Vec<CallStackEdge>, Error> {
            let text = source.text_section();
            let mut edges = text
                .resolved_internal_machine_calls
                .iter()
                .filter(|call| call.caller == machine)
                .map(CallStackEdge::from)
                .collect::<Vec<_>>();
            let emissions = source;
            let selected = emissions
                .emission_facts()
                .physical
                .calls
                .function(machine)
                .ok_or(Error::Mismatch("stack caller has no selected function"))?;
            let placed = text
                .function(machine)
                .ok_or(Error::Mismatch("stack caller has no placed function"))?;
            let catalog = emissions
                .emission_facts()
                .physical
                .contracts
                .publication()
                .code_catalogs()
                .dynamic_dispatch();
            for call in &selected.calls {
                if let NativeCallTarget::Callable { callee, contract } = call.call.target {
                    use terminal_semantics::callable_values::CallableInvocationTarget;
                    let site = crate::callable_calls::call_site(call.owner)
                        .map_err(|detail| Error::CallableInvocation { detail })?;
                    let Some(invocation) = callable_invocations.site(machine, site) else {
                        if callable_invocations
                            .reached_machines
                            .binary_search(&machine)
                            .is_ok()
                        {
                            return Err(Error::Mismatch(
                                "callable stack demand lost its reached occurrence",
                            ));
                        }
                        // Retaining an uninvoked body adds no activation demand.
                        continue;
                    };
                    if invocation.invocation.target
                        != CallableInvocationTarget::Value(callee, contract)
                    {
                        return Err(Error::Mismatch(
                            "callable stack demand changed its evaluated code or contract",
                        ));
                    }
                    if !invocation.targets_are_resolved() {
                        return Err(Error::Unsupported(
                            "callable stack demand has unresolved code and no physical ceiling",
                        ));
                    }
                    let instruction = call
                        .occurrence
                        .as_ref()
                        .ok_or(Error::Mismatch("callable stack call has no instruction"))?;
                    if instruction.target != Some(SelectedCallTarget::Callable { contract }) {
                        return Err(Error::Mismatch("callable stack call target changed"));
                    }
                    let span = placed
                        .instruction_span(call.instruction)
                        .ok_or(Error::Mismatch("callable stack call has no emitted span"))?;
                    for callee in &invocation.selected_machines {
                        if text.function(*callee).is_none() {
                            return Err(Error::Mismatch(
                                "callable stack call target has no emitted function",
                            ));
                        }
                        edges.push(CallStackEdge {
                            owner: call.owner,
                            callee: *callee,
                            section_offset: span.section_offset,
                        });
                    }
                    continue;
                }
                let NativeCallTarget::Dynamic {
                    interface, slot, ..
                } = call.call.target
                else {
                    continue;
                };
                let instruction = call
                    .occurrence
                    .as_ref()
                    .ok_or(Error::Mismatch("dynamic stack call has no instruction"))?;
                if instruction.target != Some(SelectedCallTarget::Dynamic { interface, slot }) {
                    return Err(Error::Mismatch("dynamic stack call target changed"));
                }
                let span = placed
                    .instruction_span(call.instruction)
                    .ok_or(Error::Mismatch("dynamic stack call has no emitted span"))?;
                let position = usize::try_from(slot).map_err(|_| Error::Overflow)?;
                if catalog
                    .interface(interface)
                    .and_then(|interface| interface.slots.get(position))
                    .is_none()
                {
                    return Err(Error::Mismatch("dynamic stack call has no declared slot"));
                }
                let mut targets = Vec::new();
                for table in catalog
                    .tables
                    .iter()
                    .filter(|table| table.interface == interface)
                {
                    let target = table.rows.get(position).ok_or(Error::Mismatch(
                        "dynamic stack call table is missing its slot",
                    ))?;
                    if let Some(target) = target {
                        if text.function(*target).is_none() {
                            return Err(Error::Mismatch(
                                "dynamic stack call target has no emitted function",
                            ));
                        }
                        targets.push(*target);
                    }
                }
                targets.sort();
                targets.dedup();
                edges.extend(targets.into_iter().map(|callee| CallStackEdge {
                    owner: call.owner,
                    callee,
                    section_offset: span.section_offset,
                }));
            }
            Ok(edges)
        }
    }

    #[cfg(test)]
    pub(super) use abstract_operations::requires_graph_storage_replay;
    pub(super) use stack_calls::{CallStackEdge, stack_calls};

    pub(super) fn fragments(source: &ObjectEmissionInput) -> &FunctionFragmentEmissionPlan {
        source.fragments()
    }

    /// The optimized program's function for `machine`, which must be unique.
    pub(super) fn function_facts(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<&verified_optimization_input::publication::EmissionFunctionFacts, Error> {
        let mut functions = source
            .emission_facts()
            .semantic
            .program
            .functions
            .iter()
            .filter(|function| function.machine == machine);
        match (functions.next(), functions.next()) {
            (Some(function), None) => Ok(function),
            _ => Err(Error::Mismatch(
                "shared function has no unique current source",
            )),
        }
    }

    /// Callable reach was evaluated during preparation against the exact optimized
    /// program and invocation roots; the driver carried the program entry's summary.
    pub(super) fn callable_invocations(
        source: &ObjectEmissionInput,
    ) -> Result<&terminal_semantics::callable_values::ReachableCallableInvocations, Error> {
        source
            .emission_facts()
            .physical
            .contracts
            .execution()
            .map(target_vocabulary::NativeExecutionFacts::callable_invocations)
            .ok_or(Error::Mismatch(
                "current program has no prepared callable invocation context",
            ))
    }

    pub(super) fn publication_function(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<&target_vocabulary::NativeFunctionPublication, Error> {
        source
            .emission_facts()
            .physical
            .contracts
            .publication()
            .function(machine)
            .ok_or(Error::Mismatch("current function has no publication facts"))
    }

    /// The target calling contract retained by the current physical program.
    pub(super) fn function_abi(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<&target_vocabulary::function_abi::LegalizedFunctionAbi, Error> {
        source
            .emission_facts()
            .physical
            .contracts
            .function_abi(machine)
            .ok_or(Error::Mismatch(
                "shared function has no current calling contract",
            ))
    }

    pub(super) fn frame(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<Option<&FunctionTargetFrameLayout>, Error> {
        let layout = source.frame_layout();
        let mut rows = layout.functions.iter().filter(|row| row.machine == machine);
        match (rows.next(), rows.next()) {
            (Some(row), None) => Ok(Some(row)),
            _ => Err(Error::Mismatch(
                "shared frame has no unique function geometry",
            )),
        }
    }

    pub(super) fn parameter_abi(
        function: &target_operations::TargetFunction,
    ) -> Option<(
        &calling_conventions::CallPlan,
        &[target_operations::ScalarAbiValue],
        &[target_operations::TargetStructuralParameter],
    )> {
        let graph = &function.graph;
        legalized_operations::function_abi::publishes_parameter_abi(
            &graph.call_plan,
            &graph.scalar_parameters,
            &graph.parameters,
            function.scalar_abi.is_some() || function.mixed_structural_scalar_abi.is_some(),
        )
        .then_some((
            &graph.call_plan,
            &graph.scalar_parameters,
            &graph.parameters,
        ))
    }

    pub(super) fn fragment_metadata(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<
        (
            Option<semantic_vocabulary::StructuralTypeId>,
            &target_operations::TerminalPsiProvenance,
        ),
        Error,
    > {
        let plan = fragments(source);
        if let Some(function) = plan.function(machine) {
            return Ok((function.attachment, &function.provenance));
        }
        Err(Error::Mismatch("missing placed fragment"))
    }

    pub(super) fn temporary_stack_index(
        rows: &[FunctionTemporaryStack],
        machine: MachineId,
    ) -> Result<usize, Error> {
        rows.binary_search_by_key(&machine, |row| row.machine)
            .map_err(|_| Error::Mismatch("missing selected function for temporary stack replay"))
    }

    fn ordered_temporary_stack_rows(
        mut rows: Vec<FunctionTemporaryStack>,
        duplicate: &'static str,
    ) -> Result<Vec<FunctionTemporaryStack>, Error> {
        rows.sort_unstable_by_key(|row| row.machine);
        if rows
            .windows(2)
            .any(|pair| pair[0].machine == pair[1].machine)
        {
            return Err(Error::Mismatch(duplicate));
        }
        Ok(rows)
    }

    pub(super) fn temporary_stack_bytes_by_function(
        source: &ObjectEmissionInput,
    ) -> Result<Vec<FunctionTemporaryStack>, Error> {
        ordered_temporary_stack_rows(
            source.emission_facts().physical.temporary_stack.clone(),
            "temporary stack plan repeats a function",
        )
    }

    pub(super) fn decoded_temporary_stack_bytes_by_function(
        source: &ObjectEmissionInput,
    ) -> Result<Vec<FunctionTemporaryStack>, Error> {
        let mut peaks = ordered_temporary_stack_rows(
            source
                .emission_facts()
                .physical
                .cpu
                .functions
                .iter()
                .map(|function| FunctionTemporaryStack {
                    machine: function.machine,
                    byte_count: 0,
                })
                .collect(),
            "encoding instruction roster changed for temporary stack replay",
        )?;
        let mut rows = source.readback().instruction_effects.iter();
        let mut seen = std::collections::BTreeSet::new();
        for &(machine, instruction) in &source.emission_facts().physical.cpu.instruction_roster {
            let row = rows.next().ok_or(Error::Mismatch(
                "missing encoding row for temporary stack replay",
            ))?;
            if row.machine != machine
                || row.instruction != instruction
                || !seen.insert((machine, instruction))
            {
                return Err(Error::Mismatch(
                    "encoding instruction roster changed for temporary stack replay",
                ));
            }
            let index = temporary_stack_index(&peaks, machine)?;
            if let Some(effects) = row.effects() {
                let peak = &mut peaks[index].byte_count;
                *peak = (*peak).max(temporary_stack_peak(std::iter::once(effects)));
            }
        }
        if rows.next().is_some() {
            return Err(Error::Mismatch(
                "extra encoding row for temporary stack replay",
            ));
        }
        Ok(peaks)
    }

    pub(super) fn replayed_temporary_stack_bytes_by_function(
        replay: &crate::FragmentReplay,
    ) -> Result<Vec<FunctionTemporaryStack>, Error> {
        let originals = replay.original_encoding().ok_or(Error::Mismatch(
            "original physical instructions were not retained (ReplayUnavailable)",
        ))?;
        let encoding = &originals.encoding;
        temporary_stack_bytes_for_selected_functions(
            originals.physical.functions.iter(),
            encoding.rows(),
            encoding.encoded(),
        )
    }

    #[cfg(test)]
    fn temporary_stack_bytes_for_function(
        machine: MachineId,
        selected: &selected_instructions::SelectedFunctions,
        rows: &[machine_code::SelectedFormEncodingRow],
        encoded: &machine_code::SelectedFormEncodedRowStore,
    ) -> Result<u32, Error> {
        selected.function(machine).ok_or(Error::Mismatch(
            "missing selected function for temporary stack replay",
        ))?;
        let peaks = temporary_stack_bytes_for_selected_functions(selected.iter(), rows, encoded)?;
        Ok(peaks[temporary_stack_index(&peaks, machine)?].byte_count)
    }

    fn temporary_stack_bytes_for_selected_functions<'function>(
        selected: impl IntoIterator<Item = &'function selected_instructions::SelectedFunction>,
        rows: &[machine_code::SelectedFormEncodingRow],
        encoded: &machine_code::SelectedFormEncodedRowStore,
    ) -> Result<Vec<FunctionTemporaryStack>, Error> {
        let mut encoded_rows = rows.iter();
        let mut peaks = Vec::new();
        let mut identities = std::collections::BTreeSet::new();
        let mut effects = Vec::new();
        for function in selected {
            identities.clear();
            effects.clear();
            for block in &function.blocks {
                for instruction in block.instructions.iter().chain(std::iter::once(
                    selected_instructions::terminator_instruction(&block.terminator),
                )) {
                    let row = encoded_rows.next().ok_or(Error::Mismatch(
                        "missing encoding row for temporary stack replay",
                    ))?;
                    if row.instruction != instruction.id || !identities.insert(row.instruction) {
                        return Err(Error::Mismatch(
                            "encoding instruction roster changed for temporary stack replay",
                        ));
                    }
                    if let Some(footprint) = encoded.footprint(row.footprint) {
                        effects.push(&footprint.encoded);
                    }
                }
            }
            peaks.push(FunctionTemporaryStack {
                machine: function.machine,
                byte_count: temporary_stack_peak(effects.iter().copied()),
            });
        }
        if encoded_rows.next().is_some() {
            return Err(Error::Mismatch(
                "extra encoding row for temporary stack replay",
            ));
        }
        ordered_temporary_stack_rows(
            peaks,
            "encoding instruction roster changed for temporary stack replay",
        )
    }

    fn temporary_stack_peak<'effects>(
        effects: impl Iterator<Item = &'effects selected_instructions::MachineEncodedEffects>,
    ) -> u32 {
        effects
            .filter_map(|effects| match effects.stack {
                selected_instructions::MachineEncodedStackEffect::TemporaryBytesV1 {
                    byte_count,
                    ..
                } => Some(u32::from(byte_count)),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }

    #[cfg(test)]
    mod tests {
        use super::requires_graph_storage_replay;
        use abstract_operations::AbstractOperation;
        use semantic_vocabulary::{OperationId, ValueId};

        #[test]
        fn atomic_storage_requires_retained_graph_replay() {
            use abstract_operations::atomic::AbstractAtomicLocation;
            use abstract_operations::{AbstractAtomicEvent, AtomicResidencyEdge};
            use semantic_vocabulary::PlaceId;

            let operation = AbstractOperation::AtomicEvent {
                psi_operation: OperationId::new(1).unwrap(),
                event: AbstractAtomicEvent::Store {
                    location: AbstractAtomicLocation {
                        root: PlaceId::new(1).unwrap(),
                        path: vec![terminal_psi::StructuralPathSegment::Field(
                            terminal_psi::TerminalIdentityIndex(1),
                        )],
                    },
                    ordering: terminal_psi::MemoryOrdering::NoOrdering,
                    value: ValueId::new(1).unwrap(),
                },
                reads_from: None,
                modification_after: Some(AtomicResidencyEdge::InitialResidency),
                resident_obligation: None,
            };
            assert!(requires_graph_storage_replay(&[operation]));
        }

        #[test]
        fn replay_requirement_is_specific_to_primitive_storage_operations() {
            use abstract_operations::AbstractResult;
            use semantic_vocabulary::{
                IntegerSign, IntegerType, PlaceId, ScalarType, StructuralTypeId,
            };
            use terminal_psi::{
                StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
                StructuralParameterDeclaration,
            };

            let operation = OperationId::new(1).unwrap();
            let scalar = AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            };
            let result = StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: Vec::new(),
                claims: vec![],
            };
            let existing = [AbstractOperation::BooleanConstant {
                psi_operation: operation,
                result: scalar.value,
                value: true,
            }];
            assert!(!requires_graph_storage_replay(&[]));
            assert!(!requires_graph_storage_replay(&existing));
            for operation in [
                AbstractOperation::WriteOnlyPrimitiveStore {
                    psi_operation: operation,
                    path: Vec::new(),
                    destination: abstract_operations::StructuralScalarStoreDestination::Parameter(
                        StructuralParameterDeclaration {
                            place: result.place,
                            position: 0,
                            structural_type: result.structural_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            access: StructuralAccess::MutableBorrow,
                            qualifications: vec![],
                            projected_qualifications: vec![],
                            additional_input_qualifications: Vec::new(),
                        },
                    ),
                    value: scalar,
                    range_obligation: None,
                },
                AbstractOperation::EstablishPrimitiveLocal {
                    psi_operation: operation,
                    result: result.clone(),
                    value: scalar,
                    range_obligation: None,
                },
                AbstractOperation::PrimitiveLocalStore {
                    psi_operation: operation,
                    destination: result.place,
                    value: scalar,
                },
                AbstractOperation::PrimitiveScalarRead {
                    psi_operation: operation,
                    path: Vec::new(),
                    result: scalar,
                    source: result.place,
                },
            ] {
                assert!(requires_graph_storage_replay(std::slice::from_ref(
                    &operation
                )));
                let mut mixed = existing.to_vec();
                mixed.push(operation);
                assert!(requires_graph_storage_replay(&mixed));
            }
        }

        mod temporary_stack {
            use super::super::temporary_stack_bytes_for_function;
            use machine_code::{
                SelectedFormDecodedFootprint, SelectedFormEncodedRowStore, SelectedFormEncodingRow,
                SelectedFormEncodingState, SelectedFormMachineDisposition,
            };
            use selected_instructions::{
                MachineAlternativeFamily, MachineAlternativeKey, MachineEncodedEffects,
                MachineEncodedStackEffect, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
                SelectedFunction, SelectedFunctions, SelectedInstruction, SelectedInstructionId,
                SelectedInstructionKind, SelectedTerminator,
            };
            use semantic_vocabulary::{BlockId, EdgeId, MachineId};

            fn instruction(id: u32, kind: SelectedInstructionKind) -> SelectedInstruction {
                SelectedInstruction {
                    id: SelectedInstructionId(id),
                    kind,
                    constraint: register_model::RegisterConstraintKey {
                        family: register_model::RegisterConstraintFamily::Instruction,
                        variant: 0,
                    },
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: Default::default(),
                }
            }

            fn function(machine: u64) -> SelectedFunction {
                SelectedFunction {
                    contracts: std::sync::Arc::new(
                        selected_instructions::SelectedFunctionContracts {
                            structural_block_parameters: Vec::new(),
                            machine: MachineId::new(machine).unwrap(),
                            attachment: None,
                            provenance: Default::default(),
                            structural: None,
                            local_storage_slots: Vec::new(),
                            outgoing_arguments: Vec::new(),
                            calls: Vec::new(),
                            normalized_foreign_calls: Vec::new(),
                            memory_accesses: Vec::new(),
                            boundary_settlements: Vec::new(),
                            virtual_registers: Vec::new(),
                        },
                    ),
                    entry_block: SelectedBlockId(0),
                    blocks: (vec![SelectedBlock {
                        id: SelectedBlockId(0),
                        origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                        instructions: vec![instruction(0, SelectedInstructionKind::CopyI64)],
                        terminator: SelectedTerminator::Return {
                            instruction: instruction(1, SelectedInstructionKind::ReturnUnit),
                            psi_return_edge: EdgeId::new(1).unwrap(),
                        },
                    }])
                    .into(),
                }
            }

            fn encoding(
                peaks: &[u16],
            ) -> (Vec<SelectedFormEncodingRow>, SelectedFormEncodedRowStore) {
                let mut store = SelectedFormEncodedRowStore::new();
                let mut rows = Vec::new();
                for &peak in peaks {
                    for id in 0..2 {
                        let mut effects =
                            MachineEncodedEffects::fallthrough_v1(Vec::new(), Vec::new());
                        if id == 0 && peak != 0 {
                            effects.stack = MachineEncodedStackEffect::TemporaryBytesV1 {
                                stack_pointer: register_model::RegisterViewId(4),
                                byte_count: peak,
                            };
                        }
                        let (code, footprint) = store.intern(
                            &[if id == 0 { 0x90 } else { 0xc3 }],
                            SelectedFormDecodedFootprint {
                                register_reads: Vec::new(),
                                register_writes: Vec::new(),
                                implicit_defs: Vec::new(),
                                implicit_clobbers: Vec::new(),
                                encoded: effects,
                            },
                        );
                        rows.push(SelectedFormEncodingRow {
                            instruction: SelectedInstructionId(id),
                            alternative: MachineAlternativeKey {
                                family: MachineAlternativeFamily::CopyI64,
                                variant: 0,
                            },
                            machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                            state: SelectedFormEncodingState::Encoded,
                            address: None,
                            code,
                            footprint,
                        });
                    }
                }
                (rows, store)
            }

            fn assert_function_peaks(order: &[(u64, u16)]) {
                let mut functions = SelectedFunctions::default();
                for &(machine, _) in order {
                    functions.push(function(machine));
                }
                let (rows, store) =
                    encoding(&order.iter().map(|&(_, peak)| peak).collect::<Vec<_>>());
                let actual = functions
                    .iter()
                    .map(|function| {
                        temporary_stack_bytes_for_function(
                            function.machine,
                            &functions,
                            &rows,
                            &store,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                let expected = order
                    .iter()
                    .map(|&(_, peak)| u32::from(peak))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
            }

            #[test]
            fn temporary_stack_keeps_ordinary_and_flags_functions_separate() {
                assert_function_peaks(&[(1, 0), (2, 8)]);
            }

            #[test]
            fn temporary_stack_does_not_import_another_functions_directed_fp_peak() {
                assert_function_peaks(&[(1, 0), (2, 8), (3, 16)]);
            }

            #[test]
            fn temporary_stack_identity_survives_reverse_function_order() {
                assert_function_peaks(&[(3, 16), (2, 8), (1, 0)]);
            }

            #[test]
            fn temporary_stack_rejects_changed_missing_extra_and_duplicate_roster_rows() {
                let mut functions = SelectedFunctions::default();
                functions.push(function(1));
                functions.push(function(2));
                let (rows, store) = encoding(&[0, 8]);
                let check = |rows: &[SelectedFormEncodingRow]| {
                    temporary_stack_bytes_for_function(
                        MachineId::new(1).unwrap(),
                        &functions,
                        rows,
                        &store,
                    )
                };
                check(&rows).unwrap();
                let mut changed = rows.clone();
                changed[1].instruction = SelectedInstructionId(7);
                assert!(check(&changed).is_err());
                assert!(check(&rows[..3]).is_err());
                changed = rows.clone();
                changed.push(rows[0].clone());
                assert!(check(&changed).is_err());
                let mut duplicate = functions.clone();
                duplicate[0].blocks[0].instructions[0].id = SelectedInstructionId(1);
                changed = rows.clone();
                changed[0].instruction = SelectedInstructionId(1);
                assert!(
                    temporary_stack_bytes_for_function(
                        MachineId::new(1).unwrap(),
                        &duplicate,
                        &changed,
                        &store
                    )
                    .is_err()
                );
                assert!(
                    temporary_stack_bytes_for_function(
                        MachineId::new(3).unwrap(),
                        &functions,
                        &rows,
                        &store
                    )
                    .is_err()
                );
            }

            #[test]
            fn temporary_stack_keeps_tight_under_budget_numeric_controls() {
                let mut functions = SelectedFunctions::default();
                for machine in 1..=3 {
                    functions.push(function(machine));
                }
                let (rows, store) = encoding(&[0, 8, 16]);
                for (machine, exact, insufficient) in [(2, 8, 7), (3, 16, 15)] {
                    let required = temporary_stack_bytes_for_function(
                        MachineId::new(machine).unwrap(),
                        &functions,
                        &rows,
                        &store,
                    )
                    .unwrap();
                    assert!(required <= exact);
                    assert!(required > insufficient);
                }
            }
        }
    }
}

pub(crate) use source::selected_constant_operand;
#[path = "function_fragments/internal_calls.rs"]
mod internal_calls;
mod structural {
    mod byte_input {
        use super::{Error, attribution, fragment, host, placed_text_offset, source};
        use crate::ObjectBoundarySettlement;
        use crate::object::object_artifact::replay::boundary::result_placement::hosted_read_byte_declaration_is_valid;
        use crate::object_input::ObjectEmissionInput;
        use machine_code::{
            BoundaryExecutionRecord, BoundaryResultRecord, BoundarySettlementRecord,
            BoundaryStructuralResultRecord, SemanticCodeSite,
        };
        use selected_instructions::{LocalStorageSlotId, SelectedBoundarySettlementPayload};
        use semantic_vocabulary::MachineId;
        use target::{Architecture, NativeTarget};
        use target_operations::{BoundaryRealization, CompilerBuiltinExecution};

        fn decoded_home(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
            if !target_operations::HostedReadByteRealization::supports_target(target) {
                return None;
            }
            match target.architecture {
                Architecture::X86_64 => {
                    target_x86_64::inspection::decode_x86_64_selected_hosted_read_byte(
                        target, bytes,
                    )
                }
                Architecture::Aarch64 => {
                    target_aarch64::inspection::decode_aarch64_selected_hosted_read_byte(
                        target, bytes,
                    )
                }
            }
        }

        pub(super) fn settlement(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
        ) -> Result<ObjectBoundarySettlement, Error> {
            let invalid = || Error::Mismatch("selected byte input result custody");
            let SelectedBoundarySettlementPayload::HostedReadByte {
                operation,
                boundary,
                result,
                layout,
            } = &located.settlement
            else {
                return Err(invalid());
            };
            let fragment = fragment(container, machine)?;
            let slot = LocalStorageSlotId::Structural {
                operation: *operation,
                place: result.place,
            };
            let span = fragment
                .instruction_span(located.anchor)
                .ok_or_else(invalid)?;
            let offset =
                decoded_home(container.text_section().target, &span.bytes).ok_or_else(invalid)?;
            let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
            let home = frame.local_storage_slot(slot).ok_or_else(invalid)?;
            if frame.red_zone_resident_bytes != 0
                || home.frame_offset_bytes != u64::from(offset)
                || home.size_bytes != layout.shape.byte_size
                || home.alignment_bytes != layout.shape.alignment
                || home
                    .frame_offset_bytes
                    .checked_add(u64::from(home.size_bytes))
                    .is_none_or(|end| end > frame.frame_size_bytes)
            {
                return Err(invalid());
            }
            let code_offset = host(span.offset)?;
            let record = BoundarySettlementRecord {
                invocation_site: (*operation).into(),
                boundary: *boundary,
                execution: BoundaryExecutionRecord::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedReadByte,
                ),
                realization: BoundaryRealization::HostedReadByte(Default::default()),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: Vec::new(),
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: BoundaryResultRecord::Structural(BoundaryStructuralResultRecord {
                    defining_operation: *operation,
                    result: result.clone(),
                    declaration: result_declaration(located, result.structural_type)?.clone(),
                    layout: layout.clone(),
                    home_byte_offset: offset,
                }),
                operation_ordinal: attribution::ordinal(
                    source::publication_function(container, machine)?,
                    SemanticCodeSite::Operation(*operation),
                )?,
                code_offset,
                byte_count: span.bytes.len(),
            };
            let placed = container
                .text_section()
                .function(machine)
                .ok_or_else(invalid)?;
            Ok(ObjectBoundarySettlement {
                machine,
                text_offset: placed_text_offset(placed.section_offset, code_offset)?,
                settlement: record,
            })
        }

        pub(super) fn validate(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
            proposed: &ObjectBoundarySettlement,
        ) -> Result<(), Error> {
            let invalid = || Error::Mismatch("selected byte input publication custody");
            let SelectedBoundarySettlementPayload::HostedReadByte {
                operation,
                boundary,
                result,
                layout,
            } = &located.settlement
            else {
                return Err(invalid());
            };
            let fragment = fragment(container, machine)?;
            let slot = LocalStorageSlotId::Structural {
                operation: *operation,
                place: result.place,
            };
            let span = fragment
                .instruction_span(located.anchor)
                .ok_or_else(invalid)?;
            let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
            let home = frame.local_storage_slot(slot).ok_or_else(invalid)?;
            let placed = container
                .text_section()
                .function(machine)
                .ok_or_else(invalid)?;
            let record = &proposed.settlement;
            let BoundaryResultRecord::Structural(produced) = &record.native_result else {
                return Err(invalid());
            };
            let code_offset = host(span.offset)?;
            if proposed.machine != machine
                || &produced.declaration != result_declaration(located, result.structural_type)?
                || proposed.text_offset != placed_text_offset(placed.section_offset, code_offset)?
                || record.invocation_site != (*operation).into()
                || record.boundary != *boundary
                || record.operation_ordinal
                    != attribution::ordinal(
                        source::publication_function(container, machine)?,
                        SemanticCodeSite::Operation(*operation),
                    )?
                || record.execution
                    != BoundaryExecutionRecord::CompilerBuiltin(
                        CompilerBuiltinExecution::HostedReadByte,
                    )
                || record.realization != BoundaryRealization::HostedReadByte(Default::default())
                || produced.defining_operation != *operation
                || produced.result != *result
                || produced.layout != *layout
                || frame.red_zone_resident_bytes != 0
                || u64::from(produced.home_byte_offset) != home.frame_offset_bytes
                || home.size_bytes != layout.shape.byte_size
                || home.alignment_bytes != layout.shape.alignment
                || home
                    .frame_offset_bytes
                    .checked_add(u64::from(home.size_bytes))
                    .is_none_or(|end| end > frame.frame_size_bytes)
                || record.code_offset != code_offset
                || record.byte_count != span.bytes.len()
                || decoded_home(container.text_section().target, &span.bytes)
                    != Some(produced.home_byte_offset)
                || fragment.bytes.get(
                    code_offset
                        ..code_offset
                            .checked_add(record.byte_count)
                            .ok_or(Error::Overflow)?,
                ) != Some(span.bytes.as_slice())
                || !record.scalar_arguments.is_empty()
                || !record.runtime_scalar_arguments.is_empty()
                || !record.arguments.is_empty()
                || !record.byte_sequence_arguments.is_empty()
                || !record.completion_claim_sources.is_empty()
                || !record.completion_receipts.is_empty()
                || !record.completion_provider_custody.is_empty()
            {
                return Err(invalid());
            }
            Ok(())
        }

        fn result_declaration(
            plan: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
            structural_type: semantic_vocabulary::StructuralTypeId,
        ) -> Result<&terminal_psi::StructuralTypeDeclaration, Error> {
            plan.read_result_declaration
                .as_ref()
                .filter(|declaration| {
                    declaration.id == structural_type
                        && hosted_read_byte_declaration_is_valid(declaration)
                })
                .ok_or(Error::Mismatch("selected byte input declaration custody"))
        }
    }

    mod byte_output {
        use super::{Error, attribution, fragment, host, placed_text_offset, source};
        use crate::ObjectBoundarySettlement;
        use crate::object::object_artifact::replay::boundary::runtime_scalar_custody::hosted_i32_argument;
        use crate::object_input::ObjectEmissionInput;
        use machine_code::{
            BoundaryExecutionRecord, BoundaryResultRecord, BoundarySettlementRecord,
            InternalUnitScalarArgumentSourceRecord, SemanticCodeSite,
        };
        use selected_instructions::{LocalStorageSlotId, SelectedBoundarySettlementPayload};
        use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};

        pub(super) fn settlement(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
        ) -> Result<ObjectBoundarySettlement, Error> {
            let SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation,
                boundary,
                source: value,
                execution,
            } = located.settlement
            else {
                return Err(Error::Mismatch("selected byte output payload"));
            };
            let fragment = fragment(container, machine)?;
            let slot = LocalStorageSlotId::Boundary { operation };
            let span = fragment
                .instruction_span(located.anchor)
                .ok_or(Error::Mismatch("selected byte output span"))?;
            let native = container.text_section().target;
            let (register, offset) =
                crate::object::object_artifact::replay::boundary::runtime_scalar_custody::decode_selected_byte_output(native, &span.bytes)
                    .ok_or(Error::Mismatch("selected byte output encoding"))?;
            let frame =
                source::frame(container, machine)?.ok_or(Error::Mismatch("byte output frame"))?;
            let scratch = frame
                .local_storage_slot(slot)
                .ok_or(Error::Mismatch("byte output scratch slot"))?;
            if frame.red_zone_resident_bytes != 0
                || scratch.frame_offset_bytes != u64::from(offset)
                || scratch.size_bytes != 1
                || scratch.alignment_bytes != 1
                || u64::from(offset) >= frame.frame_size_bytes
            {
                return Err(Error::Mismatch("byte output scratch geometry"));
            }
            let code_offset = host(span.offset)?;
            let scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).map_err(|_| Error::Mismatch("i32"))?,
            );
            let (execution, realization) = current_settlement(execution)?;
            let settlement = BoundarySettlementRecord {
                invocation_site: operation.into(),
                boundary,
                execution: execution.into(),
                realization,
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: vec![hosted_i32_argument(
                    InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                        source_value: value,
                        scalar_type,
                        instruction: located.anchor,
                        scratch_byte_offset: scratch.frame_offset_bytes,
                    },
                    register,
                    code_offset,
                    span.bytes.len(),
                )],
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: BoundaryResultRecord::Unit,
                operation_ordinal: attribution::ordinal(
                    source::publication_function(container, machine)?,
                    SemanticCodeSite::Operation(operation),
                )?,
                code_offset,
                byte_count: span.bytes.len(),
            };
            let placed = container
                .text_section()
                .function(machine)
                .ok_or(Error::Mismatch("byte output placed function"))?;
            Ok(ObjectBoundarySettlement {
                machine,
                text_offset: placed_text_offset(placed.section_offset, code_offset)?,
                settlement,
            })
        }

        pub(super) fn validate(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
            proposed: &ObjectBoundarySettlement,
        ) -> Result<(), Error> {
            let invalid = || Error::Mismatch("selected byte output publication custody");
            let SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation,
                boundary,
                source: value,
                execution,
            } = located.settlement
            else {
                return Err(invalid());
            };
            let fragment = fragment(container, machine)?;
            let slot = LocalStorageSlotId::Boundary { operation };
            let span = fragment
                .instruction_span(located.anchor)
                .ok_or_else(invalid)?;
            let frame = source::frame(container, machine)?.ok_or_else(invalid)?;
            let scratch = frame.local_storage_slot(slot).ok_or_else(invalid)?;
            let placed = container
                .text_section()
                .function(machine)
                .ok_or_else(invalid)?;
            let settlement = &proposed.settlement;
            let [argument] = settlement.runtime_scalar_arguments.as_slice() else {
                return Err(invalid());
            };
            let InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
                source_value,
                scalar_type,
                instruction,
                scratch_byte_offset,
            } = argument.source
            else {
                return Err(invalid());
            };
            let offset = host(span.offset)?;
            let (execution, realization) = current_settlement(execution)?;
            if settlement.execution != BoundaryExecutionRecord::from(execution)
                || settlement.realization != realization
                || proposed.machine != machine
                || proposed.text_offset != placed_text_offset(placed.section_offset, offset)?
                || settlement.invocation_site != operation.into()
                || settlement.boundary != boundary
                || settlement.operation_ordinal
                    != attribution::ordinal(source::publication_function(container, machine)?, SemanticCodeSite::Operation(operation))?
                || source_value != value
                || instruction != located.anchor
                || !matches!(scalar_type, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed && integer.bits() == Some(32))
                || frame.red_zone_resident_bytes != 0
                || scratch_byte_offset != scratch.frame_offset_bytes
                || scratch.size_bytes != 1
                || scratch.alignment_bytes != 1
                || scratch_byte_offset >= frame.frame_size_bytes
                || settlement.code_offset != offset
                || settlement.byte_count != span.bytes.len()
                || !crate::object::object_artifact::replay::boundary::runtime_scalar_custody::selected_byte_output_bytes_are_exact(
                    container.text_section().target,
                    settlement,
                    &fragment.bytes,
                )
            {
                return Err(invalid());
            }
            Ok(())
        }

        /// Read the byte writer from the current boundary contract. Only an
        /// admitted provider or the compiler's built-in host byte output may
        /// write a byte.
        fn current_settlement(
            execution: target_operations::BoundaryExecutionBinding,
        ) -> Result<
            (
                target_operations::BoundaryExecutionBinding,
                target_operations::BoundaryRealization,
            ),
            Error,
        > {
            use target_operations::{
                BoundaryExecutionBinding, BoundaryRealization, CompilerBuiltinExecution,
                HostedWriteByteI32Realization,
            };
            if !matches!(
                execution,
                BoundaryExecutionBinding::AdmittedProvider(_)
                    | BoundaryExecutionBinding::CompilerBuiltin(
                        CompilerBuiltinExecution::HostedWriteByteI32
                    )
            ) {
                return Err(Error::Mismatch("byte output current settlement custody"));
            }
            Ok((
                execution,
                BoundaryRealization::HostedWriteByteI32(HostedWriteByteI32Realization),
            ))
        }
    }

    mod process_exit {
        use super::{Error, attribution, fragment, host, placed_text_offset, source};
        use crate::ObjectBoundarySettlement;
        use crate::object::object_artifact::replay::boundary::runtime_scalar_custody::hosted_i32_argument;
        use crate::object_input::ObjectEmissionInput;
        use machine_code::{
            BoundaryResultRecord, BoundarySettlementRecord, InternalUnitScalarArgumentSourceRecord,
            SemanticCodeSite,
        };
        use selected_instructions::SelectedBoundarySettlementPayload;
        use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType};

        pub(super) fn settlement(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
        ) -> Result<ObjectBoundarySettlement, Error> {
            let SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                edge,
                execution,
                boundary,
                source: value,
            } = located.settlement
            else {
                return Err(Error::Mismatch("selected process exit payload"));
            };
            let fragment = fragment(container, machine)?;
            let span = fragment
                .instruction_span(located.anchor)
                .ok_or(Error::Mismatch("process exit span"))?;
            if span.control
                != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
                    completion_edge: edge,
                })
            {
                return Err(Error::Mismatch("process exit completion edge"));
            }
            let register =
                crate::object::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::decode(
                    container.text_section().target,
                    &span.bytes,
                )
                .ok_or(Error::Mismatch("process exit encoding"))?;
            let code_offset = host(span.offset)?;
            let scalar_type = ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, 32).map_err(|_| Error::Mismatch("i32"))?,
            );
            let settlement = BoundarySettlementRecord {
                invocation_site: semantic_vocabulary::InvocationSite::ExternalCompletion(edge),
                boundary,
                execution: execution.into(),
                realization: target_operations::BoundaryRealization::HostedExitProcessI32(
                    Default::default(),
                ),
                scalar_arguments: Vec::new(),
                runtime_scalar_arguments: vec![hosted_i32_argument(
                    InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                        source_value: value,
                        scalar_type,
                        instruction: located.anchor,
                    },
                    register,
                    code_offset,
                    span.bytes.len(),
                )],
                arguments: Vec::new(),
                byte_sequence_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
                completion_provider_custody: Vec::new(),
                native_result: BoundaryResultRecord::Unit,
                operation_ordinal: attribution::ordinal(
                    source::publication_function(container, machine)?,
                    SemanticCodeSite::Edge(edge),
                )?,
                code_offset,
                byte_count: span.bytes.len(),
            };
            let placed = container
                .text_section()
                .function(machine)
                .ok_or(Error::Mismatch("process exit placed function"))?;
            Ok(ObjectBoundarySettlement {
                machine,
                text_offset: placed_text_offset(placed.section_offset, code_offset)?,
                settlement,
            })
        }

        pub(super) fn validate(
            container: &ObjectEmissionInput,
            machine: MachineId,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
            proposed: &ObjectBoundarySettlement,
        ) -> Result<(), Error> {
            let invalid = || Error::Mismatch("process exit publication custody");
            let SelectedBoundarySettlementPayload::HostedExitProcessI32 {
                edge,
                execution,
                boundary,
                source: value,
            } = located.settlement
            else {
                return Err(invalid());
            };
            let fragment = fragment(container, machine)?;
            let span = fragment
                .blocks
                .iter()
                .flat_map(|row| &row.instructions)
                .find(|row| row.instruction == located.anchor)
                .ok_or_else(invalid)?;
            let [argument] = proposed.settlement.runtime_scalar_arguments.as_slice() else {
                return Err(invalid());
            };
            let InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
                source_value,
                instruction: source_instruction,
                ..
            } = argument.source
            else {
                return Err(invalid());
            };
            let placed = container
                .text_section()
                .function(machine)
                .ok_or_else(invalid)?;
            let offset = host(span.offset)?;
            if source_value != value
                || source_instruction != located.anchor
                || span.control
                    != (machine_code::FunctionFragmentControlProvenance::HostedExitProcess {
                        completion_edge: edge,
                    })
                || proposed.machine != machine
                || proposed.text_offset != placed_text_offset(placed.section_offset, offset)?
                || proposed.settlement.invocation_site != semantic_vocabulary::InvocationSite::ExternalCompletion(edge)
                || proposed.settlement.execution != execution.into()
                || proposed.settlement.boundary != boundary
                || proposed.settlement.operation_ordinal
                    != attribution::ordinal(source::publication_function(container, machine)?, SemanticCodeSite::Edge(edge))?
                || proposed.settlement.code_offset != offset
                || proposed.settlement.byte_count != span.bytes.len()
                || !crate::object::object_artifact::replay::boundary::runtime_scalar_custody::process_exit::bytes_are_exact(
                    container.text_section().target,
                    &proposed.settlement,
                    &fragment.bytes,
                )
            {
                return Err(invalid());
            }
            Ok(())
        }
    }

    mod validation {
        use super::{
            Error, MachineId, ObjectBoundarySettlement, ObjectEmissionInput, ObjectFunction,
            SemanticCodeAttribution, SemanticCodeSite, StructuralSourceLocation, attribution,
            boundary_settlements, byte_input, byte_output, fragment, placed_text_offset,
            process_exit, semantic_code_site, source,
        };
        use calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
        pub(in crate::function_fragments) fn validate_function(
            source: &ObjectEmissionInput,
            function: &ObjectFunction,
            rows: &[SemanticCodeAttribution],
        ) -> Result<(), Error> {
            let plan = super::super::internal_calls::publication(source, function.machine)?;
            let invalid =
                || Error::Mismatch("structural object differs from current ABI or call evidence");
            if function.unit_affine_cleanup.is_some() {
                return Err(invalid());
            }
            let (records, homes, unused, unused_homes) =
                if function.mixed_structural_scalar_abi.is_some() {
                    (
                        &function.scalar_structural_parameters,
                        &function.scalar_structural_parameter_homes,
                        &function.unit_parameters,
                        &function.unit_parameter_homes,
                    )
                } else {
                    (
                        &function.unit_parameters,
                        &function.unit_parameter_homes,
                        &function.scalar_structural_parameters,
                        &function.scalar_structural_parameter_homes,
                    )
                };
            if !unused.is_empty()
                || !unused_homes.is_empty()
                || records != &plan.parameters
                || homes != &plan.homes
            {
                return Err(invalid());
            }
            for home in homes {
                validate_incoming_location(home.access, &home.source, home.location)?;
                if !home.indirect {
                    return Err(invalid());
                }
            }
            let calls = super::super::internal_calls::place_calls(source, function.machine, rows)?;
            if function.internal_unit_calls != calls {
                return Err(invalid());
            }
            Ok(())
        }

        fn validate_incoming_location(
            access: terminal_psi::StructuralAccess,
            placement: &ValuePlacement,
            location: StructuralSourceLocation,
        ) -> Result<(), Error> {
            let invalid = || {
                Error::Mismatch("structural incoming location differs from declared access or ABI")
            };
            if access == terminal_psi::StructuralAccess::Owned {
                let [
                    ValueLocation::Indirect {
                        pointer,
                        copy_stack_byte_offset: Some(_),
                        byte_size,
                        alignment,
                    },
                ] = placement.locations.as_slice()
                else {
                    return Err(invalid());
                };
                if !placement.shape.class.is_integer()
                    || *byte_size != placement.shape.byte_size
                    || *alignment != placement.shape.alignment
                {
                    return Err(invalid());
                }
                return match (pointer, location) {
                    (
                        IndirectPointerLocation::Register(expected),
                        StructuralSourceLocation::IncomingIndirectPointer { register },
                    ) if *expected == register => Ok(()),
                    (
                        IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        },
                        StructuralSourceLocation::IncomingIndirectStackPointer {
                            stack_byte_offset: actual,
                            alignment: actual_alignment,
                        },
                    ) if *stack_byte_offset == actual && *alignment == actual_alignment => Ok(()),
                    _ => Err(invalid()),
                };
            }
            let (location, opaque) = match location {
                StructuralSourceLocation::IncomingBorrowedPointer { location } => (location, false),
                StructuralSourceLocation::IncomingOpaquePointer { location } => (location, true),
                _ => return Err(invalid()),
            };
            let scalar_descriptor =
                placement.shape.class == calling_conventions::ValueClass::BorrowedScalarReference;
            let correct_shape = if opaque {
                placement.shape == calling_conventions::ValueShape::integer(8, 8)
            } else {
                placement.shape.class == calling_conventions::ValueClass::BorrowedReference
                    || (scalar_descriptor
                        && placement.shape
                            == calling_conventions::ValueShape::borrowed_scalar_reference())
            };
            if !correct_shape {
                return Err(invalid());
            }
            match (placement.locations.as_slice(), location) {
                (
                    [
                        ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 8,
                        },
                    ],
                    IndirectPointerLocation::Register(actual),
                ) if !scalar_descriptor && *register == actual => Ok(()),
                (
                    [
                        ValueLocation::Stack {
                            stack_byte_offset,
                            value_byte_offset: 0,
                            byte_size: 8,
                            alignment: 8,
                        },
                    ],
                    IndirectPointerLocation::Stack {
                        stack_byte_offset: actual,
                        alignment: 8,
                    },
                ) if !scalar_descriptor && *stack_byte_offset == actual => Ok(()),
                (
                    [
                        ValueLocation::Indirect {
                            pointer,
                            copy_stack_byte_offset: None,
                            byte_size,
                            alignment,
                        },
                    ],
                    actual,
                ) if !opaque
                    && *pointer == actual
                    && *byte_size == placement.shape.byte_size
                    && *alignment == placement.shape.alignment
                    && match pointer {
                        IndirectPointerLocation::Register(_) => true,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment: 8,
                        } => stack_byte_offset.is_multiple_of(8),
                        _ => false,
                    } =>
                {
                    Ok(())
                }
                _ => Err(invalid()),
            }
        }

        pub(in crate::function_fragments) fn validate_settlements(
            source: &ObjectEmissionInput,
            rows: &[ObjectBoundarySettlement],
        ) -> Result<(), Error> {
            let mut cursor = 0;
            for placed in &source.text_section().functions {
                let settlements = boundary_settlements(source, placed.machine)?;
                let fragment = fragment(source, placed.machine)?;
                for located in settlements {
                    let actual = rows
                        .get(cursor)
                        .ok_or(Error::Mismatch("missing structural settlement"))?;
                    cursor += 1;
                    let selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(
                        expected,
                    ) = &located.settlement
                    else {
                        match located.settlement {
                            selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. } => byte_input::validate(source, placed.machine, located, actual)?,
                            selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. } => process_exit::validate(source, placed.machine, located, actual)?,
                            _ => byte_output::validate(source, placed.machine, located, actual)?,
                        }
                        continue;
                    };
                    let row = &actual.settlement;
                    validate_settlement_position(fragment, located, row.code_offset)?;
                    if actual.machine != placed.machine
                        || actual.text_offset
                            != placed_text_offset(placed.section_offset, row.code_offset)?
                        || row.invocation_site != expected.operation.into()
                        || row.boundary != expected.boundary
                        || row.execution
                            != machine_code::BoundaryExecutionRecord::AdmittedProvider(
                                expected.provider_execution.into(),
                            )
                        || row.realization
                            != target_operations::BoundaryRealization::ClaimCompletionOnly(
                                expected.realization,
                            )
                        || row.arguments != expected.arguments
                        || row.completion_claim_sources != expected.completion_claim_sources
                        || row.completion_receipts != expected.completion_receipts
                        || !row.scalar_arguments.is_empty()
                        || !row.runtime_scalar_arguments.is_empty()
                        || !row.byte_sequence_arguments.is_empty()
                        || !row.native_result.is_unit()
                        || row.operation_ordinal
                            != attribution::ordinal(
                                source::publication_function(source, placed.machine)?,
                                SemanticCodeSite::Operation(expected.operation),
                            )?
                        || row.byte_count != 0
                        || row.completion_provider_custody.len()
                            != expected.completion_receipts.len()
                    {
                        return Err(Error::Mismatch("structural settlement custody"));
                    }
                    for (custody, receipt) in row
                        .completion_provider_custody
                        .iter()
                        .zip(&expected.completion_receipts)
                    {
                        let expected_source = expected
                            .completion_claim_sources
                            .iter()
                            .find(|source| source.claim() == receipt.claim)
                            .ok_or(Error::Mismatch("missing completion source"))?;
                        if custody.source != *expected_source
                            || custody.receipt != *receipt
                            || custody.provider_execution != expected.provider_execution.into()
                        {
                            return Err(Error::Mismatch("completion provider custody"));
                        }
                    }
                    crate::object::object_artifact::replay::boundary::completion_receipts::validate_completion_custody(row, source.emission_facts().semantic.admission.entry_facts().identities())
                        .map_err(|_| Error::Mismatch("completion receipts"))?;
                }
            }
            if cursor != rows.len() {
                return Err(Error::Mismatch("foreign structural settlement"));
            }
            Ok(())
        }

        pub(in crate::function_fragments) fn validate_settlement_attributions(
            source: &ObjectEmissionInput,
            machine: MachineId,
            rows: &[SemanticCodeAttribution],
        ) -> Result<Vec<SemanticCodeAttribution>, Error> {
            let settlements = boundary_settlements(source, machine)?;
            let fragment = fragment(source, machine)?;
            let invalid =
                || Error::Mismatch("settlement attribution differs from admitted operation");
            let mut sites = Vec::new();
            for located in settlements {
                let site = semantic_code_site(located.settlement.invocation_site());
                if sites.contains(&site) {
                    return Err(invalid());
                }
                sites.push(site);
                let mut matching = rows.iter().filter(|row| row.site == site);
                let row = matching.next().ok_or_else(invalid)?;
                let expected_byte_count = if matches!(
                    located.settlement,
                    selected_instructions::SelectedBoundarySettlementPayload::HostedWriteByteI32 { .. }
                        | selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. }
                        | selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. }
                ) {
                    fragment
                        .instruction_span(located.anchor)
                        .ok_or_else(invalid)?
                        .bytes
                        .len()
                } else {
                    0
                };
                if matching.next().is_some()
                    || row.byte_count != expected_byte_count
                    || row.operation_ordinal
                        != attribution::ordinal(
                            source::publication_function(source, machine)?,
                            site,
                        )?
                {
                    return Err(invalid());
                }
                validate_settlement_position(fragment, located, row.code_offset)?;
            }
            Ok(rows
                .iter()
                .filter(|row| {
                    !settlements.iter().any(|located| {
                        matches!(
                            located.settlement,
                            selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(_)
                        ) && row.site == semantic_code_site(located.settlement.invocation_site())
                    })
                })
                .copied()
                .collect())
        }

        fn validate_settlement_position(
            fragment: &machine_code::FunctionFragment,
            located: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
            proposed_offset: usize,
        ) -> Result<(), Error> {
            if super::settlement_offset(fragment, located)? != proposed_offset {
                return Err(Error::Mismatch(
                    "settlement position has no exact physical anchor",
                ));
            }
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use calling_conventions::{
                IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
            };
            use machine_code::StructuralSourceLocation;
            use terminal_psi::StructuralAccess;

            use super::validate_incoming_location;

            fn borrowed(location: IndirectPointerLocation) -> StructuralSourceLocation {
                StructuralSourceLocation::IncomingBorrowedPointer { location }
            }

            #[test]
            fn opaque_home_replay_requires_its_exact_pointer_and_frame_evidence() {
                let register = MachineRegister::X86Rdx;
                let incoming = IndirectPointerLocation::Register(register);
                let opaque = StructuralSourceLocation::IncomingOpaquePointer { location: incoming };
                let mut placement = ValuePlacement {
                    shape: ValueShape::integer(8, 8),
                    locations: vec![ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size: 8,
                    }],
                };
                assert!(
                    validate_incoming_location(StructuralAccess::SharedBorrow, &placement, opaque)
                        .is_ok()
                );
                assert!(
                    validate_incoming_location(
                        StructuralAccess::SharedBorrow,
                        &placement,
                        borrowed(incoming)
                    )
                    .is_err()
                );
                placement.shape = ValueShape::borrowed_reference(16, 8);
                assert!(
                    validate_incoming_location(StructuralAccess::SharedBorrow, &placement, opaque)
                        .is_err()
                );
            }

            #[test]
            fn borrowed_register_and_stack_locations_preserve_exact_pointer_residence() {
                for access in [
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::MutableBorrow,
                    StructuralAccess::WriteOnlyBorrow,
                ] {
                    let register = MachineRegister::Aarch64X(0);
                    let placement = ValuePlacement {
                        shape: ValueShape::borrowed_reference(4, 2),
                        locations: vec![ValueLocation::Register {
                            register,
                            value_byte_offset: 0,
                            byte_size: 8,
                        }],
                    };
                    assert!(
                        validate_incoming_location(
                            access,
                            &placement,
                            borrowed(IndirectPointerLocation::Register(register))
                        )
                        .is_ok()
                    );
                    assert!(
                        validate_incoming_location(
                            access,
                            &placement,
                            borrowed(IndirectPointerLocation::Register(
                                MachineRegister::Aarch64X(1)
                            ))
                        )
                        .is_err()
                    );
                    assert!(
                        validate_incoming_location(
                            access,
                            &placement,
                            StructuralSourceLocation::IncomingIndirectPointer { register }
                        )
                        .is_err()
                    );
                    for stack_byte_offset in [0, 8, 32, 56] {
                        let pointer = IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment: 8,
                        };
                        for location in [
                            ValueLocation::Stack {
                                stack_byte_offset,
                                value_byte_offset: 0,
                                byte_size: 8,
                                alignment: 8,
                            },
                            ValueLocation::Indirect {
                                pointer,
                                copy_stack_byte_offset: None,
                                byte_size: 4,
                                alignment: 2,
                            },
                        ] {
                            let placement = ValuePlacement {
                                shape: ValueShape::borrowed_reference(4, 2),
                                locations: vec![location],
                            };
                            assert!(
                                validate_incoming_location(access, &placement, borrowed(pointer))
                                    .is_ok()
                            );
                            assert!(
                                validate_incoming_location(
                                    access,
                                    &placement,
                                    borrowed(IndirectPointerLocation::Stack {
                                        stack_byte_offset: stack_byte_offset + 8,
                                        alignment: 8
                                    })
                                )
                                .is_err()
                            );
                            assert!(
                                validate_incoming_location(
                                    access,
                                    &placement,
                                    StructuralSourceLocation::Stack {
                                        byte_offset: stack_byte_offset
                                    }
                                )
                                .is_err()
                            );
                        }
                    }
                }
            }

            #[test]
            fn borrowed_locations_reject_copies_carrier_width_alignment_and_access_substitution() {
                let pointer = IndirectPointerLocation::Register(MachineRegister::Aarch64X(0));
                let mut placement = ValuePlacement {
                    shape: ValueShape::borrowed_reference(16, 8),
                    locations: vec![ValueLocation::Indirect {
                        pointer,
                        copy_stack_byte_offset: None,
                        byte_size: 16,
                        alignment: 8,
                    }],
                };
                let location = borrowed(pointer);
                assert!(
                    validate_incoming_location(
                        StructuralAccess::WriteOnlyBorrow,
                        &placement,
                        location
                    )
                    .is_ok()
                );
                assert!(
                    validate_incoming_location(StructuralAccess::Owned, &placement, location)
                        .is_err()
                );
                for malformed in [
                    ValueLocation::Indirect {
                        pointer,
                        copy_stack_byte_offset: Some(0),
                        byte_size: 16,
                        alignment: 8,
                    },
                    ValueLocation::Indirect {
                        pointer,
                        copy_stack_byte_offset: None,
                        byte_size: 8,
                        alignment: 8,
                    },
                    ValueLocation::Indirect {
                        pointer,
                        copy_stack_byte_offset: None,
                        byte_size: 16,
                        alignment: 4,
                    },
                    ValueLocation::Register {
                        register: MachineRegister::Aarch64X(0),
                        value_byte_offset: 1,
                        byte_size: 8,
                    },
                    ValueLocation::Register {
                        register: MachineRegister::Aarch64X(0),
                        value_byte_offset: 0,
                        byte_size: 4,
                    },
                ] {
                    placement.locations[0] = malformed;
                    assert!(
                        validate_incoming_location(
                            StructuralAccess::WriteOnlyBorrow,
                            &placement,
                            location
                        )
                        .is_err()
                    );
                }
                placement.locations = vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                }];
                placement.shape = ValueShape::integer(8, 8);
                assert!(
                    validate_incoming_location(
                        StructuralAccess::WriteOnlyBorrow,
                        &placement,
                        location
                    )
                    .is_err()
                );
            }

            #[test]
            fn owned_indirect_location_keeps_owned_copy_meaning() {
                let register = MachineRegister::Aarch64X(0);
                let placement = ValuePlacement {
                    shape: ValueShape::integer(16, 8),
                    locations: vec![ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(register),
                        copy_stack_byte_offset: Some(0),
                        byte_size: 16,
                        alignment: 8,
                    }],
                };
                let mut stack = placement.clone();
                let [ValueLocation::Indirect { pointer, .. }] = stack.locations.as_mut_slice()
                else {
                    panic!("one indirect parameter");
                };
                *pointer = IndirectPointerLocation::Stack {
                    stack_byte_offset: 32,
                    alignment: 8,
                };
                let location = StructuralSourceLocation::IncomingIndirectStackPointer {
                    stack_byte_offset: 32,
                    alignment: 8,
                };
                assert!(
                    validate_incoming_location(StructuralAccess::Owned, &stack, location).is_ok()
                );
                stack.shape = ValueShape::integer(8, 8);
                assert!(
                    validate_incoming_location(StructuralAccess::Owned, &stack, location).is_err()
                );
                assert!(
                    validate_incoming_location(
                        StructuralAccess::Owned,
                        &placement,
                        StructuralSourceLocation::IncomingIndirectPointer { register }
                    )
                    .is_ok()
                );
                assert!(
                    validate_incoming_location(
                        StructuralAccess::Owned,
                        &placement,
                        borrowed(IndirectPointerLocation::Register(register))
                    )
                    .is_err()
                );
                assert!(
                    validate_incoming_location(
                        StructuralAccess::SharedBorrow,
                        &placement,
                        borrowed(IndirectPointerLocation::Register(register))
                    )
                    .is_err()
                );
            }
        }
    }

    use super::{Error, attribution, host, source};
    use crate::object_input::ObjectEmissionInput;
    use crate::{ObjectBoundarySettlement, ObjectFunction};
    use machine_code::{
        SemanticCodeAttribution, SemanticCodeSite, StructuralSourceLocation, semantic_code_site,
    };
    use semantic_vocabulary::MachineId;
    pub(super) use validation::{
        validate_function, validate_settlement_attributions, validate_settlements,
    };

    pub(super) fn fragment(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<&machine_code::FunctionFragment, Error> {
        source::fragments(source)
            .function(machine)
            .ok_or(Error::Mismatch("missing ordinary function fragment"))
    }
    pub(super) fn requires_call_graph_replay(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<bool, Error> {
        Ok(!super::internal_calls::plan(source, machine)?
            .unpublished_calls
            .is_empty())
    }

    fn boundary_settlements(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<&[machine_code::boundary_settlements::PhysicalBoundarySettlement], Error> {
        source
            .emission_facts()
            .physical
            .boundary_settlements
            .function(machine)
            .map_err(Error::Mismatch)
    }

    fn settlement_offset(
        fragment: &machine_code::FunctionFragment,
        row: &machine_code::boundary_settlements::PhysicalBoundarySettlement,
    ) -> Result<usize, Error> {
        let mut blocks = fragment
            .blocks
            .iter()
            .filter(|block| block.block == row.block);
        let block = blocks
            .next()
            .ok_or(Error::Mismatch("settlement has no physical block"))?;
        if blocks.next().is_some() {
            return Err(Error::Mismatch("settlement has duplicate physical blocks"));
        }
        let mut spans = block
            .instructions
            .iter()
            .filter(|span| span.instruction == row.anchor);
        let span = spans
            .next()
            .ok_or(Error::Mismatch("settlement has no physical anchor"))?;
        if spans.next().is_some() {
            return Err(Error::Mismatch("settlement has duplicate physical anchors"));
        }
        host(span.offset)
    }

    pub(crate) fn placed_text_offset(
        section_offset: u64,
        code_offset: usize,
    ) -> Result<usize, Error> {
        host(section_offset)?
            .checked_add(code_offset)
            .ok_or(Error::Overflow)
    }

    pub(super) fn populate(
        source: &ObjectEmissionInput,
        function: &mut ObjectFunction,
        rows: &[SemanticCodeAttribution],
    ) -> Result<(), Error> {
        let plan = super::internal_calls::publication(source, function.machine)?;
        let (parameters, homes) = if function.mixed_structural_scalar_abi.is_some() {
            (
                &mut function.scalar_structural_parameters,
                &mut function.scalar_structural_parameter_homes,
            )
        } else {
            (
                &mut function.unit_parameters,
                &mut function.unit_parameter_homes,
            )
        };
        parameters.extend_from_slice(&plan.parameters);
        homes.extend_from_slice(&plan.homes);
        function
            .internal_unit_calls
            .extend(super::internal_calls::place_calls(
                source,
                function.machine,
                rows,
            )?);
        Ok(())
    }
    pub(super) fn settlement_attributions(
        source: &ObjectEmissionInput,
        machine: MachineId,
    ) -> Result<Vec<SemanticCodeAttribution>, Error> {
        let settlements = boundary_settlements(source, machine)?;
        let fragment = fragment(source, machine)?;
        settlements
            .iter()
            .filter(|row| {
                matches!(
                    row.settlement,
                    selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(_)
                )
            })
            .map(|row| {
                let site = semantic_code_site(row.settlement.invocation_site());
                Ok(SemanticCodeAttribution {
                    site,
                    operation_ordinal: attribution::ordinal(
                        source::publication_function(source, machine)?,
                        site,
                    )?,
                    code_offset: settlement_offset(fragment, row)?,
                    byte_count: 0,
                })
            })
            .collect()
    }
    pub(super) fn settlements(
        source: &ObjectEmissionInput,
    ) -> Result<Vec<ObjectBoundarySettlement>, Error> {
        let mut result = Vec::new();
        for placed in &source.text_section().functions {
            let settlements = boundary_settlements(source, placed.machine)?;
            let fragment = fragment(source, placed.machine)?;
            for located in settlements {
                let selected_instructions::SelectedBoundarySettlementPayload::ClaimCompletion(row) =
                    &located.settlement
                else {
                    result.push(match located.settlement {
                        selected_instructions::SelectedBoundarySettlementPayload::HostedReadByte { .. } => byte_input::settlement(source, placed.machine, located)?,
                        selected_instructions::SelectedBoundarySettlementPayload::HostedExitProcessI32 { .. } => process_exit::settlement(source, placed.machine, located)?,
                        _ => byte_output::settlement(source, placed.machine, located)?,
                    });
                    continue;
                };
                let offset = settlement_offset(fragment, located)?;
                let execution = machine_code::BoundaryExecutionRecord::AdmittedProvider(
                    row.provider_execution.into(),
                );
                result.push(ObjectBoundarySettlement {
                    machine: placed.machine,
                    text_offset: placed_text_offset(placed.section_offset, offset)?,
                    settlement: machine_code::BoundarySettlementRecord {
                        invocation_site: row.operation.into(),
                        boundary: row.boundary,
                        execution,
                        realization: target_operations::BoundaryRealization::ClaimCompletionOnly(
                            row.realization,
                        ),
                        scalar_arguments: Vec::new(),
                        runtime_scalar_arguments: Vec::new(),
                        arguments: row.arguments.clone(),
                        byte_sequence_arguments: Vec::new(),
                        completion_claim_sources: row.completion_claim_sources.clone(),
                        completion_receipts: row.completion_receipts.clone(),
                        completion_provider_custody:
                            machine_code::derive_completion_provider_custody(
                                execution,
                                &row.completion_claim_sources,
                                &row.completion_receipts,
                            )
                            .ok_or(Error::Mismatch("completion custody"))?,
                        native_result: machine_code::BoundaryResultRecord::Unit,
                        operation_ordinal: attribution::ordinal(
                            source::publication_function(source, placed.machine)?,
                            SemanticCodeSite::Operation(row.operation),
                        )?,
                        code_offset: offset,
                        byte_count: 0,
                    },
                });
            }
        }
        Ok(result)
    }
}

pub(crate) mod validation;

pub(crate) use production::ProjectedObject;
pub use production::prepare_object_for_image;

#[derive(Debug)]
pub enum FunctionFragmentObjectArtifactError {
    PhysicalSourceObservation(String),
    CallableInvocation {
        detail: String,
    },
    Source(object_file::RelocationFreeObjectContainerError),
    Unsupported(&'static str),
    UnaccountedOperation {
        machine: semantic_vocabulary::MachineId,
        operation_ordinal: usize,
        operation_description: String,
    },
    Mismatch(&'static str),
    PrivateEntryRecipeConflict {
        identity: function_identity::MachineFunctionIdentity,
        detail: String,
    },
    InstalledRelativeBranches {
        machine: semantic_vocabulary::MachineId,
        source: String,
    },
    InstalledFaultSurface {
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        detail: String,
    },
    InstalledMemorySource {
        machine: semantic_vocabulary::MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
        reason: &'static str,
    },
    InstallationHelperReference {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    InstallationResultDefinition {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    StructuralSourceHome {
        machine: semantic_vocabulary::MachineId,
        place: semantic_vocabulary::PlaceId,
        reason: &'static str,
    },
    InstallationBacking {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        argument: Option<usize>,
        reason: &'static str,
        detail: Option<String>,
    },
    AccessBackingGeometry {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        reason: &'static str,
    },
    PrivateFunctions(crate::ObjectStackError),
    ForeignCalls(object_file::ForeignCallPlacementError),
    NativeEntryBridge(physical_recipes::native_entry_bridge::NativeEntryBridgeError),
    PhysicalEntry(diagnostics::Diagnostic),
    NativeCodeLayout(machine_code::code_layout::NativeCodeLayoutError),
    Overflow,
}

impl std::fmt::Display for FunctionFragmentObjectArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhysicalSourceObservation(detail) => formatter.write_str(detail),
            Self::CallableInvocation { detail } => {
                write!(formatter, "callable invocation custody: {detail}")
            }
            Self::Source(error) => write!(formatter, "shared object replay: {error}"),
            Self::NativeEntryBridge(error) => write!(formatter, "native entry transport: {error}"),
            Self::PhysicalEntry(error) => error.fmt(formatter),
            Self::Unsupported(reason) | Self::Mismatch(reason) => formatter.write_str(reason),
            Self::PrivateEntryRecipeConflict { identity, detail } => write!(
                formatter,
                "private entry has conflicting original admission recipes for {identity:?}: {detail}"
            ),
            Self::InstalledRelativeBranches { machine, source } => write!(
                formatter,
                "installed handler {machine:?} relative control: {source}",
            ),
            Self::InstalledFaultSurface {
                machine,
                instruction,
                detail,
            } => write!(
                formatter,
                "installed handler {machine:?} instruction {instruction:?} has no software-fault exclusion: {detail}"
            ),
            Self::InstalledMemorySource {
                machine,
                operation,
                reason,
            } => write!(
                formatter,
                "installed handler {machine:?} operation {operation:?} has no closed RAM source: {reason}"
            ),
            Self::InstallationHelperReference {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "installation checked helper receives a different runtime reference at {machine:?}/{operation:?}: {detail}",
            ),
            Self::InstallationResultDefinition {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "installation refusal result is not preserved at {machine:?}/{operation:?}: {detail}",
            ),
            Self::StructuralSourceHome {
                machine,
                place,
                reason,
            } => write!(
                formatter,
                "structural source home at {machine:?}/{place:?}: {reason}",
            ),
            Self::InstallationBacking {
                machine,
                operation,
                argument,
                reason,
                detail,
            } => {
                write!(
                    formatter,
                    "installation activation backing at {machine:?}/{operation:?}, operand {argument:?}: {reason}"
                )?;
                if let Some(detail) = detail {
                    write!(formatter, "; {detail}")?;
                }
                Ok(())
            }
            Self::AccessBackingGeometry {
                machine,
                operation,
                reason,
            } => write!(
                formatter,
                "placed backing geometry at {machine:?}/{operation:?}: {reason}",
            ),
            Self::UnaccountedOperation {
                machine,
                operation_ordinal,
                operation_description,
            } => write!(
                formatter,
                "shared function {machine:?} contains an unaccounted operation at ordinal {operation_ordinal}: {operation_description}",
            ),
            Self::PrivateFunctions(error) => {
                write!(formatter, "compiler-private function custody: {error}")
            }
            Self::ForeignCalls(error) => {
                write!(formatter, "foreign-call custody: {error}")
            }
            Self::NativeCodeLayout(error) => {
                write!(formatter, "selected native instruction geometry: {error:?}")
            }
            Self::Overflow => {
                formatter.write_str("shared object coordinate exceeds supported size")
            }
        }
    }
}
impl From<installation_admission::cpu::Error> for FunctionFragmentObjectArtifactError {
    fn from(error: installation_admission::cpu::Error) -> Self {
        use installation_admission::cpu::Error as Source;
        match error {
            Source::Mismatch(reason) => Self::Mismatch(reason),
            Source::Unsupported(reason) => Self::Unsupported(reason),
            Source::SelectedOperand(reason) => reason.into(),
        }
    }
}

impl From<installation_admission::selected::Error> for FunctionFragmentObjectArtifactError {
    fn from(error: installation_admission::selected::Error) -> Self {
        use installation_admission::selected::Error as Source;
        match error {
            Source::Mismatch(reason) => Self::Mismatch(reason),
        }
    }
}
impl std::error::Error for FunctionFragmentObjectArtifactError {}

use FunctionFragmentObjectArtifactError as Error;

fn host(value: u64) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::Overflow)
}

pub(crate) fn refuse_symbol_collision(
    object: &object_file::ObjectPlan,
    name: &str,
    reason: &'static str,
) -> Result<(), Error> {
    if object.layout.symbols.iter().any(|(_, s)| s.name == name) {
        return Err(Error::Mismatch(reason));
    }
    Ok(())
}

pub(crate) use validation::validate_available as validate_available_object;

impl From<installation_admission::ram::Error> for FunctionFragmentObjectArtifactError {
    fn from(error: installation_admission::ram::Error) -> Self {
        match error {
            installation_admission::ram::Error::Mismatch(reason) => Self::Mismatch(reason),
            installation_admission::ram::Error::InstalledMemorySource {
                machine,
                operation,
                reason,
            } => Self::InstalledMemorySource {
                machine,
                operation,
                reason,
            },
        }
    }
}

impl From<object_file::body_object::Error> for FunctionFragmentObjectArtifactError {
    fn from(error: object_file::body_object::Error) -> Self {
        use object_file::body_object::Error;
        match error {
            Error::Mismatch(reason) => Self::Mismatch(reason),
            Error::Unsupported(reason) => Self::Unsupported(reason),
            Error::Overflow => Self::Overflow,
            Error::NativeCodeLayout(error) => Self::NativeCodeLayout(error),
            Error::ForeignCalls(error) => Self::ForeignCalls(error),
        }
    }
}
