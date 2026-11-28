# Owned resource handoff

Run `mbx xtask sample-gate --filter samples/cli/probes/owned_resource_handoff`.
The authored exit is 0: resources issued with identities 41 and 73 retain
their exact Open qualification through state arrivals, including an acquisition
result payload, and are consumed once after observation.

The selected arm moves the payload and disposes only its remaining owner.
The handoff must preserve existing provenance and custody; rebuilding equal
fields or assuming a state's qualification is not a substitute for transferring
its established value.
