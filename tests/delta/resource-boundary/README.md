# Delta resource-boundary gate

Run `sh tests/delta/resource-boundary/run.sh` from the repository root. The gate
materializes and pins the complete canonical compiler, then compiles 37 full
authored Delta sources through `DCREQ` profile 1 and the selected Gamma evaluator.
The host neither parses declarations nor injects counters or compiler rows.

[`function_rows.py`](function_rows.py) retains the three function-row controls
at D30's selected limit of 32,768:

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 32,768 distinct functions | 720,923 | Reject 11, source coordinate 38 |
| Same prefix plus new `f32768` | 720,945 | Incomplete 4, source coordinate 720,928, limit 32,768, requested 32,769 |
| Same prefix plus duplicate `f00000` | 720,945 | Reject 8, source coordinate 720,928 |

Each source begins with `(data Flag (Off) (On))`, which must consume no function
rows. Its first function is `(def f00000 () Missing 0)`, followed by 32,767
distinct ordinary definitions. At the exact boundary, the complete global
census must finish and declaration resolution must report `Missing` at byte 38.
An adjacent new name instead refuses its function-row allocation before that
later phase. A duplicate is not a new row and retains the earlier-phase
duplicate diagnosis. All three inputs are below the separate 4-MiB source limit.
Fixture sizes, SHA256 identities, and full 40-byte output frames are pinned.

[`constructor_rows.py`](constructor_rows.py) supplies five constructor-row
controls at D30's selected limit of 65,536:

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 65,536 distinct constructors in one data declaration | 720,953 | Reject 11, source coordinate 18 |
| Same constructor prefix plus new `C65536` | 720,964 | Incomplete 3, source coordinate 720,915, limit 65,536, requested 65,537 |
| Same constructor prefix plus duplicate `C00000` | 720,964 | Reject 7, source coordinate 720,915 |
| 32,768 constructors in `T`, then 32,769 in `U` | 720,974 | Incomplete 3, source coordinate 720,925, limit 65,536, requested 65,537 |
| Full `T`, then another `T` with new `C65536` | 720,971 | Reject 6, source coordinate 720,920 |

Every constructor source starts with `C00000 Missing` and ends with an ordinary
`main : Bytes -> Bytes`. At the exact boundary, the whole census completes and
declaration resolution rejects the unknown payload type at byte 18. The fresh
adjacent constructor instead refuses before that later phase; the duplicate
retains its identity diagnosis without requesting a row. Splitting the same
constructors between two data owners proves the counter is global, not per
type. The last control proves a duplicate type rejects before provisioning its
fresh constructor, even when the constructor table is full. All five cases are
below the separate source-byte provision. Their length, digest, source
coordinate, status, and complete 40-byte frame expectations are fixed in the
fixture owner; no expected diagnosis is obtained from compiler output.

[`type_rows.py`](type_rows.py) supplies four total-type-row controls at D30's
selected limit of 65,536, including the two built-in rows for `Int` and `Bytes`:

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 65,534 nominal types plus two built-in rows | 1,507,329 | Reject 11, source coordinate 21 |
| Same nominal prefix plus fresh `T65534` | 1,507,352 | Incomplete 2, source coordinate 1,507,296, limit 65,536, requested 65,537 |
| Same nominal prefix plus duplicate `T00000` | 1,507,352 | Reject 6, source coordinate 1,507,296 |
| Same nominal prefix plus fresh `T65534` containing duplicate `C00000` | 1,507,352 | Incomplete 2, source coordinate 1,507,296, limit 65,536, requested 65,537 |

Each prefix begins with `(data T00000 (C00000 Missing))`, followed by 65,533
distinct nominal types with one distinct constructor each. An ordinary
`main : Bytes -> Bytes` ends every source. The exact-boundary case reaches
declaration resolution and rejects `Missing` at byte 21. The adjacent fresh
type instead refuses its row before that later phase; a duplicate type retains
its identity diagnosis without requesting a row. The fourth case requires type
provision before inspecting that fresh type's duplicate constructor. There are
only 65,534 prefix constructors, and at most 65,535 authored constructor entries
in any case, below their separate 65,536-row boundary. All four sources are
below the 4-MiB source-byte limit. Literal sizes, digests, coordinates, and
complete 40-byte frames are independently pinned in the fixture owner.

## Active-environment fixture inventory

[`environment_rows.py`](environment_rows.py) defines eleven full authored
controls for D30's 65,536 active local rows through the same selected compiler
and unchanged diagnostic allowance.

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 65,536 parameters, then unknown result annotation | 1,114,135 | Reject 11 at 1,114,124 |
| Same parameters plus fresh `value65536 : Int` | 1,114,148 | Incomplete 5 at 1,114,124 |
| Same parameters plus duplicate `value00000 : Missing` | 1,114,152 | Reject 9 at 1,114,124 |
| Same parameters plus fresh `value65536 : Missing` | 1,114,152 | Reject 11 at 1,114,135 |
| Full parameter environment plus fresh let | 1,114,167 | Incomplete 5 at 1,114,133 |
| Full environment plus let with unknown annotation and duplicate name | 1,114,173 | Reject 11 at 1,114,144 |
| Full environment plus fresh pattern binder | 1,114,196 | Incomplete 5 at 1,114,174 |
| Full environment plus pattern binder conflicting with a parameter | 1,114,197 | Reject 9 at 1,114,174 |
| 65,535 parameters, then two repeated names in one pattern | 1,114,195 | Reject 10 at 1,114,173 |
| 65,535 parameters, nested initializer and sibling lets | 1,114,214 | Reject 20 at 5 |
| 65,535 parameters, disjoint one-binder match arms | 1,114,230 | Reject 20 at 44 |

Every resource frame uses source coordinate space 1, limit 65,536, and requested
65,537. Parameter conflict checking precedes its own annotation; successful
annotation resolution precedes fresh-row provision. Let annotations precede
conflict checking, and fresh-row provision precedes checking the initializer
against the unchanged outer environment. Pattern conflicts distinguish an
existing outer binder from a repeated binder within the same pattern; both
precede another row provision. Deliberately unknown initializers or arm bodies
must not displace the earlier refusal.

The two restoration controls allow one additional active binder at a time.
The nested initializer must use the old environment, sibling lets must restore
it, and each match arm must start from its saved outer environment. Their bodies
finish checking before the deliberately nonconforming `main` signature produces
schema code 20. This avoids requiring emission or generated Gamma admission of
a 65,535-parameter function. All sources remain below 4 MiB, use at most two
constructor payload fields, and have fixed byte lengths, SHA256 identities,
literal coordinate anchors, and full 40-byte outcome frames.

## Match-coverage fixture inventory

[`match_coverage.py`](match_coverage.py) defines four full authored controls
using all 65,536 globally admitted constructors of one nominal type. Constructor
names remain the six-byte `C00000` through `C65535`; every constructor is nullary.

| Authored source | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 65,536 distinct arms in declaration order | 1,310,762 | Reject 20 at 589,838 |
| Same complete arms, then another `C00000` arm | 1,310,773 | Reject 17 at 1,310,761 |
| Full inner match inside the sole arm of an incomplete outer match | 1,310,786 | Reject 18 at 589,850 |
| 65,536 distinct arms in reverse declaration order | 1,310,762 | Reject 20 at 589,838 |

The two exhaustive cases must finish body checking before the deliberately
nonconforming `main : () -> Int` reaches entry-schema rejection. The extra arm
must retain the duplicate-case diagnosis at its later constructor name. The
nested case keeps one outer arm active while checking all 65,536 inner arms;
the inner coverage set neither completes the outer set nor incurs an invented
aggregate 65,537-row refusal. After the inner body succeeds, the outer match
must reject its own missing coverage at its expression start. Source lengths,
SHA256 identities, literal coordinate anchors, and all 40 output bytes are
fixed independently of compiler output.

D110 defines an immutable exact-name coverage set local to each match and
permits arbitrary authored arm order. The separate global constructor limit
admits at most 65,536 constructors. Consequently, a fresh 65,537th same-owner
coverage member cannot arise: an extra pattern instead encounters constructor
identity, owner, arity, or duplicate-case checking. These controls establish
capacity reach, diagnostic precedence, and per-match isolation, not an invented
code-6 refusal or complete D30 resource closure.

## Syntax-storage fixture inventory

[`syntax_storage.py`](syntax_storage.py) defines ten authored controls for
D30's 114,294,752-byte cumulative syntax provision. The selected Gamma pair
occupies 40 bytes, so 2,857,368 complete pairs consume 114,294,720 bytes; the
remaining 32 bytes cannot hold another pair. No test injects a usage counter.

| Authored source construction | Bytes | Expected exact DCOUT |
| --- | ---: | --- |
| 357,169 empty lists, then `a a a` | 714,343 | Reject 4 at 1 after maximal aligned parser admission |
| Same source plus one empty list | 714,345 | Incomplete 7 at EOF 714,345, requested 114,295,040 |
| 952,457 opening parentheses | 952,457 | Incomplete 7 at 952,456, requested 114,294,840 |
| 714,342 copies of `a `, then `a` | 1,428,685 | Incomplete 7 at 1,428,684, requested 114,294,880 |
| One list containing 571,473 copies of `a ` | 1,142,948 | Incomplete 7 at opening 0, requested 114,294,880 |
| 59,528 copies of `(def f () Int 0)` | 952,448 | Reject 8 at 21 after grammar completes |
| 59,529 copies of the same definition | 952,464 | Incomplete 7 at 952,416, requested 114,294,760 |
| EOF-overflow source plus forbidden zero byte | 714,346 | Reject 3 at 714,345 before syntax parsing |
| EOF-overflow source plus unmatched closing parenthesis | 714,346 | Reject 4 at 714,345 before EOF provision |
| Empty list followed by 59,529 definitions | 952,466 | Reject 4 at 1 before later grammar provision |

Every Incomplete row has source coordinate space 1 and limit 114,294,752.
Expected requested amounts include the entire refused allocation group, not
just its first pair. Parser opening groups contain three frame pairs; atoms
contain three node pairs plus one reversed-spine pair; closing groups contain
the complete ordered child spine plus four node/parent-spine pairs. EOF
provisions the complete top-level ordered spine and one program-root pair.
Close failures retain the list's opening coordinate; EOF failures use source
extent. The unclosed-opening control proves an earlier provision can fail
before a later missing-close judgment.

For the balanced constructions, parser allocation is exactly `8L + 5A + 1`
pairs for `L` lists and `A` atoms. Each repeated definition has two lists and
four atoms: 36 parser pairs, then a 12-pair grammar frame group. At 59,528
definitions the combined provision is 114,293,800 bytes. With one more
definition, grammar starts after 2,143,045 parser pairs and refuses its
59,527th frame group at byte 952,416. These controls check cumulative accounting
across declarations and parser-to-grammar transfer. Phase outcome and custody wrappers
are not syntax allocations. The host only constructs these fixed source
families, checks lengths/digests, and compares full observations; it does not
parse them or compute the compiler's usage state.

The previous 27 fixtures remain unchanged. Their authored shapes stay below
this syntax provision: the largest full-match variant uses 102,240,760 bytes,
and adjacent full-type controls use 91,752,200 bytes. No prior boundary is
reduced or relabeled to make room for these controls.

Each evaluation uses the existing full-customer diagnostic allowance of 300
seconds. The gate prints elapsed time for each exact observation and reports a
raw evaluator failure or timeout without relabeling it as compiler
`Incomplete`. A selected evaluator heap or stack failure does not pass, and the
boundary is not reduced to accommodate it.

These controls test the type-, function-, constructor-, and
active-environment-row boundaries, cumulative syntax storage, and full
per-match coverage behavior.
They do not establish all D30 capacities, acceptance or emission of every
in-bound program, or closure of the Delta edge. Other frontend and request
behavior remains in the
[frontend](../frontend-boundary/README.md) and
[request](../request-boundary/README.md) gates.
