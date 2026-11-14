# Validation subjects

Named decision: `validation-subjects`, ratified 2026-11-08.
Registration-handle and explicit package-root rules completed 2026-11-12.

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
| Any of the above with `Build::validate` rows | The product plus each selected registered dependency in its checking context |

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

`Build::validate(builder: &mut Build, dependency: &DependencyRegistration)`
borrows an exact dependency registration and records a full-package validation
request. It performs no alias-string lookup and adds no dependency identifier
syntax. Both Product and Build dependencies registered directly by the supplied
root Build are eligible, in their corresponding checking contexts.

```omega
machine build(builder: &mut Build) {
    Build::application(builder, "app");
    let mut description = dependency::from_source(Source::Path { location: "producer" });
    dependency::with_alias(&mut description, "producer");
    let registered = Build::dependency(builder, description);
    Build::validate(builder, &registered);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
```

The registration is an immutable reference to one coordinator registration in
its owning Build activation, including its Build/Product purpose scope. It must
carry real registration identity through the existing opaque Build-handle
mechanism, not depend on variable names, equal empty records or an independently
constructed `{}`. Aliases may coincide across purpose scopes without identifying
the registrations. This is the same registration identity used by
[dependency preparation](scoped_execution.md#dependency-declarations-and-binding),
not a second registry for validation.

Registration values are copyable immutable handles within their owning
activation. Copies and borrows retain the same identity; preparation's by-value
argument does not consume the caller's registration. Validation can therefore
be requested before or after preparation, without retargeting the registration
or replaying Build statements. A handle from a different or expired activation
is not valid for the supplied root builder, and it is not a persistent portable
identity. Possessing or constructing a same-shaped value supplies no authority.

A root helper may use the root's actual `Build` and registration to add the
request; a dependency's separate activation cannot enlarge its consumer's set.
The operation only widens the subjects. It neither prepares/executes the
dependency immediately nor recursively launches a compilation inside Build.
Ordinary preparation and checking satisfy the request before successful
completion. A registration identifies what to check, not a certificate that
the package has passed, and grants no provider, filesystem or trust authority.

Repeated requests are idempotent for the same resolved immutable dependency
instance and checking context, not merely the same alias, path or source
revision. That context retains purpose and applicable execution profile/target.
When Build completes, retain that resolved subject identity with the request,
not a transient coordinator handle. Transitive dependencies enter through their
ordinary demanded semantic closure; selecting one dependency does not make
every transitive dependency a full-package subject. Register a dependency
directly to request its full check. The check report lists its subjects and
checking contexts.

## Explicit package roots

The ordinary explicit check request can select an existing bundled package:

```bash
omega --check source/library/core/main.omg
omega --check source/library/std/main.omg
omega --check source/library/alloc/main.omg
```

Recognize the selected package's existing owner, immutable source revision and
dependency relationships. Do not reimport its sources into a new application,
mint another core identity, or infer toolchain privilege from a directory name.
Ordinary `std` and `alloc` use ordinary package-root checking regardless of
whether their source was distributed with the compiler.

For core, narrowly admit the explicitly selected existing toolchain package as
the validation subject while preserving its toolchain owner and compiler-owned
declaration rules. Other root/source isolation checks remain; checking core
does not install, approve or select replacement toolchain primitives. The
current implementation's overlap guard is passed the core subtree, not the
entire library bundle, so this exception must not be generalized into special
bundled-package kinds or unsupported claims about `std`/`alloc` overlap.

No magic `"core"` alias is added to `Build::validate`. A product requests checks
through registrations; a direct package check selects the package as its root.
The [ordinary-library boundary](declarations.md#ordinary-library-packages)
separates distribution from package ownership and compiler custody.

## Admission and release

A package revision is checked in full whenever it is a subject: during its own
development, in a workspace check, and when a project admits, updates or
audits it. Full-package validation covers the revision's complete declared
inventory and the obligations its declarations state, including unused
declarations; an invalid declaration is a validation failure, not a
discretionary acceptance choice. Admission validates the revision being
admitted; a consumer's edit does not promote its unchanged dependencies to
subjects. A corpus of programs is not a full-package check. The toolchain's
own release validation explicitly checks each library it distributes as a
subject through the ordinary package-root route, with the narrow core rule
above. This checks declarations and their obligations; it does not execute
every declaration or treat a compiler-realized operation as an ordinary source
body. Its exact primitive realization obligations still apply.

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
its full declaration set.
