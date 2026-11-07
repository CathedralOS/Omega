# Oracle freeze before Omega execution

The 70-case manifest SHA-256 is
`b41088fe8a26883f067b46fe02e1c7c8a7663ef1afda90ab059502adf26bc0cf`.
All stdout comparisons and integer values have tolerance zero. No expected
result may be regenerated from an observed Omega output.

Independent hand calculations constrain the reference program:

- Balanced: four groups each contain one record in strata 1 and 2. At 1/2,
  choosing two groups gives two test records in each stratum. All errors vanish;
  the smallest such mask is 3 (groups 1 and 2).
- Impossible: group 1 has counts (3,1,0), group 2 has (1,3,0). At 1/2,
  totals are (4,4,0). The only proper group subsets have scaled error
  `abs(2*3-4)+abs(2*1-4)=4`, total error 0. Mask 1 wins the tie.
- Singleton: groups 1 and 2 each contain (1,1,0), group 3 has (0,0,1).
  At 1/3, choosing group 1 gives errors (1,1,1), sum 3, total error 1,
  and mask 1. Exact balance in the singleton third stratum would require
  an integral test count equal to 1/3 and is impossible.
- Noncontiguous: totals (2,2,2), fraction 2/5. Each stratum's best integral
  test count is 1, with scaled error 1. Groups 3 and 5 together yield
  (1,1,1), score 3 and total error 3. Their mask is 4+16=20; the other
  subset giving these counts uses groups 1 and 8 with mask 129.
- Tiny two-record distinct-group singleton-stratum case: fraction 1/2,
  totals (1,1,0). Either group subset has score 2 and total error 0;
  mask 1 selects group 1. If both records share a group there is no
  nonempty proper group subset and the intended runtime exit is 65.

The 36 two-record layouts exhaust all four group layouts over IDs 1–2 and
all nine stratum layouts over IDs 1–3. The eight distinct-group three-record
layouts exhaust binary stratum assignments. Reference enumeration selects
combinations of present group IDs rather than walking the Omega mask loop.
