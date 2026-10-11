"""Independent exhaustive haplotype map; never imports Omega source or results."""
from itertools import product
from functools import lru_cache
import re

def words(alphabet, maximum):
    return [''.join(word) for n in range(maximum + 1) for word in product(alphabet, repeat=n)]

@lru_cache(None)
def canonical_map(reference):
    # Enumerate actual replacement strings and edit applications, unlike Omega's
    # prefix/suffix equivalence test. Freeze the best edit for every haplotype.
    result = {}
    for start in range(len(reference) + 1):
        for removed in range(min(4, len(reference) - start) + 1):
            for inserted in words('ACGT', 4):
                haplotype = reference[:start] + inserted + reference[start + removed:]
                key = (removed + len(inserted), start, removed, inserted)
                if haplotype not in result or key < result[haplotype]:
                    result[haplotype] = key
    return result

def row(variant):
    contig, position, removed, inserted, mask = variant
    return f'{contig}|{position}|{removed}|{inserted or "-"}|{mask}'

def overlaps(left, right):
    return left[0] == right[0] and left[1] <= right[1] + right[2] and right[1] <= left[1] + left[2]

def simulate(text):
    references, ledger = {}, []
    locked = False
    merged = conflicts = rejected = 0
    output = ['VARIANT_NORMALIZATION|1']
    for line_number, raw in enumerate(text.splitlines(keepends=True), 1):
        if len(raw.encode()) >= 64 and not (len(raw.encode()) == 64 and raw.endswith('\n')):
            output.append('FATAL|line_capacity')
            rejected += 1
            break
        line = raw.removesuffix('\n')
        reason = 0
        declaration = re.fullmatch(r'R([01]):([ACGT]{1,8})', line)
        variant = re.fullmatch(r'V([01]):([0-8]):([ACGT]{0,4}|-):([ACGT]{0,4}|-):([0-7])', line)
        if declaration:
            contig, reference = int(declaration[1]), declaration[2]
            if locked or contig in references:
                reason = 2
            else:
                references[contig] = reference
        elif variant:
            locked = True
            contig, start, deleted, added, source = variant.groups()
            contig, start, source = int(contig), int(start), int(source)
            deleted, added = deleted.replace('-', ''), added.replace('-', '')
            reference = references.get(contig)
            if reference is None:
                reason = 2
            elif start > len(reference) or start + len(deleted) > len(reference):
                reason = 3
            elif reference[start:start + len(deleted)] != deleted:
                reason = 4
            else:
                haplotype = reference[:start] + added + reference[start + len(deleted):]
                if haplotype == reference:
                    reason = 5
                else:
                    cost, position, removed, inserted = canonical_map(reference)[haplotype]
                    incoming = [contig, position, removed, inserted, 1 << source]
                    match = next((v for v in ledger if v[:4] == incoming[:4]), None)
                    if match is not None:
                        match[4] |= incoming[4]
                        merged += 1
                        action = 1
                    elif len(ledger) == 32:
                        reason = 6
                    else:
                        conflicts += sum(overlaps(v, incoming) for v in ledger)
                        ledger.append(incoming.copy())
                        action = 0
                    if not reason:
                        output.append(f'CANON|{row(incoming)}|{action}')
        else:
            reason = 1
        if reason:
            rejected += 1
            output.append(f'REJECT|{line_number}|{reason}')
    for index, current in enumerate(ledger):
        output.append(f'MERGE|{index}|{row(current)}')
        for previous, other in enumerate(ledger[:index]):
            if overlaps(other, current):
                output.append(f'CONFLICT|{previous}|{index}')
    output.append(f'SUMMARY|{len(ledger)}|{merged}|{conflicts}|{rejected}')
    return '\n'.join(output) + '\n', 2 if rejected else 0
