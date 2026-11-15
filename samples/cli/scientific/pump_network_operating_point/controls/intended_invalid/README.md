This independent complete source is expected to fail frontend checking. `branch_total`
requires a shared borrow of a two-element fixed array, but its caller supplies a
one-element array literal. `expected.txt` names this extent violation; unrelated
rejection is a failure. There is no application import and no package dependency or
native execution.
