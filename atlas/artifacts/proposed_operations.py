"""Visible private operations of the proposed owners, not current crate numbers.

The plan records this dependency order. Source and native worklists may revisit
operations; only the outer owner-to-result handoff claims a completed stage.
"""
import textwrap

OPERATIONS = [
    [('custody','Source custody','Acquired bytes → scoped inputs'),
     ('requests','Admit requests','Selections, budgets, receipts')],
    [('lex','Lex','Source bytes → tokens'),
     ('parse','Parse','Tokens → syntax trees'),
     ('resolve','Resolve names','Syntax → resolved trees'),
     ('type','Type','Resolved → typed trees'),
     ('prepare','Prepare + evaluate','Typed → prepared trees',
      ['Required invocation:', 'prepare → lower → check', 'admit → execute → value', 'Return value + work receipts']),
     ('lower','Lower','Prepared trees → lowered Psi'),
     ('check','Check','Lowered Psi → accepted Psi'),
     ('optimize','Optimize Psi','Accepted → transformed Psi', ['Only selected passes']),
     ('recheck','Check transformed Psi','Transformed → accepted Psi', ['Same checker; nonempty', 'selection only'])],
    [('encode','Encode portable Psi','Accepted module → Psi bytes'),
     ('seal','Seal publication','Identity, evidence, requests')],
    [('providers','Join provider selections','Build rows + checked catalog'),
     ('policies','Compute calling policies','Accepted policy → ABI plan', ['Admit → execute → validate']),
     ('abstract','Lower abstract operations','Psi → abstract operations'),
     ('optimize-abstract','Optimize abstract code','Selected abstract passes'),
     ('target','Lower target operations','Abstract → target operations'),
     ('select','Select instructions','Target → selected instructions'),
     ('optimize-selected','Optimize instructions','Selected instruction passes'),
     ('closure','Close body demand','Retained bodies + recipes')],
    [('generated','Construct generated bodies','Handlers, entries, adapters'),
     ('allocate','Allocate registers','Instructions → register homes'),
     ('physical','Lower physical operations','Homes → physical operations'),
     ('frames','Plan frames + effects','Frames, effects, resource uses')],
    [('summarize','Summarize resources','Complete transitive demand'),
     ('bind','Check + bind installations','Demand + limits → bound code')],
    [('bodies','Encode bodies + frames','Physical code → fragments'),
     ('fixups','Describe alternatives','Encodings + complete fixups')],
    [('place','Place fragments','Fragments → placed code'),
     ('relax','Relax + patch','Choose carried alternatives'),
     ('optimize','Optimize layout','Selected layout passes')],
    [('sections','Build sections + symbols','Text, symbols, relocations'),
     ('container','Form objects','Sections → object containers')],
    [('place','Place objects','Objects → image regions'),
     ('relocate','Resolve image references','Relocations + loader metadata'),
     ('finish','Finish image metadata','Signatures + container records')],
    [('install','Install products','Finished bytes → output files'),
     ('receipts','Record publication','Installed files → receipts')],
]

CURRENT_STAGES = {
    'source-settlement-lex':'Current Psi 00',
    'source-settlement-parse':'Current Psi 01',
    'source-settlement-resolve':'Current Psi 02',
    'source-settlement-type':'Current Psi 03',
    'source-settlement-prepare':'Current Psi 04 + evaluation',
    'source-settlement-lower':'Current Psi 05',
    'source-settlement-check':'Current Psi 06',
    'source-settlement-recheck':'Same Psi 06 checker',
    'native-planning-abstract':'Current Omega 00',
    'native-planning-optimize-abstract':'Current Omega 01',
    'native-planning-target':'Current Omega 02',
    'native-planning-select':'Current Omega 03',
    'native-planning-optimize-selected':'Current Omega 04',
    'physical-planning-allocate':'Current Omega 05',
    'physical-planning-physical':'Current Omega 06',
    'encoding-bodies':'Current Omega 07 + late code',
    'layout-stage-place':'Current Omega 08',
    'layout-stage-optimize':'Current Omega 09',
}


def unfold(nodes, revision):
    expanded=[]
    phases=[]
    for owner,result in zip(nodes[::2],nodes[1::2]):
        number=owner['number']
        owner['title'] = f'{number:02d} '+('Source compilation' if number==1 else owner['title'][3:])
        owner['notes'] = (['Demand-driven worklist;', 'operations may repeat'] if number in (1,3)
                          else ['Operations below are', 'inside this stage'])
        owner['height']=112
        expanded.append(owner)
        for index,row in enumerate(OPERATIONS[number],1):
            key,title,form,*extra=row
            notes=[form]+(extra[0] if extra else [])
            operation=dict(owner,id=f'{owner["id"]}-{key}',owner=owner['id'],
                           title=f'{number:02d}.{index} {title}',role='operation',
                           number=None,kind='check' if number==1 and key in ('check','recheck') else 'data',
                           description=f'{title}: {form}. '+(' '.join(extra[0]) if extra else '')+' '+owner['description'],
                           notes=notes,height=84+len(notes)*22,
                           source=f'https://github.com/CathedralOS/Omega/blob/{revision}/atlas/plans/foundation-and-backend.md#L{222+number}',
                           anchor='keep-the-operations-visible')
            expanded.append(operation)
        expanded.append(result)
        phases.append([f'stage-{number}',owner['id'],result['id'],[f'Stage {number:02d}']])
    for node in expanded:
        if node.get('notes'):
            if node['id'] in CURRENT_STAGES:
                node['notes'].append(CURRENT_STAGES[node['id']])
            title_lines=len(textwrap.wrap(node['title'],21,break_long_words=False,break_on_hyphens=False))
            note_lines=sum(len(textwrap.wrap(note,29,break_long_words=False,break_on_hyphens=False)) for note in node['notes'])
            node['height']=50+title_lines*28+note_lines*20
    return expanded,phases
