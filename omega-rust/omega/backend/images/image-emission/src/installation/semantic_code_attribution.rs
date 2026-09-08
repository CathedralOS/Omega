//! Installed interval geometry; physical membership remains the admitted image's custody.
use super::{InstallationError, ObjectCodeAttribution, SemanticCodeSite};
use semantic_vocabulary::{MachineId, OperationId};

pub(super) fn validate_order(rows: &[ObjectCodeAttribution]) -> Result<(), InstallationError> {
    let mut previous_key = None;
    let mut sites = std::collections::BTreeMap::new();
    for row in rows {
        let attribution = row.attribution;
        let key = (row.machine, attribution.operation_ordinal, row.text_offset);
        if previous_key.is_some_and(|previous| previous >= key) {
            return Err(InstallationError::NonCanonicalSemanticCodeAttributionOrder);
        }
        let end = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        if let Some((ordinal, previous_end)) = sites.insert(
            (row.machine, attribution.site),
            (attribution.operation_ordinal, end),
        ) && (matches!(attribution.site, SemanticCodeSite::Edge(_))
            || ordinal != attribution.operation_ordinal
            || previous_end >= attribution.code_offset)
        {
            return Err(InstallationError::DuplicateSemanticCodeAttributionSite {
                machine: row.machine,
                site: attribution.site,
            });
        }
        previous_key = Some(key);
    }
    Ok(())
}

pub(super) fn contains_call(
    rows: &[ObjectCodeAttribution],
    machine: MachineId,
    operation: OperationId,
    operation_ordinal: usize,
    code_offset: usize,
    byte_count: usize,
    function_byte_count: usize,
) -> bool {
    let Some(call_end) = code_offset
        .checked_add(byte_count)
        .filter(|end| *end <= function_byte_count)
    else {
        return false;
    };
    let mut containing = 0;
    for row in rows.iter().filter(|row| {
        row.machine == machine && row.attribution.site == SemanticCodeSite::Operation(operation)
    }) {
        let attribution = row.attribution;
        let Some(end) = attribution
            .code_offset
            .checked_add(attribution.byte_count)
            .filter(|end| *end <= function_byte_count)
        else {
            return false;
        };
        if attribution.operation_ordinal != operation_ordinal {
            return false;
        }
        if attribution.code_offset <= code_offset && call_end <= end {
            containing += 1;
        }
    }
    containing == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::SemanticCodeAttribution;

    fn interval(offset: usize, length: usize) -> ObjectCodeAttribution {
        ObjectCodeAttribution {
            machine: MachineId::new(1).unwrap(),
            text_offset: 100 + offset,
            attribution: SemanticCodeAttribution {
                site: SemanticCodeSite::Operation(OperationId::new(2).unwrap()),
                operation_ordinal: 3,
                code_offset: offset,
                byte_count: length,
            },
        }
    }

    #[test]
    fn separated_operation_intervals_keep_one_exact_call_container() {
        let rows = [interval(0, 4), interval(12, 9)];
        validate_order(&rows).unwrap();
        let contains = |rows: &[ObjectCodeAttribution], offset, size| {
            contains_call(
                rows,
                MachineId::new(1).unwrap(),
                OperationId::new(2).unwrap(),
                3,
                offset,
                size,
                24,
            )
        };
        assert!(contains(&rows, 16, 5));
        assert!(
            !contains(&rows, 6, 5),
            "unattributed gap cannot contain call"
        );
        assert!(
            !contains(&rows, 2, 12),
            "two intervals cannot jointly cover a call"
        );
        assert!(
            !contains(&rows[..1], 16, 5),
            "call interval cannot disappear"
        );
        assert!(
            !contains(&[rows[1].clone(), rows[1].clone()], 16, 5),
            "duplicate containers reject"
        );
        let mut wrong_ordinal = rows.clone();
        wrong_ordinal[0].attribution.operation_ordinal += 1;
        assert!(!contains(&wrong_ordinal, 16, 5));
        let mut out_of_bounds = rows;
        out_of_bounds[0].attribution.byte_count = 25;
        assert!(!contains(&out_of_bounds, 16, 5));
    }

    #[test]
    fn repeated_sites_require_same_ordinal_disjoint_maximal_intervals() {
        for rows in [
            vec![interval(0, 4), interval(4, 5)],
            vec![interval(0, 4), interval(3, 5)],
            vec![interval(0, 4), interval(0, 4)],
            vec![interval(12, 4), interval(0, 4)],
        ] {
            assert!(validate_order(&rows).is_err());
        }
        let mut different_ordinal = [interval(0, 4), interval(12, 5)];
        different_ordinal[1].attribution.operation_ordinal += 1;
        assert!(validate_order(&different_ordinal).is_err());
        let mut edge = [interval(0, 0), interval(12, 0)];
        for row in &mut edge {
            row.attribution.site =
                SemanticCodeSite::Edge(semantic_vocabulary::EdgeId::new(2).unwrap());
        }
        assert!(validate_order(&edge).is_err());
    }
}
