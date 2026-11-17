"""Cited overview connections with named endpoints, including operation inputs.

Aliases repeat a retained artifact or an owner in a separate invocation context.
Coordinates route connections; they cannot supply anonymous arrow endpoints.
"""
import html
import json
import re


def validate(model):
    nodes = {n['id']: n for n in model['nodes']}
    if len(nodes) != len(model['nodes']):
        raise ValueError('Duplicate overview endpoint')
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
        for endpoint, port in ((start, edge['start_port']), (end, edge['end_port'])):
            if port not in ('left', 'right', 'top', 'bottom'):
                raise ValueError('Unknown endpoint port')
            if endpoint['role'] == 'transform' and endpoint['width'] != 0:
                raise ValueError('Transform inputs must attach to a named join')


def overview_model(architecture):
    records = {n['id']: n for n in architecture['nodes']}
    base = records['o0']['source'].split('/omega-rust/')[0] + '/omega-rust/'
    nodes, edges, labels = [], [], []

    def source(owner, path=None):
        # Cite the coordinating calls when a representation's declaration alone
        # would establish its shape but not the arrow's claimed operation.
        calls = {'p2':'psi/compilation/pipeline/02_syntax-trees-to-symbol-resolved-trees/src/resolution.rs#L2017',
                 'p4':'psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/preparation.rs#L153',
                 'p5':'psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs#L24',
                 'native-run':'omega/application/src/execution/mod.rs#L244'}
        native_lines = {'o1':99,'o2':84,'o4':175,'o5':197,'o6':202,'frame-plan':205,'o7':211,'o8':216,'o9':220}
        if path is None and owner in native_lines:
            path = f'omega/compilation/native-compiler/src/pipeline.rs#L{native_lines[owner]}'
        if path is None:
            path = calls.get(owner)
        return base + path if path else records[owner]['source']

    def box(key, owner, group, title, x, y, width=180, kind='psi', note=None):
        nodes.append(dict(id=key, owner=owner, group=group, title=title, x=x, y=y,
                          width=width, height=72 if note else 60,
                          role='owner' if kind == 'owner' else 'artifact',
                          kind=kind, note=note))

    def label(x, y, text, kind='overview-layer-label', anchor='start'):
        labels.append(dict(x=x, y=y, text=text, kind=kind, anchor=anchor))

    def connect(a, b, title, evidence, ports=('right', 'left'), via=(), pos=None, kind='data'):
        edges.append(dict(id=f'overview-edge-{len(edges)}', start=a, end=b,
                          label=title, source=evidence, start_port=ports[0],
                          end_port=ports[1], via=list(via), pos=pos, kind=kind))

    def transform(key, a, b, title, owner, x, y, ports=('right', 'left'),
                  via=(), evidence=None, kind='data', label_y=None):
        nodes.append(dict(id=key, owner=owner, group=next(n['group'] for n in nodes if n['id']==b),
                          title=title, x=x, y=y, width=0, height=0,
                          role='transform', kind=kind, note=None))
        citation = evidence or source(owner)
        connect(a, key, '', citation, ports=(ports[0], 'left'), via=via, kind=kind)
        connect(key, b, title, citation, ports=('right', ports[1]),
                pos=(x, label_y if label_y is not None else y-48), kind=kind)

    label(50, 28, 'SOURCE COMPILATION · PSI · target-neutral')
    box('build', 'build', 'inputs', 'Build · packages · library', 50, 48, 450, 'owner')
    box('compiler', 'source', 'source', 'Source compiler', 800, 48, 430, 'owner')
    connect('build', 'compiler', 'source request', source('build'), pos=(650, 66), kind='request')
    # The source owner supplies the first representation; the outline is not a node.
    for key, title, x in [('files','Source files',50),('tokens','Tokens',300),
                          ('trees','Syntax trees',550),('resolved','Resolved trees',800),
                          ('typed','Typed trees',1050)]:
        box(key, 'source', 'source', title, x, 180)
    connect('compiler','files','loaded sources',source('source'),('bottom','top'),
            [(1015,145),(140,145)],(600,137))
    for i, a, b, x, title in [(0,'files','tokens',265,'00 lex'),(1,'tokens','trees',515,'01 parse'),
                              (2,'trees','resolved',765,'02 resolve'),(3,'resolved','typed',1015,'03 type')]:
        transform(f'psi-{i}',a,b,title,f'p{i}',x,210)
    box('frozen','p4','source','Frozen typed trees',1050,310)
    box('lowered','p5','source','Lowered Psi',800,310)
    box('accepted','checked','source','Accepted Psi',550,310,kind='accepted')
    transform('psi-4','typed','frozen','04 prepare','p4',1140,275,('bottom','top'),label_y=280)
    transform('psi-5','frozen','lowered','05 lower','p5',1015,340,('left','right'))
    transform('psi-6','lowered','accepted','06 check','p6',765,340,('left','right'),kind='check')
    label(1230,400,'The lowered-form checker gives the only acceptance verdict','overview-verdict','end')

    label(50,452,'DURING SOURCE PREPARATION · a separate checked invocation for a compile-time value')
    box('source-caller','source','source','Source compiler',50,480,260,'owner')
    box('source-eval','evaluation','evaluation','Source evaluator',530,480,260,'owner')
    box('source-engine','engine','runtime','Psi interpreter',1010,480,220,'owner')
    connect('source-caller','source-eval','evaluate value',source('source','psi/compilation/source-compiler/src/source_checking.rs#L119'),pos=(420,493),kind='request')
    connect('source-eval','source-caller','computed value → preparation continues',source('evaluation'),('bottom','bottom'),[(660,590),(180,590)],(420,580),kind='result')
    connect('source-eval','source-engine','admit, then invoke',source('evaluation'),pos=(900,493),kind='request')
    connect('source-engine','source-eval','result + work',source('evaluation'),('bottom','bottom'),[(1120,570),(660,570)],(900,560),kind='result')

    label(50,645,'PROVIDER CLOSURE · OMEGA · target and provider choices enter here')
    box('closure-input','checked','providers','Accepted Psi + Build choices',50,675,300,'owner')
    box('closure','providers','providers','Provider closure',530,675,260,'owner')
    box('chosen','selected','providers','Selected inputs',1010,675,220,'owner')
    connect('closure-input','closure','code, target, bindings',source('providers'),pos=(440,690))
    connect('closure','chosen','stable bindings',source('providers'),pos=(900,690))
    box('policy-inputs','policy-inputs','providers','Prepare policy inputs',530,795,260,'owner')
    box('policy-eval','policy','evaluation','Calling-policy evaluator',1010,795,220,'owner')
    connect('closure','policy-inputs','changed reach',source('providers'),('bottom','top'),pos=(735,770),kind='request')
    connect('policy-inputs','closure','prepared inputs; repeat',source('policy-inputs'),('left','left'),[(475,825),(475,705)],(365,770),kind='result')
    connect('policy-inputs','policy-eval','evaluate policy',source('policy-inputs'),pos=(900,810),kind='request')
    connect('policy-eval','policy-inputs','validated plan + work',source('policy'),('bottom','bottom'),[(1120,895),(660,895)],(900,885),kind='result')
    box('policy-engine','engine','runtime','Psi interpreter',1010,925,220,'owner')
    connect('policy-eval','policy-engine','admit, then invoke',source('policy'),('bottom','top'),pos=(1200,910),kind='request')
    connect('policy-engine','policy-eval','result + work',source('policy'),('right','right'),[(1290,955),(1290,825)],(1250,865),kind='result')
    label(50,925,'This loop reuses accepted code and repeats policy-input preparation.','overview-layer-note')
    label(50,950,'It does not restart source compilation.','overview-layer-note')

    label(50,982,'RUN AS PSI · retained code and admitted runtime provisions must meet at the host')
    box('runtime-input','selected','runtime','Selected inputs',50,1010,220,'owner')
    box('admit','runtime-admission','runtime','Runtime admission',430,1010,230,'owner')
    box('runtime-host','host','runtime','Omega runtime host',810,1010,220,'owner')
    box('run-engine','engine','runtime','Psi interpreter',1090,1130,180,'owner')
    connect('runtime-input','admit','runtime inputs',source('runtime-admission'),pos=(350,1023))
    connect('admit','runtime-host','admitted provisions',source('runtime-admission'),pos=(735,1023))
    box('retained','checked','runtime','Retained Psi code',430,1130,230)
    connect('retained','runtime-host','same publication identity',source('host','omega/interpreter/terminal-runtime/src/execution.rs#L180'),('right','bottom'),[(920,1160)],(810,1145))
    connect('runtime-host','run-engine','bind, invoke / resume',source('host','omega/interpreter/terminal-runtime/src/execution.rs#L353'),('right','top'),[(1180,1040)],(1160,1100),kind='request')
    connect('run-engine','runtime-host','status + output + work',source('host','omega/interpreter/terminal-runtime/src/execution.rs#L421'),('bottom','bottom'),[(1180,1235),(920,1235)],(1050,1225),kind='result')
    box('worker','worker','runtime','Native-provider worker',50,1130,280,'owner')
    worker_source=source('worker','omega/interpreter/terminal-runtime/src/execution/host_imports/native_provider.rs#L1')
    connect('runtime-host','worker','external calls via host adapter',worker_source,('left','top'),[(770,1040),(770,1090),(190,1090)],(485,1082),kind='request')
    connect('worker','runtime-host','checked replies',worker_source,('right','bottom'),[(360,1160),(360,1120),(850,1120)],(570,1110),kind='result')
    label(50,1235,'Private external-call service; separate from launching the native program below.','overview-layer-note')
    label(50,1265,'Runtime admission performs native admission and abstract optimization, but emits no native image.','overview-layer-note')

    label(50,1330,'COMPILE NATIVE · OMEGA · per target · Selected inputs are the same selection shown above')
    box('native-input','selected','providers','Selected inputs',1050,1360,180,'owner')
    for key, title, x, owner in [('abstract','Abstract operations',1050,'o0'),('target','Target operations',800,'o2'),
                                ('instructions','Selected instructions',550,'o3'),('homes','Register homes',300,'o5'),
                                ('physical','Physical operations',50,'o6')]:
        box(key,owner,'native',title,x,1480,kind='native')
    transform('native-0','native-input','abstract','00 admit / lower','o0',1140,1450,('bottom','top'),label_y=1443)
    transform('native-2','abstract','target','02 lower','o2',1015,1510,('left','right'))
    transform('native-3','target','instructions','03 select','o3',765,1510,('left','right'),evidence=source('o3','omega/compilation/native-compiler/src/pipeline.rs#L159'))
    transform('native-5','instructions','homes','05 allocate','o5',515,1510,('left','right'))
    transform('native-6','homes','physical','06 lower','o6',265,1510,('left','right'))
    # Same-form transformations are explicit self-loops on named artifacts.
    connect('abstract','abstract','01 optimize',source('o1'),('right','top'),[(1270,1510),(1270,1455),(1140,1455)],(1260,1432))
    connect('instructions','instructions','04 optimize',source('o4'),('top','top'),[(640,1430),(690,1430)],(665,1414))
    box('installations','binding','native','Handlers → measured binding',800,1620,350,'owner')
    connect('target','installations','if required',source('handlers'),('bottom','top'),pos=(955,1588),kind='request')
    connect('installations','native-3','bound installation facts',source('binding','omega/compilation/native-compiler/src/pipeline.rs#L159'),('left','bottom'),[(765,1650)],(685,1635),kind='auxiliary')
    label(1230,1705,'Handler compilation can recurse through the native driver; resource cycles are refused.','overview-layer-note','end')
    box('framed','frame-plan','native','Framed program',50,1620,kind='native')
    transform('plan-frame','physical','framed','plan frames','frame-plan',140,1580,('bottom','top'),label_y=1580)
    for key, title, x, owner, group in [('encoded','Encoded forms',50,'o7','encoding'),('layout','Resolved layout',300,'o8','encoding'),
                                       ('optimized','Optimized layout',550,'o9','encoding'),('fragments','Function fragments',800,'fragments','emission'),
                                       ('framed-fragments','Framed fragments',1050,'frames','emission')]:
        box(key,owner,group,title,x,1790,kind='native')
    transform('native-7','framed','encoded','07 encode','o7',140,1740,('bottom','top'),label_y=1740)
    transform('native-8','encoded','layout','08 place','o8',265,1820)
    transform('native-9','layout','optimized','09 optimize','o9',515,1820)
    transform('emit','optimized','fragments','emit','fragments',765,1820)
    transform('apply-frame','fragments','framed-fragments','apply frames','frames',1015,1820,kind='attention')
    connect('target','emit','EmissionFacts · retained abstract program',source('o2','omega/compilation/native-compiler/src/pipeline.rs#L261'),('top','top'),[(890,1428),(990,1428),(990,1338),(1310,1338),(1310,1760),(765,1760)],(1010,1740),kind='auxiliary')
    for key, title, x, owner, group in [('text','Text section',1050,'text','emission'),('objects','Objects',800,'objects','emission'),
                                       ('composed','Composed bodies',550,'compose','emission'),('image','Native image',300,'image','image'),
                                       ('published','Published executable',50,'products','image')]:
        box(key,owner,group,title,x,1950,kind='native')
    transform('place-text','framed-fragments','text','place text','text',1140,1900,('bottom','top'),label_y=1905)
    for key,a,b,title,owner,x,kind in [('object','text','objects','build objects','objects',1015,'data'),
                                     ('compose','objects','composed','compose','compose',765,'data'),
                                     ('image-build','composed','image','form image','image',515,'attention'),
                                     ('publish','image','published','publish','products',265,'data')]:
        transform(key,a,b,title,owner,x,1980,('left','right'),kind=kind)
    label(660,2050,'Image construction also generates entry and adapter code','overview-return-label','middle')
    box('native-run','native-run','run','Native program · operating system',530,2100,440,'owner')
    connect('published','native-run','CLI launches when requested',source('native-run'),('bottom','left'),[(140,2130)],(330,2115),kind='request')
    label(50,2210,'Repeated boxes name the same artifact or owner in a different route. Requests and results have separate arrows.','overview-layer-note')
    box('support','docs','support','Outside execution: Omega compiler · bootstrap · foundation · specification · tests · tools',50,2240,1180,'owner')
    # Leave a clear gutter between policy execution and program execution.
    def lower_y(y):
        return y+90 if y>=980 else y
    for node in nodes:
        node['y']=lower_y(node['y'])
    for item in labels:
        item['y']=lower_y(item['y'])
    for edge in edges:
        edge['via']=[(x,lower_y(y)) for x,y in edge['via']]
        if edge['pos']:
            x,y=edge['pos'];edge['pos']=(x,lower_y(y))
    model = dict(width=1320, height=2420, nodes=nodes, edges=edges, labels=labels)
    validate(model)
    return model


def render_overview(architecture):
    model = overview_model(architecture)
    nodes = {n['id']: n for n in model['nodes']}
    esc = html.escape

    def point(node, port):
        x,y,w,h = (node[k] for k in ('x','y','width','height'))
        return {'left':(x,y+h/2),'right':(x+w,y+h/2),
                'top':(x+w/2,y),'bottom':(x+w/2,y+h)}[port]

    parts = ['<div class="arch-reading" id="compiler-overview"><div class="arch-overview-tools"><p>Forms flow through numbered stages. Named owners receive requests. Select a box for evidence.</p><button type="button" data-map="fullscreen">Expand diagram</button></div>',
             '<div class="arch-overview-legend"><span class="legend-psi">Psi form</span><span class="legend-native">Native form</span><span>Shaded box: owner / grouped inputs</span><span class="legend-check">Checker verdict</span><span>Dashed: request or result</span><span class="arch-attention">Red: extra input or late code production</span></div>',
             f'<svg class="arch-overview-svg" viewBox="0 0 {model["width"]} {model["height"]}" role="img" aria-label="Compiler routes with named, source-cited arrow endpoints"><title>Compiler routes and operation inputs</title><defs>']
    for name in ('arrow','return','check'):
        parts.append(f'<marker id="overview-{name}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto"><path d="M0 0 L10 5 L0 10 z"/></marker>')
    parts.append('</defs>')
    for edge in model['edges']:
        a,b = nodes[edge['start']],nodes[edge['end']]
        points = [point(a,edge['start_port']), *edge['via'], point(b,edge['end_port'])]
        path = 'M'+' L'.join(f'{x:g} {y:g}' for x,y in points)
        kind = edge['kind']
        css = 'attention' if kind in ('attention','auxiliary') else 'request' if kind in ('request','result') else kind
        marker = 'return' if css=='attention' else 'check' if kind=='check' else 'arrow'
        arrow = '' if b['role']=='transform' and kind!='auxiliary' else f' marker-end="url(#overview-{marker})"'
        parts.append(f'<g class="overview-edge {css}" data-from="{a["id"]}" data-to="{b["id"]}"><title>{esc(a["title"])} → {esc(b["title"])}: {esc(edge["label"])} · {esc(edge["source"])}</title><path d="{path}"{arrow}/>')
        if edge['label'] and edge['pos']:
            x,y=edge['pos']
            parts.append(f'<text x="{x}" y="{y}" text-anchor="middle">{esc(edge["label"])}</text>')
        parts.append('</g>')
    for n in model['nodes']:
        if n['role']=='transform':
            if any(e['kind']=='auxiliary' and e['end']==n['id'] for e in model['edges']):
                parts.append(f'<circle class="overview-join" cx="{n["x"]}" cy="{n["y"]}" r="3"><title>{esc(n["title"])}</title></circle>')
            continue
        x,y,w,h=(n[k] for k in ('x','y','width','height'))
        parts.append(f'<a class="overview-node {n["kind"]}" data-layer="{n["group"]}" data-endpoint="{n["id"]}" href="#layer-{n["group"]}"><rect x="{x}" y="{y}" width="{w}" height="{h}"/>')
        # Explicit title lines avoid truncation and accidental prose wrapping.
        title=n['title']
        lines = title.split(' · ') if len(title)>37 and w<300 else [title]
        if (len(title)>18 and w<=180) or (len(title)>22 and w<=230):
            words=title.split();middle=len(words)//2
            lines=[' '.join(words[:middle]),' '.join(words[middle:])]
        for i,line in enumerate(lines):
            parts.append(f'<text class="overview-box-title" x="{x+w/2}" y="{y+h/2+7-(len(lines)-1)*14+i*28}" text-anchor="middle">{esc(line)}</text>')
        parts.append('</a>')
    for label in model['labels']:
        parts.append(f'<text class="{label["kind"]}" x="{label["x"]}" y="{label["y"]}" text-anchor="{label["anchor"]}">{esc(label["text"])}</text>')
    parts.append('</svg><details class="arch-arrow-evidence"><summary>Inspect every arrow and its pinned source</summary><ol>')
    for edge in model['edges']:
        if not edge['label']:
            continue
        parts.append(f'<li>{esc(nodes[edge["start"]]["title"])} → {esc(nodes[edge["end"]]["title"])}: {esc(edge["label"])} · <a href="{edge["source"]}">Source</a></li>')
    parts.append('</ol></details><script type="application/json" id="overview-model">'+json.dumps(model,ensure_ascii=False).replace('<','\\u003c')+'</script>')
    return ''.join(parts)
