"""Proposed stages 00–10 carry inputs through one forward chain.

These are design contracts, not observed implementation boundaries. Compiler
work stays inside its stage; runtime execution consumes completed products.
"""
from .overview import validate
from .proposed_operations import unfold

PLAN_REVISION = 'd6ea87967106d47eb007b4780c7df2974de8fe37'
PLAN_PATH = 'atlas/plans/foundation-and-backend.md'


# Each entry points at the repair condition in the audited plan and at the
# operations a reader can inspect on the proposed canvas. The runtime repair
# names the data to carry but leaves its producing stage open.
DETOUR_REPAIRS = {
    'evaluation': (87, ['source-settlement-prepare', 'accepted'],
        'Source preparation evaluates required invocations inside stage 01. Its result carries the computed values and receipts.'),
    'policy': (88, ['accepted', 'native-planning-policies'],
        'Stage 01 settles source choices. Stage 03 computes calling policies from the carried choices and accepted policy code.'),
    'handlers': (459, ['native-planning-target', 'native-planning-select', 'physical-planning-generated', 'binding-summarize', 'binding-bind'],
        'Open 03–05 handler handoff. 03.5 carries the permitted nesting for each retained installation. 03.6 uses its dedicated-stack pairs to select guards. 05 checks the finished bodies and measures the stack along those same nesting paths.'),
    'emission': (85, ['native-planning-target', 'encoding-bodies'],
        'From target lowering onward, each form carries the facts its successor needs. Encoding can consume physical code and those facts directly.'),
    'callbacks': (89, ['native-planning-closure', 'object-stage-container'],
        'Native planning closes demand for callback bodies before encoding. Object construction receives the resulting laid-out code.'),
    'frames': (90, ['encoding-bodies', 'encoding-fixups', 'layout-stage-relax'],
        'Encoding emits frames, alternatives and fixups together. Layout picks an encoding and patches it.'),
    'image': (91, ['physical-planning-generated', 'encoding-bodies', 'image-stage-finish'],
        'Entries and adapters join the physical program in stage 04 and pass through common encoding. The image stage finishes container metadata.'),
    'runtime-layout': (92, [],
        'Runtime provisions would consume explicit storage-layout records. The plan leaves the producing stage open. Psi-only execution must still work.'),
}


# Source observations describe the current compiler. The proposed stage order
# and the feasibility of filling numeric operands at 05 remain separate claims.
def annotate_handler_handoff(nodes):
    code='https://github.com/CathedralOS/Omega/blob/c657528b7e33646edee29219834b30060a3fabbc/omega-rust/omega/compilation/'
    selection=code+'pipeline/03_target-operations-to-selected-instructions/src/selection_interrupts.rs'
    object_code=code+'backend/images/image-emission/src/object_artifact.rs'
    requirement=code+'backend/runtime/installation-evidence/src/lib.rs'
    admission=code+'realization/provider-admission/src/static_interrupts/scopes.rs'
    driver=code+'native-compiler/src/realization/callback_thunks.rs'
    annotations={
        'native-planning-target': (
            ['Keep each retained installation’s', 'nesting rules and stack choices.', 'Derive the dedicated-stack pairs.'],
            'Today admission records each installed entry’s gate and stack. Target lowering identifies which installations survive. For the supported x86-64 delivery rules, any installed entry can arrive first. A non-maskable interrupt (vector 2) and a machine check (vector 18) can then nest, each at most once per history. The proposal carries that rule and the entry placements forward. At 03.5, derive the dedicated-stack pairs from those rules and carry them into selection.',
            [('Admitted gates and stack choices',admission+'#L239-L284'),('Delivery assumptions',admission+'#L294-L353'),('Retained installations',driver+'#L123-L159'),('Permitted nesting and pairs',object_code+'#L1115-L1141')]),
        'native-planning-select': (
            ['Fix gate, protocol and stack mode.',
             'Select capacity and alignment checks.',
             'For each simultaneous dedicated pair,',
             'check distinct slots and disjoint ranges.'],
            'Select the entry checks from the gate, protocol and stack mode. For every pair of dedicated entries that can be active together, select checks for distinct stack slots and disjoint memory ranges. A three-entry nesting on dedicated stacks needs all three pair checks.',
            [('Entry and stack guards',selection+'#L1602-L1678'),('Pair guards',selection+'#L972-L1080')]),
        'physical-planning-generated': (
            ['Plan handler and entry-bridge bodies.',
             'Establish frames and register use.'],
            'Physical planning must include handlers and their entry bridges. Their frames, call costs and effects supply the later stack calculation. Binding must preserve these choices.',
            [('Body and bridge costs',object_code+'#L1524-L1555')]),
        'binding-summarize': (
            ['Measure each permitted nesting path.', 'Capacity sets the comparison value.', 'Alignment sets the mask.'],
            'Add the body, its callees, the entry bridge and call bias on each active stack. Current 03 checks required bytes ≤ available bytes and masks the stack top with alignment − 1. For interrupted-stack entries, an insufficient root reserve rejects the plan. Every valid dedicated-stack size or mask uses the same 64-bit constant-load instruction with one destination register. The x86-64 baseline move has room for all eight literal bytes. Open: carry the two operands through optimization and allocation, then bind and replay the exact guards.',
            [('Stack peaks',object_code+'#L1079-L1113'),('Required bytes',requirement+'#L147-L159'),('Capacity check',selection+'#L1230-L1242'),('Interrupted root reserve',selection+'#L1502-L1524'),('Alignment check',selection+'#L722-L735'),('Selected constant load',code+'pipeline/03_target-operations-to-selected-instructions/src/selection_scalar_graph.rs#L7918-L7929'),('Full-width x86 move',code+'backend/isa/isa-x86_64/src/selected_form_encoding.rs#L13855-L13864'),('Guard replay',code+'backend/images/image-emission/src/function_fragments.rs#L7568-L7583')]),
        'binding-bind': (
            ['Check the final body and every callee.', 'Account for RAM, CPU state and traps.', 'Reject any unexplained fault route.'],
            'For x86-64 fatal installations, the platform supplies exclusive table and stack ownership, the declared IRQ, NMI and machine-check delivery rules, and the absence of debug injection, tracing and active breakpoints. Match each physical instruction to its selected source. Calls must stay inside the checked bodies. Memory accesses need checked RAM or stack backing. Privileged operations rely on kernel privilege; relative branches need checked targets. Reject unsupported traps, hosted failures and unaccounted floating-point or vector state. Open: carry enough evidence through 04 to prove this for the exact bound body.',
            [('Platform assumptions',code+'backend/runtime/installation-evidence/src/installation_environment.rs#L78-L86'),('Memory and calls',object_code+'#L1219-L1259'),('Instruction correspondence',object_code+'#L1285-L1335'),('CPU state',code+'backend/images/image-emission/src/entries.rs#L54-L74'),('Fault classification',object_code+'#L1346-L1441')]),
    }
    for key,(notes,gate,evidence) in annotations.items():
        node=nodes[key]
        # Keep the current-stage reference last for the stage layout.
        node['notes'][1:1]=['Open · 03–05 handler handoff',*notes]
        node['gate']=gate
        node['evidence']=evidence


def proposed_model():
    # Each row is an owner followed by its complete result, on the same spine.
    contracts=[
        ('admission','Admit inputs','inputs','admitted','Admitted inputs',
         'Record acquired source/package custody, scoped roots, Build/configuration requests, target/component/provider/loader/output inputs, selections, budgets and staged receipts. Values requiring evaluation are produced in 01.',
         'Source inputs and the requests needed later, carried in one compilation record.',
         'Specify complete input fields and authority without activating unused source.'),
        ('source-settlement','Compile source + values','source','accepted','Accepted program + values',
         'Own lazy syntax, resolution, typing, preparation, lowering and hermetic evaluation. The lowered-form checker gives the sole semantic verdict. Preserve independent certificate admission and the specified optimization/checker order.',
         'Checked candidate bodies, computed values, neutral selection records, invocation receipts and provenance, with downstream requests.',
         'Prove activation/error order, invocation admission, value isolation and charging. Replace old APIs with private operations, not a wrapper.'),
        ('seal','Seal portable program','source','portable','Psi + carried inputs',
         'Publish canonical target-neutral Psi from accepted code. Forward remaining inputs in their declared compilation record; native configuration does not become portable language semantics.',
         'Canonical checked code and its catalog, evidence and summaries, with the records needed downstream. No source owner or lookup callback.',
         'Define acyclic type ownership: Psi sees only neutral projections and never imports Omega target/ABI types.'),
        ('native-planning','Plan native code','native','selected-program','Selected native program',
         'Join carried Build selections to the checked catalog. Compute and validate calling policies inside this stage. Close retained native demand, call/ABI plans and generated-body recipes without source lookup.',
         'Retained bodies, ABI plans, obligations and generated-body recipes; carry target, installation and output records forward.',
         'Prove body-demand closure and exact policy admission/results, preserving optimization-pruned unsupported targets.'),
        ('physical-planning','Build physical program','native','physical-program','Complete physical program',
         'Represent ordinary and generated bodies together, including handlers, entries and adapters. Own register choices, frames, effects and symbolic resource uses.',
         'All executable bodies, register/effect/frame records and resource dependencies. Handler analysis needs no object-emission detour.',
         'Prove complete generated-body effects, caller constraints and finite resource ordering.'),
        ('binding','Bind resources','native','bound-program','Bound physical program',
         'Use complete physical resource summaries and carried installation limits. Check transitive stack, bridge storage, call bias, alignment, fault premises and placement identities.',
         'Complete physical code with bound resources and installation identities; preserve encoding, format, loader and output requests.',
         'Binding must not change allocation, effects, frames or body membership.'),
        ('encoding','Encode all code','encoding','encoded-program','Encoded code + fixups',
         'Encode ordinary and generated bodies, including frames. Publish legal alternatives and complete fixups; imported encoded components remain admitted data.',
         'Body/frame fragments, alternatives, fixups and attribution, with placement/output facts. No earlier executable graph.',
         'Complete alternatives/fixups for both ISAs and all formats. No later executable-body producer.'),
        ('layout-stage','Lay out code','encoding','laid-out','Laid-out program',
         'Place fragments, choose carried alternatives and patch references. Preserve body/frame relaxation, ranges and alignment without an encoder callback.',
         'Resolved content, symbols and placement records with remaining object/image/output requests.',''),
        ('object-stage','Build objects','encoding','objects','Objects + image inputs',
         'Construct object containers from laid-out content and carried symbols/format requests. No source/ABI rediscovery or private helper generation.',
         'Objects, attribution, entry/evidence records and carried container, loader and binding inputs.',''),
        ('image-stage','Build image','encoding','image','Completed image',
         'Place objects and produce relocations, signatures and container/loader metadata from carried inputs. Executable code is already encoded.',
         'Final image, regions, symbols, relocation evidence and provision records, plus the publication request.',''),
        ('publish','Publish products','products','published','Published products',
         'Install completed artifacts using the carried output request and return receipts. Publication performs no compiler transformations.',
         'Finished executable/component/Psi products and receipts. Runtime execution consumes finished products after compilation; it is not a pipeline helper.',''),
    ]
    nodes=[]
    for number,(key,title,group,result,result_title,description,result_description,gate) in enumerate(contracts):
        source=f'https://github.com/CathedralOS/Omega/blob/{PLAN_REVISION}/{PLAN_PATH}#L{173+number}'
        for endpoint,label,role,detail in ((key,f'{number:02d} {title}','owner',description),(result,result_title,'artifact',result_description)):
            nodes.append(dict(id=endpoint,owner=endpoint,title=label,role=role,
                              kind='owner' if role=='owner' else ('native' if group in ('native','encoding','products') else 'data'),
                              group=group,description=detail,page='foundation-backend-plan',
                              anchor='3-recommended-destination-and-stage-contracts',source=source,
                              gate=gate if role=='owner' else '',number=number if role=='owner' else None,width=220,height=60))
    nodes,phases=unfold(nodes,PLAN_REVISION)
    spine=[n['id'] for n in nodes]
    by_id={n['id']:n for n in nodes}
    annotate_handler_handoff(by_id)
    edges=[dict(id=f'proposed-{a}-{b}',start=a,end=b,label='',kind='data',main=True,source=by_id[b]['source'])
           for a,b in zip(spine,spine[1:])]
    model=dict(nodes=nodes,edges=edges,spine=spine,view='proposed',status='unverified',plan_revision=PLAN_REVISION,
               badge=f'Proposed 00–10 · plan {PLAN_REVISION[:12]}',
               intro='Draft stage order. Switch to Current to trace the observed compiler detours. Source and native planning can revisit work.',
               phaseGaps={n['id']:32 for n in nodes if n['role']=='owner'},
               phases=phases,placements=[])
    audit = 'https://github.com/CathedralOS/Omega/blob/9c3100603b816814610190bf73f78814abefb760/' + PLAN_PATH
    model['repairs'] = {key: dict(stops=stops, description=description, source=f'{audit}#L{line}')
                        for key,(line,stops,description) in DETOUR_REPAIRS.items()}
    validate(model)
    return model
