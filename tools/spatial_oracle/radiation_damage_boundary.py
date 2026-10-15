"""Original radiation boundary regression after native Verses precision correction.

This records selected final health/admission, not full radiation equivalence.
See radiation_damage_boundary.md for the observed producer precision residual.
"""
import hashlib,json,os,struct,sys
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
from tools.rules_oracle.bridge_landing_inputs import Landing,WH
from tools.spatial_oracle.building_body_rules import TYPE,INI,SP,dwords
from tools.native_oracle import run_checked,RET_MAGIC,finish_vectors,provenance
from tools.spatial_oracle.estimated_damage import CLASSES
RAW='100%,100%,100%,50%,10%,10%,0%,0%,0%,100%,100%'
def execute(frame=16,cell=(6,5),armor=5,category='unit', *, field=None):
 cls=CLASSES[category]
 m=Landing();u=m.u
 visits=[]
 u.hook_add(UC_HOOK_CODE,lambda _u,a,_s,_d: visits.append(a) if a in (0x65C780,0x65C7E0,0x65B9C0,0x487CB0,0x489180,0x5F5390) else None)
 wh=m.warhead('RadSite')
 m.make_ini(field['sections'] if field else {'RadSite':{'Verses':RAW},'Radiation':{'RadDurationMultiple':'1','RadApplicationDelay':'16','RadLevelMax':'500','RadLevelDelay':'90','RadLevelFactor':'.2','RadSiteWarhead':'RadSite'}})
 for key,(kp,a,b) in WH.items():
  u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ESI,wh);u.reg_write(UC_X86_REG_EDI,INI);u.reg_write(UC_X86_REG_EBP,wh+0x24)
  run_checked(u,a,b)
 m.invoke(0x66CF70,TYPE,(INI,));assert m.read32(TYPE+0x1834)==wh
 site=m.alloc(0x100);table=m.alloc(0x100000);obj=m.alloc(0x1000);typ=m.alloc(0x2000);scenario=m.alloc(0x2000)
 u.mem_write(0xA8B230,dwords(scenario));u.mem_write(0x87f924,dwords(table,0x40000))
 cells={}
 for y in (range(48) if field else range(3,8)):
  for x in (range(48) if field else range(3,8)):
   c=m.alloc(0x200);cells[x,y]=c;u.mem_write(c,dwords(0x7E4EEC));u.mem_write(c+0x24,struct.pack('<hh',x,y));u.mem_write(c+0x44,dwords(-1));u.mem_write(table+(y*512+x)*4,dwords(c))
 if field:
  stages,source_inputs=overlap_actions(m,cells,field)
 else:
  u.mem_write(site+0x40,struct.pack('<hh',5,5));m.invoke(0x65B4D0,site,(2,));m.invoke(0x65B4F0,site,(500,))
  m.invoke(0x65B9C0,site)
 levelbits=struct.unpack('<Q',u.mem_read(cells[cell]+0xf0,8))[0]
 u.mem_write(obj,dwords(cls['object_vtable']));u.mem_write(obj+cls['type_offset'],dwords(typ));u.mem_write(typ,dwords(cls['type_vtable']));u.mem_write(typ+0x9c,dwords(armor));u.mem_write(typ+0xa0,dwords(300));u.mem_write(obj+0x6c,dwords(300));u.mem_write(obj+0x90,b'\1');u.mem_write(obj+0x9c,dwords(cell[0]*256+128,cell[1]*256+128,0));u.mem_write(0xA8ED84,dwords(frame))
 u.reg_write(UC_X86_REG_ESI,obj);u.reg_write(UC_X86_REG_EBX,0);u.reg_write(UC_X86_REG_ESP,SP)
 endpoint=run_checked(u,0x4DA554,(0x4DA629,0x4DA63B))
 out=dict(frame=frame,cell=cell,armor=armor,category=category,verses_bits=m.whstate(wh)['verses_bits'],factor_bits=f'{struct.unpack("<Q",u.mem_read(TYPE+0x1818,8))[0]:016x}',level_bits=f'{levelbits:016x}',level=struct.unpack('<d',u.mem_read(cells[cell]+0xf0,8))[0],admitted=endpoint==0x4DA629)
 if endpoint==0x4DA629:
  sp=u.reg_read(UC_X86_REG_ESP);damage_ptr=m.read32(sp);base=struct.unpack('<i',u.mem_read(damage_ptr,4))[0];args=[m.read32(sp+4*i) for i in range(7)]
  assert args[1:]==[0,wh,0,0,1,0],args
  out['base_damage']=base;out['cell_damaging_level']=m.invoke(0x487CB0,cells[cell])
  # Original kernel uses __fastcall: damage ECX, warhead EDX, armor/distance stack.
  u.mem_write(SP,dwords(RET_MAGIC,armor,0));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,base);u.reg_write(UC_X86_REG_EDX,wh)
  run_checked(u,0x489180,RET_MAGIC)
  out['kernel_damage']=u.reg_read(UC_X86_REG_EAX)
  dp=m.alloc(4);u.mem_write(dp,dwords(base));out['object_receive_result']=m.invoke(0x5F5390,obj,(dp,0,wh,0,0,1,0));out['final_health']=m.read32(obj+0x6c)
 out['final_health']=m.read32(obj+0x6c)
 out['visited']=[f'{a:08X}' for a in visits]
 out['rng_calls']=[f'{a:08X}' for a in visits if a in (0x65C780,0x65C7E0)]
 assert not out['rng_calls']
 if field:
  out.pop('level')  # Raw bits also represent exceptional native cell values.
  out.setdefault('cell_damaging_level',struct.unpack('<i',dwords(m.invoke(0x487CB0,cells[cell])))[0])
  out.setdefault('base_damage',0)
  out.update(name=field['name'],actions=field['actions'],stages=stages,source_inputs=source_inputs)
 return out

def generate():
 rows=[execute(f,c,a,k) for f in (15,16,17) for c,a,k in (((5,5),0,'infantry'),((6,5),5,'unit'))]
 return dict(schema_version=1,input=dict(verses=RAW,radiation={'RadDurationMultiple':'1','RadApplicationDelay':'16','RadLevelMax':'500','RadLevelDelay':'90','RadLevelFactor':'.2','RadSiteWarhead':'RadSite'},site_level=500,spread=2,source_cell=[5,5],initial_health=300),rows=rows)

def metadata():
 return provenance(scope='Independent original RadSite warhead percentage reader, Radiation reader, flat-map RadSite spread, Foot radiation admission/producer and direct Object nonlethal receiver. Six bounded rows; no whole Radiation or concrete Techno receiver parity claim.',assumptions=[
 'Synthetic radiation_rules fixture input: eleven authored Verses tokens, RadLevelFactor .2, application delay16, cap500, level500/spread2, flat cells3..7 around5,5, fresh objecthealth300 and armor0/5. These are explicit regression inputs, not a claim of full retail layer loading.',
 'Original Warhead factory/constructor and complete selected scalar/Verses blocks75DDCC..75DE5A execute. Full original Rules Radiation reader66CF70 executes using supplied lexical INI indexes; original CRT floating scanner initialization executes. FPCW0E7F.',
 'Original spread/radius setter65B4D0, level/duration setter65B4F0 and full25-cell spread65B9C0 execute using original Cell vtables/getters and map lookup5657A0. Fresh zeroed cell radiation fields. No light activation or site AI/decay scheduling.',
 'Foot4DA554 begins after its ordinary TechnoAI/alive prologue, with valid fresh nonimmune ground Infantry/Unit objects and EBX0; stops before concrete ReceiveDamage4DA629 or at no-application continuation4DA63B. Exact damage ABI asserted: distance0, selectedwarhead, source0, ignoredefenses0, arg6true, sourcehouse0.',
 'The emitted base then independently enters original kernel489180 and original Object5F5390 nonlethal receiver, retaining actual Infantry/Unit object/type vtables. Concrete Infantry/Unit/Techno receivedamage modifiers and downstream nonlethal effects are not executed. This fixture corresponds to ordinary unranked targets with neutral effective modifiers, but does not prove those omitted gates.',
 'Rows atframes15/16/17 are independent fixtures, not native elapsed-site scheduling. No source in this comparison emits RNG draws; site duration stores are from original65B4F0. No death/detach calls are reached.',
 'Native side cell stores299.99999999999994, clamps/truncates299 and emits59. Rust currently stores300 and emits60; both resolve5 versus corrected10%heavy. Native RadLevelFactor ReadDouble bits3fc99999a0000000 differ from current directf64 parser. These are separately recorded unresolved radiation prerequisites, not matching intermediates.'],substitutions=[
 'Landing reader helper supplies cached lexical INI entries/CRC indexes, allocator bump storage, no-op delete and CRTTLS storage. No original executable instructions replaced.',
 'Fixture supplies sparse actual map cell table, explicit instance/type memory and neutral scenario/rules state. Receiver entry is selected explicitly after capturing original Foot arguments; no claim that the intervening concrete receiver body executed.'
 ],entry_points={'warhead_verses':0x75DDCC,'radiation_reader':0x66CF70,'spread_setter':0x65B4D0,'level_setter':0x65B4F0,'spread':0x65B9C0,'foot_gate':0x4DA554,'cell_level':0x487CB0,'foot_receiver_call':0x4DA629,'warhead_damage':0x489180,'object_receiver':0x5F5390})

# Additive field controls use the same reader/map/Foot/Object owner above.
# The default six-row payload and its historical sidecar remain unchanged.
OVERLAP_KEYS = {
 'Radiation': ('RadDurationMultiple','RadApplicationDelay','RadLevelMax','RadLevelDelay',
               'RadLightDelay','RadLevelFactor','RadLightFactor','RadTintFactor','RadColor','RadSiteWarhead'),
 'RadSite': ('Verses',),
 'RadEruptionWeapon': ('RadLevel',),
 'RadEruptionWarhead': ('CellSpread',),
 'DOG': ('Strength','Armor','ImmuneToRadiation'),
}

def overlap_inputs():
 from tools.projectile_oracle.bridge_render_inputs import lexical
 root=Path(os.environ['VERA20K_RADIATION_INPUTS'])
 sections={};layers=[]
 for name in ('RULESMD.INI','LANGRULE.INI','MPBattleMD.ini','Hills.mmx'):
  path=root/name
  if not path.exists():
   if name!='LANGRULE.INI':raise FileNotFoundError(path)
   layers.append(dict(file=name,absent=True));continue
  raw=path.read_bytes();parsed,_=lexical(raw,set(OVERLAP_KEYS))
  selected={section:{key:value for key,value in values.items() if key in OVERLAP_KEYS[section]}
            for section,values in parsed.items()}
  layers.append(dict(file=name,sha256=hashlib.sha256(raw).hexdigest(),sections=selected))
  for section,values in selected.items():sections.setdefault(section,{}).update(values)
 return sections,layers

def overlap_actions(m,cells,field):
 """Execute original field operations; no Python falloff/float arithmetic."""
 u=m.u;sections=field['sections']
 # Read source weapon and Warhead independently; they are not fixture constants.
 level=m.invoke(0x5276D0,INI,(m.cstring('RadEruptionWeapon'),m.cstring('RadLevel'),0))
 eruption=m.warhead('RadEruptionWarhead')
 for _,(_,a,b) in WH.items():
  u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ESI,eruption)
  u.reg_write(UC_X86_REG_EDI,INI);u.reg_write(UC_X86_REG_EBP,eruption+0x24);run_checked(u,a,b)
 spread=int(m.whstate(eruption)['cell_spread'])
 sites={};stages=[];frame=0
 # Declared empty vector storage; original RadSite constructor owns append.
 u.mem_write(0xB04BD0,dwords(0x7EB6D4,m.alloc(64),16,1,0,10))
 def activate(site):
  # Exact activation timer/level-step/scalar prefix; stop before map/light
  # allocation. Both original light branches subsequently call65B9C0.
  u.mem_write(SP,dwords(RET_MAGIC));u.reg_write(UC_X86_REG_ESP,SP)
  u.reg_write(UC_X86_REG_ECX,site);run_checked(u,0x65B580,0x65B74C)
  m.invoke(0x65B9C0,site)
 def sample(cell):
  p=cells[tuple(cell)];bits=struct.unpack('<Q',u.mem_read(p+0xF0,8))[0]
  amount=struct.unpack('<i',dwords(m.invoke(0x487CB0,p)))[0]
  base=0
  if amount>0:
   u.reg_write(UC_X86_REG_ESP,SP);u.mem_write(SP+0x10,dwords(amount))
   run_checked(u,0x4DA5FA,0x4DA60F);base=struct.unpack('<i',dwords(u.reg_read(UC_X86_REG_EAX)))[0]
  return dict(cell=cell,level_bits=f'{bits:016x}',damaging_level=amount,base_damage=base)
 for action in field['actions']:
  kind=action['kind']
  if kind=='create':
   center=tuple(action['cell']);site=m.alloc(0x100);sites[center]=site
   m.invoke(0x65B1E0,site)
   u.mem_write(site+0x40,struct.pack('<hh',*center))
   m.invoke(0x65B4D0,site,(spread,));m.invoke(0x65B4F0,site,(level,));activate(site)
   m.invoke(0x487C70,cells[center],(site,))
  elif kind=='tick_to':
   # Run each original lifetime/level-timer prefix. The independent light
   # timer and final deletion suffix are outside this numeric comparison.
   for frame in range(frame+1,action['frame']+1):
    u.mem_write(0xA8ED84,dwords(frame))
    # Logic55B5CD..55B5E8 visits the native vector in reverse.
    vector=m.read32(0xB04BD4);count=m.read32(0xB04BE0)
    for index in range(count-1,-1,-1):
     site=m.read32(vector+index*4)
     u.mem_write(SP,dwords(RET_MAGIC));u.reg_write(UC_X86_REG_ESP,SP)
     u.reg_write(UC_X86_REG_ECX,site);run_checked(u,0x65B800,0x65B858)
  elif kind in ('decrease','decay'):
   m.invoke(0x65BB50 if kind=='decrease' else 0x65BD00,sites[tuple(action['cell'])])
  elif kind=='merge':
   site=sites[tuple(action['cell'])]
   u.mem_write(SP,dwords(RET_MAGIC,level));u.reg_write(UC_X86_REG_ESP,SP)
   u.reg_write(UC_X86_REG_ECX,site);run_checked(u,0x65B530,0x65B570)
   activate(site)
  else:raise ValueError(kind)
  native_order=[m.read32(m.read32(0xB04BD4)+index*4) for index in range(m.read32(0xB04BE0)-1,-1,-1)]
  stages.append(dict(action=action,frame=frame,update_order=[list(next(cell for cell,p in sites.items() if p==site)) for site in native_order],
   samples=[sample(cell) for cell in field['sample_cells']],
   sites=[dict(cell=list(cell),radius=m.read32(p+0x48),level=m.read32(p+0x4C),
     current_level=struct.unpack('<i',dwords(m.invoke(0x65B510,p)))[0],
     level_steps=m.read32(p+0x50),duration=m.read32(p+0x6C),remaining=m.read32(p+0x70),
     level_timer_start=struct.unpack('<i',u.mem_read(p+0x28,4))[0],level_timer_duration=m.read32(p+0x30)) for cell,p in sites.items()]))
 return stages,dict(rad_level=level,spread=spread,
  duration_multiple=m.read32(TYPE+0x1804),application_delay=m.read32(TYPE+0x1808),
  level_max=m.read32(TYPE+0x180C),level_delay=m.read32(TYPE+0x1810),
  factor_bits=f'{struct.unpack("<Q",u.mem_read(TYPE+0x1818,8))[0]:016x}')

def concrete_dogs(packets):
 """Use the existing populated Infantry/House/Walk/Map owner, not zeroes."""
 from tools.spatial_oracle.anytown_damage.foot_missions import FootMissions,native_owner
 from tools.projectile_oracle.bridge_render_inputs import lexical
 from tools.spatial_oracle.building_body_rules import RULES
 q=FootMissions();q.initialize_companion();m,u=q.m,q.u
 root=Path(os.environ['VERA20K_SHRAPNEL_INPUTS'])
 art_raw=(root/'ARTMD.INI').read_bytes()
 art,_=lexical(art_raw,{'DOG','DogSequence'});m.make_ini(art)
 typ=m.alloc(0x1900);m.invoke(0x5236A0,typ,(m.cstring('DOG'),))
 layers=[]
 for name,path in native_owner.base.layers():
  if not path.exists():layers.append(dict(file=name,absent=True));continue
  raw=path.read_bytes();selected,_=lexical(raw,{'DOG','DogJaw','RadSite'})
  m.rules_cache(selected);admitted=m.invoke(0x5240A0,typ,(RULES,))&255
  wh=m.invoke(0x75E3B0,m.cstring('RadSite'));m.invoke(0x75D3A0,wh,(RULES,))
  layers.append(dict(file=name,sha256=hashlib.sha256(raw).hexdigest(),admitted=admitted))
 rows=[]
 for index,(name,base_damage) in enumerate(packets):
  q.phase='setup'
  actor=m.alloc(0x1000);m.invoke(0x517A50,actor,(typ,0))
  u.mem_write(actor+0x21C,dwords(q.house));u.mem_write(actor+0x14C,dwords(q.house))
  m.invoke(0x486840,q.resident.ptrs[(86,49)],(q.coord,))
  assert m.invoke(0x51DFF0,actor,(q.coord,0x80))&255
  m.invoke(0x51D6F0,actor,(0,0,0));q.phase='logic'
  damage=m.alloc(4);u.mem_write(damage,dwords(base_damage));seen=[]
  def observe(uc,a,size,data):
   if a in (0x517FA0,0x4D7330,0x701900,0x5F5390,0x489180,0x65C780,0x65C7E0):seen.append(a)
  hook=u.hook_add(UC_HOOK_CODE,observe)
  before={key:bytes(u.mem_read(ptr,0x3F4)) for key,ptr in q.resident.rngs.items()}
  result=m.invoke(0x517FA0,actor,(damage,0,wh,0,0,1,0));u.hook_del(hook)
  assert not {0x65C780,0x65C7E0}&set(seen)
  rows.append(dict(name=name,base_damage=base_damage,result=result,health=m.read32(actor+0x6C),
   damage_after=m.read32(damage),alive=bool(u.mem_read(actor+0x90,1)[0]),
   limbo=bool(u.mem_read(actor+0x81,1)[0]),strength=m.read32(typ+0xA0),armor=m.read32(typ+0x9C),
   calls=[f'{a:08X}' for a in seen],rng_unchanged=all(before[key]==bytes(u.mem_read(ptr,0x3F4)) for key,ptr in q.resident.rngs.items()),
   art_sha256=hashlib.sha256(art_raw).hexdigest(),layers=layers))
 return rows

def generate_overlap():
 sections,layers=overlap_inputs();a,b,c=[17,24],[31,24],[24,17]
 create=lambda cell:dict(kind='create',cell=cell)
 cases=[('one_site',[create(a)]),('two_sites',[create(a),create(b)]),
        ('three_sites',[create(a),create(b),create(c)]),
        ('three_sites_reversed',[create(c),create(b),create(a)]),
        ('same_center_merge',[create(a),dict(kind='merge',cell=a)]),
        ('large_level_merge',[create(a),dict(kind='merge',cell=a)]),
        ('first_decay',[create(a),dict(kind='tick_to',frame=90)]),
        ('merge_after_first_decay',[create(a),dict(kind='tick_to',frame=90),dict(kind='merge',cell=a)]),
        ('overlap_first_decay',[create(a),create(b),create(c),dict(kind='tick_to',frame=90)]),
        ('overlap_reversed_first_decay',[create(c),create(b),create(a),dict(kind='tick_to',frame=90)]),
        ('zero_steps_decay',[create(a),dict(kind='decay',cell=a)]),
        ('zero_steps_decrease',[create(a),dict(kind='decrease',cell=a)])]
 samples=[[24,24],a,[18,24],[24,31],[27,24],[27,34],[22,29]]
 rows=[]
 for name,actions in cases:
  selected={key:dict(value) for key,value in sections.items()}
  if name.startswith('zero_steps_'):selected['Radiation']['RadLevelDelay']='600'
  if name=='large_level_merge':selected['RadEruptionWeapon']['RadLevel']='50000'
  rows.append(execute(cell=(24,24),armor=0,category='infantry',field=dict(name=name,sections=selected,
       actions=actions,sample_cells=samples)))
 return dict(schema_version=1,layers=layers,rows=rows,
             concrete_receivers=concrete_dogs([(row['name'],row['base_damage']) for row in rows[:3]]))

def overlap_metadata():
 return provenance(scope='Stock DESO overlapping radiation field arithmetic, merge/first decay, Foot packet, and populated DOG concrete receiver',
  assumptions=[
   'Selected lexical keys are read from physical RULESMD, optional LANGRULE, Battle and Hills files; native scalar readers execute. No whole physical native INI/scenario load.',
   'Original RadSite constructors append to supplied empty vector storage; the Logic55B5CD reverse-vector traversal selects numeric update order. Original48x48 flat CellClass table. Source RadLevel and CellSpread are native-read. Twelve small histories sample seven cells each; zero-step direct-kernel controls explicitly override RadLevelDelay600 and do not claim site lifetime reachability; no Python field arithmetic supplies results.',
   'Activation65B580..65B74C initializes real timers/steps then original spread65B9C0 executes. Independent LightSource creation/output is excluded. Tick prefix65B800..65B858 executes every frame through90; light-timer and lifetime-deletion suffix excluded.',
   'The large-level control explicitly overrides RadLevel50000. Per-site current_level executes signed getter65B510; it is distinct from same-center merge65B530..65B570, which executes its float effective-level and Decrease65BB50 before activation; original Decay65BD00 and CellSubtract487D00 execute. Memory-store boundaries are retained.',
   'Field rows use the historical original Foot4DA554 boundary, neutral300HP Object receiver control. The separate populated DOG control uses existing FootMissions native type/actor/Walk/House/map construction and full517FA0 receiver with the captured three-site packet.',
   'DOG concrete control uses physical Battle/Anytown reader context; selected DOG/RadSite values match the field Battle/Hills context. Independent source-layer identities are retained. No three-DESO live native match or rendering claim.'
  ],substitutions=[
   'Existing Landing allocator/delete/CRT TLS and cached lexical INI transport. Supplied flat map cell table and bounded object state for field rows.',
   'Populated DOG control inherits FootMissions OS/COM, map crop, one human House and sound transport boundaries. Original class/receiver code executes. No unknown receiver state is replaced with zeroes.'
  ],entry_points={'radiation_reader':0x66CF70,'spread':0x65B9C0,'merge':0x65B530,'decrease':0x65BB50,
    'decay':0x65BD00,'cell_add':0x487CE0,'cell_subtract':0x487D00,'foot':0x4DA554,
    'factor':0x4DA5FA,'infantry_receiver':0x517FA0,'foot_receiver':0x4D7330,'techno_receiver':0x701900,'object_receiver':0x5F5390})

if __name__=='__main__':
 if '--overlap' in sys.argv:
  finish_vectors(generate_overlap,Path(__file__).with_name('radiation_overlap.json'),
   provenance=overlap_metadata,argv=[arg for arg in sys.argv[1:] if arg!='--overlap'])
 else:finish_vectors(generate,Path(__file__).with_suffix('.json'),provenance=metadata)
