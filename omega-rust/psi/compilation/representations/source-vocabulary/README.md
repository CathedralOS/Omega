# Core language definitions

This owner defines language-level leaf contracts and their laws. It has no
source-program graph, target compiler, interpreter, Build execution or host I/O
dependency. Its normal dependencies are numeric semantics, exact literal records
and record encoding.

| Export group | Responsibility |
| --- | --- |
| `atomic` | Memory order and atomic ordering-plan definitions; order compatibility laws. |
| `core_vocabulary`, `sealed_intrinsics`, `float_operations` | Sealed language names, arities, numeric operation contracts and identities. |
| `inline_assembly` | Authored assembly declaration vocabulary and operand constraints; no instruction encoding. |
| `source_semantics` exports | Binding, multiplicity, carry, access, domain and acknowledgement records; reserved source spellings and label operations. |
| `cast_form`, `operator_spelling` | Authored cast/operator forms and their spellings. |
| `constant_value` | Exact evaluated scalar values. |
| `activation_storage`, `layout_storage`, `data_layout_queries`, `descriptor_tables`, `static_interrupts` | Core operation and query declarations, independent of a target implementation. |

These functions neither compile a program nor grant a checking verdict. Source
settlement and independent checkers consume the same declarations and laws.
Generic storage and arithmetic remain in foundation; syntax trees and later
program forms remain in their representation owners.
