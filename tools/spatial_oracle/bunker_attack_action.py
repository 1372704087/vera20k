"""Bounded original action dispatch; range verdicts and environmental leaves supplied.

No game code is patched. Full DetermineAction -> Unit/Foot/Techno WhatAction
entries execute; the complete ordinary Display4AAE90 path resolves a CellClass
target before its normal action switch. Supplied palette/waypoint storage has
no graphics backend; the original cursor setter is an observed sink.
This establishes action control flow, not weapon/range arithmetic or UI capture.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
from tools.native_oracle import load_image, run_checked, STACK_BASE, STACK_SIZE, SCRATCH, RET_MAGIC, image_sha256, NativeCallTrace
from tools.spatial_oracle.map_queries import dwords
from tools import native_oracle
from tools.native_oracle import finish_vectors, provenance

ACTOR, TYPE, HOUSE, TARGET, TARGET_TYPE, OTHER, CELL, WEAPON, SLOT, COORD, PROJECTILE, WARHEAD = [SCRATCH+i*0x2000 for i in range(12)]
UNIT_VT=0x7F5C70
SP=STACK_BASE+STACK_SIZE-0x1000

def run(case, continuation=None):
    u=Uc(UC_ARCH_X86,UC_MODE_32); load_image(u)
    u.mem_map(SCRATCH,0x30000);u.mem_map(STACK_BASE,STACK_SIZE);u.mem_map(RET_MAGIC,0x1000)
    read=lambda p:struct.unpack('<I',u.mem_read(p,4))[0]
    def ret(value,argc):
        sp=u.reg_read(UC_X86_REG_ESP)
        u.reg_write(UC_X86_REG_EAX,value & 0xffffffff)
        u.reg_write(UC_X86_REG_EIP,read(sp));u.reg_write(UC_X86_REG_ESP,sp+4+argc*4)
    for p,t,h in ((ACTOR,TYPE,HOUSE),(TARGET,TARGET_TYPE,HOUSE if case['relation']=='own' else OTHER)):
        u.mem_write(p,dwords(UNIT_VT));u.mem_write(p+0x6C4,dwords(t));u.mem_write(p+0x6C0,dwords(t));u.mem_write(p+0x21C,dwords(h))
        u.mem_write(p+0x14,dwords(5));u.mem_write(p+0x6C,dwords(100));u.mem_write(p+0x90,b'\x01')
        u.mem_write(p+0x9C,dwords(2688,2688,0));u.mem_write(p+0x6A0,dwords(-1))
        u.mem_write(p+0x6D8,dwords(-1))
        u.mem_write(t+0x898,dwords(WEAPON));u.mem_write(t+0xB4,dwords(100))
        u.mem_write(t+0x230,b'\x01') # normal ObjectType Selectable=true
        u.mem_write(t+0x231,b'\x01') # LegalTarget=true
    u.mem_write(ACTOR+0x2E4,dwords(TARGET if case['installed'] else 0))
    u.mem_write(HOUSE+0x30,dwords(0));u.mem_write(OTHER+0x30,dwords(1))
    u.mem_write(0xA8B238,dwords(1));u.mem_write(0xA83D4C,dwords(HOUSE))
    u.mem_write(0xA8B230,dwords(SCRATCH+0x20000));u.mem_write(0x8871E0,dwords(SCRATCH+0x22000))
    if case.get('allied_objects'):
        # Actual UnitType7470D0 constructor vtable (7471BF); fixture state,
        # not a constructor/load claim. Required by transport-capacity controls.
        u.mem_write(TYPE,dwords(0x7F6218));u.mem_write(TARGET_TYPE,dwords(0x7F6218))
        for house, name in ((HOUSE, 'actor'), (OTHER, 'target')):
            u.mem_write(house+0x30,dwords(case[name+'_house_index']))
            u.mem_write(house+0x5788,dwords(case[name+'_alliance_mask']))
        u.mem_write(TARGET+0x83,bytes([case.get('target_selected',False)]))
        u.mem_write(TYPE+0x6C0,bytes([case.get('attack_friendlies',False)]))
        u.mem_write(TYPE+0x6C1,bytes([case.get('attack_cursor_on_friendlies',False)]))
        u.mem_write(TARGET_TYPE+0x5E0,dwords(case.get('target_passengers',0)))
        u.mem_write(WARHEAD+0x157,bytes([case.get('ivan_bomb',False)]))
        u.mem_write(WARHEAD+0x14B,bytes([case.get('sonic',False)]))
        u.mem_write(TARGET+0x694,dwords(SCRATCH+0x2F000 if case.get('infested',False) else 0))
    u.mem_write(0xA8ECC8,dwords(1));u.mem_write(0xA8ECBC,dwords(COORD+0x100));u.mem_write(COORD+0x100,dwords(ACTOR))
    for addr,key in ((0xA8EC00,17),(0xA8EC04,17),(0xA8EBF8,18),(0xA8EBFC,18),(0xA8EC08,16),(0xA8EC0C,16)):
        u.mem_write(addr,dwords(key))
    u.mem_write(COORD,struct.pack('<hh',30,30));u.mem_write(CELL,dwords(0x7E4EEC));u.mem_write(CELL+0x24,struct.pack('<hh',30,30));u.mem_write(CELL+0x50,dwords(-1))
    u.mem_write(CELL+0x44,dwords(-1))
    u.mem_write(SLOT,dwords(WEAPON));u.mem_write(WEAPON+0xA0,dwords(PROJECTILE));u.mem_write(WEAPON+0xAC,dwords(WARHEAD));u.mem_write(WEAPON+0xB4,dwords(1280))
    u.mem_write(TYPE+0xE0E,bytes([case.get('harvester',False)]))
    u.mem_write(TYPE+0xE13,bytes([case.get('simple_deployer',False)]))
    u.mem_write(TYPE+0x6AC,bytes([case.get('deploy_fire',False)]))
    if case.get('repair'):
        # Supplied FV-shaped slot/health/control inputs. The original selector,
        # GetWeapon, signed Damage+AmbientDamage and health ratio execute below.
        u.mem_write(TYPE+0x808,dwords(18))
        u.mem_write(ACTOR+0x138,dwords(case.get('weapon_mode',1)))
        u.mem_write(TYPE+0x8B4,dwords(WEAPON+0x500))
        u.mem_write(WEAPON+0x98,dwords(60))
        u.mem_write(WEAPON+0x500+0x98,dwords(case.get('damage',-50)))
        u.mem_write(WEAPON+0x500+0xA4,dwords(case.get('ambient_damage',0)))
        u.mem_write(WEAPON+0x500+0xA0,dwords(PROJECTILE))
        u.mem_write(WEAPON+0x500+0xAC,dwords(WARHEAD))
        u.mem_write(TARGET_TYPE+0xA0,dwords(100))
        u.mem_write(TARGET+0x6C,dwords(case.get('health',50)))
        u.mem_write(TARGET_TYPE+0xE1B,bytes([case.get('non_vehicle',False)]))
        u.mem_write(SCRATCH+0x22000+0x16F8,struct.pack('<d',1.0))
        u.mem_write(HOUSE+0x5788,dwords(2 if case.get('allied',False) else 0))
        u.mem_write(ACTOR+0x298,bytes([case.get('actor_berserk',False)]))
    u.mem_write(ACTOR+0x6E0,bytes([case.get('deployed',False)]))
    u.mem_write(CELL+0xEC,dwords(case.get('land_type',0)))
    trail=[];trace=[]
    watch={0x692610:'DetermineAction',0x73FD50:'UnitObject',0x4DDED0:'FootObject',0x6FFEC0:'TechnoObject',0x7404B0:'UnitCell',0x4DDDE0:'FootCell',0x700600:'TechnoCell',0x700C40:'IsControllable',0x700C66:'BunkerGate',0x700C26:'NoneReturn',0x70055D:'ObjectAttackReturn',0x700AA8:'CellAttackReturn',0x7005DA:'SelectReturn'}
    if case.get('name'):
        watch.update({0x740623:'HarvesterMoveOnly',0x74037A:'ObjectSimpleDeployerTail',0x74072B:'CellSimpleDeployerTail',0x7403C7:'ObjectDeploySelectWeaponNULL',0x740793:'CellDeploySelectWeaponNULL'})
    watch.update({0x4AAE90:'DisplayEntry',0x4AAFC9:'DisplayNoObject',0x4AAFEC:'DisplayResolveCell',0x4AAFF1:'DisplayStoreCell',0x70F0B0:'DisplayModifierAction',0x4AB449:'DisplayNormalSwitch',0x4AB4EE:'DisplayAttackArm'})
    if case.get('repair'):
        watch.update({0x6F3970:'GetWeaponDamageValue',0x746CD0:'UnitSelectWeapon',
            0x6F3330:'TechnoSelectWeapon',0x70E140:'GetWeapon',0x5F5C60:'HealthRatio',
            0x73FE22:'UnitRepair',0x738890:'UnitClickedObject',0x7388D0:'RepairToAttack',
            0x4D74E0:'FootClickedObject',0x4D769F:'AttackMissionProducer'})
    call_trace=NativeCallTrace(u,read)
    if case.get('allied_objects'):
        watch.update({0x7003BD:'AttackFriendliesGrantReady',0x7003CD:'AllyGrantTest',
            0x70056C:'TechnoNonAttackTail',0x7005E6:'TechnoNoneReturn',
            0x7403EB:'UnitNonePostprocess',0x4AB9B0:'LeftUp',
            0x4ABE4A:'LeftUpSelect',0x4AE750:'LeftUpDispatch',
            0x738890:'UnitClickedObject',0x4D74E0:'FootClickedObject',
            0x4D77BD:'NoMoveObjectReceiver',0x4D77FB:'MoveAdmission',
            0x6FFBE0:'ClickedMission',0x6FFCBD:'OrderVoiceEnableGate',
            0x6FFD9B:'OrderMoveVoice',0x708FC0:'VoiceMove',0x708DB5:'QueueVoiceStore'})
    observed_specs={0x4F9A90:('IsAllyByObject',1,4),
        0x65C780:('MainRandom',0,0),0x65C7E0:('RandomRanged',2,8),
        0x708D90:('QueueVoice',1,4),0x70D150:('ActionTimerStart',0,0),
        0x7258D0:('Detach',2,8),0x5F5280:('ObjectDetach',1,4)}
    def hook(_u,pc,size,data):
        trail.append(hex(pc));trail[:]=trail[-30:]
        if pc in watch:trace.append({'kind':'original','pc':hex(pc),'name':watch[pc]})
        sp=u.reg_read(UC_X86_REG_ESP)
        if case.get('allied_objects'):
            call_trace.returned(pc,sp)
            if pc in observed_specs:call_trace.entered(pc,sp,observed_specs[pc])
        if pc==0x6FFBE0 and case.get('repair'):
            trace.append({'kind':'command_sink','pc':hex(pc),
                'mission':read(sp+4),'target':read(sp+8),
                'destination':read(sp+12),'queue':read(sp+16)})
            ret(1,4)
        if pc==0x4AAFF1:
            trace.append({'kind':'display_target_producer','pc':hex(pc),'target':u.reg_read(UC_X86_REG_EAX),'is_cell':u.reg_read(UC_X86_REG_EAX)==CELL})
        if pc==0x4AB4EE:
            trace.append({'kind':'display_target_consumer','pc':hex(pc),'target':read(sp+0x18),'is_cell':read(sp+0x18)==CELL})
        if pc==0x5353D0: # Single-selection object has no competitor.
            trace.append({'kind':'seam','name':'SelectBest-singleton','result':ACTOR});ret(ACTOR,0)
        elif pc==0x54F5C0:ret(int((read(sp+4)==17 and case.get('ctrl',True))
            or (read(sp+4)==18 and case.get('alt',False))
            or (read(sp+4)==16 and case.get('shift',False))),1)
        elif pc in (0x5657A0,0x565730):ret(CELL,1)
        elif pc==0x578080:ret(0,1)
        elif pc in (0x586360,0x5865E0):ret(0,1)
        elif pc==0x578460:ret(1,2)
        elif pc==0x74CD60:ret(0,0) # no terrain object
        elif pc==0x746CD0 and not case.get('repair'):
            entry={'kind':'seam','name':'SelectWeapon','result':0}
            if case.get('name'): entry['target']=read(sp+4)
            trace.append(entry);ret(0,1)
        elif pc==0x6F3970 and not case.get('repair'):ret(60,1) # positive supplied damage
        elif pc==0x6F7970 or pc==0x6F77B0:
            trace.append({'kind':'seam','name':'CanFireAtCell' if pc==0x6F7970 else 'CanFireAtObject','pc':hex(pc),'verdict':case['in_range'],'target':read(sp+4),'weapon_index':read(sp+8)})
            ret(int(case['in_range']),2)
        elif pc==0x6F7780:
            trace.append({'kind':'seam','name':'DisplayCanFireAtTarget','verdict':case['in_range']})
            ret(int(case['in_range']),1)
        elif pc==read(UNIT_VT+0x3C0):ret(0,3) # GetFireError OK; no ROF/legality parity claimed
    u.hook_add(UC_HOOK_CODE,hook)
    before=hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()
    techno_only=case.get('techno_only',False)
    u.mem_write(SP,dwords(RET_MAGIC,*((TARGET,0) if techno_only else (COORD,TARGET if case['route']=='object' else 0,0))));u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,ACTOR if techno_only else 0x87F7E8)
    required=[0x6FFEC0] if techno_only else [0x692610,0x73FD50 if case['route']=='object' else 0x7404B0,0x6FFEC0 if case['route']=='object' else 0x700600]
    try:run_checked(u,0x6FFEC0 if techno_only else 0x692610,RET_MAGIC,count=100000,required_addresses=required)
    except Exception as e:raise RuntimeError(f'{case}: {e}; trail={trail}; trace={trace}') from e
    action=u.reg_read(UC_X86_REG_EAX)
    assert before==hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()
    if techno_only:
        call_trace.returned(RET_MAGIC,u.reg_read(UC_X86_REG_ESP))
        assert not call_trace.pending
        assert not any(row['name']!='IsAllyByObject' for row in call_trace.calls)
        return dict(input=case,entry='0x006FFEC0',action=action,trace=trace,
                    ally_queries=[bool(row['return_eax']&255) for row in call_trace.calls if row['name']=='IsAllyByObject'],
                    main_draw_count=0,timer_start_calls=0,detach_calls=0)
    # Execute the actual Display prefix: a missing hovered Object is NOT a
    # missing Abstract target. Valid Cell coordinates are promoted to CellClass*
    # at4AAFEC..4AAFF1 before4AB4EE asks the selected actor's vt+3AC.
    receiver,vt,sink=SCRATCH+0x28000,SCRATCH+0x29000,RET_MAGIC+0x100
    surface,pixels,waypoints=SCRATCH+0x2A000,SCRATCH+0x2A200,SCRATCH+0x2B000
    u.mem_write(receiver,dwords(vt));u.mem_write(vt+0x48,dwords(sink))
    # Display's unchanged palette/waypoint prefix has harmless storage, rather
    # than skipping the Cell target producer or supplying its stack output.
    u.mem_write(0x87F6C8,dwords(surface));u.mem_write(surface+0x174,dwords(pixels))
    for index in range(12):
        u.mem_write(HOUSE+0x210+index*4,dwords(waypoints+index*0x40))
    u.mem_write(0xB0FE58,b'\x00')  # persistent attack-move toggle absent
    u.mem_write(0xA8ECC8,dwords(case.get('display_selected_count',1)))
    assert bytes(u.mem_read(COORD,4)) != bytes(u.mem_read(0x8A03F8,4))
    u.mem_write(SP+0x20,dwords(RET_MAGIC,COORD,0,TARGET if case['route']=='object' else 0,action,0))
    u.reg_write(UC_X86_REG_ESP,SP+0x20);u.reg_write(UC_X86_REG_ECX,receiver)
    required=[0x4AAE90,0x70F0B0,0x4AB449]
    if case['route']=='cell': required += [0x4AAFC9,0x4AAFEC,0x4AAFF1]
    run_checked(u,0x4AAE90,sink,count=10000,required_addresses=required)
    sp=u.reg_read(UC_X86_REG_ESP);row,arg=struct.unpack('<II',u.mem_read(sp+4,8))
    assert u.reg_read(UC_X86_REG_ECX)==receiver and sp==SP-12 and arg==0
    assert before==hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()
    descriptor=bytes(u.mem_read(0x82D028+row*28,28))
    cursor={'output':{'entry':'004AAE90','selected_count':case.get('display_selected_count',1),'cursor_row':row,'descriptor_hex':descriptor.hex(),'descriptor_dwords':struct.unpack('<7i',descriptor)}}
    if case.get('repair'):
        # ClickedAction re-queries WhatAction, then converts Repair29 to Attack5.
        # Stop enemy Move2 at the existing coordinate receiver: its locomotor
        # and event-byte production are outside this bounded action fixture.
        u.mem_write(SP,dwords(RET_MAGIC,action,TARGET,0))
        u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,ACTOR)
        stop=run_checked(u,0x738890,(RET_MAGIC,0x4D78BA),count=100000,
            required_addresses=[0x738890,0x73FD50])
        trace.append({'kind':'clicked_action_stop','pc':hex(stop)})
        assert before==hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()
    result={'input':case,'action':action,'trace':trace,'cursor':cursor,'text_sha256':before}
    if continuation is not None:
        result.update(continuation(u,read,ret,call_trace,result,receiver))
        assert before==hashlib.sha256(bytes(u.mem_read(0x401000,0x3E0000))).hexdigest()
    return result

def generate():
    cases=[dict(route=route,installed=installed,in_range=near,relation=relation) for route in ('cell','object') for installed in (False,True) for near in (False,True) for relation in (('enemy','own') if route=='object' else ('none',))]
    cases += [dict(route='object', installed=installed,in_range=near,relation='enemy',ctrl=False) for installed in (False,True) for near in (False,True)]
    from itertools import product
    postprocess_cases=[dict(route='cell',installed=False,in_range=near,relation='none',ctrl=True,
        harvester=harv,land_type=land,name='harvester')
        for harv,land,near in product((False,True),(0,5),(False,True))]
    postprocess_cases += [dict(route=route,installed=False,in_range=near,
        relation='enemy' if route=='object' else 'none',ctrl=True,
        simple_deployer=True,deployed=deployed,deploy_fire=fire,name='simple_deployer')
        for route,deployed,fire,near in product(('cell','object'),(False,True),(False,True),(False,True))]
    display_cases=[dict(route=route,installed=True,in_range=True,
        relation='enemy' if route=='object' else 'none',ctrl=True,display_selected_count=count)
        for route,count in product(('cell','object'),(2,3))]
    repair_base=dict(route='object',installed=False,in_range=True,relation='own',ctrl=False,repair=True)
    repair_cases=[repair_base | dict(name=name,**changes) for name,changes in (
        ('damaged_friend',{}),('full_friend',dict(health=100)),
        ('overfull_friend',dict(health=101)),('empty_ifv',dict(weapon_mode=0)),
        ('enemy',dict(relation='enemy')),('non_vehicle',dict(non_vehicle=True)),
        ('ctrl_friend',dict(ctrl=True)),('alt_friend',dict(alt=True)),
        ('ctrl_alt_friend',dict(ctrl=True,alt=True)),
        ('shift_friend',dict(shift=True)),('far_friend',dict(in_range=False)),
        ('one_way_ally',dict(relation='enemy',allied=True)),
        ('zero_total_damage',dict(ambient_damage=50)),
        ('positive_total_damage',dict(ambient_damage=51)),
        ('berserk_friend',dict(actor_berserk=True)),
    )]
    return {'schema':1,'rows':[run(c) for c in cases],
            'postprocess_rows':[run(c) for c in postprocess_cases],
            'display_controls':[run(c) for c in display_cases],
            'repair_rows':[run(c) for c in repair_cases]}

def metadata():
    return provenance(scope=__doc__,
        entry_points={'determine_action':0x692610,'unit_cell':0x7404B0,'foot_cell':0x4DDDE0,
            'techno_cell':0x700600,'unit_object':0x73FD50,'foot_object':0x4DDED0,
            'techno_object':0x6FFEC0,'is_controllable':0x700C40,
            'display_entry':0x4AAE90,'display_cell_target':0x4AAFF1,'display_normal_dispatch':0x4AB449,'cursor_table':0x82D028,
            'unit_clicked_object':0x738890,'foot_clicked_object':0x4D74E0,
            'weapon_damage_value':0x6F3970,'get_weapon':0x70E140,'health_ratio':0x5F5C60},
        assumptions=['One selected live normal Unit with an original vtable; supplied complete type/house pointers, positive weapon, Selectable/LegalTarget=true, idle death frame=-1, no transport capability. Normal rows have no deploy capability; postprocess rows supply the retained UnitType Harvester/IsSimpleDeployer/DeployFire and Unit+6E0 deployed fields. This is a bounded actor action fixture, not original construction/load or retail data validation.',
            'Installed controls supply nonnull Techno+2E4. Native IsControllable reads it; install lifecycle and weapon range arithmetic belong to locomotor_force_track.bunker_install and walk_cell_range_bunker corpora.',
            'Map lookup supplies one clear revealed playable flat Cell. Ctrl state supplied via original keyboard-call seam; four normal enemy Object controls have Ctrl=false.',
            'Eight Harvester controls vary Harvester flag, clear/ore LandType0/5 and range verdict under Ctrl. Sixteen SimpleDeployer controls vary Cell/Object route, deployed byte, DeployFire and range. Both wrappers select weapon with NULL in the deployed tail; trace records this input. No native deployment lifecycle, INI or weapon-selection arithmetic is claimed.',
            'Repair rows supply an authorized local Unit, TurretCount18, current slot1, Damage=-50 and AmbientDamage=0 (except sign controls), a positive slot0, target Strength100 with health50/100/101, and Rules+16F8=1.0. These are supplied control inputs, not native retail reader or construction claims. GetWeaponDamageValue6F3970, Unit/Techno SelectWeapon and GetWeapon70E140 execute unchanged. No Engineer flag, cargo type, warhead name or repair-result decision is supplied. Own/allied/enemy/NonVehicle, near/far and Ctrl/Alt/Shift controls execute the complete original action/display paths. The caller sets one-way alliance bit1 in House+5788.',
            'Repair ClickedAction738890 re-queries the object action and executes the original Repair29-to-Attack5 conversion and Foot4D769F mission/target/destination argument producer. The berserk_friend control differs from damaged_friend only by the supplied actor+298 Berserk byte1: action/display still resolve Repair29/cursor33, while unchanged UnitClickedAction suppresses the command. This is a bounded NoCommand producer observation, not berserk initialization, expiry or Hunt lifecycle validation. PlayerSendCommand6FFBE0 is an observed argument sink, not a supplied outcome. Enemy Move2 stops at4D78BA before its coordinate/locomotor dependency. Complete event encoding/delivery, movement, firing/repair cadence, RNG, audio and Windows pixels are excluded.',
            'Display4AAE90 executes from its real entry. The original prefix resolves valid no-Object coordinates through Map5657A0 to CellClass* at4AAFF1;4AB4EE consumes that target. Twelve empty House waypoint containers and palette storage support unchanged prefix instructions; the persistent attack-move toggle is0 and ordinary Ctrl does not select Ctrl+Shift. No Windows renderer, setter/frame/SHP output or full waypoint lifecycle is claimed.'],
        substitutions=['SelectBest5353D0 returns the supplied actor. Ordinary action rows have one selected actor; four display_controls change only the later Display selected-count input to2/3 after action calculation, establishing4AB4F6 count admission without claiming multi-object SelectBest or selection lifecycle.',
            'Keyboard54F5C0 answers the supplied Ctrl state; map lookups5657A0/565730 return supplied Cell; height578080=0, shroud586360/5865E0=false, playable578460=true and terrain-object74CD60=null.',
            'Ordinary/postprocess rows: SelectWeapon746CD0 supplies index0 and damage6F3970 supplies60. Repair rows execute both bodies unchanged. CanFireAt6F77B0 and coordinate wrapper6F7970 supply the case range verdict for every row. This establishes caller control flow, not range arithmetic.',
            'GetFireError virtual3C0 supplies OK; Display CanFireAtTarget6F7780 supplies the same range verdict. No reload/ROF/legality timing claim.',
            'Display receiver virtual48 terminates at an observed sink; complete immutable text SHA checked before/after both runs.'])


def allied_case(name, **changes):
    value=dict(name=name,route='object',installed=False,in_range=True,relation='other',
        ctrl=False,alt=False,shift=False,allied_objects=True,
        actor_house_index=0,target_house_index=1,actor_alliance_mask=2,
        target_alliance_mask=1,target_selected=False,attack_friendlies=False,
        attack_cursor_on_friendlies=False,target_passengers=0,ivan_bomb=False,
        sonic=False,infested=False)
    value.update(changes)
    if value['relation']=='own':value['target_house_index']=value['actor_house_index']
    return value


def allied_continuation(u, read, ret, calls, result, receiver):
    """Continue the existing action VM to literal UI/receiver boundaries."""
    action=result['action']
    u.mem_write(COORD+0x40,dwords(2688,2688,0))
    u.mem_write(SP,dwords(RET_MAGIC,COORD+0x40,COORD,TARGET,action,0))
    u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,receiver)
    stop=run_checked(u,0x4AB9B0,
        (0x4AE750,0x4ABE4A,0x4ABD63,0x4ABD94,0x4AC294,RET_MAGIC),
        count=100000,required_addresses=[0x4AB9B0,0x4ABD1D])
    boundary={0x4AE750:'object_dispatch',0x4ABE4A:'replace_selection',
        0x4ABD63:'shift_selected',0x4ABD94:'shift_unselected',
        0x4AC294:'no_dispatch',RET_MAGIC:'returned'}[stop]
    left_up=dict(entry='0x004AB9B0',stop=hex(stop),boundary=boundary)
    clicked=None
    if stop==0x4AE750:
        # Actual per-object receiver. The ordinary NoMove path stops BEFORE
        # native navigation/passability selection; no destination is supplied.
        u.mem_write(SP,dwords(RET_MAGIC,action,TARGET,0))
        u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,ACTOR)
        stop=run_checked(u,0x738890,(RET_MAGIC,0x4D78BA,0x6FFBE0),
            count=100000,required_addresses=[0x738890,0x73FD50])
        clicked=dict(entry='0x00738890',stop=hex(stop),
            boundary={RET_MAGIC:'refused',0x4D78BA:'object_navigation_coordinates',
                      0x6FFBE0:'mission_arguments'}[stop])
        if stop==0x6FFBE0:
            sp=u.reg_read(UC_X86_REG_ESP)
            clicked['mission']=read(sp+4)
            clicked['target_is_hovered_object']=read(sp+8)==TARGET
    if calls.pending:
        raise RuntimeError(f'Unreturned original calls: {calls.pending}')
    assert not any(row['name'] in ('MainRandom','RandomRanged','ActionTimerStart','Detach','ObjectDetach')
                   for row in calls.calls)
    return dict(left_up=left_up,clicked_action=clicked,
        ally_queries=[dict(asking_house='actor' if int(row['ecx'],16)==HOUSE else 'target',
                           target_is_hovered_object=row['args'][0]==TARGET,
                           allied=bool(row['return_eax']&255))
                      for row in calls.calls if row['name']=='IsAllyByObject'],
        main_draw_count=0,timer_start_calls=0,detach_calls=0)


def move_voice_control(spec):
    """Use the SAME action bootstrap; shared RNG owner supplies seed/continuation."""
    from tools.rmg_oracle.gen_rng_vectors import STRUCT_LEN, seeded_struct, draws

    def continuation(u, read, ret, calls, result, receiver):
        rngs={'main':0x886B88,'scenario':SCRATCH+0x20000+0x218,'mapgen':0xABE890}
        for pointer in rngs.values():u.mem_write(pointer,seeded_struct(spec['seed']))
        before={name:bytes(u.mem_read(pointer,STRUCT_LEN)) for name,pointer in rngs.items()}
        voice_data=SCRATCH+0x2D000
        values=spec['voice_ids']
        u.mem_write(voice_data,dwords(*(values or [0])))
        u.mem_write(TYPE+0x46C,dwords(voice_data,len(values)))
        u.mem_write(TYPE+0x478,dwords(len(values)))
        u.mem_write(ACTOR+0x4F0,dwords(777))
        u.mem_write(ACTOR+0x21C,dwords(HOUSE if spec['actor_owner']=='local' else OTHER))
        u.mem_write(OTHER+0x1EC,bytes([spec.get('other_human',False)]))
        u.mem_write(OTHER+0x1ED,bytes([spec.get('other_player_control',False)]))
        u.mem_write(0xA8B238,dwords(spec.get('mode',1)))
        u.mem_write(0x822CF2,bytes([spec['voices_enabled']]))
        first_call=len(calls.calls)
        first_trace=len(result['trace'])
        # Non-planning ordinary input. Planning637AA0 runs unchanged; its
        # original false return selects6FFCBD. Stop before target/event codecs.
        entry=int(spec['caller_entry'],16)
        args=(2,0,TARGET,0) if entry==0x6FFBE0 else ()
        stop=0x6FFDA5 if args else RET_MAGIC
        u.mem_write(SP,dwords(RET_MAGIC,*args))
        u.reg_write(UC_X86_REG_ESP,SP);u.reg_write(UC_X86_REG_ECX,ACTOR)
        producer=None
        if args:
            # Native Move producer after the declared spatial choice seam.
            # EDI=chosen Object / EAX=null planning Cell are supplied inputs;
            # the mission2 and NULL attack-target stack values are NOT supplied.
            u.mem_write(SP,dwords(RET_MAGIC))
            u.reg_write(UC_X86_REG_ESI,ACTOR)
            u.reg_write(UC_X86_REG_EDI,TARGET)
            u.reg_write(UC_X86_REG_EAX,0)
            run_checked(u,0x4D7C76,entry,required_addresses=[0x4D7CB8])
            sp=u.reg_read(UC_X86_REG_ESP)
            producer=dict(entry='0x004D7C76',mission=read(sp+4),
                attack_target_is_null=read(sp+8)==0,destination_is_supplied_object=read(sp+12)==TARGET,
                planning_cell_is_null=read(sp+16)==0)
        run_checked(u,entry,stop,count=100000,required_addresses=[entry])
        calls.returned(stop,u.reg_read(UC_X86_REG_ESP))
        if calls.pending:raise RuntimeError(f'Unreturned voice calls: {calls.pending}')
        after={name:bytes(u.mem_read(pointer,STRUCT_LEN)) for name,pointer in rngs.items()}
        observed=calls.calls[first_call:]
        assert all(int(row['ecx'],16)==rngs['main'] for row in observed if row['name']=='MainRandom')
        assert not any(row['name'] in ('RandomRanged','ActionTimerStart','Detach','ObjectDetach') for row in observed)
        raw=[row['return_eax'] for row in observed if row['name']=='MainRandom']
        reference,reference_after=draws(before['main'],len(raw))
        assert reference==raw and reference_after==after['main']
        assert all(before[name]==after[name] for name in ('scenario','mapgen'))
        signed=lambda x:struct.unpack('<i',struct.pack('<I',x))[0]
        return dict(voice_output=dict(
            main_before_hex=before['main'].hex(),main_after_hex=after['main'].hex(),
            main_draws=raw,main_next_four=draws(after['main'],4)[0],
            scenario_unchanged=True,mapgen_unchanged=True,
            queued_voice_before=777,queued_voice_after=signed(read(ACTOR+0x4F0)),
            queue_requests=[signed(row['args'][0]) for row in observed if row['name']=='QueueVoice'],
            voice_move_calls=sum(row.get('name')=='VoiceMove' for row in result['trace'][first_trace:]),
            move_producer=producer,timer_start_calls=0,detach_calls=0,
            trace=result['trace'][first_trace:],stop=hex(stop)))

    result=run(allied_case(spec['name']),continuation)
    return dict(name=spec['name'],input=spec,output=result['voice_output'])


def infantry_bomb_control(spec):
    """Original51EB24 consumer, with canonical base action explicitly supplied.

    No Infantry WhatAction wrapper or preceding weapon/ability result is
    substituted. A failed admission stops BEFORE the unrelated Infantry tail.
    """
    def continuation(u, read, ret, calls, result, receiver):
        u.mem_write(ACTOR,dwords(0x7EB058))  # actual Infantry vtable
        u.mem_write(TYPE,dwords(0x7EB610))  # original5237E8 InfantryType table
        u.mem_write(ACTOR+0x21C,dwords(HOUSE if spec['actor_owner']=='local' else OTHER))
        u.mem_write(TYPE+0x6C0,bytes([spec['attack_friendlies']]))
        u.mem_write(TYPE+0x6C1,bytes([spec['attack_cursor_on_friendlies']]))
        u.mem_write(TYPE+0xEAE,bytes([spec['ivan']]))
        u.mem_write(TARGET_TYPE+0x22E,bytes([spec['bombable']]))
        u.mem_write(TARGET+0x38,dwords(SCRATCH+0x2F000 if spec['has_bomb'] else 0))
        u.mem_write(0xA8B238,dwords(spec['mode']))
        native_calls=NativeCallTrace(u,read)
        first_call=len(calls.calls)
        visited=[]
        points={0x51EB24,0x51EB33,0x51EB38,0x51EB48,0x51EB5C,0x51EB66,0x51EB75}

        def observe(machine,pc,size,data):
            sp=u.reg_read(UC_X86_REG_ESP)
            native_calls.returned(pc,sp)
            if pc==0x50B6F0:native_calls.entered(pc,sp,('HouseControl',0,0))
            if pc in points:visited.append(hex(pc))

        observer=u.hook_add(UC_HOOK_CODE,observe)
        # Recreate the real51E3B0 prologue's saved-register/local stack shape.
        # The fragment's native epilogue pops4 registers, adds28h and RET8.
        u.mem_write(SP,dwords(0,0,0,0))
        u.mem_write(SP+0x38,dwords(RET_MAGIC,TARGET,0))
        u.reg_write(UC_X86_REG_ESP,SP)
        u.reg_write(UC_X86_REG_EDI,ACTOR);u.reg_write(UC_X86_REG_ESI,TARGET)
        u.reg_write(UC_X86_REG_EBP,spec['base_action'])
        try:
            stop=run_checked(u,0x51EB24,(RET_MAGIC,0x51EB81),
                count=1000,required_addresses=[0x51EB24,0x50B6F0])
            native_calls.returned(stop,u.reg_read(UC_X86_REG_ESP))
            assert not native_calls.pending
            # The existing shared observer would record RNG/timer/detach
            # entries; none occurs in this body or its real getter/House calls.
            assert not calls.calls[first_call:]
        finally:u.hook_del(observer)
        returned=stop==RET_MAGIC
        if returned:assert u.reg_read(UC_X86_REG_ESP)==SP+0x44
        return dict(infantry_bomb_output=dict(entry='0x0051EB24',stop=hex(stop),
            boundary='returned' if returned else 'infantry_tail',
            action=u.reg_read(UC_X86_REG_EAX) if returned else None,
            base_action_at_tail=None if returned else u.reg_read(UC_X86_REG_EBP),
            house_control=bool(native_calls.calls[0]['return_eax']&255),
            visited=visited,main_draw_count=0,timer_start_calls=0,detach_calls=0))

    result=run(allied_case(spec['name']),continuation)
    return dict(name=spec['name'],input=spec,output=result['infantry_bomb_output'])


def generate_allied_objects():
    from tools.rules_oracle.weapon_laser import physical_layers
    from tools.projectile_oracle.bridge_render_inputs import lexical
    keys={'MTNK':('Primary','AttackFriendlies','AttackCursorOnFriendlies','LegalTarget','VoiceMove'),
          '105mm':('Warhead','Damage','TerrainFire'),'AP':('IvanBomb','Sonic')}
    retail_inputs=[]
    for filename,archive,raw in physical_layers():
        if raw is None:
            retail_inputs.append(dict(file=filename,absent=True))
            continue
        sections,_=lexical(raw,set(keys))
        retail_inputs.append(dict(file=filename,archive=archive,
            sha256=hashlib.sha256(raw).hexdigest(),
            selected_strings={name:{key:sections[name].get(key) for key in fields}
                              for name,fields in keys.items() if name in sections}))
    controls=[allied_case(name,**changes) for name,changes in (
        ('own_unselected',dict(relation='own')),
        ('own_selected',dict(relation='own',target_selected=True)),
        ('other_ally',{}),
        ('actor_only_ally',dict(target_alliance_mask=0)),
        ('target_only_ally',dict(actor_alliance_mask=0)),
        ('enemy',dict(actor_alliance_mask=0,target_alliance_mask=0)),
        ('same_index_other_pointer',dict(target_house_index=0,actor_alliance_mask=0,target_alliance_mask=0)),
        ('unowned_index',dict(target_house_index=-1,actor_alliance_mask=0,target_alliance_mask=0)),
        ('ctrl_ally',dict(ctrl=True)),
        ('ctrl_own',dict(relation='own',ctrl=True)),
        ('ctrl_enemy',dict(actor_alliance_mask=0,target_alliance_mask=0,ctrl=True)),
        ('shift_ally',dict(shift=True)),
        ('shift_own',dict(relation='own',shift=True)),
        ('alt_ally',dict(alt=True)),
        ('attack_friendlies',dict(attack_friendlies=True)),
        ('attack_cursor_friendlies',dict(attack_cursor_on_friendlies=True)),
        ('ivan_empty_keeps_cursor_grant',dict(attack_cursor_on_friendlies=True,ivan_bomb=True)),
        ('sonic_infested_ally',dict(sonic=True,infested=True)),
        ('sonic_uninfested_ally',dict(sonic=True)),
        ('ordinary_infested_ally',dict(infested=True)),
    )]
    base=dict(seed=1,voice_ids=[101],voices_enabled=True,actor_owner='local',
              mode=1,other_human=False,other_player_control=False,caller_entry='0x006FFBE0')
    voices=[dict(base,name=name,**changes) for name,changes in (
        ('move_singleton',{}),('move_empty',dict(voice_ids=[])),
        ('move_disabled_caller',dict(voices_enabled=False)),
        ('move_nonhuman_owner',dict(actor_owner='other')),
        ('move_campaign_human_owner',dict(actor_owner='other',mode=0,other_human=True)),
        ('move_campaign_player_control',dict(actor_owner='other',mode=0,other_player_control=True)),
        ('move_minus_one',dict(voice_ids=[-1])),
        ('move_multiple',dict(voice_ids=[101,202,303])),
        ('direct_disabled_still_draws',dict(caller_entry='0x00708FC0',voices_enabled=False)),
    )]
    return dict(schema=1,native_sha256=image_sha256(),
        retail_inputs=retail_inputs,
        action_controls=[run(case,allied_continuation) for case in controls],
        grant_controls=[run(allied_case(name,techno_only=True,**changes)) for name,changes in (
            ('ivan_transport_blocks_cursor_grant',dict(attack_cursor_on_friendlies=True,ivan_bomb=True,target_passengers=1)),
            ('attack_friendlies_overrides_ivan',dict(attack_friendlies=True,ivan_bomb=True,target_passengers=1)),
        )],
        voice_controls=[move_voice_control(spec) for spec in voices],
        infantry_bomb_controls=[infantry_bomb_control(dict(
            dict(base_action=5,attack_friendlies=True,attack_cursor_on_friendlies=False,
                 ivan=True,bombable=True,has_bomb=False,actor_owner='local',mode=1),
            name=name,**changes)) for name,changes in (
            ('attack_friendlies_only_ivan',{}),
            ('ivan_nonbombable',dict(bombable=False)),
            ('ivan_already_bombed',dict(has_bomb=True)),
            ('non_ivan_falls_through',dict(ivan=False)),
            ('nonhuman_ivan_falls_through',dict(actor_owner='other')),
            ('nonattack_base_falls_through',dict(base_action=2)),
        )])


def allied_metadata():
    return provenance(scope='Ordinary MTNK allied-object action/cursor/LeftUp boundaries, shared attack grants, Move voice admission and bounded Infantry Ivan consumer on the existing bunker action VM. Whole action functions and full Move/event/audio lifecycles remain partial.',
        entry_points={'determine_action':0x692610,'unit_object':0x73FD50,
            'foot_object':0x4DDED0,'techno_object':0x6FFEC0,'is_ally_by_object':0x4F9A90,
            'display_cursor':0x4AAE90,'left_up':0x4AB9B0,'left_up_dispatch':0x4AE750,
            'unit_clicked_object':0x738890,'foot_clicked_object':0x4D74E0,
            'navigation_coordinate_boundary':0x4D78BA,'move_argument_producer':0x4D7C76,'clicked_mission':0x6FFBE0,
            'order_voice_gate':0x6FFCBD,'voice_move':0x708FC0,'queue_voice':0x708D90,
            'house_player_control_query':0x50B6F0,'rng_raw':0x65C780,
            'infantry_bomb_consumer':0x51EB24,'infantry_bomb_tail_boundary':0x51EB81},
        assumptions=[
            'Original Unit/UnitType vtables and supplied live actor/type/House/weapon state reuse run(). This is not native constructor/scenario/INI execution. Ordinary actor/target represent stock MTNK: Selectable/LegalTarget=true, AttackFriendlies/AttackCursorOnFriendlies=false, positive weapon sign corresponding to105mm/AP; no disguise/transport/deploy/EMP/bunker/berserk. Other-grant and same-index/unowned controls are explicit input boundaries, not stock MTNK configuration/lifecycle claims.',
            'Original ctor710AF0 stores AttackFriendlies+6C0=false at71117B and AttackCursorOnFriendlies+6C1=false at711181. Reader712170 admits through ObjectType5F92D0; selected current-default ReadBool5295F0 calls71523D and71525E store715242/715263 from exact keys843620/843604. LegalTarget current-default reader5F94AE/store5F94C2 retains constructor5F7168=true. No clamp. Physical RULESMD/LANGRULE-if-present/MPBattleMD/XMP03T4 have no MTNK overrides of these fields. Fixed ART is not layered and does not decide these action gates.',
            'MTNK physical selected keys Primary=105mm,105mm.Warhead=AP,VoiceMove=GenAllVehicleMove. Warhead IvanBomb+157 and Sonic+14B false for ordinary AP; explicit controls supply each branch and target Type.Passengers+5E0 / target Foot+694 pointer. They do not execute IvanBomb or parasite creation/cleanup. grant_controls execute complete Techno6FFEC0 only: the rejected passenger-capacity case would subsequently enter a separate Unit transport mechanism absent from this ordinary fixture. No transport verdict is substituted. Other special target/disguise/terrain/LegalTarget and arbitrary RTTI routes remain partial.',
            'House pointers, signed indices and directional+5788 alliance masks are declared inputs. Original4F9A90 executes target+3C=6F9DC0 real owner; same pointer/index or asking-House bit decides. Target+38=6F9DB0 returns real owner index for70058D..59B; no disguised-owner result is substituted.',
            'LeftUp4AB9B0 executes from entry to original selection/dispatch boundary. UnitClickedAction738890 re-queries native action and executes Foot4D74E0. NoMove2 reaches4D78BA before navigation coordinates, nearby-cell search or command enqueue; it can issue Move and is not a no-command result. Instruction4D7C76..7CBA emits mission2 with the chosen destination. No destination or movement/RNG result is invented.',
            'Voice controls execute in that same VM, with supplied list+468 and fixture-relative sound IDs; MTNK name binding is physical but original type/sound readers and full catalog IDs are outside this bounded action fixture. Reuse shared gen_rng_vectors.seeded_struct/draws for original seed/continuation. Three RNG states have equal supplied native seed. The original Foot Move producer4D7C76..7CBA runs with declared ESI=actor, EDI=chosen target Object, EAX=null planning Cell after the spatial choice seam. It produces mission2/NULL attack target and enters complete6FFBE0 prefix; nonplanning637AA0 executes unchanged and control stops6FFDA5 before target/event codecs. This component does not assert that the bounded Foot receiver produced that destination.',
            'VoiceMove708FC0 consumes one actual Main65C780 draw for every nonempty list including singleton/-1. Original QueueVoice708D90 then tests global enable, selected ID!=-1 and real House50B6F0 before storing+4F0. QueueMegaMission6FFBE0 tests enable BEFORE calling VoiceMove, so disabled caller spends zero. Direct disabled VoiceMove is a separate ordering control. Scenario and MapGen remain byte-identical. No OS audio playback claim.',
            'infantry_bomb_controls reuse the same action VM, then supply actual Infantry7EB058/InfantryType7EB610 vtables, EDI=actor, ESI=ordinary target and EBP=canonical base action at original51EB24. The real51E3B0 saved-register/local stack shape supports its unchanged RET8 epilogue. Native House50B6F0, Ivan Type+EAE, target+88 ObjectType getter, Bombable+22E and attachedBomb+38 execute without substituted virtual answers. BaseAction5/localIvan/Bombable/noBomb returns53; nonbombable or bombed returns54. Rejected House/Ivan/base-action admission stops BEFORE51EB81 and records final action=null plus unchanged base action: no final Infantry-tail outcome is invented. Whole Infantry51E3B0 prefix/ability/transport/lifecycle and scalar native reader execution remain excluded. AttackFriendlies=true/AttackCursorOnFriendlies=false are declared canonical-admission priors; this consumer does not read those grant bytes again. No RNG, action-timer or detach entry occurs.',
            'Action/cursor/LeftUp-prefix/receiver controls reach no RNG draw, timer write or detach. Full LeftUp dispatch later clears822CF2 after every actor/restores1 (4AE852/85F), restarts action-line timer70D150 to25 at4ABFAE, and dispatches through existing Move/event/voice owners. These later effects are instruction evidence, not a whole-window execution claim.'],
        substitutions=[
            'Reuse unchanged historical run() substitutions: singleton SelectBest, supplied keyboard, revealed flat Cell/map/height/playfield/terrain, weapon slot/damage, range and FireError. Original House alliance/owner queries, complete DetermineAction/class action bodies, Display cursor switch, LeftUp prefix, UnitClickedAction/Foot prefix, VoiceMove/RNG/QueueVoice/House admission execute; no native instructions are patched.',
            'The mock Display virtual48 remains an observed cursor setter sink with palette/empty-waypoint storage. UI cursor SHP/glyph raster is outside the fixture. Original text hash is checked before/after every shared-VM continuation.'])


if __name__=='__main__':
    allied='--allied-objects' in sys.argv
    sources={'harness':Path(__file__),'native_oracle':Path(native_oracle.__file__)}
    if allied:
        sources.update(rng_owner=Path(__file__).parents[1]/'rmg_oracle/gen_rng_vectors.py',
            physical_inputs_owner=Path(__file__).parents[1]/'rules_oracle/weapon_laser.py',
            lexical_owner=Path(__file__).parents[1]/'projectile_oracle/bridge_render_inputs.py')
    finish_vectors(generate_allied_objects if allied else generate,
        Path(__file__).with_name('allied_object_action.json') if allied else Path(__file__).with_suffix('.json'),
        provenance=allied_metadata if allied else metadata,
        argv=[arg for arg in sys.argv[1:] if arg!='--allied-objects'],
        source_paths=sources)
