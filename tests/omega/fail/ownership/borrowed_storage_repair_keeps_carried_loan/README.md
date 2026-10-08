# Repair does not end a contained loan

Moving a reference-bearing carrier through an exclusive helper and putting it
back retains its shared loan into the original backing. Mutating that backing
before the repaired carrier's final read must reject. Neither extraction nor
repair may discard the loan because its carrier changed homes.
