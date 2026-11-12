# Validation subjects

Named decision: `validation-subjects`, ratified 2026-11-08.

A check fully validates the packages selected as its subjects and checks the
complete semantic dependency closure those declarations require, freshly or by
[exact-key reuse of a dependency slice](#what-this-rule-does-not-do).
Everything else is outside the check's claim. A passed check therefore means
exactly: every declaration of the subject packages is valid, and every
declaration they depend on is valid.

## Subjects

The subject set is decided by the request, not by where source lives:

| Request | Subjects |
| --- | --- |
| Check or build an application or package | That package |
| Check a workspace with no member named | Every declared member |
| Build or check one workspace member | That member |
| Admit, update or audit a package revision | That revision |
| Any of the above with `Build::validate` rows | The product plus each named dependency |

Being bundled with the toolchain, vendored, located in the workspace or
editable does not make a package a subject. A dependency is a dependency.

## Closure

The closure starts from every declaration of every subject, including unused
ones, plus the Build and preparation entries the compile executes, the product
entries, and the obligations of the Build's selections. From there it follows
calls, callable values, callbacks, initialization and cleanup, types and their
invariants, contracts, proofs and lemmas, selected conformances with their
required operations and laws, layouts, and target bindings. An unused subject
declaration that depends on an invalid dependency declaration fails the check.

Target obligations are exactly what the Build's selections commit to: each
declared target row demands its selected closure, including helpers and
callbacks, under [provider selection](provider_selection.md#build-owned-target-selection).
Sharing a target directory is not a dependency.

## Orchestration

The subject is request information and survives every internal re-rooting.
Executing a dependency's Build demands that Build's closure and nothing more;
it never promotes the dependency to a subject. Generated source belongs to the
package that owns it and enters a check the way that package does.

The saving comes from scheduling: establish the declaration environment needed
for correct name resolution, then check demanded declarations and add
dependencies as checking discovers them. A deferred body stays in name
resolution, so deferral cannot remove an overload candidate, hide an ambiguity
or change what a name denotes. A demanded declaration receives every ordinary
check: a generic body under its requirements, a selected conformance with its
laws, a lemma with valid evidence. Closure completeness is the correctness
obligation of this rule: no fact used to accept a product may come from an
unchecked declaration.

## Build::validate

```omega
machine build(builder: &mut Build) {
    Build::application(builder, "app");
    Build::dependency(builder, wrapper, locator);
    Build::dependency(builder, core, locator);
    Build::validate(builder, wrapper);
    Build::validate(builder, core);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
```

`Build::validate` records a validation requirement: the named registered
dependency is a subject of every check of this product. It only widens the
subject set. The argument is a dependency this Build registered directly; a
transitive dependency must be registered to be named. Ownership follows the
root Build activation: a helper acting on the root's supplied `Build` may add
subjects, a dependency's separate Build activation cannot enlarge its
consumer's set. The requirement is satisfied by the ordinary checking pipeline
before success is reported; it does not launch a compilation from inside Build
execution. The check report lists its subjects.

## Admission and release

A package revision is checked in full whenever it is a subject: during its own
development, in a workspace check, and when a project admits, updates or
audits it. Full-package validation covers the revision's complete declared
inventory and the obligations its declarations state, including unused
declarations; an invalid declaration is a validation failure, not a
discretionary acceptance choice. Admission validates the revision being
admitted; a consumer's edit does not promote its unchanged dependencies to
subjects. A corpus of programs is not a full-package check. The toolchain's
own release validation checks each bundled library as a subject.

## What this rule does not do

A consumer checks every dependency declaration it needs, and the full-package
gate is a claim about the library, not evidence a consumer consumes in place of
checking. One reuse is allowed: the checked result of a dependency slice may be
kept on local disk and loaded by a later compile when its key matches exactly.
The key covers the content of exactly the dependency sources and declarations
the slice contains and depends on, the identity of the compiler that produced
the result, and the target. A different key, or an entry that cannot be read
or does not validate, means a fresh check; nothing is reused on a partial
match. The consumer's own package is never reused. Changing one dependency
file invalidates only the slices that contain or depend on it, and a compiler
with a different identity invalidates every entry. Obligations that depend on
the Build's provider or target selections are not part of a reused slice and
are checked in every compile. `--verify` withholds trust acceptance
and re-verifies under retained source pins; it does not widen the subject set.
Optimization settings change how a valid program is realized, never which
declarations a check claims. There is no setting that narrows a subject below
its full declaration set. Checked-tree pruning still runs only after the
subjects and their closure are checked, within that declared scope.
