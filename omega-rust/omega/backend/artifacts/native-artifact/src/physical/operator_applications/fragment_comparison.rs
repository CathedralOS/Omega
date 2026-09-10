//! Comparison spans from the immutable, independently replayed fragment object.
use super::*;
use image_emission::ObjectCodeAttribution;
use machine_code::SemanticCodeSite;
use semantic_vocabulary::{MachineId, OperationId};

pub(super) fn derive(
    occurrence: &OptimizedOperatorOccurrence,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    // The caller admits only exact intrinsic IEEE comparisons after the checked
    // Terminal/provider join, and only under FragmentPublicationBinding. That
    // binding replays the entire object (including attribution and spill bytes).
    let mut functions = object
        .functions()
        .iter()
        .filter(|function| function.machine == occurrence.machine());
    let function = functions
        .next()
        .ok_or("D29 comparison names an absent object function")?;
    if functions.next().is_some() {
        return Err("D29 comparison names duplicate object functions");
    }
    let (offset, length) = comparison_interval(
        occurrence.machine(),
        occurrence.operation(),
        occurrence.operation_ordinal(),
        function.text_offset,
        function.byte_count,
        object.semantic_code_attribution(),
    )?;
    let start = function
        .text_offset
        .checked_add(offset)
        .ok_or("D29 comparison text offset overflow")?;
    let end = start
        .checked_add(length)
        .ok_or("D29 comparison text end overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                start,
                end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("D29 comparison direct bytes contain a relocation");
    }
    derive_span(
        function,
        offset,
        length,
        object,
        image,
        PhysicalRelocationDisposition::DirectInstructionBytes,
        None,
    )
    .map(Some)
}

fn comparison_interval(
    machine: MachineId,
    operation: OperationId,
    ordinal: usize,
    function_offset: usize,
    function_length: usize,
    rows: &[ObjectCodeAttribution],
) -> Result<(usize, usize), &'static str> {
    let site = SemanticCodeSite::Operation(operation);
    let mut first = None;
    let mut end = 0usize;
    for row in rows
        .iter()
        .filter(|row| row.machine == machine && row.attribution.site == site)
    {
        let attribution = &row.attribution;
        let row_end = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .ok_or("D29 comparison interval overflow")?;
        if attribution.operation_ordinal != ordinal
            || attribution.byte_count == 0
            || row_end > function_length
            || function_offset.checked_add(attribution.code_offset) != Some(row.text_offset)
            || first.is_some() && attribution.code_offset < end
        {
            return Err("D29 comparison changed its exact interval coordinates");
        }
        first.get_or_insert(attribution.code_offset);
        end = row_end;
    }
    let first = first.ok_or("D29 comparison lacks semantic code attribution")?;
    let start = function_offset
        .checked_add(first)
        .ok_or("D29 comparison start overflow")?;
    let limit = function_offset
        .checked_add(end)
        .ok_or("D29 comparison end overflow")?;
    // Selected comparison instructions form one sequence. Allocation may insert
    // private spill accesses between attributed intervals; full object replay
    // authenticates those gaps. Never absorb another authored operation or edge,
    // even a zero-width control marker, into this one-operation child.
    for row in rows {
        if row.machine == machine && row.attribution.site == site {
            continue;
        }
        let row_end = row
            .text_offset
            .checked_add(row.attribution.byte_count)
            .ok_or("D29 neighboring attribution overflow")?;
        if ranges_overlap(start, limit, row.text_offset, row_end)
            || row.attribution.byte_count == 0 && start < row.text_offset && row.text_offset < limit
        {
            return Err("D29 comparison interval absorbs another semantic site");
        }
    }
    Ok((first, end - first))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(offset: usize, length: usize) -> ObjectCodeAttribution {
        ObjectCodeAttribution {
            machine: MachineId::new(1).unwrap(),
            attribution: machine_code::SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(OperationId::new(2).unwrap()),
                operation_ordinal: 3,
                code_offset: offset,
                byte_count: length,
            },
            text_offset: 100 + offset,
        }
    }
    fn check(rows: &[ObjectCodeAttribution]) -> Result<(usize, usize), &'static str> {
        comparison_interval(
            MachineId::new(1).unwrap(),
            OperationId::new(2).unwrap(),
            3,
            100,
            64,
            rows,
        )
    }
    #[test]
    fn comparison_child_covers_all_intervals_and_private_spill_gaps() {
        assert_eq!(check(&[row(8, 12)]), Ok((8, 12)));
        assert_eq!(check(&[row(8, 12), row(24, 16)]), Ok((8, 32)));
        assert_eq!(check(&[row(8, 12), row(20, 16)]), Ok((8, 28)));
    }
    #[test]
    fn comparison_child_rejects_hostile_coordinates_and_foreign_sites() {
        assert!(check(&[]).is_err());
        for mutation in 0..8 {
            let mut rows = vec![row(8, 12), row(24, 16)];
            match mutation {
                0 => rows[0].attribution.operation_ordinal += 1,
                1 => rows[0].text_offset += 1,
                2 => rows[0].attribution.byte_count = 0,
                3 => rows[1] = row(16, 16),
                4 => rows[1] = row(60, 16),
                5 => rows.swap(0, 1),
                6 => rows[1].attribution.code_offset = usize::MAX,
                _ => rows
                    .iter_mut()
                    .for_each(|row| row.machine = MachineId::new(9).unwrap()),
            }
            assert!(check(&rows).is_err(), "coordinate mutation {mutation}");
        }
        for (site, width) in [
            (SemanticCodeSite::Operation(OperationId::new(7).unwrap()), 4),
            (
                SemanticCodeSite::Edge(semantic_vocabulary::EdgeId::new(7).unwrap()),
                0,
            ),
        ] {
            let mut foreign = row(21, width);
            foreign.attribution.site = site;
            assert!(check(&[row(8, 12), row(24, 16), foreign]).is_err());
        }
    }
}
