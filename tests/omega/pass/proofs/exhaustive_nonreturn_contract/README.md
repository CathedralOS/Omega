An event loop that handles every case must not acquire an implicit return merely because its dispatch has no wildcard. Cathedral exposed this when its waiting and received-event paths both continued rather than returning.

The positive first selects Idle, then Claimed with the expected payload, and exits through the ordinary process-exit provider with status 0. A wrong payload exits with status 7. Thus both case arms execute before the observed result. The reject twin changes only the received-event body to return normally, which contradicts the outer `ensures false` contract.

The fixture isolates exhaustive-dispatch completion. It does not exercise interrupts or establish Cathedral timer acceptance.
