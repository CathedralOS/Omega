# Boundary signature shapes

A [calling policy](calling_plans.md) may classify a semantic type directly only
when its public normalized structure determines every required ABI fact.
Otherwise the native leaf must declare the concrete foreign shape. Knowing a
private compiler layout does not make it a public ABI.

## Public aggregates

Fixed arrays and fixed records expose element/member types, counts, size,
alignment, and public layout. Classification is recursive: equal size does not
imply equal ABI class. A policy may place or reject the aggregate under its
target's rules.

An erased field retains semantic/proof identity but contributes no ABI field,
offset, size, alignment, register/stack fragment, or transfer. Omission is
recursive. Terminal Psi retains the exact erased type identity separately.
A record containing only erased fields has no by-value ABI carrier and rejects.
Case-bearing or unresolved generic aggregates cannot be classified without a
specified public ABI shape; private compiler layout is not a substitute.

`BoundarySignature` uses a bounded flat graph: parameter/result roots index
shape nodes, fixed-array nodes name an element root and count, and record nodes
name a contiguous field range with child roots and byte offsets. The ordered
native telescope projects semantic formals and interleaves declared native-only
callback entries. A callback has target function-pointer shape without a
semantic runtime node. The policy returns ABI shape and placement per native
entry; the compiler checks both against the graph and telescope.

Opaque by-value roots first require the exact
[representation application](opaque_representations.md). Its declaration,
source, target-semantics version, closed shape, movement, lifecycle, and evidence
origin participate in boundary-application identity.

## Foreign declarations

There is no implicit C array decay. `[T; N]` declares an aggregate by value;
`&[T; N]` declares a reference. A C parameter written as an array but passed as
a pointer must use the corresponding pointer/reference contract in Omega.

Slices, text views, vectors, and bounded text do not by themselves specify a
foreign length type, nullability, retention, ownership, terminator, or descriptor
layout. Default native policies reject those bare carriers at a leaf. An API
expecting separate pointer and length arguments declares them separately:

```omega
boundary machine NativeConsole::write(bytes: Ptr<u8>, length: u64) -> i32;
```

An API expecting a terminator or descriptor declares that shape instead. A
pointer/length record is not interchangeable with two native parameters.
A custom policy may explicitly define a slice/text ABI; it is not inferred
from the compiler's current carrier.

A checked adapter exposes the safe Omega view. A synchronous non-retaining
leaf uses a call-scoped borrowed-out contract; its calling plan cannot extend
the loan. Retention requires a pinned loan, ownership transfer, or registration
protocol. Outbound text supplies bytes without its `Utf8` qualification;
inbound text validates bytes before establishing that qualification. Proof
objects do not cross the foreign boundary.

Process entry's selected requirement fixes its exit-status scalar, not the
source name `main`. Firmware and other boundaries may permit aggregate results
when their selected policy does. See [entry roots](entry_roots.md).
