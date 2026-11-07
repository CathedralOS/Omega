# Fixed-array reference home

References stored in records mutate their original fixed arrays. The program
checks local and forwarded arrays, nested projections, returned references,
shared observations, write-only stores and empty arrays. A child reborrow keeps
its original backing after its parent descriptor is replaced. Expected exit: 0.

Run `omega run --both samples/cli/memory/fixed_array_reference_home/main.omg`.

This exercises retained array backing, not descriptor replacement during a
pending assignment.
