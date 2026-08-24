# Design Brief — Constants And Provisioned Entry State

> **Status:** Settled design; engineering incomplete. **Driver:** Omega needs
> immutable compile-time values and long-lived program state without ambient
> mutable names or a magical `main` receiver. **Depends on:**
> [`build_time_evaluation.md`](build_time_evaluation.md),
> [`build_and_package_model.md`](build_and_package_model.md), and
> [`freestanding_boot_and_hardware_facts.md`](freestanding_boot_and_hardware_facts.md).

## Bottom line

Omega separates three things:

1. `const` declares an immutable compile-time value with no storage identity.
2. A free program-entry machine requests no implicit state.
3. A program-entry machine with one `&mut self` receiver requests exactly one
   target-provisioned, program-lifetime receiver instance. That instance is
   reachable only through the entry borrow; it is not an ambient `static`.

There is no `static` keyword, no globally nameable mutable object, and no
special `main` spelling. `build.omg` binds a target-owned entry slot to an exact
machine symbol.

## Constants are pure values

A `const` is a named value evaluated in a constant position:

```omega
pub const PAGE_SIZE: u64 = 4096;
pub const EFI_SUCCESS: EfiStatus = EfiStatus { code: 0 };
pub const IMPORT_NAME: [u8; 9] = "WriteFile";
```

Constants may be free-standing or genuinely type-scoped:

```omega
const EfiStatus::SUCCESS = EfiStatus { code: 0 };
```

They are never data fields and therefore never contribute to `sizeof`.
Their types must have no cleanup obligation, shared ownership, or interior
mutability. Each use may copy the value freely. A constant carries no storage
identity and grants no authority.

Constants are not scalar-only. Fixed arrays, records, and copy-eligible sums
are eligible when their complete types recursively
obey the pure-value/multiplicity rule. An unrestricted active case does not make
a structurally linear sum eligible. An initializer may be an admitted call to
an ordinary machine; the constant position requests semantic evaluation, and
the concrete invocation contract decides whether it is legal. No parallel
`const machine` species exists.

Evaluation and runtime materialization are separate judgments. A compile-time
value may be used by proofs, layouts, or further evaluation without ever
occupying bytes. If a runtime use demands representation, the selected layout
must determine every observable bit of the active value. The check traverses
the realized active case and fields, reports the exact offending component and
producer origin, and emits zero for semantically unobservable padding. A
`const` still has no stable address; one addressable immutable image occurrence
would be a separate future storage feature, not an extension of `const`.

## Stateless entry

A hosted program that needs no receiver state binds a free machine:

```omega
machine start() {
    Console::write_line("Hello, Omega.");
}

machine build(builder: &mut Build) {
    builder.target = windows_x86_64;
    builder.roots.bind(windows_x86_64::ProgramEntry, start);
}
```

The target bridge performs its platform handoff and calls `start`. No object is
allocated merely because the artifact has an entry point.

## Receiver-bound entry

Attaching the selected entry to data and taking `&mut self` requests one
receiver instance:

```omega
data Application {
    greeting_count: u64;
}

machine Application::start(&mut self) {
    Console::write_line("Hello, Omega.");
    self.greeting_count += 1;
}

machine build(builder: &mut Build) {
    builder.target = windows_x86_64;
    builder.roots.bind(
        windows_x86_64::ProgramEntry,
        Application::start
    );
}
```

The binding selects the machine; it does not construct or pass a value itself.
The selected machine's receiver states that one `Application` must exist before
the launch call. The generated bridge:

1. derives storage from an admitted entry root selected by the target;
2. establishes one ZII-valid `Application` in that storage;
3. lends the only reference as `&mut self`;
4. runs ordinary cleanup if the entry returns normally; and
5. records abandonment through the ordinary crash frontier otherwise.

`data Application` remains an ordinary data declaration. Binding the attached
machine does not turn the type or its values into a content root, assign a
storage class to the declaration, or establish a domain qualification on every
`Application`. The bridge establishes this one receiver occurrence in storage
whose root lineage and backing are already accounted for.

The receiver is not globally nameable. Interrupt handlers, tasks, and other
roots cannot reach it unless the program explicitly transfers a capability,
lease, or synchronized share through the normal concurrency machinery.

If the receiver type is not valid under ZII, the binding rejects. The general
form is a free entry machine that explicitly constructs state from resources
exposed by its target schema.

## Placement is target lowering

The source requests the instance by selecting a receiver-bound entry; it does
not declare a storage class. A hosted target may reserve the receiver in a
writable image section. A freestanding target may partition it from supplied
initial storage. Both lowerings preserve the same portable facts:

- exactly one instance belongs to the selected program-entry activation;
- its storage is a subextent of an admitted entry root;
- its initialization is checked;
- its only initial access is the explicit `&mut self`; and
- its placement, root lineage, and backing provenance are recorded.

Image sections, string bytes, and a receiver placed in writable image storage
are derived subextents of the installed image root. A receiver placed in runtime
storage is a conserved partition of that root. Neither case originates a new
physical root merely because the compiler knows the size.

## Hosted and freestanding source surfaces

The target entry schema controls which non-receiver parameters the program sees.
A hosted schema normally hides raw storage setup and exposes no extent
parameters. A freestanding schema may expose image and initial-storage extents
because provisioning them is the program's job. Receiver provisioning composes
with either form: a freestanding entry may take both `&mut self` and the raw
roots its schema publishes. The receiver's storage must not overlap an owned
root handed to the source entry: the schema either provisions it from separate
hidden supply or forwards only the conserved residual partition.

The physical arrival contract and target bootstrap remain target-owned and are
distinct from the stable semantic arrival requirement. A generated ABI shell
plus that authored adapter form the installed external root, derive their
complete contract, establish the semantic occurrence, and call the selected
source entry. The source machine does not become magical because it is bound.
`build.omg` names that source machine only; the launch environment supplies the
physical arrival values under the target-fixed contract, and the bridge
supplies only the schema's visible semantic arguments.

## Capability consequence

This is the ordinary capability model applied to persistent state. Mutable state
is usable only through a held borrow descending from an explicit root. Removing
ambient `static` names keeps aliasing, task transfer, interrupt sharing, and
replacement within the normal ownership and conservation rules.
