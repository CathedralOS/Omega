"""Proposed stages 00–10 carry inputs through one forward chain.

These are design contracts, not observed implementation boundaries. Compiler
work stays inside its stage; runtime execution consumes completed products.
"""
from .overview import validate
from .proposed_operations import unfold

PLAN_REVISION = '9d9e40a22677ade9f9697fdf1181ef0ac940c612'
PLAN_PATH = 'atlas/plans/foundation-and-backend.md'


# Each entry points at the repair condition in the audited plan and at the
# operations a reader can inspect on the proposed canvas. The runtime repair
# names the data to carry but leaves its producing stage open.
DETOUR_REPAIRS = {
    'evaluation': (87, ['source-settlement-prepare', 'accepted'],
        'Source preparation evaluates required invocations inside stage 01. Its result carries the computed values and receipts.'),
    'policy': (88, ['accepted', 'native-planning-policies'],
        'Stage 01 settles source choices. Stage 03 computes calling policies from the carried choices and accepted policy code.'),
    'handlers': (459, ['native-planning-target', 'physical-planning-generated', 'physical-planning-handler-order', 'physical-planning-select', 'binding-summarize', 'binding-bind'],
        '03 closes the roster. In 04, each handler gets its frame before its installer selects a numeric guard. 05 checks the result against every finished body and permitted nesting path.'),
    'emission': (85, ['native-planning-target', 'encoding-bodies'],
        'From target lowering onward, each form carries the facts its successor needs. Encoding can consume physical code and those facts directly.'),
    'callbacks': (89, ['native-planning-closure', 'object-stage-container'],
        'Native planning closes demand for callback bodies before encoding. Object construction receives the resulting laid-out code.'),
    'frames': (90, ['encoding-bodies', 'encoding-fixups', 'layout-stage-relax'],
        'Encoding emits frames, alternatives and fixups together. Layout picks an encoding and patches it.'),
    'image': (91, ['physical-planning-generated', 'encoding-bodies', 'image-stage-finish'],
        'Entries and adapters join the physical program in stage 04 and pass through common encoding. The image stage finishes container metadata.'),
    'runtime-layout': (92, [],
        'The recorded plan left ownership of runtime layout open. On the migration branch, select Published products to see how the host checks a completed image and when Psi runs alone.'),
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
    observed='https://github.com/CathedralOS/Omega/blob/2ff15fbb3855fd7c88a0cb9775774763cec946f8/omega-rust/omega/compilation/'
    annotations={
        'native-planning-target': (
            ['Keep each retained installation’s', 'nesting rules and stack choices.', 'Derive the dedicated-stack pairs.'],
            'In the pre-migration compiler, admission records each installed entry’s gate and stack. Target lowering identifies which installations survive. For the supported x86-64 delivery rules, any installed entry can arrive first. A non-maskable interrupt (vector 2) and a machine check (vector 18) can then nest, each at most once per history. The proposal carries that rule and the entry placements forward. Stage 04 uses the carried scope to derive dedicated-stack pairs before selecting guards.',
            [('Admitted gates and stack choices',admission+'#L239-L284'),('Delivery assumptions',admission+'#L294-L353'),('Retained installations',driver+'#L123-L159'),('Permitted nesting and pairs',object_code+'#L1115-L1141')]),
        'physical-planning-handler-order': (
            ['Plan shared handlers once.', 'An installer uses each child’s frame.'],
            'Follow only installation edges in the retained roster. Emit a handler before every installer that needs its stack measurement. A demanded cycle refuses compilation. Ordinary callback edges remain callable in either direction.',
            [('Handler order and tests',observed+'pipeline/04_physical-program-planning/src/handler_order.rs'),('Applying the order',observed+'native-compiler/src/realization/callback_thunks.rs#L122')]),
        'physical-planning-select': (
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
            'Add the body, its callees, the entry bridge and call bias on each active stack. Current 03 checks required bytes ≤ available bytes and masks the stack top with alignment − 1. For interrupted-stack entries, an insufficient root reserve rejects the plan. Every valid dedicated-stack size or mask uses the same 64-bit constant-load instruction with one destination register. The x86-64 baseline move has room for all eight literal bytes. On the migration branch, 04 measures the handler before selecting the guard. Stage 05 checks every field against the whole retained program.',
            [('Stack peaks',object_code+'#L1079-L1113'),('Required bytes',requirement+'#L147-L159'),('Capacity check',selection+'#L1230-L1242'),('Interrupted root reserve',selection+'#L1502-L1524'),('Alignment check',selection+'#L722-L735'),('Selected constant load',code+'pipeline/03_target-operations-to-selected-instructions/src/selection_scalar_graph.rs#L7918-L7929'),('Full-width x86 move',code+'backend/isa/isa-x86_64/src/selected_form_encoding.rs#L13855-L13864'),('Guard replay',code+'backend/images/image-emission/src/function_fragments.rs#L7568-L7583')]),
        'binding-bind': (
            ['Check the final body and every callee.', 'Account for RAM, CPU state and traps.', 'Reject any unexplained fault route.'],
            'For x86-64 fatal installations, the platform supplies exclusive table and stack ownership, the declared IRQ, NMI and machine-check delivery rules, and the absence of debug injection, tracing and active breakpoints. Match each physical instruction to its selected source. Calls must stay inside the checked bodies. Memory accesses need checked RAM or stack backing. Privileged operations rely on kernel privilege; relative branches need checked targets. Reject unsupported traps, hosted failures and unaccounted floating-point or vector state. The migration binder consumes the complete physical frame and retained handler measurements. The full cross-target acceptance run is pending.',
            [('Whole-program binding',observed+'pipeline/05_installation-binding/src/lib.rs#L41'),('Platform assumptions',code+'backend/runtime/installation-evidence/src/installation_environment.rs#L78-L86'),('Memory and calls',object_code+'#L1219-L1259'),('Instruction correspondence',object_code+'#L1285-L1335'),('CPU state',code+'backend/images/image-emission/src/entries.rs#L54-L74'),('Fault classification',object_code+'#L1346-L1441')]),
    }
    for key,(notes,gate,evidence) in annotations.items():
        node=nodes[key]
        # Keep the current-stage reference last for the stage layout.
        node['notes'][1:1]=['Handler stack',*notes]
        node['gate']=gate
        node['evidence']=evidence


def annotate_implementation(nodes):
    revision = '788ecf2489dabc436e00f4cf97dc5cf28b91604c'
    code = f'https://github.com/CathedralOS/Omega/blob/{revision}/omega-rust/'
    source = code + 'psi/compilation/pipeline/01_source-compilation/src/'
    omega = code + 'omega/compilation/'
    planning = omega + 'pipeline/03_native-program-planning/src/'
    implemented = 'https://github.com/CathedralOS/Omega/blob/0e5d56643021b7633553af23cfede3c93021e1d5/omega-rust/omega/'
    roster = 'https://github.com/CathedralOS/Omega/blob/80cc31a4df809971ed701b6ce5edcbaff6a86603/omega-rust/omega/compilation/native-compiler/src/realization/'
    checkpoints = {
        'source-settlement-lex': (
            'On the migration branch, a worker lexes one file into reusable token storage. The parser reads those tokens before the worker advances to the next file.',
            [('Per-file lex and parse', source + 'source_parsing.rs#L91-L133')]),
        'source-settlement-parse': (
            'Package assembly can adopt the syntax of an already discovered file when its bytes match. Fresh files use the per-file stream. A later lexical error takes precedence over an earlier syntax error.',
            [('Package assembly', code + 'omega/packages/package-compilation/src/source/source_assembly.rs#L1386-L1399'),
             ('Exact source reuse', source + 'source_parsing.rs#L281-L304'),
             ('Error ordering', source + 'source_parsing.rs#L53-L74')]),
        'seal-seal': (
            'Stage 02 accepts provider facts for the requested source judgment and target. It issues the whole set with a read-only receipt identifying the source that supplied it.',
            [('Issue provider demand', implemented + 'compilation/pipeline/02_portable-publication/src/provider_demand.rs#L106-L125'),
             ('Select and retain the receipt', implemented + 'compilation/native-compiler/src/provider_selection.rs#L39-L73'),
             ('Source choices and roots', source + 'provider_supply/demand.rs#L61-L110'),
             ('Earlier published demand', omega + 'pipeline/02_portable-publication/src/provider_demand.rs#L73-L104'),
             ('Normal compiler handoff', omega + 'compiler/src/execution_settlement.rs#L145-L176')]),
        'native-planning-providers': (
            'Native planning keeps the same source receipt while binding a folded entry and freezing its provider plans. Products checks the source owner and original program before accepting them. Executable entry preparation also checks the selected target. The planner rejects a changed body and records the selected plan when an entry needs a provider to compute a constant.',
            [('Preserve source receipt', implemented + 'compilation/native-compiler/src/provider_selection.rs#L39-L73'),
             ('Receive the selected result', implemented + 'compilation/products/src/checked_compilation.rs#L116-L147'),
             ('Join executable and entry', implemented + 'compilation/products/src/program_entry.rs#L17-L42'),
             ('Exact body comparison', planning + 'execution.rs#L53-L75'),
             ('Folded entry plans', planning + 'selected_execution.rs#L96-L124'),
             ('Target-specific code', planning + 'folded_entry.rs#L12-L53')]),
        'native-planning-policies': (
            'The target planner computes the required calling conventions from the published inputs. Its package-review result remains separate from the provider calls needed to run the entry.',
            [('Target planner', planning + 'execution.rs#L30-L50'),
             ('Default calling policies', planning + 'provider_execution_inputs.rs#L288-L310')]),
        'physical-planning-generated': (
            'The branch plans supported hosted entries and physical firmware entries before encoding. Stage 06 encodes the resulting entry plan for stage 09 to place.',
            [('Hosted entry planning', omega + 'native-compiler/src/realization/object_emission.rs#L140-L178'),
             ('Firmware entry planning and encoding', omega + 'native-compiler/src/realization/object_emission.rs#L206-L255')]),
        'binding-bind': (
            'Stage 04 measures the handler’s stack demand, bridge and CPU state from its physical body. Stage 05 compares the guard with the measurement at that installation and checks the instructions and frame before issuing authority.',
            [('Handler measurement', omega + 'pipeline/04_physical-program-planning/src/installed_handlers.rs#L55-L110'),
             ('Binding the finished frame', omega + 'pipeline/05_installation-binding/src/lib.rs#L54-L91')]),
        'admission-requests': (
            'When a package supplies a permission lock, target preparation keeps its accepted rows and captures the declarations it covers. Product completion checks those rows. Without a lock, a runtime product that relies on an assumed binding refuses.',
            [('Capture for this product', implemented + 'compilation/compiler/src/checked/target_preparation.rs#L39-L68'),
             ('Check requested consent', implemented + 'compilation/compiler/src/product_execution.rs#L489-L515'),
             ('Locked and ordinary package test', implemented + 'compilation/compiler/tests/package_review.rs#L3020-L3059')]),
        'image-stage-finish': (
            'The completed image supplies each root machine’s linked entry address and code regions. The reader checks that the descriptor belongs to the root and that the entry lies within its declared code.',
            [('Read the completed image', implemented + 'compilation/pipeline/09_image-construction/native-artifact/src/runtime_image.rs#L19-L58'),
             ('Validate linked ranges', implemented + 'compilation/pipeline/09_image-construction/native-artifact/src/runtime_image.rs#L105-L141')]),
        'published': (
            'The host matches a supplied native image to the Terminal artifact, manifest and target before giving its layout to the interpreter. A Psi-only request can run without an image. A native adapter request reaches a host service that installs only an adapter held in the published product.',
            [('Host identity and target checks', implemented + 'build-system/orchestration/src/published_terminal.rs#L45-L91'),
             ('Published adapter installation', implemented + 'host-support/native-provider-sponsor/src/lib.rs#L54-L100'),
             ('Layout execution and target refusal test', implemented + 'interpreter/terminal-runtime/tests/retained_runtime.rs#L156-L196')]),
        'object-stage-container': (
            'Stage 08 constructs objects for the root and every selected private body, including installed handlers. Component compilation also finishes its export objects before handing them to stage 09.',
            [('Root and private objects', roster + 'object_emission.rs#L304-L321'),
             ('Component export objects', roster + 'component_exports.rs#L292-L315'),
             ('Object publication', omega + 'pipeline/08_object-construction/src/fragment_container.rs#L225-L251'),
             ('Normal and retained inputs', omega + 'native-compiler/src/pipeline.rs#L459-L476')]),
        'image-stage-place': (
            'Stage 09 receives the completed group of objects. If the final private object fails to build, the driver returns that error before it emits the first image fragment.',
            [('Whole-program handoff', roster + 'object_emission.rs#L304-L321'),
             ('Late-refusal test', roster + 'object_emission.rs#L353-L386'),
             ('Hosted and physical entry placement', omega + 'pipeline/09_image-construction/native-artifact/src/fragment_object_publication.rs#L65-L132'),
             ('Image input', omega + 'pipeline/09_image-construction/image-emission/src/object_input.rs#L17-L54')]),
    }
    for key, (description, evidence) in checkpoints.items():
        nodes[key]['implementation'] = dict(description=description, evidence=evidence)


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
        ('native-planning','Plan native code','native','selected-program','Retained target program',
         'Join carried Build selections to the checked catalog. Compute and validate calling policies inside this stage. Close retained native demand, call/ABI plans and generated-body recipes without source lookup.',
         'Retained bodies, ABI plans, obligations and generated-body recipes; carry target, installation and output records forward.',
         'Prove body-demand closure and exact policy admission/results, preserving optimization-pruned unsupported targets.'),
        ('physical-planning','Build physical program','native','physical-program','Complete physical program',
         'Represent ordinary and generated bodies together, including handlers, entries and adapters. Order handlers before installers. Select and optimize instructions, allocate registers and record frame and effect measurements.',
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
    annotate_implementation(by_id)
    edges=[dict(id=f'proposed-{a}-{b}',start=a,end=b,label='',kind='data',main=True,source=by_id[b]['source'])
           for a,b in zip(spine,spine[1:])]
    model=dict(nodes=nodes,edges=edges,spine=spine,view='proposed',status='unverified',plan_revision=PLAN_REVISION,
               badge=f'Proposed 00–10 · plan {PLAN_REVISION[:12]}',
               intro='Recorded plan. Select an operation for implementation evidence and open questions. Current shows the pre-migration compiler.',
               phaseGaps={n['id']:32 for n in nodes if n['role']=='owner'},
               phases=phases,placements=[])
    audit = 'https://github.com/CathedralOS/Omega/blob/9c3100603b816814610190bf73f78814abefb760/' + PLAN_PATH
    model['repairs'] = {key: dict(stops=stops, description=description, source=f'{audit}#L{line}')
                        for key,(line,stops,description) in DETOUR_REPAIRS.items()}
    validate(model)
    return model
