"""Draw the recorded migration contracts, separately from observed compiler code.

These are proposed phase boundaries. The plan explicitly leaves their feasibility
gates open and assigns no replacement stage numbers.
"""
from .overview import validate

PLAN_REVISION = 'a34aa689afbd48481d1b9f76f6dde013892af051'
PLAN_PATH = 'atlas/plans/foundation-and-backend.md'


def proposed_model():
    def source(first=183,last=197):
        return f'https://github.com/CathedralOS/Omega/blob/{PLAN_REVISION}/{PLAN_PATH}#L{first}-L{last}'

    nodes=[]
    def node(key,title,role,kind,group,description,anchor='3-recommended-destination-and-stage-contracts',gate=''):
        spans={'4-source-settlement-evaluation-and-provider-body-closure':(204,242),
               'evaluation-is-an-explicit-operation-on-accepted-code':(244,266),
               '5-whole-program-reach-generated-code-and-resources':(292,346),
               'binding-uses-a-complete-resource-summary':(386,413),
               'encoding-and-layout-have-a-closed-executable-input':(415,431),
               'runtimes-consume-products-and-admitted-provision-records':(485,501),
               'verification-has-explicit-inputs-and-independent-authority':(503,516)}
        nodes.append(dict(id=key,owner=key,title=title,role=role,kind=kind,group=group,
                          description=description,page='foundation-backend-plan',anchor=anchor,
                          source=source(*spans.get(anchor,(183,197))),gate=gate,width=220,height=60))

    node('build','Build/package admission','owner','owner','inputs',
         'Packages and Build admit source bytes, scoped roots and explicit requests. Availability does not activate every file.')
    node('source-input','Admitted source inputs','artifact','data','source',
         'Immutable admitted bytes, source custody, scoped roots, neutral Build selections and execution budgets.')
    node('source-settlement','Source settlement','owner','owner','source',
         'Owns lazy source activation, resolution, typing, preparation and logical provider/body discovery. It prepares and checks exact evaluation invocations; only the lowered-form checker accepts code.',
         '4-source-settlement-evaluation-and-provider-body-closure',
         'Activation/error order, charging and logical provider separation must preserve existing results and refusals.')
    node('accepted','Accepted program','artifact','data','source',
         'Accepted target-neutral code, closed logical body selections, prepared facts and narrow provenance. Live source lookup stays inside settlement.')
    node('seal','Portable sealing','owner','owner','source',
         'Serializes the accepted result and its required evidence; sealing does not issue a new semantic verdict.')
    node('portable','Canonical Psi','artifact','data','source',
         'The portable product and accepted code catalog, with exact identities and summaries for downstream consumers.')
    node('native-planning','Native program planning','owner','owner','native',
         'Closes retained native body/reference demand over accepted code. Establishes call/ABI plans and generated-body recipes without reopening source.',
         '5-whole-program-reach-generated-code-and-resources',
         'Prove later encoding/layout cannot introduce another required body, while preserving optimization pruning.')
    node('selected-program','Selected native program','artifact','native','native',
         'Selected bodies, ABI plans, obligations and generated-body recipes. Required metadata travels with the current form.')
    node('physical-planning','Physical program planning','owner','owner','native',
         'Represents ordinary and generated bodies together, including handlers, entries and adapters; establishes registers, frame recipes, effects and symbolic resource uses.',
         '5-whole-program-reach-generated-code-and-resources',
         'Complete generated-body/effect inputs and a finite resource dependency order remain to be proved.')
    node('physical-program','Complete physical program','artifact','native','native',
         'Every retained executable body is represented with effects, frames and resource dependencies. Handler measurements no longer require a detour through object emission.')
    node('binding','Resource/installation binding','owner','owner','native',
         'Derives complete resource summaries and checks admitted installation limits for this physical program, including transitive stack, bridges, alignment and fault premises.',
         'binding-uses-a-complete-resource-summary',
         'Resolving a bound must not alter allocation, effects, frame demand or body membership.')
    node('bound-program','Bound physical program','artifact','native','native',
         'The complete physical program plus bound resources and installation identities, ready for common encoding.')
    node('encoding','Common encoding','owner','owner','encoding',
         'Encodes every ordinary and generated body, including frame sequences. Publishes legal alternatives and complete fixup descriptions.',
         'encoding-and-layout-have-a-closed-executable-input',
         'Complete alternatives/fixups for both ISAs and all formats; no later executable-body producer.')
    node('encoded-program','Encoded program','artifact','native','encoding',
         'Encoded bodies, frame fragments, alternatives and fixups; preserves attribution and identity records without an earlier executable graph.')
    node('layout-stage','Layout and relaxation','owner','owner','encoding',
         'Places encoded fragments and selects/patches carried alternatives. Preserves body-first relaxation and frame placement without calling an earlier encoder.')
    node('laid-out','Laid-out program','artifact','native','encoding',
         'Resolved text, references and placement records; all executable content was supplied by encoding.')
    node('object-stage','Object construction','owner','owner','encoding',
         'Constructs object containers from laid-out content, symbols and format requests. No source or ABI rediscovery and no private helper generation.')
    node('objects','Objects','artifact','native','encoding',
         'Completed objects with carried attribution, entry, relocation and evidence records.')
    node('image-stage','Image construction','owner','owner','encoding',
         'Places objects and produces container/loader metadata, relocations and signatures. Entry and adapter machine code is already encoded.')
    node('image','Completed image','artifact','native','encoding',
         'Final image, regions, symbols and required runtime/provision carriers. No fresh register, frame or instruction decisions remain.')
    node('publish','Product publication','owner','owner','products',
         'Products and host support install completed artifacts and return receipts. Filesystem publication performs no compiler transformations.')
    node('published','Published product','artifact','native','products',
         'Installed executable/component products and publication receipts.')
    node('native-run','Native execution','owner','owner','products',
         'The operating system runs the requested published native product; execution is a consumer of compilation.')
    node('executor','Accepted-code executor','owner','owner','services',
         'Runs an already accepted invocation with sealed admission, actual arguments and a work account. Returns values/status and receipts. Source preparation stays with settlement; policy-result interpretation stays with native planning.',
         'evaluation-is-an-explicit-operation-on-accepted-code')
    node('engine','Psi interpreter','owner','owner','services',
         'One target-neutral interpreter serves compile-time invocations and runtime execution. It consumes accepted code and admitted execution inputs.',
         'runtimes-consume-products-and-admitted-provision-records')
    node('runtime-host','Runtime host','owner','owner','services',
         'Consumes the portable product and admitted provisions, joins host authority, and requests execution. Psi-only use need not compile the main body natively.',
         'runtimes-consume-products-and-admitted-provision-records')
    node('runtime-provisions','Admitted runtime provisions','artifact','owner','services',
         'A declared admission input carrying program/target/host identities. Required foreign adapters are completed auxiliary native products through the common route, not code generated inside this carrier.',
         'runtimes-consume-products-and-admitted-provision-records',
         'Exact carrier fields and issuing authority, including auxiliary products, remain a planning gate.')
    node('native-inputs','Admitted native inputs','artifact','owner','services',
         'Selected target description, admitted components/installations and the native request enter native planning explicitly; needed records travel forward.')
    node('verifier','Independent verifier','owner','owner','services',
         'Consumes the requested current form and explicitly retained evidence. It does not produce program meaning or provide a driver callback inside the program.',
         'verification-has-explicit-inputs-and-independent-authority')
    node('replay','Requested audit evidence','artifact','owner','services',
         'Exact optional originals/evidence retained for a validator request, outside ordinary program production. Missing requested evidence refuses that audit.')
    node('audit-result','Audit result','artifact','data','services',
         'Independent validation results and diagnostics, rather than another program transformation.')

    spine=[n['id'] for n in nodes if n['group']!='services']
    by_id={n['id']:n for n in nodes}
    edges=[]
    def edge(start,end,label='',kind='data',main=False):
        edges.append(dict(id=f'proposed-{start}-{end}',start=start,end=end,label=label,
                          kind=kind,main=main,source=by_id[end]['source']))
    for start,end in zip(spine,spine[1:]):
        edge(start,end,main=True)
    for start,end,label,kind in (
        ('source-settlement','executor','Accepted invocation','request'),
        ('executor','source-settlement','Value + receipts','result'),
        ('native-planning','executor','Accepted policy invocation','request'),
        ('executor','native-planning','Policy result + receipts','result'),
        ('executor','engine','Admit and execute','request'),
        ('engine','executor','Result + work','result'),
        ('portable','runtime-host','Portable product','data'),
        ('runtime-provisions','runtime-host','Admitted capabilities','data'),
        ('runtime-host','engine','Invoke / resume','request'),
        ('engine','runtime-host','Status + work','result'),
        ('native-inputs','native-planning','Target + admissions','data'),
        ('encoded-program','verifier','Requested current form','request'),
        ('replay','verifier','Requested evidence','data'),
        ('verifier','audit-result','Result / diagnostics','data'),
    ):
        edge(start,end,label,kind)
        if {start,end}=={'native-planning','executor'}:
            edges[-1]['source']=source(266,290)
    model=dict(nodes=nodes,edges=edges,spine=spine,view='proposed',status='unverified',plan_revision=PLAN_REVISION,
               badge=f'Proposed · plan {PLAN_REVISION[:12]}',
               intro='Proposed · unverified. Select a box for its contract and open questions.',
               phaseGaps={'source-settlement':24,'native-planning':32,'encoding':32,'publish':24},
               phases=[['source-group','source-settlement','portable',['Source and','acceptance']],
                       ['native-group','native-planning','bound-program',['Native planning','and binding']],
                       ['encoding-group','encoding','image',['Encoding','and images']],
                       ['product-group','publish','native-run',['Publication','and execution']]],
               placements=[dict(id='executor',x=680,anchor='source-settlement',offset=0),
                           dict(id='engine',x=1020,anchor='executor',offset=130),
                           dict(id='runtime-host',x=680,anchor='portable',offset=40),
                           dict(id='runtime-provisions',x=1020,anchor='runtime-host',offset=160),
                           dict(id='native-inputs',x=680,anchor='native-planning',offset=60),
                           dict(id='verifier',x=680,anchor='encoded-program',offset=0),
                           dict(id='replay',x=1020,anchor='verifier',offset=0),
                           dict(id='audit-result',x=680,anchor='verifier',offset=150)])
    validate(model)
    return model
