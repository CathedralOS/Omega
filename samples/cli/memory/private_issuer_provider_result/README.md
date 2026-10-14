# Private issuer result through a checked provider

`omega run samples/cli/memory/private_issuer_provider_result/main.omg` must exit 0.
The selected provider calls a private issuer, returns its qualified resource in
an acquisition result, and the caller transfers it into another state, observes
identity 73, and consumes it. The public requirement is not an issuer.
The entry's explicitly provisioned `Binding<Source>` authorizes the call; Build
selects `CheckedSource` as its implementation.
