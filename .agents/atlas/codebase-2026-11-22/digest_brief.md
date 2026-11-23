# Digest brief: one area of the Omega codebase atlas

You write the digest for one area. Your inputs are that area's crate map (`crate-map/<area>.md`) and
every record that covers a file in the area (`records/unit_NNN.md` and second-pass
`records/unit_NNN_gap_MMM.md`; `units.tsv` and `second_pass_ranges.tsv` say which). Read all of them. Then
write a digest that a person who knows the project but not this area can read in one sitting, and that
corrects the records where they are wrong.

Read-only except your one digest file. Do not build or run anything. Use file reads, git and grep.
Checkout: `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

## What to write

Write `.agents/atlas/codebase-2026-11-22/digests/<area>.md` with the Write tool. Start with one line naming
the records it was built from and the commit. Then these sections, in order:

1. **What this area does.** One paragraph for the area, then one short paragraph per crate or stage: what
   comes in, what goes out, lines, and the function where the work happens (`file:line`).
2. **How it decides.** The judgments that matter, grouped by what they are keyed on (identity, place,
   value, position, spelling), with counts per crate where the records give them.
3. **The same job in more than one place.** Merge the records' entries; keep the citations.
4. **Fixed lists of shapes.** The places that accept a fixed list of program shapes and refuse the rest.
5. **Accepts when it does not know.** Every fail-open place the records report, with its class (stated,
   traced, open). Do not upgrade a claim's certainty.
6. **Silent drops.** Places where something that cannot be planned, resolved or lowered produces no row
   and no diagnostic.
7. **Unused or unwired.** With the grep counts the records give.
8. **Stated limits.** Grouped by topic.
9. **Disagreements between records**, and between the records and the crate map.
10. **What I checked myself.** Open at least eight of the area's claims in the code, choosing the ones
    most likely to mislead (an "unused", a "same job", an "open" fail-open). For each: the claim, what the
    code shows at `file:line`, and matched / partly right / wrong. Correct wrong claims in the sections
    above and say so.

## Rules

- Every claim keeps its `file:line`. Keep the readers' words "suspected", "inferred" and "not traced".
- Counts are of places cited, not of defects.
- Plain words, no praise, no recommendations, no redesign.

## What to return

One line: the digest's path, how many records it merged, and the matched / partly right / wrong counts
from section 10.
