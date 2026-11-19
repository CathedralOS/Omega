Frozen before any Omega check or execution, on 2026-10-11.

All values are integers; tolerance is exactly zero. Expected exit and stdout
bytes (including final LF and matrix trailing spaces) must match expected.json.
The Python reference enumerates every labelled product of treatment permutations
and independently counts period columns and adjacent directed pairs. It chooses
the lexicographically least sorted allocation. A second tiny raw-cell enumeration
includes invalid repeated-treatment rows and proves the permutation restriction
preserves exactly the feasible labelled designs for 12 tiny parameter sets.

The application instead enumerates sorted sequence multisets. For six subjects
and six permutations it visits at most C(11,6)=462 complete candidates; every
labelled allocation has one representative because all constraints are invariant
under subject relabeling. It does not optimize or infer treatment effectiveness.

Input: four single ASCII digits T,N,C,F separated by commas and terminated by LF.
T is treatment/period count (2 or 3); N subjects (1..6); C is maximum count of
each directed adjacent pair (0..6). F bans one edge: 0 none, 1 AB, 2 AC, 3 BA,
4 BC, 5 CA, 6 CB. With two treatments only F=0,1,3 are admitted. One complete
line is consumed; EOF, CRLF, extra bytes in that line and unsupported values
reject with exit 64. A valid but infeasible request returns exit 2.

Each subject receives every treatment exactly once. Every period must have N/T
subjects per treatment, and all directed carryover counts must be <=C with the
banned edge zero. Count bounds are <=6, index bounds <18, and cursor bounds <=6;
Wrapping i32 counters cannot wrap in the admitted finite search.
