# IEEE total-order satisfiers

The authored program checks signed zero and NaN payload ordering through direct
`F32::TotalOrder` / `F64::TotalOrder` calls and an `Order::before` satisfier.
Its success result is exit 70.

The public customer command is:

```sh
omega run --both tests/omega/pass/float/runtime_total_order_satisfiers_exit/main.omg
```

An unaccepted ordinary package review can stop this command before compilation
(exit 200). The repository corpus harness supplies its reviewed fixture policy:

```sh
python tools/corpus_gate.py --both --filter float/runtime_total_order_satisfiers_exit
```

Shared views of scalar parameters now survive checked local planning. The
companion `recast/runtime_scalar_parameter_shared_views_exit` program observes
an ordinary `&i32` parameter view and equal-width `f32`/`f64` integer bit views;
it exits 70 in both engines on Linux x86-64:

```sh
omega run --both tests/omega/pass/recast/runtime_scalar_parameter_shared_views_exit/main.omg
python tools/corpus_gate.py --both --filter recast/runtime_scalar_parameter_shared_views_exit
```

The total-order program still fails native lowering at `computed primitive
borrow lost its established local`; the checked interpreter exits 70. Passing
`left_bits` / `right_bits` to the key helpers needs an established backing home
for the scalar parameter and a retained shared representation-view loan of that
home. Stage 05 `ScalarBindings::primitive_borrow` consumes established structural
locals, while the checked scalar-view local currently retains only the scalar
read recipe. The next repair must connect that loan to its original storage and
replay the cross-carrier view; copying the bits into a different local would
change the source's backing identity. Keep the total-order source unchanged.

On base `9782e38b64ab`, a clean-source runner rejects the scalar-parameter
control at `read_bits64`'s first local while interpretation returns 70. The
same runner built with the admission repair executes that unchanged control
with native/interpreter exit 70. A focused base/candidate comparison of all
35 recast fixtures changes only this control from rejected to checked; all
negative expected fragments are satisfied. Three older golden mismatches
(`runtime_mutable_equivalent_domain_recast_exit`,
`runtime_mutable_equivalent_range_recast_exit`, and
`runtime_shared_domain_weakening_recast_exit`) also occur on the base at
`Unit graph dropped or added a body effect`.

Strict Clippy of the affected crate reports existing warnings at 17 untouched
source files (34 library warnings, 36 including tests). The dependency-enabled
run also stops on two existing `proof-admission` warnings. No reported lint
site is in the changed local planner. These are retained validation failures,
not passing checks.

The focused Rust selector
`test(execution::terminal_unit::calls::computation_arguments) or test(representation_view)`
runs 19 tests: 18 pass and
`a_bool_view_loan_read_spells_the_nonzero_image_of_its_integer_place` fails at
its missing guard-expression assertion. Rebuilding with the original compiler
source at `9782e38b64ab` reproduces that exact failure. The public total-order
command also confirms missing ordinary package-review acceptance (exit 200).
