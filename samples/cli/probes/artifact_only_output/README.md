# artifact_only_output

`Build::artifact_only` mode + full RequiredOutput lifecycle: require/resolve/create/write/close/complete + `OutputCompletion::Sealed{receipt}`/`Retry{obligation,file}` arms + `BuildOutput::fail`.

Compile-checked only: run a check target, not native.
