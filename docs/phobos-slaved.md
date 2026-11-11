# Slaves' house on master-killed (`Slaved.OwnerWhenMasterKilled`)

Who a slave miner's infantry belongs to once its master dies.

## Keys

| Key | Type | Default |
|---|---|---|
| `Slaved.OwnerWhenMasterKilled=` | `Valueable<SlaveChangeOwnerType>` | `Killer` |

Read: `InfantryTypeExt::LoadFromINI` (`Ext/InfantryType/Body.cpp:18`),
declaration `Ext/InfantryType/Body.h:14`, constructor `Body.h:39`
(`SlaveChangeOwnerType::Killer`). VERA reads it in
`src/rules/phobos_slaved.rs`, owner `ObjectType::phobos_slaved`. Tests:
`src/rules/phobos_slaved_tests.rs`.

`Utilities/Enum.h:178-184`:

```cpp
enum class SlaveChangeOwnerType { Killer = 0, Master = 1, Suicide = 2, Neutral = 4 };
```

Note `Neutral = 4`, not `3` — those are the exact `switch` values
`SlaveManagerClass_Killed_DecideOwner` compares against.

## Behaviour (not yet consumed)

`SlaveManagerClass_Killed_DecideOwner`
(`Ext/Techno/Hooks.Misc.cpp:27-72`, hooks `0x6B0B9C` and `0x6B0BA4`) runs once
per slave, before the vanilla owner decision:

| Key | killer present | killer == 0 (master sold / self-destroyed) |
|---|---|---|
| `Suicide` | `KillTheSlave` (`0x6B0BDF`) | `KillTheSlave` |
| `Master` | `R->EAX(pSlave->Owner)` → `ChangeSlaveOwner` (`0x6B0BB4`) | same |
| `Neutral` | `HouseClass::FindNeutral()` → `ChangeSlaveOwner`; no neutral → falls through to `Killer` | same |
| `Killer` (default) | `return 0`, i.e. the vanilla path gives the slave to the killer | neutral house, else `KillTheSlave` |

The `killer == 0` arm exists because `0x6B0BA4` is the vanilla
`test eax,eax` on the killer: Phobos bypasses it and branches explicitly so
the "sold master" case lands on the neutral house rather than crashing
through a null killer.

A slave in limbo (inside its master) still dies with the master regardless of
this key; the hook only decides the outside ones.

## Residual

**The key is read; the slave-freeing hook is not wired.** VERA owns the
mechanism in `sim::slave_manager::SlaveManager::free_slaves_retaining_storage`
(`src/sim/slave_manager.rs`, the `0x006B0AE0` `FreeSlaves` port). Its
per-slave loop currently has the stock three-way decision — limboed dies
inside, an outside slave goes to `killer_house.or(house).or(civilian)`, and
one with no house at all dies unfreed. Inserting the policy means reading
`entity_object_type(slave, rules).phobos_slaved.owner_when_master_killed`
(slave **type**, not master) at the top of that arm and mapping the four
cases onto `slave_dies_inside` / `change_owner_with_rules` / the neutral
house, including the `killer == 0` origin split that `0x6B0BA4` carries.

Until then `Slaved.OwnerWhenMasterKilled=` is inert and every slave follows
the stock killer-else-house-else-civilian rule.

## Tests

- `src/rules/phobos_slaved_tests.rs` — the four names and their exact native
  discriminants (`Neutral` is `4`), the case-insensitive parse, and that an
  unknown or blank read keeps `Killer` because a `Valueable` has no reset
  token.
