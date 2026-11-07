# Cutting stock orders (NEW071)

This complete Omega CLI plans four synthetic manufacturing unit orders against
exactly two identical fresh bars and an optional existing remnant. It reads one
bounded line, exhaustively optimizes fulfillment, and emits the selected cut
patterns, fulfilled quantities, leftovers, and a material conservation certificate.
It imports only stable bundled console, binding, and numeric conversion facilities.
All planning and parsing algorithms and storage belong to this application.

The scientific sample model uses integer millimetres. Every produced piece costs
its requested length plus one kerf, including a piece that ends exactly at a bar's
end. Stock has no defects and all orders have equal priority. Lengths are synthetic;
this is a bounded exact model, not a production shop prescription.

Input is exactly `SSS RRR K LLL LLL LLL LLL`, followed by LF or EOF. `SSS` is
1..999 for each of the two fresh bars; `RRR` is 0..999, with zero meaning absent;
`K` is 0..9; each order length is 1..999. All fields have the displayed fixed
width. Four orders represent four unit quantities; duplicate lengths still have
separate order identities. Extra bytes, CRLF, malformed numbers, zero stock,
zero orders, missing fields, empty input, and full input buffers reject with exit
64. An infeasible order is valid input and remains unfulfilled. No interactive
pause is required.

`planner.omg` checks all 4^4 labeled assignments, allowing each order to use stock
0, 1, 2, or remain unfulfilled (3). The lexicographic objective maximizes fulfilled
count, then useful length, then minimizes fresh bars opened, then opened material.
The smallest base-4 assignment code breaks ties; order 0 is the least significant
digit. `ties` counts all optimal labeled assignments, including symmetry between
the two fresh bars. Arithmetic uses declared Wrapping carriers; input and search
bounds keep actual calculations far below signed overflow. No floating values or
numerical tolerance are used.

Output contains four-digit decimal integers with one trailing space per field:

| Row | Fields |
| --- | --- |
| summary | fulfilled units, useful length, fresh bars opened, opened material, ties, assignment code |
| order | order identity, requested length, assigned stock (3 means unfulfilled) |
| stock | stock identity, available capacity, cut count, useful length, kerf loss, opened leftover |
| certificate | opened material, useful length, total kerf, reusable leftovers, scrap |

The four order rows identify each stock's actual cut pattern; stock cut counts
are fulfilled quantities. An untouched stock reports zero opened leftover and
keeps its capacity available. Opened leftover >=5 is reusable; smaller leftover
is scrap. The certificate covers opened material only, and the Omega entrypoint
checks `opened = useful + kerf + reusable + scrap`, exiting 70 on internal failure.
No plan satisfies impossible orders by inventing material.

The existing `inventory_lookup` and `inventory_system` samples search and total
fixed inventories; `insertion_sort` sorts constants; `maze_flood` computes shortest
paths; `alarm_scheduler` applies scripted events. Their real semantic overlap is
bounded records, guarded array access, and state transitions. None models kerf,
optimizes capacity-constrained manufacturing orders, handles unfulfilled units,
reuses remnants, counts tied packings, or certifies material conservation. A search
of existing samples for cutting stock, bin packing, knapsack, kerf, and remnant
found no corresponding workflow before authoring.

`reference.py` independently partitions feasible order subsets stock by stock;
it does not follow the Omega assignment-code search. Its expectations were frozen
before any Omega check/native/Terminal execution. `data/frozen.json` contains 991
cases: 972 exhaustive tiny inputs (fresh 3..5, remnant 0 or 3, kerf 0 or 1, four
lengths independently 1..3), nine named cases, and ten invalid runtime cases.
`reusable_leftover` leaves eight millimetres on stock 0. The original
`remnant_reuse` and `reuse_round2` fixtures test available remnants that the
objective leaves untouched. A separate pre-execution freeze,
`data/frozen_reuse.json`, adds `remnant_required` and `reuse_actual_round2`: the
latter feeds those eight millimetres into a second batch with fresh stock 6 and
verifies actual remnant cuts. Both extra fixtures assert that stock 2 is opened.
The complete validation inventory has 993 cases. Supplemental frozen SHA-256: `0bd8d5341a852594f1352b7fc1ce17f65eaf04b894df4a8d516c47b47c5fee44`. Named `.stdin` and
`.stdout` files expose exact fixtures. Tolerance is zero. Frozen SHA-256:
`8725271f0cb3cc403e695898157d925baaebaebe74045178229bc3ac2b6d9f33`.
The `reference.py --freeze` and `--freeze-reuse` commands refuse to overwrite their existing oracles; validation never
regenerates expected output from observed results.

From the repository root, use the compiler binary built for this checkout and
its matching library. `OMEGA` below names that binary:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
"$OMEGA" update --project samples/cli/independent/cutting_stock_orders --target linux_x86_64
# Read the generated ordinary package review, accept only reviewed decisions,
# then resume using the documented package review workflow.
"$OMEGA" update --resume --project samples/cli/independent/cutting_stock_orders
"$OMEGA" --check --target linux_x86_64 samples/cli/independent/cutting_stock_orders/main.omg
"$OMEGA" --target linux_x86_64 --build-dir samples/cli/independent/cutting_stock_orders/build/native samples/cli/independent/cutting_stock_orders/main.omg
"$OMEGA" run --both samples/cli/independent/cutting_stock_orders/main.omg < samples/cli/independent/cutting_stock_orders/data/kerf_ties.stdin
"$OMEGA" --check samples/cli/independent/cutting_stock_orders/controls/invalid_contract/main.omg
python3 samples/cli/independent/cutting_stock_orders/validate.py --executable /absolute/path/to/produced/executable --output samples/cli/independent/cutting_stock_orders/build/native-validation
python3 samples/cli/independent/cutting_stock_orders/validate.py --omega "$OMEGA" --both --named-only --output samples/cli/independent/cutting_stock_orders/build/terminal-validation
```

The separately complete invalid source calls a cut requiring sufficient stock
with stock 4 and requested length 5. Acceptance requires rejection specifically
for the disproved call requirement. A parsing, entry, package, custody, or other
rejection is a wrong-reason failure. Ordinary package review is a prerequisite
and a denial is recorded without retrying acceptance. The sample declares only
the validated Linux x86-64 entry; other target routes are not advertised.
Generated output belongs under ignored `build/`; validation records include
actual commands, exits, output hashes, and explicit Terminal declines.

Validation via `omega run --both` establishes exact **native stdout** and
Terminal **exit agreement**. The CLI forwards native stdout and compares only
exit status; this sample makes no claim of exact Terminal stream parity.

Compiler regression boundaries
------------------------------

The application uses published main's bounded-byte loan carrier registration and
validated split/spill recovery for
repeated scalar division across Console writes. Every rewritten instruction must
receive a legal home, and malformed classes or unsupported tied topology remain
errors. A complete decimal formatting regression requires exact five-byte stdout
independently from native and Terminal. The application retains its original
division/remainder formatter and its frozen output oracles.

An earlier exploratory dynamic-captured-parent/static-byte-field loan witness
was rejected on the tested e09-derived compiler. Its complete source, exact
revision and original rejection evidence are retained in the review bundle as a
historical boundary, outside passing corpus fixtures. The application uses whole
local buffer loans.
