# Repeat filtering through a selected predicate

Run with `omega run --both main.omg` after normal package review.
Expected exit: 0.

A generic in-place compactor dispatches through a runtime-selected named
implementation and returns a mutable retained-prefix slice. Repeated passes
select increased readings, positive readings, all remaining readings, none,
and an already empty slice. The program checks exact addresses, both payload
fields, counts, and untouched backing-array sentinels.

The original native compiler lost the returned slice's backing provenance
after an element write. The regression fixtures also exercise a view stored
in a record: descriptor storage and the elements it lends are distinct.
This is a compiler capability witness, not a claim that the Squalr app is done.
