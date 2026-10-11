"""Independent oracle: combinations of present group IDs, not 256 masks.
Frozen outputs must be generated before Omega execution. Integer tolerance = 0.
"""
from itertools import combinations

def reference(numerator, denominator, records):
    groups = sorted({group for _, group, _ in records})
    totals = [sum(stratum == s for _, _, stratum in records) for s in (1, 2, 3)]
    candidates = []
    for size in range(1, len(groups)):
        for subset in combinations(groups, size):
            counts = [sum(group in subset and stratum == s for _, group, stratum in records) for s in (1, 2, 3)]
            errors = [abs(denominator*t - numerator*n) for t,n in zip(counts, totals)]
            mask = sum(2**(group-1) for group in subset)
            candidates.append(((sum(errors), abs(denominator*sum(counts)-numerator*len(records)), mask), subset, counts, errors))
    if not candidates:
        return 65, "ERROR indivisible_single_group\n"
    (score,total_error,mask), subset,counts,errors = min(candidates)
    lines = [f"mask {mask:03}", f"train {len(records)-sum(counts):03} test {sum(counts):03}", f"score {score:03} total_error {total_error:03}"]
    lines += [f"id {id:03} group {group:03} {'test' if group in subset else 'train'}" for id,group,stratum in sorted(records)]
    lines += [f"stratum {s:03} total {n:03} test {t:03} error {e:03}" for s,n,t,e in zip((1,2,3),totals,counts,errors)]
    lines += ["balance exact" if score == 0 else "balance impossible_exact", "leakage 000"]
    return 0, "\n".join(lines)+"\n"
