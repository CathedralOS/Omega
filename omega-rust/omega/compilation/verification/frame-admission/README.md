# Frame admission

Checks callee-save storage, frame coordinates and encoded prologue/epilogue
arenas against explicit physical demands and target contracts. The checked
storage, layout and protocol records keep private constructors. Physical frame
construction lives in `physical-planning`; byte production lives in the common
encoder.

Storage replay reconstructs the register groups by keyed membership, separately
from the producer's ordered traversal. Frame validation checks minimal offsets,
alignment, non-overlap, outgoing arguments, continuation custody, red-zone
eligibility and exact probe coverage. Shared helpers preserve target geometry,
input checks, work charging and immutable projections of physical demands.

Protocol replay compares the complete byte arena and exact spans against the
existing ISA frame recipes. It preserves the existing replay and refusal
behavior; it does not provide a second independent implementation of those
recipes. No source, selected-program callback or producer stage is retained.
