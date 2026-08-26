# graph-workbench

Root graph fixture. It depends on one pure package and one capability-bearing
package so graph audit output can name the dependency path that introduced
authority.

Expected package evidence:

- dependency aliases include `arithmetic_kernels` and `file_journal`;
- transitive audit reports the path through `file_journal` for filesystem
  reach;
- package policy admits or rejects the final transitive set, not individual
  edges in isolation.

