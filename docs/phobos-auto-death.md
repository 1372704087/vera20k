# Phobos AutoDeath

A Techno type that destroys itself when a condition holds. This page covers the
**timed** arm only: `AutoDeath.Behavior=` plus `AutoDeath.AfterDelay=`.

## Keys

| key | section | default | source |
|---|---|---|---|
| `AutoDeath.Behavior` | TechnoType | unset | `Ext/TechnoType/Body.cpp:817` |
| `AutoDeath.AfterDelay` | TechnoType | `0` | `Ext/TechnoType/Body.cpp:825` |
| `AutoDeath.OnAmmoDepletion` | TechnoType | `no` | `Ext/TechnoType/Body.cpp:820` |
| `AutoDeath.PlayerPowerState` | TechnoType | `none` | `Ext/TechnoType/Body.cpp:834` |
| `AutoDeath.PlayerMoney.Max` | TechnoType | `-1` (off) | `Ext/TechnoType/Body.cpp:835` |
| `AutoDeath.PlayerMoney.Min` | TechnoType | `-1` (off) | `Ext/TechnoType/Body.cpp:836` |
| `AutoDeath.TechnosDontExist` | TechnoType | empty | `Ext/TechnoType/Body.cpp:826` |
| `AutoDeath.TechnosDontExist.Any` | TechnoType | `no` | `Ext/TechnoType/Body.cpp:827` |
| `AutoDeath.TechnosDontExist.AllowLimboed` | TechnoType | global | `Ext/TechnoType/Body.cpp:828` |
| `AutoDeath.TechnosDontExist.Houses` | TechnoType | `owner` | `Ext/TechnoType/Body.cpp:829` |
| `AutoDeath.TechnosExist` | TechnoType | empty | `Ext/TechnoType/Body.cpp:830` |
| `AutoDeath.TechnosExist.Any` | TechnoType | `yes` | `Ext/TechnoType/Body.cpp:831` |
| `AutoDeath.TechnosExist.AllowLimboed` | TechnoType | global | `Ext/TechnoType/Body.cpp:832` |
| `AutoDeath.TechnosExist.Houses` | TechnoType | `owner` | `Ext/TechnoType/Body.cpp:833` |

The two `AllowLimboed` keys fall back to `[CombatDamage]
AutoDeath.TechnosDontExist.AllowLimboed` / `AutoDeath.TechnosExist.AllowLimboed`
(`Ext/Rules/Body.cpp:631-632`), both `Valueable<bool>` defaulting to `no` and
read once from the merged layer state.

`AutoDeath.PlayerPowerState` takes `none`, `low`/`consumer` or `full`/`normal`
(`Utilities/TemplateDef.h:1762-1781`). The two money bounds are inclusive and
`-1` leaves that end open.

`AutoDeath.Behavior` is a `Nullable<AutoDeathBehavior>` (`Utilities/Enum.h:196`):
`kill` (0), `sell` (2) or `vanish` (1), matched with `_strcmpi`
(`Utilities/TemplateDef.h:1063-1083`). Absent, blank or unrecognised leaves the
`Nullable` unset, and `TechnoExt::CheckDeathConditions`
(`Ext/Techno/Body.Update.cpp:142`) returns at its first test — so
`AutoDeath.AfterDelay` alone arms nothing.

`AutoDeath.AfterDelay` is a `Valueable<int>` default `0`; a non-positive value
never starts the countdown (`:173`).

## Behavior

The check runs from `TechnoExt::OnEarlyUpdate` (`:26`), called at
`TechnoClass_AI @ 0x6F9E50+0x5` after `ApplyMindControlRangeLimit` and before
`ApplyInterceptor`. On the first visit it starts `AutoDeathTimer` with
`AfterDelay`; once the `CDTimerClass` completes it calls `KillSelf`.

Phobos tests `HasStarted()` before `Completed()`, which matters: an unstarted
`CDTimerClass` reads as already completed. VERA keeps that order by comparing
`start_frame()` with `PAUSED_START_FRAME`.

`KillSelf`'s `Kill` arm (`:809-821`) deals the object's own Health as damage
with `[CombatDamage] C4Warhead=`, no attacker and no source house, ignoring
defenses: `ReceiveDamage(&pThis->Health, 0, C4Warhead, nullptr, true, false,
nullptr)`. VERA routes that through the shared direct-receiver owner, so the
object dies through the ordinary damage and death path.

The low-power arm is written
`(Full && !isLowPower) || (Low && isLowPower) && !isFirstFrame`. `&&` binds
tighter than `||`, so the first-frame guard applies to the `Low` arm only and a
`Full` type also dies on frame 0. That is reproduced as written.

The presence arms (`Ext/Techno/Body.Update.cpp:186-255`) scan the selected
houses for each listed type. Without `AllowLimboed` they use
`CountOwnedAndPresent` (`0x0049FAE0`); with it, the distinct limbo count is
added. `Houses=` selects the house set through `CanTargetHouse`, `owner`
short-circuiting to the object's own house. `TechnosDontExist` passes `!Any`
into the scan, so its default `Any=no` means "kill when none of the listed types
exists" and `Any=yes` means "kill unless all exist". `TechnosExist` uses `Any`
directly.

A type change through `TechnoClass::ChangeType` calls
`FootExt::UpdateTypeData` (`Ext/Foot/Body.cpp:219-221`), which stops a running
countdown when the new type has no `AfterDelay`. Without that reset a round trip
through an untimed type would return the unit with an already-completed clock
and kill it on its first timed visit. VERA reaches `ChangeType` from the Ares
`Convert.Water=`/`Convert.Land=` surface conversion, Ares `Convert.Deploy=`
(`sim/unit_simple_deploy.rs`) and the team-script conversion action
(`sim/team_script_vm/actions.rs`); all of them pass through the one owner, so
every conversion drops a stale countdown.

`GetValuesForDisplay` (`Ext/Techno/Body.Visuals.cpp:696-717`) reports the clock's
remaining time over its full duration for `DigitalDisplay` entries of type
`AutoDeath`; a type without `AutoDeath.Behavior=` draws nothing, and an unstarted
clock has a zero maximum, which the renderer skips.

## Residuals

- The remaining triggers are unported: `ShouldBeDead` and
  `AutoDeath.OnOwnerChange` with its `IgnoreRevertOnExit`,
  `HumanToComputer` and `ComputerToHuman` sub-keys. A type that authors only
  those never dies.
- `AutoDeath.Behavior=vanish` and `=sell` are unported, so their timer never
  arms; `AutoDeath.VanishAnimation` is unread.
- `AutoDeath.AllowLimboed` (the type key and the `[CombatDamage]` global at
  `Ext/Rules/Body.cpp:629-632`) and `KillSelf`'s limbo arm (`:721-766`) are
  unported, so a limboed object never dies here.
- Ares `SpawnSurvivors` before the fatal hit (`Ext/Techno/Body.Update.cpp:810-816`)
  is not ported. The hit ignores defenses, and VERA's death path then refuses its
  own survivor release and marks the transport handled, so an `AutoDeath`-killed
  `Crewed=`/driver Unit or Aircraft loses its crew where native ejects it.
  Reproducing Phobos' pre-call needs VERA's survivor owner taught the
  pre-damage context (it currently assumes the unit is already lifted), which is
  its own chain.
- The scenario-level `AutoDeathObjects` collection
  (`Ext/Scenario/Body.cpp:101-131`) tracks limboed objects for the limbo arm and
  is not ported with it.
- Native's early `return` from `OnEarlyUpdate` only skips the unported
  `ApplyInterceptor`; VERA continues the visit either way.
