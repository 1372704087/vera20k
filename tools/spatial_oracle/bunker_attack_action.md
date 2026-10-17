# Object action and allied-House controls

The default `bunker_attack_action` mode retains its historical action, bunker,
deployer, Display and repair payload. Its assumptions remain in that sidecar;
the ordinary/default fixture is not a retail construction or full weapon-range
oracle.

```
python -m tools.spatial_oracle.bunker_attack_action --check
python -m tools.spatial_oracle.bunker_attack_action --allied-objects --check
```

Select the supported original executable with `VERA20K_GAMEMD_EXE` (see
[native_oracle.md](../native_oracle.md)). Add `--write` only to deliberately
regenerate a reference from original execution. The additive mode writes
`allied_object_action.json` and its metadata sidecar. No native instructions are
patched. Both modes share `run()` and the same actor/map/Display bootstrap;
Move acknowledgment executes on that same VM. RNG initialization and continuation
reuse `gen_rng_vectors.seeded_struct`/`draws`, not a new generator.

## Consumption boundaries

`action_controls` execute original DetermineAction692610 ->Unit73FD50
->Foot4DDED0 ->Techno6FFEC0 and the original Display4AAE90 cursor path.
An original House4F9A90 observer records its real result; it is not substituted.
House indices and directional alliance masks are supplied inputs. The real
Unit owner getters6F9DC0/+3C and6F9DB0/+38 distinguish same-owner/index from
another allied House. The supplied ordinary MTNK's attack grants are false.

Each row keeps `input`, `action`, `cursor`, `trace`, plus:

- `ally_queries`: original real-owner alliance decisions;
- `left_up`: original4AB9B0 entry to its selection or dispatch boundary;
- `clicked_action`: original738890 re-query and Foot receiver, when dispatched;
- zero observed RNG draws, action-timer starts and detach calls within that bound.

Ordinary other-House ally: action2/cursor19, LeftUp dispatch4AE750, then
NoMove receiver4D77BD ->Move admission4D77FB ->navigation boundary4D78BA.
Same-owner unselected: action7/cursor17 and replacement-selection boundary.
Enemy and forced Ctrl: action5/cursor20. Shift explicitly selects the toggle
path (action8); it must not be conflated with unmodified allied input.

The NoMove result is **not** a command refusal. The fixture stops before
navigation coordinates and map passability/nearby-cell choice; no expected
destination or movement RNG is invented. Original4D7C76..4D7CBA subsequently
emits mission2 with the chosen Object/Cell through existing ClickedMission.
The app/Core movement and input tests consume that existing production owner.

`grant_controls` execute complete Techno6FFEC0 only. IvanBomb+target
Passengers>0 blocks AttackCursorOnFriendlies; AttackFriendlies overrides that
block. After rejection, the full Unit wrapper would enter transport admission,
which this ordinary action fixture does not initialize. No transport callback
is replaced to manufacture an ordinary-path verdict. Sonic+Foot infestation
and other no-capacity grant controls retain the full ordinary caller path.
These are declared consumer priors, not parasite/bomb/transport lifecycle proof.

`voice_controls` distinguish `input.caller_entry`:

- `0x006FFBE0`: native Foot Move argument producer4D7C76 runs first with
  ESI=actor, EDI=supplied chosen Object and EAX=null planning Cell. It produces
  mission2/NULL attack target, then enters complete ClickedMission prefix,
  original non-planning query637AA0 and enable gate6FFCBD. Stop6FFDA5 precedes
  target/event encoding. The spatial choice is explicitly supplied.
- `0x00708FC0`: direct VoiceMove, documenting receiver ordering separately
  from admission through the order caller.

`output` contains full `main_before_hex`/`main_after_hex`, observed raw
`main_draws`, original `main_next_four`, Scenario/MapGen unchanged flags,
pending voice before/after, actual queue requests, VoiceMove call count,
native-produced Move arguments and boundary trace. Fixture sound IDs101/202/303
are supplied, not stock catalog indices. A nonempty list spends one Main draw
even for singleton or-1, before QueueVoice708D90's enabled/ID/House checks.
Disabled order caller skips the draw; direct disabled VoiceMove still draws.
House50B6F0's campaign human/player-control alternatives execute unchanged.
No sample/device playback is claimed.

`infantry_bomb_controls` append the original Infantry consumer51EB24..51EB7E
on the same VM. EDI=actor, ESI=ordinary target and EBP=canonical base action
are explicit priors; the actual Infantry/InfantryType vtables and native saved
register/local stack shape are supplied. No whole Infantry WhatAction prefix or
transport lifecycle is claimed. Original House50B6F0 and target+88 type getter
execute; no virtual result is substituted. The branch tests local control,
baseAction5, Ivan+EAE, Bombable+22E and attachedBomb+38. It consumes the canonical
Attack result without re-reading AttackFriendlies/AttackCursorOnFriendlies.

The AttackFriendlies=true/AttackCursorOnFriendlies=false positive returns53
(IvanBomb); nonbombable or already-bombed controls return54 (NoIvanBomb). Failed
House/Ivan/base-action admissions stop at51EB81 before the remaining Infantry
tail. Their `output.action` is null and `base_action_at_tail` is recorded; it is
not an invented final fallback action. No RNG, action-timer or detach entry is
observed. Production tests separately exercise the actual layered IVAN patch
and shared canonical attack admission before these cursor/click consumers.

## Retail and residual scope

`retail_inputs` records actual selected physical strings and archive/file hashes
using existing physical-layer and lexical owners. It is not a native scalar
reader execution. The sidecar cites original constructor/current-default reader
instructions for AttackFriendlies, AttackCursorOnFriendlies and LegalTarget.
The selected stack is RULESMD.INI, optional LANGRULE.INI, MPBattleMD.ini, then
XMP03T4.MAP. ART is fixed and is irrelevant to these logical gates. MTNK uses
105mm/AP and VoiceMove=GenAllVehicleMove; no selected later layer changes these
grant keys. Production-reader regressions validate their actual retained reads.

This comparison does not certify whole DetermineAction, Techno/Foot/Unit action
functions, Display input, arbitrary disguise/terrain/LegalTarget/RTTI routes,
transport, nearby-cell movement, events or audio lifecycle. Original full
Display dispatch clears voice enable after every actor/restores1 at4AE852/85F;
LeftUp restarts action-line timer25 at4ABFAE ->70D150. Those later effects are
instruction evidence and existing-owner production tests, not this bounded
window execution. No detach appears in the measured immediate route.

Historical sidecars retain their actual executable identities. If a historical
reference names3e81 while the selected file is1cdd, a strict provenance check
correctly refuses that identity change even when all payload behavior matches.
Use the shared [section-compatibility evidence](../native_oracle.md#steam-compatibility-and-evidence-identity)
to qualify a separate semantic replay; do not relabel or rebaseline old evidence.
