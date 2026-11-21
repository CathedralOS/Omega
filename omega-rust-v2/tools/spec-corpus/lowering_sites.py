"""L5 explicit Rust call candidates. Join only a same-revision scan to observed helper origins."""
from collections import Counter
from pathlib import Path
import re
import subprocess

from diagnostic_fields import unescape
from lowering_origins import MODULE, POSITIVE

COMMAND = ['mbx', 'run', '--locked', '-j', '1', '-p', 'spec-corpus', '--features', 'lowering-sites', '--bin', 'lowering_diagnostic_sites']
HEADER = ['#lowering-diagnostic-sites\tv1', 'path\tline\tcolumn\trole\tcallee\tclass\ttext']
FOOTER = re.compile(r'#end\tsites=([0-9]+)\tfiles=([1-9][0-9]*)')
LIMITS = ('Counts describe explicit syntax, not distinct reasons, policy completeness or checks of refusal behavior. '
          'Direct calls must spell LoweringError::new. Only synchronous bad/unsupported_node functions with track_caller '
          'and a single tail constructor are recognized as forwarders. The scanner matches bare block-local functions, '
          'simple module paths (including inline modules at module scope), and explicit module imports. '
          'A constructor stays in the denominator unless a call matches its particular forwarding definition. '
          'This fallback is syntax, not a predicted Rust caller coordinate. Block-local modules/imports, glob imports, '
          're-exports and complex or ambiguous bindings can leave caller sites uncounted. '
          'Closure calls are not caller-forwarding sites. Wrapper text classifies the last argument, not the finished diagnostic. '
          'There is no compiler name resolution, cfg evaluation, macro expansion, or tracing through indirect calls or chains of tracked functions. '
          'The scan skips tests.rs, tests directories and common test-only functions, modules, impls and files.')


def empty():
    return {'files': None, 'counts': None, 'rows': [], 'errors': []}


def parse(text):
    inv = empty()
    lines = text.splitlines()
    if lines[:2] != HEADER or not (footer := FOOTER.fullmatch(lines[-1])):
        inv['errors'].append('missing or changed header/footer')
        return inv
    for number, line in enumerate(lines[2:-1], 3):
        try:
            path, ln, col, role, callee, kind, argument = line.split('\t')
            path, callee, argument = (unescape(v).decode('utf-8') for v in (path, callee, argument))
            if not MODULE.fullmatch(path) or not all(POSITIVE.fullmatch(v) for v in (ln, col)):
                raise ValueError('invalid source location')
            if (role not in ('constructor', 'wrapper', 'forwarder-body') or kind not in ('literal', 'template', 'dynamic')
                    or not re.fullmatch(r'(?:[A-Za-z_][A-Za-z_0-9]*::)*[A-Za-z_][A-Za-z_0-9]*', callee)
                    or (role != 'wrapper') != (callee.split('::')[-2:] == ['LoweringError', 'new'])):
                raise ValueError('invalid candidate description')
            inv['rows'].append(dict(path=path, line=int(ln), column=int(col), role=role, callee=callee, **{'class': kind}, text=argument))
        except ValueError as error:
            inv['errors'].append(f'row {number}: {error}')
    rows = inv['rows']
    keys = [key(row) for row in rows]
    files = int(footer[2])
    if keys != sorted(set(keys)) or len(rows) != int(footer[1]) or len({r['path'] for r in rows}) > files:
        inv['errors'].append('unsorted or duplicate sites, or inconsistent footer counts')
    counts = Counter(row['role'] for row in rows)
    if counts['forwarder-body'] > counts['wrapper'] or (counts['wrapper'] and not counts['forwarder-body']):
        inv['errors'].append('inconsistent forwarding body and caller counts')
    if not inv['errors']:
        inv.update(files=files, counts={'explicit_constructors': counts['constructor'] + counts['forwarder-body'],
                   'forwarder_bodies': counts['forwarder-body'], 'wrapper_calls': counts['wrapper'],
                   'origin_candidates': counts['constructor'] + counts['wrapper'],
                   'constructor_arguments': argument_counts(rows, wrapper=False),
                   'wrapper_arguments': argument_counts(rows, wrapper=True)})
    return inv


def argument_counts(rows, wrapper):
    counts = Counter(r['class'] for r in rows if (r['role'] == 'wrapper') == wrapper)
    return {kind: counts[kind] for kind in ('literal', 'template', 'dynamic')}


def key(site):
    return (site['path'], site['line'], site['column'])


def verify(inventory, report):
    """Use a freshly parsed TSV from the run, not current coordinates against an older archive."""
    if inventory['counts'] is not None:
        candidates = {key(row): row for row in inventory['rows'] if row['role'] != 'forwarder-body'}
        for f in report.get('package_fixtures', ()):
            if f.get('origin_status') != 'observed':
                continue  # lowering_origins already enforces the unique passing capture.
            origin = key(f['reported_compiler_site'])
            name = f"{f['target']}::{f['test']}"
            row = candidates.get(origin)
            if row is None:
                inventory['errors'].append(f'{name}: no candidate at {origin[0]}:{origin[1]}:{origin[2]}')
                continue
            if row['role'] == 'constructor' and row['class'] == 'literal':
                message = re.sub(r'[\r\n\t]', ' ', row['text'])  # shared helper receipt encoding
                if f['outcome'] != f"refused: {message} at {f['location']['file']}":
                    inventory['errors'].append(f'{name}: literal disagrees at {origin[0]}:{origin[1]}:{origin[2]}')
                    continue
            row.setdefault('observations', []).append(name)
        inventory['observed_candidates'] = sum(bool(row.get('observations')) for row in candidates.values())
    report['measurement_errors'].extend('lowering site inventory: ' + e for e in inventory['errors'])
    report['lowering_sites'] = inventory


def summary(report):
    inv = report.get('lowering_sites')
    if inv is None:
        return []
    counts = inv['counts']
    if counts is None:
        return ['', '- L5 source candidates unmeasured. [Scanner log](lowering-sites.log); see measurement errors.']
    observed = inv['observed_candidates']
    candidates = counts['origin_candidates']
    return ['', '### Lowering refusal sites', '',
            f"Passing native fixtures reached **{observed} of {candidates}** candidate Rust call sites. "
            f"{candidates - observed} have no matching return from the instrumented helper.",
            f"The scanner found {counts['explicit_constructors']} explicit constructors across {inv['files']} scanned files. "
            f"It replaced {counts['forwarder_bodies']} forwarding bodies with {counts['wrapper_calls']} caller locations. "
            "Several policy decisions can share one constructor. "
            "The JSON lists each site and its matching tests. "
            f"Inventory and matching errors: {len(inv['errors'])}. [Source inventory](lowering-sites.tsv) · [Witnesses](evidence.json)", '', LIMITS]


def finish(report, directory, environment):
    """Call for a live L5 run after lowering_origins.finish; preserve TSV and log for offline review."""
    if report.get('last_layer') != 'L5':
        return
    directory = Path(directory)
    tsv, log = directory / 'lowering-sites.tsv', directory / 'lowering-sites.log'
    inv = empty()
    code = None
    try:
        with tsv.open('wb') as out, log.open('wb') as err:
            code = subprocess.run(COMMAND, env=environment, stdin=subprocess.DEVNULL, stdout=out, stderr=err).returncode
        if code:
            inv['errors'].append(f'scanner exited {code}; see lowering-sites.log')
        else:
            inv = parse(tsv.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        inv['errors'].append(f'scanner unavailable: {error}')
    inv.update(command={'command': COMMAND, 'exit': code}, limits=LIMITS)
    verify(inv, report)
