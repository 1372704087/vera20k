# TeamMember.ConsideredAs

A TechnoType can stand in for other types when an AI team fills its TaskForce
slots, so an elite or converted unit still satisfies the plan.

```ini
[MTNK]                        ; TechnoType
TeamMember.ConsideredAs=HTNK  ; ValueableVector<TechnoTypeClass*>, empty default
```

Reader: `Ext/TechnoType/Body.cpp:1160` (`TeamMember_ConsideredAs`, constructor
`Body.h:820`). The native vector is a `TechnoTypeClass*` list resolved by the
rules registry; an unknown token is dropped.

## Native body

Four `TeamClass` member matches test a TaskForce entry's type against the
member's type, or a type the member lists (`Ext/Team/Hooks.cpp:61-127`):

- `TeamClass::Can_AddMember` (`0x6EA6BE`) advances the entry index while the
  entry type is neither the member's own type nor listed.
- `TeamClass::LiberateMember` (`0x6EA8E7`) stops its forward scan on the same
  test to find the departing member's slot.
- `TeamClass::RecruitMember` (`0x6EAD73`) admits a unit candidate when the
  entry type is its type or listed.
- `TeamClass::GetTaskForceMissingMemberTypes` (`0x6EF57F`) skips a member type
  already covered by a listed type.

## VERA

`ObjectType::phobos_team_member_considered_as` reads the names with the shared
`ValueableVector` reader (a successful read replaces the list, a missing key
keeps the previous layer's), keeping tokens unresolved. The team membership
owner's `member_fills_entry` accepts a member when its identity equals the
entry type, else when the entry type's name is one of the member type's listed
names (case-insensitive, the native `_strcmpi` lookup). `team_can_add`,
`team_remove_member` and the unit arm of `team_recruit` all go through it.

## Tests

`src/sim/team_script_vm/considered_as_tests.rs`:
`the_key_reads_a_type_name_list_and_an_unset_type_keeps_it_empty` (tokens keep
their authored spaces, a type without the key stays empty) and
`a_considered_type_fills_the_task_force_slot` (the listed type fills the HTNK
entry, case-insensitively; an unset or unrelated list finds no slot). The
match was mutated red and restored.

## Residuals

- `TeamClass::GetTaskForceMissingMemberTypes` (`0x6EF57F`) is not ported as a
  function; the AI production paths that would consume its missing-type list
  do not exist in VERA, so that fourth match arm is dormant here.
- A listed token that resolves to no TechnoType stays in the list and can
  never equal an entry type; native drops it at parse time. Both never match.
