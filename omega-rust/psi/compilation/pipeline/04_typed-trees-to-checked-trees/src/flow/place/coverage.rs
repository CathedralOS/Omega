//! Structural extent coverage shared by live membership and predicate queries.
//! Coverage changes coordinates, never the lifetime or authority of a fact.

/// Whether a fact's segment chain covers the subject's elementwise: every
/// position equal under `canonical_place_segments_equal`, or the candidate's
/// `FixedRange` extent contains the subject's selector. `x[a..b]` covers a
/// literal `x[i]` when `a <= i < b`, a narrower `x[c..d]` when `a <= c` and
/// `d <= b`, and -- ONLY at the whole-extent `0..usize::MAX` row -- a runtime
/// `x[index]` (whichever element the selector names sits inside the complete
/// extent). A runtime selector is never itself a fact coordinate, and any
/// element write retires the covering row through the usual overlap
/// machinery before it can be consulted, so coverage cannot outlive its
/// evidence.
pub(crate) fn segments_cover_subject(
    program: &dyn ::validation::CheckingInput,
    candidate: &[facts::PlaceSegment],
    subject: &[facts::PlaceSegment],
) -> bool {
    candidate.len() == subject.len()
        && candidate
            .iter()
            .zip(subject.iter())
            .all(|(candidate, subject)| {
                crate::flow::canonical_place_segments_equal(program, *candidate, *subject)
                    || segment_covers_subject(program, *candidate, *subject)
            })
}

fn segment_covers_subject(
    program: &dyn ::validation::CheckingInput,
    candidate: facts::PlaceSegment,
    subject: facts::PlaceSegment,
) -> bool {
    let facts::PlaceSegment::FixedRange { start, end } = candidate else {
        return false;
    };
    if start >= end {
        return false;
    }
    match subject {
        facts::PlaceSegment::FixedIndex { index } => start <= index && index < end,
        facts::PlaceSegment::FixedRange {
            start: subject_start,
            end: subject_end,
        } => subject_start < subject_end && start <= subject_start && subject_end <= end,
        facts::PlaceSegment::Index { expression } => {
            // Only the complete extent covers an unresolved selector. A
            // constant expression narrows from any containing extent; a
            // non-literal selector is universal only inside `0..usize::MAX`.
            if start == 0 && end == usize::MAX {
                true
            } else {
                program
                    .expression_table
                    .constant_integer_value(expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some_and(|index| start <= index && index < end)
            }
        }
        _ => false,
    }
}
