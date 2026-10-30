# Borrowed rank composition: disjoint write

A private helper preserves the invocation rank on a tail edge, and every complete cycle strictly descends while advancing the explicit counter once.

Owner: `NESTED-MACHINE-UNIFICATION`; this is a composition of the
[borrowed-value row](../README.md), with the same seven-step and empty inputs,
counter3→10 and caller-visible original-record/canary checks.
[Positive](main.omg) must agree in native and Terminal execution.
[Twin](invalid/main.omg) removes only descent and must refuse at the joint rank.
