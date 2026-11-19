# Linux completed-output publication

Run `python3 tests/product/completed_outputs/check.py --omega <fresh-omega>`
on Linux x86-64. It uses the public CLI and the corpus generator fixture;
it does not construct package/compiler internals or provide an alternate
package harness. Review acceptance is automated only for this private local
fixture and its ordinary local generator dependency.

The validator checks exact source/companion bytes, native exit 0, Terminal exit
0, valid set reuse, eleven rejected incomplete/substituted exports, successful
restored retries, and no output set after the matching generated-source failure
control. It reports actual command streams and statuses as JSON. Symlink and
hard-link controls use only files inside its private temporary workspace.
Two differently declared packages retain distinct result identities, and
Linux x86-64/ARM64 children export into separate target directories. ARM64
is cross-generation evidence only; this script does not execute its image.
