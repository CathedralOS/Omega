# case_subject_root_cleanup

A case dispatch on a field of an affine local (`item.kind`) observes the
field only; the record keeps custody. A forwarding arm leaves cleanup to
the successor's return, the `_` arm runs it on the edge, and a returning
arm runs it at the return — each disposition disposes once.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
