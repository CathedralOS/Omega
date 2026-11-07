# Frozen independent oracle

Frozen before the first application check/native/Terminal execution. All inputs
are synthetic nondimensional loads, not service-life predictions. Tolerance is
zero for cycle order, range, half-unit weight, rational numerator/denominator,
cumulative numerator, stdout bytes and process status. There is no floating
point computation or numerical tolerance to tune after execution.

Rule: remove consecutive equal values, then interior monotone observations.
On the resulting reversal stream push each value; examine the final four
(a,b,c,d). Close the full cycle (b,c) when |b-c| <= |a-b| AND
|b-c| <= |c-d|, delete b,c and repeat. At EOF every adjacent residual
pair is a half cycle. Equality closes. This explicitly declared four-point
rule is the sample model; no claim of standards conformance is made.

Synthetic material curve N(R)=C/R^p with p in {1,2}, C in [1,1000].
Each emitted row contributes h*R^p/(2*C), h=2 full or h=1 half. Miner
sum uses the common denominator 2*C, intentionally unreduced for auditing.
Zero histories have no cycles and zero damage. No mean-stress correction.

Hand traces:
- Nested [0,9,2,7,4,6,0]: 4->6 closes (range 2); then 2->7
  closes (range 5); residual 0->9->0 gives two range-9 halves.
  p=2: (8+50+81+81)/200=11/10; p=1: (4+10+9+9)/200=4/25.
- Crossing [0,8,2,6,4,9,0]: 6->4 closes (2), 8->2 closes (6),
  residual 0->9->0. (8+72+81+81)/200=121/100.
- Residual [1,7,3]: no enclosing four points; ranges 6 and 4,
  both half. (36+16)/100=13/25.
- Equal ranges [0,4,0,4,0]: the middle 4->0 closes by equality,
  residual 0->4->0: (32+16+16)/20=16/5.
- Plateau [0,0,3,9,9,2,2,6,0] reduces to [0,9,2,6,0].
  2->6 closes (4), residual 0->9->0: (32+81+81)/200=97/100.
- Monotone [1,1,2,4,6,8] reduces to [1,8]: 49/200.
- Descending [9,8,6,6,3,0] reduces to [9,0]: 81/200.
- Exactly 256 bytes with tab, CRLF and leading zeroes, [0,9]: 81/200.
- Constant [5,5,5,5] reduces to [5]: zero.
- Minimum [0,9], p=1,C=1: one half, 9/2.
- Capacity 16 alternating 0,9 values: seven full range-9 cycles
  followed by one half: (7*162+81)/2000=243/400.

All invalid input fixtures must emit exactly the documented error and exit 64.
The separate unguarded_index program must be rejected for an unproved array
subscript bound. Compilation rejection for package admission, effects or any
other rule is not a valid intended-invalid result.
