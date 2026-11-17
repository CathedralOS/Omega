"""One connected compiler graph with one ID for each shared component.

Citations define the connections. The shared build-time layout positions and
routes both views; the browser moves their SVG without changing either graph.
"""
import html
import json
from pathlib import Path
import re
import subprocess


def validate(model):
    if model.get('view')=='proposed':
        if model.get('status')!='unverified' or any('/atlas/plans/foundation-and-backend.md#' not in item['source'] for item in model['nodes']+model['edges']):
            raise ValueError('Proposed contracts must be unverified and cite the recorded plan')
        spine=model['spine']
        if ([n['id'] for n in model['nodes']]!=spine or
            [(e['start'],e['end']) for e in model['edges']]!=list(zip(spine,spine[1:])) or
            any(not e.get('main') for e in model['edges'])):
            raise ValueError('Proposed must be one forward chain with no side nodes or edges')
        stages=[n for n in model['nodes'] if n['role']=='owner']
        if ([n.get('number') for n in stages]!=list(range(11)) or
            any(not n['title'].startswith(f'{i:02d} ') for i,n in enumerate(stages))):
            raise ValueError('Proposed stages must be numbered 00–10 in order')
    nodes = {n['id']: n for n in model['nodes']}
    if len(nodes) != len(model['nodes']):
        raise ValueError('Duplicate overview endpoint')
    owners = [n['owner'] for n in model['nodes'] if n['role']=='owner']
    if len(owners) != len(set(owners)):
        raise ValueError('A shared component must have one visible node')
    neighbors = {key:set() for key in nodes}
    for edge in model['edges']:
        if edge['start'] not in nodes or edge['end'] not in nodes:
            raise ValueError('Unknown overview endpoint')
        if not re.fullmatch(r'https://github.com/CathedralOS/Omega/blob/[0-9a-f]{40}/[^#]+#L\d+(?:-L\d+)?', edge['source']):
            raise ValueError('Every connection needs pinned file-and-line evidence')
        start, end = nodes[edge['start']], nodes[edge['end']]
        if edge['kind'] == 'auxiliary' and end['role'] != 'transform':
            raise ValueError('An operation input must end at the operation')
        if edge['kind'] == 'request' and end['role'] != 'owner':
            raise ValueError('A request must name its receiving owner')
        if start['owner'] == 'engine' and end['owner'] == 'source':
            raise ValueError('The interpreter does not invoke the source compiler')
        neighbors[edge['start']].add(edge['end'])
        neighbors[edge['end']].add(edge['start'])
    pending = [next(iter(nodes))]
    reached = set()
    while pending:
        node = pending.pop()
        if node not in reached:
            reached.add(node)
            pending.extend(neighbors[node]-reached)
    if reached != nodes.keys():
        raise ValueError('The overview must be one connected graph')
    if 'spine' in model:
        pairs={(e['start'],e['end']) for e in model['edges'] if e.get('main')}
        if len(model['spine'])!=len(set(model['spine'])) or any(pair not in pairs for pair in zip(model['spine'],model['spine'][1:])):
            raise ValueError('The main route must follow existing connections in order')


def overview_model(architecture):
    records = {n['id']: n for n in architecture['nodes']}
    base = records['o0']['source'].split('/omega-rust/')[0] + '/'
    nodes = [
        {'id': 'build', 'owner': 'build', 'group': 'inputs', 'title': 'Build · packages · library', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'compiler', 'owner': 'source', 'group': 'source', 'title': 'Source compiler', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'files', 'owner': 'source', 'group': 'source', 'title': 'Source files', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'tokens', 'owner': 'source', 'group': 'source', 'title': 'Tokens', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'trees', 'owner': 'source', 'group': 'source', 'title': 'Syntax trees', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'resolved', 'owner': 'source', 'group': 'source', 'title': 'Resolved trees', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'typed', 'owner': 'source', 'group': 'source', 'title': 'Typed trees', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'psi-0', 'owner': 'p0', 'group': 'source', 'title': '00 lex', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'psi-1', 'owner': 'p1', 'group': 'source', 'title': '01 parse', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'psi-2', 'owner': 'p2', 'group': 'source', 'title': '02 resolve', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'psi-3', 'owner': 'p3', 'group': 'source', 'title': '03 type', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'frozen', 'owner': 'p4', 'group': 'source', 'title': 'Frozen typed trees', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'lowered', 'owner': 'p5', 'group': 'source', 'title': 'Lowered Psi', 'role': 'artifact', 'kind': 'psi', 'width': 220, 'height': 80},
        {'id': 'accepted', 'owner': 'checked', 'group': 'source', 'title': 'Accepted Psi', 'role': 'artifact', 'kind': 'accepted', 'width': 220, 'height': 80},
        {'id': 'psi-4', 'owner': 'p4', 'group': 'source', 'title': '04 prepare', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'psi-5', 'owner': 'p5', 'group': 'source', 'title': '05 lower', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'psi-6', 'owner': 'p6', 'group': 'source', 'title': '06 check', 'role': 'transform', 'kind': 'check', 'width': 0, 'height': 0},
        {'id': 'source-eval', 'owner': 'evaluation', 'group': 'evaluation', 'title': 'Source evaluator', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'engine', 'owner': 'engine', 'group': 'runtime', 'title': 'Psi interpreter', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'closure', 'owner': 'providers', 'group': 'providers', 'title': 'Provider closure', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'selected', 'owner': 'selected', 'group': 'providers', 'title': 'Selected inputs', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'policy-inputs', 'owner': 'policy-inputs', 'group': 'providers', 'title': 'Prepare policy inputs', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'policy-eval', 'owner': 'policy', 'group': 'evaluation', 'title': 'Calling-policy evaluator', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'admit', 'owner': 'runtime-admission', 'group': 'runtime', 'title': 'Runtime admission', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'runtime-host', 'owner': 'host', 'group': 'runtime', 'title': 'Omega runtime host', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'worker', 'owner': 'worker', 'group': 'runtime', 'title': 'Native-provider worker', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'abstract', 'owner': 'o0', 'group': 'native', 'title': 'Abstract operations', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'target', 'owner': 'o2', 'group': 'native', 'title': 'Target operations', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'instructions', 'owner': 'o3', 'group': 'native', 'title': 'Selected instructions', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'homes', 'owner': 'o5', 'group': 'native', 'title': 'Register homes', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'physical', 'owner': 'o6', 'group': 'native', 'title': 'Physical operations', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'native-0', 'owner': 'o0', 'group': 'native', 'title': '00 admit / lower', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-2', 'owner': 'o2', 'group': 'native', 'title': '02 lower', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-3', 'owner': 'o3', 'group': 'native', 'title': '03 select', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-5', 'owner': 'o5', 'group': 'native', 'title': '05 allocate', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-6', 'owner': 'o6', 'group': 'native', 'title': '06 lower', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'installations', 'owner': 'binding', 'group': 'native', 'title': 'Handlers → measured binding', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
        {'id': 'framed', 'owner': 'frame-plan', 'group': 'native', 'title': 'Framed program', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'plan-frame', 'owner': 'frame-plan', 'group': 'native', 'title': 'plan frames', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'encoded', 'owner': 'o7', 'group': 'encoding', 'title': 'Encoded forms', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'layout', 'owner': 'o8', 'group': 'encoding', 'title': 'Resolved layout', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'optimized', 'owner': 'o9', 'group': 'encoding', 'title': 'Optimized layout', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'fragments', 'owner': 'fragments', 'group': 'emission', 'title': 'Function fragments', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'framed-fragments', 'owner': 'frames', 'group': 'emission', 'title': 'Framed fragments', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'native-7', 'owner': 'o7', 'group': 'encoding', 'title': '07 encode', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-8', 'owner': 'o8', 'group': 'encoding', 'title': '08 place', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-9', 'owner': 'o9', 'group': 'encoding', 'title': '09 optimize', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'emit', 'owner': 'fragments', 'group': 'emission', 'title': 'emit', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'apply-frame', 'owner': 'frames', 'group': 'emission', 'title': 'apply frames', 'role': 'transform', 'kind': 'attention', 'width': 0, 'height': 0},
        {'id': 'text', 'owner': 'text', 'group': 'emission', 'title': 'Text section', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'objects', 'owner': 'objects', 'group': 'emission', 'title': 'Objects', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'composed', 'owner': 'compose', 'group': 'emission', 'title': 'Composed bodies', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'image', 'owner': 'image', 'group': 'image', 'title': 'Native image', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'published', 'owner': 'products', 'group': 'image', 'title': 'Published executable', 'role': 'artifact', 'kind': 'native', 'width': 220, 'height': 80},
        {'id': 'place-text', 'owner': 'text', 'group': 'emission', 'title': 'place text', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'object', 'owner': 'objects', 'group': 'emission', 'title': 'build objects', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'compose', 'owner': 'compose', 'group': 'emission', 'title': 'compose', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'image-build', 'owner': 'image', 'group': 'image', 'title': 'form image', 'role': 'transform', 'kind': 'attention', 'width': 0, 'height': 0},
        {'id': 'publish', 'owner': 'products', 'group': 'image', 'title': 'publish', 'role': 'transform', 'kind': 'data', 'width': 0, 'height': 0},
        {'id': 'native-run', 'owner': 'native-run', 'group': 'run', 'title': 'Native program · operating system', 'role': 'owner', 'kind': 'owner', 'width': 220, 'height': 80},
    ]
    edges = [
        dict(id='overview-edge-0', start='build', end='compiler', label='source request', kind='request', source=base+'omega-rust/omega/build-system/orchestration/src/compile/source_compilation.rs#L21-L115'),
        dict(id='overview-edge-1', start='compiler', end='files', label='loaded sources', kind='data', source=base+'omega-rust/psi/compilation/source-compiler/src/source_compilation.rs#L894'),
        dict(id='overview-edge-2', start='files', end='psi-0', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources.rs#L14'),
        dict(id='overview-edge-3', start='psi-0', end='tokens', label='00 lex', kind='data', source=base+'omega-rust/psi/compilation/pipeline/00_source-files-to-tokens/src/lex_sources.rs#L14'),
        dict(id='overview-edge-4', start='tokens', end='psi-1', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs#L20'),
        dict(id='overview-edge-5', start='psi-1', end='trees', label='01 parse', kind='data', source=base+'omega-rust/psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs#L20'),
        dict(id='overview-edge-6', start='trees', end='psi-2', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs#L2017'),
        dict(id='overview-edge-7', start='psi-2', end='resolved', label='02 resolve', kind='data', source=base+'omega-rust/psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs#L2017'),
        dict(id='overview-edge-8', start='resolved', end='psi-3', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596'),
        dict(id='overview-edge-9', start='psi-3', end='typed', label='03 type', kind='data', source=base+'omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1596'),
        dict(id='overview-edge-10', start='typed', end='psi-4', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs#L153'),
        dict(id='overview-edge-11', start='psi-4', end='frozen', label='04 prepare', kind='data', source=base+'omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs#L153'),
        dict(id='overview-edge-12', start='frozen', end='psi-5', label='', kind='data', source=base+'omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs#L24'),
        dict(id='overview-edge-13', start='psi-5', end='lowered', label='05 lower', kind='data', source=base+'omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs#L24'),
        dict(id='overview-edge-14', start='lowered', end='psi-6', label='', kind='check', source=base+'omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/verdict.rs#L1440'),
        dict(id='overview-edge-15', start='psi-6', end='accepted', label='06 check', kind='check', source=base+'omega-rust/psi/compilation/pipeline/06_terminal-psi-to-terminal-psi/src/verdict.rs#L1440'),
        dict(id='overview-edge-16', start='compiler', end='source-eval', label='evaluate value', kind='request', source=base+'omega-rust/psi/compilation/source-compiler/src/source_checking.rs#L119'),
        dict(id='overview-edge-17', start='source-eval', end='compiler', label='computed value → preparation continues', kind='result', source=base+'omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L402'),
        dict(id='overview-edge-18', start='source-eval', end='engine', label='admit, then invoke', kind='request', source=base+'omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L402'),
        dict(id='overview-edge-19', start='engine', end='source-eval', label='result + work', kind='result', source=base+'omega-rust/psi/compilation/build-time-evaluation/src/source_jobs.rs#L402'),
        dict(id='overview-edge-20', start='accepted', end='closure', label='reached requirements', kind='data', source=base+'omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L202'),
        dict(id='overview-edge-21', start='closure', end='selected', label='stable bindings', kind='data', source=base+'omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L202'),
        dict(id='overview-edge-22', start='closure', end='policy-inputs', label='changed reach', kind='request', source=base+'omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L202'),
        dict(id='overview-edge-23', start='policy-inputs', end='closure', label='prepared inputs; repeat', kind='result', source=base+'omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14'),
        dict(id='overview-edge-24', start='policy-inputs', end='policy-eval', label='evaluate policy', kind='request', source=base+'omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14'),
        dict(id='overview-edge-25', start='policy-eval', end='policy-inputs', label='validated plan + work', kind='result', source=base+'omega-rust/omega/build-system/build-settings/src/calling_policy_execution.rs#L45'),
        dict(id='overview-edge-26', start='policy-eval', end='engine', label='admit, then invoke', kind='request', source=base+'omega-rust/omega/build-system/build-settings/src/calling_policy_execution.rs#L45'),
        dict(id='overview-edge-27', start='engine', end='policy-eval', label='result + work', kind='result', source=base+'omega-rust/omega/build-system/build-settings/src/calling_policy_execution.rs#L45'),
        dict(id='overview-edge-28', start='selected', end='admit', label='runtime inputs', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization.rs#L152'),
        dict(id='overview-edge-29', start='admit', end='runtime-host', label='admitted provisions', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization.rs#L152'),
        dict(id='overview-edge-30', start='accepted', end='runtime-host', label='same publication identity', kind='data', source=base+'omega-rust/omega/interpreter/terminal-runtime/src/execution.rs#L180'),
        dict(id='overview-edge-31', start='runtime-host', end='engine', label='bind, invoke / resume', kind='request', source=base+'omega-rust/omega/interpreter/terminal-runtime/src/execution.rs#L353'),
        dict(id='overview-edge-32', start='engine', end='runtime-host', label='status + output + work', kind='result', source=base+'omega-rust/omega/interpreter/terminal-runtime/src/execution.rs#L421'),
        dict(id='overview-edge-33', start='runtime-host', end='worker', label='external calls via host adapter', kind='request', source=base+'omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider.rs#L1'),
        dict(id='overview-edge-34', start='worker', end='runtime-host', label='checked replies', kind='result', source=base+'omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider.rs#L1'),
        dict(id='overview-edge-35', start='selected', end='native-0', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization.rs#L44'),
        dict(id='overview-edge-36', start='native-0', end='abstract', label='00 admit / lower', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization.rs#L44'),
        dict(id='overview-edge-37', start='abstract', end='native-2', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L84'),
        dict(id='overview-edge-38', start='native-2', end='target', label='02 lower', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L84'),
        dict(id='overview-edge-39', start='target', end='native-3', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L159'),
        dict(id='overview-edge-40', start='native-3', end='instructions', label='03 select', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L159'),
        dict(id='overview-edge-41', start='instructions', end='native-5', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L197'),
        dict(id='overview-edge-42', start='native-5', end='homes', label='05 allocate', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L197'),
        dict(id='overview-edge-43', start='homes', end='native-6', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L202'),
        dict(id='overview-edge-44', start='native-6', end='physical', label='06 lower', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L202'),
        dict(id='overview-edge-45', start='abstract', end='abstract', label='01 optimize', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L99'),
        dict(id='overview-edge-46', start='instructions', end='instructions', label='04 optimize', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L175'),
        dict(id='overview-edge-47', start='target', end='installations', label='if required', kind='request', source=base+'omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L117'),
        dict(id='overview-edge-48', start='installations', end='native-3', label='bound installation facts', kind='auxiliary', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L159'),
        dict(id='overview-edge-49', start='physical', end='plan-frame', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L205'),
        dict(id='overview-edge-50', start='plan-frame', end='framed', label='plan frames', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L205'),
        dict(id='overview-edge-51', start='framed', end='native-7', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L211'),
        dict(id='overview-edge-52', start='native-7', end='encoded', label='07 encode', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L211'),
        dict(id='overview-edge-53', start='encoded', end='native-8', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L216'),
        dict(id='overview-edge-54', start='native-8', end='layout', label='08 place', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L216'),
        dict(id='overview-edge-55', start='layout', end='native-9', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L220'),
        dict(id='overview-edge-56', start='native-9', end='optimized', label='09 optimize', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L220'),
        dict(id='overview-edge-57', start='optimized', end='emit', label='', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-58', start='emit', end='fragments', label='emit', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-59', start='fragments', end='apply-frame', label='', kind='attention', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-60', start='apply-frame', end='framed-fragments', label='apply frames', kind='attention', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-61', start='target', end='emit', label='EmissionFacts · retained abstract program', kind='auxiliary', source=base+'omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L261'),
        dict(id='overview-edge-62', start='framed-fragments', end='place-text', label='', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-63', start='place-text', end='text', label='place text', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-64', start='text', end='object', label='', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-65', start='object', end='objects', label='build objects', kind='data', source=base+'omega-rust/omega/compilation/backend/artifacts/native-artifact/src/fragment_object_publication.rs#L167'),
        dict(id='overview-edge-66', start='objects', end='compose', label='', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L115'),
        dict(id='overview-edge-67', start='compose', end='composed', label='compose', kind='data', source=base+'omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L115'),
        dict(id='overview-edge-68', start='composed', end='image-build', label='', kind='attention', source=base+'omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L1823'),
        dict(id='overview-edge-69', start='image-build', end='image', label='form image', kind='attention', source=base+'omega-rust/omega/compilation/backend/images/image-emission/src/image.rs#L1823'),
        dict(id='overview-edge-70', start='image', end='publish', label='', kind='data', source=base+'omega-rust/omega/compilation/products/src/publication.rs#L26'),
        dict(id='overview-edge-71', start='publish', end='published', label='publish', kind='data', source=base+'omega-rust/omega/compilation/products/src/publication.rs#L26'),
        dict(id='overview-edge-72', start='published', end='native-run', label='CLI launches when requested', kind='request', source=base+'omega-rust/omega/application/src/execution/mod.rs#L244'),
        dict(id='build-choices', start='build', end='closure', label='target and provider choices', kind='data', source=base+'omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L202'),
    ]
    for node in nodes:
        owner = records[node['owner']]
        node.update(source=owner['source'], description=owner['description'], page=owner['page'])
    # Runtime admission calls a helper inside Omega 02. Object composition can
    # discover private bodies from emitted symbols. Give each its own endpoint.
    for key,owner,title,description,path in (
        ('runtime-layout','runtime-admission','02 storage-layout helper','Runtime admission optimizes the abstract program and calls a target-lowering helper for borrowed storage layouts. It installs the returned layouts in the runtime provisions.','realization.rs#L152-L184'),
        ('callbacks','compose','Compile callback bodies','Root object symbols select reached callbacks. The driver realizes private objects and follows their symbols until it closes the set.','realization/callback_thunks.rs#L195-L264'),
    ):
        nodes.append(dict(id=key,owner=owner,title=title,role='operation',kind='attention',
                          group='runtime' if key=='runtime-layout' else 'emission',page='omega',
                          description=description,source=base+'omega-rust/omega/compilation/native-compiler/src/'+path))
    for index,a,b,label,path in (
        (73,'admit','runtime-layout','query stage 02 for storage layouts','realization.rs#L166-L178'),
        (74,'runtime-layout','admit','borrowed storage layouts for runtime provisions','realization.rs#L173-L184'),
        (75,'objects','callbacks','scan emitted symbols for required callback bodies','realization/callback_thunks.rs#L195-L229'),
        (76,'callbacks','compose','private function objects for composition','realization/object_emission.rs#L115-L128'),
    ):
        edges.append(dict(id=f'overview-edge-{index}',start=a,end=b,label=label,kind='attention',
                          source=base+'omega-rust/omega/compilation/native-compiler/src/'+path))
    # Same-form optimization still has an input and an output. Showing those
    # versions separately keeps 01 and 04 in the numbered reading order.
    for number, input_id, output_id, title in (
        (1,'abstract','optimized-abstract','Optimized abstract operations'),
        (4,'instructions','allocation-input','Allocation input'),
    ):
        operation=f'native-{number}'
        citation=next(e['source'] for e in edges if e['start']==input_id and e['end']==input_id)
        edges=[e for e in edges if not (e['start']==input_id and e['end']==input_id)]
        for e in edges:
            if e['start']==input_id and e['end']==f'native-{number+1}':
                e['start']=output_id
        nodes.extend([
            dict(id=operation,owner=f'o{number}',group='native',title=f'{number:02} optimize',role='transform',kind='data',width=0,height=0,source=citation,description=records[f'o{number}']['description'],page='omega'),
            dict(id=output_id,owner=f'o{number}',group='native',title=title,role='artifact',kind='native',width=220,height=60,source=citation,description=records[f'o{number}']['description'],page='omega'),
        ])
        edges.extend([
            dict(id=f'{operation}-input',start=input_id,end=operation,label='',kind='data',source=citation),
            dict(id=f'{operation}-output',start=operation,end=output_id,label=f'{number:02} optimize',kind='data',source=citation),
        ])
    spine = ['build','compiler','files','psi-0','tokens','psi-1','trees','psi-2','resolved','psi-3','typed','psi-4','frozen','psi-5','lowered','psi-6','accepted','closure','selected',
             'native-0','abstract','native-1','optimized-abstract','native-2','target','native-3','instructions','native-4','allocation-input','native-5','homes','native-6','physical','plan-frame','framed',
             'native-7','encoded','native-8','layout','native-9','optimized','emit','fragments','apply-frame','framed-fragments','place-text','text','object','objects','compose','composed','image-build','image','publish','published','native-run']
    main_pairs=set(zip(spine,spine[1:]))
    for n in nodes:
        if n['role']!='transform':
            n['height']=60
    for e in edges:
        e['main']=(e['start'],e['end']) in main_pairs
    # The repair plan identifies these specific calls and handoffs. Other
    # side paths, such as ordinary runtime calls, keep their own color.
    detours=[
        ('emission','Emit reads stage 02',[61],
         'Emit reads admission facts and a retained abstract program saved at target lowering, across selection, allocation and encoding.'),
        ('handlers','Handler round trip',[47,48],
         'Before instruction selection, the compiler builds required handlers as objects. Their stack and entry requirements return as installation facts.'),
        ('evaluation','Source evaluation',[16,17],
         'Source preparation sends a required value to the evaluator. The evaluator prepares and checks the invocation, runs it, and returns the computed value.'),
        ('policy','Policy loop',[22,23],
         'When provider demand changes, policy preparation reads the accepted publication and checked source inputs again. Settlement repeats with the prepared inputs.'),
        ('callbacks','Object-driven compilation',[75,76],
         'The compiler scans emitted object symbols, compiles the callbacks they reach, and sends those private objects to composition.'),
        ('runtime-layout','Runtime layout query',[73,74],
         'Runtime admission separately calls a helper from Omega 02 to compute storage layouts and installs them in the runtime provisions.'),
        ('frames','Late frame rewrite',[59,60],
         'After emission, applying frames inserts prologue and epilogue instructions and can re-encode branches whose destinations moved.'),
        ('image','Image-time codegen',[68,69],
         'Image construction can generate entry or adapter code after the body objects already exist.'),
    ]
    traces=[]
    by_id={e['id']:e for e in edges}
    for key,title,numbers,description in detours:
        ids=[f'overview-edge-{number}' for number in numbers]
        for edge_id in ids:
            by_id[edge_id]['detour']=key
        traces.append(dict(id=key,title=title,edges=ids,description=description))
    model = dict(nodes=nodes, edges=edges, spine=spine,traces=traces)
    validate(model)
    return model


def layout_model(model):
    if model.get('view')=='proposed':
        from .stage_flow import layout_stages
        return layout_stages(model)
    result = subprocess.run(['node', str(Path(__file__).with_name('layout.mjs'))],
                            input=json.dumps(model), text=True, encoding='utf-8',
                            capture_output=True)
    if result.returncode:
        raise ValueError('Diagram layout failed: '+result.stderr.strip())
    geometry = json.loads(result.stdout)
    positions = {n['id']: n for n in geometry['nodes']}
    routes = {e['id']: e for e in geometry['edges']}
    if positions.keys() != {n['id'] for n in model['nodes']} or routes.keys() != {e['id'] for e in model['edges']}:
        raise ValueError('Layout must retain every node and connection')
    return dict(model,width=geometry['width'], height=geometry['height'], groups=geometry['groups'], spine=model['spine'], startLabel=geometry['startLabel'],
                nodes=[dict(n, **{k:v for k,v in positions[n['id']].items() if k!='id'}) for n in model['nodes']],
                edges=[dict(e, **{k:v for k,v in routes[e['id']].items() if k not in ('id','sources','targets')}) for e in model['edges']])


def render_scene(model):
    nodes = {n['id']: n for n in model['nodes']}
    esc = html.escape
    parts = [f'<g class="connected-scene{" proposed-scene" if model.get("view")=="proposed" else ""}">']
    proposed=model.get('view')=='proposed'
    for g in model['groups']:
        parts.append(f'<g class="connected-group" data-group="{g["id"]}"><rect x="{g["x"]}" y="{g["y"]}" width="{g["width"]}" height="{g["height"]}"/>')
        for i,line in enumerate(g['caption']['lines']):
            parts.append(f'<text class="connected-phase" x="{g["caption"]["x"]+5}" y="{g["caption"]["y"]+i*26}">{esc(line)}</text>')
        parts.append('</g>')
    start=model['startLabel']
    parts.append(f'<text class="connected-start" x="{start["x"]}" y="{start["y"]+20}">{("START ↓" if proposed else "START")}</text>')
    for e in model['edges']:
        kind=e['kind'];css='attention' if kind in ('attention','auxiliary') else 'request' if kind in ('request','result') else kind
        if e.get('detour'):
            css='attention detour'
        marker='return' if 'attention' in css else 'check' if css=='check' else 'service' if not e.get('main') else 'arrow'
        parts.append(f'<g class="overview-edge {css}{" main-path" if e.get("main") else ""}" data-edge="{e["id"]}" data-from="{e["start"]}" data-to="{e["end"]}" tabindex="0" role="button" aria-label="{esc(nodes[e["start"]]["title"])} to {esc(nodes[e["end"]]["title"])}: {esc(e["label"])}"><title>{esc(e["label"])} · {esc(e["source"])}</title>')
        arrow=f' marker-end="url(#overview-{marker})"'
        for section in e.get('sections',[]):
            path='M'+' L'.join(f'{p["x"]:g} {p["y"]:g}' for p in section['points'])
            if e.get('detour') and not e.get('main'):
                tail=section['points'][0]
                parts.append(f'<circle class="wire-origin" cx="{tail["x"]}" cy="{tail["y"]}" r="3.2"/>')
            parts.append(f'<path d="{path}"{arrow}/><path class="connected-hit" d="{path}"/>')
        for label in e.get('labels',[]):
            parts.append(f'<text class="wire-caption" x="{label["x"]}" y="{label["y"]}" text-anchor="middle">{esc(label["text"])}</text>')
        parts.append('</g>')
    for n in model['nodes']:
        x,y,w,h=(n[k] for k in ('x','y','width','height'))
        href=f'{n["page"]}.html#{n["anchor"]}' if model.get('view')=='proposed' else f'#layer-{n["group"]}'
        parts.append(f'<a class="overview-node {n["kind"]} {"operation" if n["role"]=="transform" else ""}" data-role="{n["role"]}" data-layer="{n["group"]}" data-endpoint="{n["id"]}" href="{href}" aria-label="{esc(n["title"])}">')
        if not proposed:
            parts.append(f'<rect class="node-body" x="{x}" y="{y}" width="{w}" height="{h}"/>')
            title={'apply-frame':'Apply frames · rewrite','image-build':'Form image · late code','optimized-abstract':'Optimized abstract ops','installations':'Measure handler objects','native-run':'Run native executable','build':'Build · packages · library'}.get(n['id'],n['title'])
            if n['role']=='transform':
                title=title[:1].upper()+title[1:]
            parts.append(f'<text class="current-title" x="{x+w/2}" y="{y+h/2}" dominant-baseline="central" text-anchor="middle">{esc(title)}</text>')
        else:
            parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}"/>')
            parts.append(f'<text class="stage-title" x="{x+16}" y="{y+26}">{esc(n["title"])}</text>')
            for i,line in enumerate(n.get('notes',[])):
                parts.append(f'<text class="overview-step-note" x="{x+16}" y="{y+47+i*19}">{esc(line)}</text>')
            if n['reference']:
                parts.append(f'<text class="overview-step-note" x="{x+w-12}" y="{y+25}" text-anchor="end">{esc(n["reference"])}</text>')
        parts.append('</a>')
    parts.append('</g>')
    return ''.join(parts)


def render_overview(architecture):
    from .proposed import proposed_model
    model = layout_model(overview_model(architecture))
    revision=re.search(r'/blob/([0-9a-f]{40})/',model['nodes'][0]['source']).group(1)
    model.update(view='current',badge=f'Current · compiler snapshot {revision[:12]}',
                 intro='Follow the numbered stages across the map. Orange arrows mark the dependencies flagged in the repair plan.')
    proposed=layout_model(proposed_model())
    nodes = {n['id']: n for n in model['nodes']}
    esc = html.escape
    trace_options=''.join(f'<option value="{t["id"]}">{esc(t["title"])}</option>' for t in model['traces'])
    options=''.join(f'<option value="{n["id"]}">{esc(n["title"])}</option>' for n in proposed['nodes'] if n['role']=='owner')
    footer=(
        '<div class="map-footer">'
        '<div class="arch-overview-legend" data-legend="current"><span class="key-main">Stage handoff</span><span class="key-service">Supporting call or data</span><span class="key-detour">Pipeline detour or late code</span></div>'
        '<div class="arch-overview-legend" data-legend="proposed" hidden><span>00–10: stage owners</span><span>01.1, 01.2…: unfolded operations</span><span>Within a stage: dependency order; work may repeat</span></div>'
        '<div class="camera-tools" role="group" aria-label="Canvas navigation" hidden>'
        '<button type="button" data-camera="start">Start</button><button type="button" data-camera="out" aria-label="Zoom out">−</button><button type="button" data-camera="natural" aria-label="Reset zoom to 100%" title="Reset zoom to 100%"><output class="connected-scale">100%</output></button><button type="button" data-camera="in" aria-label="Zoom in">+</button><button type="button" data-camera="fit">Fit whole compiler</button><button type="button" data-camera="expand">Expand</button></div></div>'
    )
    parts = ['<div class="arch-reading" id="compiler-overview" data-view="current">',
             '<div class="connected-views" role="group" aria-label="Architecture view" hidden><button type="button" data-view="current" aria-pressed="true">Current</button><button type="button" data-view="proposed" aria-pressed="false">Proposed</button></div>',
             f'<p class="connected-intro" role="status" aria-live="polite">{esc(model["intro"])}</p>',
             f'<div class="connected-tools" hidden><label class="trace-picker">Trace <select aria-label="Trace a pipeline detour"><option value="">All connections</option>{trace_options}</select></label><label class="stage-picker" hidden>Stage <select aria-label="Jump to stage">{options}</select></label><label class="component-picker">Find <select class="connected-jump" aria-label="Find a component"><option value="">Choose a component…</option>']
    for n in model['nodes']:
        if n['role']!='transform':
            parts.append(f'<option value="{n["id"]}">{esc(n["title"])}</option>')
    parts.append('</select></label><button type="button" data-camera="clear">Clear selection</button></div><p class="trace-explanation" hidden></p><div class="connected-workspace"><div class="connected-viewport" tabindex="0" role="region" aria-label="Connected compiler canvas. Drag to pan, scroll to zoom. Arrow keys pan, F fits all, Escape clears selection.">')
    parts.append(f'<svg class="arch-overview-svg" role="img" width="{model["width"]}" height="{model["height"]}" viewBox="0 0 {model["width"]} {model["height"]}" aria-label="Current compiler architecture"><defs>')
    for name in ('arrow','return','check','service'):
        parts.append(f'<marker id="overview-{name}" viewBox="0 0 10 10" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="10" markerHeight="10" orient="auto"><path d="M0 0 L10 5 L0 10 z"/></marker>')
    parts.append('</defs>'+render_scene(model))
    parts.append('</svg></div><aside class="connected-detail" aria-label="Selected component and source" hidden></aside><div class="connected-minimap" hidden><span>Whole compiler</span><svg role="img" aria-label="Whole compiler minimap"></svg></div>'+footer+'</div><p class="connected-help">Drag to pan · scroll or pinch to zoom · select a connection to follow it · F fits the whole compiler.</p><noscript><p>Scroll inside the current drawing to explore it. The source and proposed-contract lists below work without JavaScript.</p></noscript><details class="arch-arrow-evidence"><summary>Current arrows and pinned source</summary><ol>')
    for e in model['edges']:
        if e['label']:
            parts.append(f'<li>{esc(nodes[e["start"]]["title"])} → {esc(nodes[e["end"]]["title"])}: {esc(e["label"])} · <a href="{e["source"]}">Source</a></li>')
    parts.append('</ol></details><script type="application/json" id="overview-model">'+json.dumps(model,ensure_ascii=False).replace('<','\\u003c')+'</script>')
    parts.append('<template id="overview-proposed-scene"><svg xmlns="http://www.w3.org/2000/svg" aria-hidden="true">'+render_scene(proposed)+'</svg></template>')
    parts.append('<script type="application/json" id="overview-proposed-model">'+json.dumps(proposed,ensure_ascii=False).replace('<','\\u003c')+'</script>')
    parts.append('<details class="proposed-contracts"><summary>Proposed contracts and open questions</summary><p>Proposed phase boundaries are judgment, not implemented or verified behavior. <a href="foundation-backend-plan.html#8-planning-completion-gates-before-implementation">Open planning gates</a>.</p><dl>')
    for n in proposed['nodes']:
        parts.append(f'<dt>{esc(n["title"])}</dt><dd><p>{esc(n["description"])}</p>')
        if n['gate']:
            parts.append(f'<p>Open contract: {esc(n["gate"])}</p>')
        parts.append(f'<a href="{n["page"]}.html#{n["anchor"]}">Read proposal</a> · <a href="{n["source"]}">Recorded plan</a></dd>')
    parts.append('</dl></details>')
    return ''.join(parts)
