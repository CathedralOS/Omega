# Stoichiometric batch balance (NEW051)

A complete Omega application that reads a declared elemental model and reagent
inventory from stdin, balances the reaction with bounded positive integers,
processes one synthetic batch, and reports residual and product inventories.
All algorithms and storage are private to this sample. Its only dependency is
the existing standard Console provider; it does not import a numerics library.

The input contains exactly 19 unsigned decimal fields, separated by ASCII space,
tab, CR or LF, and no more than 1024 bytes:

```
species_count coefficient_bound
species0_axis0 species0_axis1 species0_axis2
species1_axis0 species1_axis1 species1_axis2
species2_axis0 species2_axis1 species2_axis2
species3_axis0 species3_axis1 species3_axis2
inventory0 inventory1 inventory2 inventory3 extent_cap
```

There are 3 or 4 species, three explicitly declared elemental axes, and exactly
two reactants (columns 0 and 1). The remaining columns are products. A three
species reaction must leave column 3 and its inventory zero. Every active species
has at least one atom. Atom counts range from 0 to 16, coefficient bound from 1
to 8, and inventories/cap from 0 to 2147483647. Negative numbers, extra fields,
nondecimal input and numbers beyond that limit are runtime errors.

The models are declarations, not molecular formula parsing or a claim of chemical
kinetics, thermodynamics, real laboratory safety, or fractional-mole accuracy.
The three axes may name different elements in different independent fixtures.
Water uses H/O/unused; methane combustion uses H/O/C; ammonia H/unused/N;
iron oxide Fe/O/unused. Inventories use synthetic integer formula-unit counts.

`chemistry/balance.omg` visits the complete bounded coefficient cube, checks all
three independent exact elemental equations and excludes tuples whose gcd is
not one. A match of calls selects the three-species or four-species signature
record. A second match selects the reactant or product half of the coefficient
row; both halves have length 2, and that length is read back through a local,
a record field, a parameter, and the call result. The reactant write uses the
same match under a mutable loan. It counts primitive solutions and selects minimum total coefficient
sum, breaking ties lexicographically. `NO_BALANCE_WITHIN_BOUND` is a bounded
search result; it does not assert impossibility outside that declared bound.
The impossible fixture has nonzero nitrogen only on the product side, so its
impossibility also follows independently from the third elemental equation.

`chemistry/inventory.omg` takes the minimum whole-batch reagent capacity and
requested cap. `limiting_mask` uses bit 1 for reactant 0 and bit 2 for reactant 1;
it identifies tied integer reagent capacities before applying the cap.
`cap_limited` indicates a stricter cap. All products are proposed before any
inventory mutation; an i32 inventory overflow reports the unchanged inventory.
`Main::main` independently checks total elemental conservation before reporting.

All arithmetic uses i64 carriers with bounds that exclude wrapping: each
balance side is at most 256, product proposals at most 9*INT32_MAX, and elemental
inventory totals at most 64*INT32_MAX. No floating values or numerical tolerances
are involved. Exact integer and output-byte tolerances are zero.

The frozen Python oracle in `validation/reference.py` enumerates positive
compositions in increasing total using arbitrary precision, rather than the
Omega radix enumeration. `validation/frozen_oracle.json` authenticates each
fixture input/output. Those expectations were authored before Omega execution.
Validation verifies their hashes and never updates them to match a result.

From the repository root, after the ordinary documented package review:

```
OMEGA_LIBRARY_ROOT="$PWD/source/library" omega update --project samples/cli/independent/stoichiometric_batch_balance --target linux_x86_64
# Read and explicitly accept only the intended pending review decisions.
omega update --resume --project samples/cli/independent/stoichiometric_batch_balance
omega --check --target linux_x86_64 samples/cli/independent/stoichiometric_batch_balance/main.omg
omega run --both --keep samples/cli/independent/stoichiometric_batch_balance/main.omg < samples/cli/independent/stoichiometric_batch_balance/data/water_first.stdin
omega --target linux_x86_64 --build-dir samples/cli/independent/stoichiometric_batch_balance/build/native samples/cli/independent/stoichiometric_batch_balance/main.omg
python3 samples/cli/independent/stoichiometric_batch_balance/validation/verify.py --omega /absolute/path/to/omega --mode native --executable /absolute/path/to/produced/application
python3 samples/cli/independent/stoichiometric_batch_balance/validation/verify.py --omega /absolute/path/to/omega --mode both
omega --check --target linux_x86_64 samples/cli/independent/stoichiometric_batch_balance/controls/intended_invalid/main.omg
```

The invalid source is a separate complete application. Its required rejection
specifically states that index `3` is outside length 3. Package review, borrow,
type, syntax or entry failures do not satisfy that control. A declined Terminal
route is reported explicitly and does not count as a comparison pass.

Exit codes: 0 balanced/conserved; 64 invalid input; 65 no balance within bound;
66 atomic overflow refusal; 70 failed conservation. Compiler rejection is a
separate failure, never one of these successful application outcomes.

Semantic overlap review: `collections/matrix_multiply` demonstrates matrix dot
products; `collections/inventory_system` populates, searches and totals a fixed
inventory; `systems/bank_ledger` sums transactions; `algorithms/horner_eval`
evaluates one polynomial. None performs primitive exact reaction enumeration,
multiple-balance selection, reagent capacity or atomic product overflow refusal.
The source search for chemical, stoichiometric and reagent workflows found none.

Only Linux x86_64 is declared and validated here. Generated compiler products,
logs, locks and review state belong under ignored local build locations or
local package files. The review handoff records the exact binary, base, commands,
source hashes, outcomes and any remaining package-review blockers.

The application exposed a current-main arithmetic checking gap: an indexed
coefficient's positive `requires` fact did not reach division definedness.
The bounded repair retains the collection and selector as a private ordered
operand, checks their exact state scope when admitting requirements, and
invalidates their facts on either write. A scalar local copies the live
literal bounds and exclusions of its evaluated observation before destination
invalidation, preserving nonzero facts through dynamic-index hoisting. The application retains its original
preconditions and divisions. Feature-shaped controls under
`tests/omega/{pass,fail}/arithmetic/indexed_divisor_*` pin valid contracts,
state forwarding, storage mutation, selector mutation and different collections.
The generic CLI comparison verifies Terminal exits only; native stdout is checked
byte for byte against the frozen fixtures. Terminal stdout comparison is not
exposed by this CLI route and is not claimed.
