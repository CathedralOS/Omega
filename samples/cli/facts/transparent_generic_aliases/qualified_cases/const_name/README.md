# Qualified alias cases through const name

Fact: A transparent generic alias retains its underlying case identity and payload type through qualified construction and matching.

Actual copy-sum PICKED/EMPTY constants retain the alias carrier before materialized-value interchange; the runtime record contrast independently checks carried42. The same case matching and explicit Choice/Chosen interchange remain. The sole twin changes only the Some payload to true and matches the original bool-to-u64 field guard.

Actual [41416732401](https://github.com/CathedralOS/Omega/actions/runs/41416732401), source `2360394fa4f435703471a58e92071de839e17e70`: sample and corpus positive native0 / Terminal0 / agreement; both intended-invalid observations pass. Linux x86-64 opt1 CLI `bc9628f1cf660eebda22f9765dd8be0ecc681c3ff40c52f2fe33befd96747114`, runner `c77d4ac0d6c327af73a943dc22aad051f155c3a2da9e03ec88ec7918a0196b98`.

No compiler edit or prior-place replay. Raw reports/frozen sources: `/Users/zcanann/Omega-build/alias-qualified-case-joins-20261102/`. [Row status](../README.md) retains the remaining two places, other hosts and Product as open.
