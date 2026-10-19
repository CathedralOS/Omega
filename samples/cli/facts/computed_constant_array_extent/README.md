# Computed constant array extent completion

MODULE-NAMESPACE-RESOLUTION: an array initializer and its alias use the exact
completed value of the selected computed extent. The unchanged root computes
`SIZE=1+0`, `SOURCE=[3+0]`, then observes `ALIAS[0]==3` in native and Terminal
execution. A wrong observation crashes, so exit zero asserts the value 3.

The frozen clean compiler at `53cd2938437a29356a48d6eaf29f14de07a4bb99`
refused the root at `array extent constant is not a completed integer literal`.
The repair schedules declared-type dependencies before canonical value encoding,
retains symbolic pending values privately, and rejoins exact declaration custody.
It does not use provisional aggregate values as dependency evidence.

`composition` exercises transitive extents, empty and nested arrays, record field
geometry, a closed array result and a match join. `module_owner` selects the
imported producer's extent despite a consumer shadow. `qualified_element` keeps
a qualified literal element behind pending geometry. All bind the four hosted
ProgramEntry contracts. Execution evidence is Linux x86_64 only.

Controls change one obligation: `controls/mismatch` adds an initializer element,
`controls/index` changes the index to 1, and `controls/qualified_element` changes
the qualified element value from 3 to 4. They must reject cardinality, bounds and
the original default-domain predicate respectively. The root and controls are
pinned in `tests/omega/{pass,fail}/facts`; compositions are pinned under modules.

These are mechanism-based witnesses, not a nine-place completion claim. Static
formation and exact module identity feed ordinary checking and checked-to-Terminal
lowering; actual both-engine executions exercise provider settlement, concrete
array layout, calling/activation realization and native ELF emission. No new
provider, ABI, loan or cleanup mechanism is introduced. Untouched target bindings
do not establish other-host execution.

Governing contracts: [constants](../../../../wiki/spec/language/constants.md),
[evaluation](../../../../wiki/spec/language/evaluation.md),
[counts and addresses](../../../../wiki/spec/language/counts_and_addresses.md),
[modules](../../../../wiki/spec/language/modules.md). Broader module evaluation
remains open, including specialized provider/target applications, abstract/open
carriers and other structured proof subjects. Literal-array argument evaluation,
array-indexed record predicates and array `.len` observations have separate
baseline limitations; these witnesses do not claim those mechanisms.

Constant evaluation of generic constructors through named const arguments remains
unsupported; explicit literal generic construction is preserved. This is outside
the completed declaration-array readiness mechanism.
