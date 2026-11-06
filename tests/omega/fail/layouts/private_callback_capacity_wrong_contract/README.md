# Reject a callback slot with the wrong requirement

This is the 33-slot foreign registrar positive with only its materialization
binder selection changed. Swapping binders 0 and 32 preserves all 33 unique
binder identities but assigns two different callback requirements to the wrong
destinations. Checking must report that the binder and native-place demand
require different callback contracts, before native execution.
