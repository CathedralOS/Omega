import subprocess
t = subprocess.run(['git','show','origin/main:tests/omega/corpus_outcomes.txt'],
                   capture_output=True, text=True).stdout.splitlines()
missing = {}
i = 0
while i < len(t):
    l = t[i]
    if l.startswith('fail/facts/') and 'rejected' in l:
        # Exact path derivation, never a substring. A substring filter for `bound`
        # also matches bounded_products, bounded_scalar_sums and
        # boundary_provider_call (36 extra rows) and reported bound as 83 rows
        # instead of 9. Each row is counted once under its own exact fact path.
        key = l.split()[0]
        rel = key.split('fail/facts/')[1]
        fact = rel.split('/')[0]
        for marker in ('_through_', '_unproved', '_rejected'):
            if marker in fact:
                fact = fact.split(marker)[0]
        j = i + 1
        has = False
        while j < len(t) and (t[j].startswith('\t') or t[j].startswith('    ')) and t[j].strip():
            has = True
            j += 1
        if not has:
            missing[fact] = missing.get(fact, 0) + 1
        i = j
    else:
        i += 1
print("%d facts, %d rows missing diagnostics" % (len(missing), sum(missing.values())))
for f, c in sorted(missing.items()):
    print("  %3d  %s" % (c, f))
