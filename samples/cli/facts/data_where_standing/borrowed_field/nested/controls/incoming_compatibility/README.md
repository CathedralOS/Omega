# Check the saved reference against the actual write destination

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

The helper saves `holder.counter.value`, writes 3 through a separate mutable
argument, then reads the saved reference and adds 70. The positive supplies two
different scalar fields. It expects 130, verifies the destination became 3,
and checks that both scalars' nonzero neighbors remain unchanged. It binds all
four hosted targets and expects exit 0.

The invalid twin changes only the destination argument from `right.value` to
`left.value`, which the saved reference also denotes. It must retain the
helper's borrow-conflict diagnostic. A separate caller diagnostic alone would
not prove that the helper's captured pointer was compared with its actual
write destination.

This is a control for the incoming-holder repair in the existing nested
borrowed-field fact, under **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Optimized
producer `b52ca03df8a90543384f55a5e7416a8ee8903e1e` rejects both programs at the
helper's write while `old` remains active. The invalid also rejects the
conflicting actual argument in main. Neither engine ran. The positive's source,
oracle and baseline diagnostic were frozen before the compatibility repair;
qualification was pending at that baseline.

Optimized producer `20fd129907b31f1ce92280ee5f22a50c0d17e867` runs the positive
through both engines with exits 0/0. The invalid retains the helper's intended
borrow-conflict diagnostic. Its corpus pass pin originally lacked the exit files,
so the corpus built row alone is compile-only; those files now require both
exits explicitly. This control does not complete the parent row's returned
reference and retirement acceptance.

Producer `6baea78fce36d968924dbd557d5b26650f902251` executes both the sample and
its corpus pin with native and Terminal exits 0/0. The alias twin still reports
the conflict inside the helper body. This qualifies this control, while the
parent row remains blocked on cleanup of replaced pointer fields.
