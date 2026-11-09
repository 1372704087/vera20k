# BuildLimitGroup.*

Phobos `BuildLimitGroup.*` lets a TechnoType share one build cap across a group
of types, add extra quota for owning other types, and gate the cap on the
production queue. The keys are read in `src/Ext/TechnoType/Body.cpp:1033-1041`
(fields/defaults `Body.h:265-273` and `Body.h:697-705`); there is **no**
`IsBuildLimitGroupReached` in TechnoType. The whole algorithm lives in
`src/Ext/House/Body.cpp:773-1189`.

## Keys

Nine per-TechnoType keys (all `#[cfg(feature = "phobos")]` on `ObjectType`):

| Key | Native type | Default |
| --- | --- | --- |
| `BuildLimitGroup.Types` | `ValueableVector<TechnoTypeClass*>` | empty |
| `BuildLimitGroup.Nums` | `ValueableVector<int>` | empty |
| `BuildLimitGroup.Factor` | **`Valueable<int>`** (not double) | `1` |
| `BuildLimitGroup.ContentIfAnyMatch` | **`Nullable<bool>`** | unset → `[General]` |
| `BuildLimitGroup.NotBuildableIfQueueMatch` | **`Nullable<bool>`** | unset → `[General]` |
| `BuildLimitGroup.ExtraLimit.Types` | `ValueableVector<TechnoTypeClass*>` | empty |
| `BuildLimitGroup.ExtraLimit.Nums` | `ValueableVector<int>` | empty |
| `BuildLimitGroup.ExtraLimit.MaxCount` | `ValueableVector<int>` | empty |
| `BuildLimitGroup.ExtraLimit.MaxNum` | `Valueable<int>` | `0` (`<= 0` = no cap) |

Two `[General]` `Valueable<bool>` globals are the `Nullable` fallbacks
(`src/Ext/Rules/Body.cpp:223-224`, declared `Body.h:130-131`, constructed
`false` at `Body.h:667-668`): `BuildLimitGroup.ContentIfAnyMatch` and
`BuildLimitGroup.NotBuildableIfQueueMatch`. VERA reads them in
`PhobosGlobals::from_ini`'s `[General]` block; the new fields carry
`#[serde(default)]` and enter `RuleSet`'s hash automatically through
`PhobosGlobals`' derived `Hash` (`ruleset.rs:9138` hashes the whole struct).

## Algorithm (`Ext/House/Body.cpp:773-1189`)

- `CountOwnedIncludeDeploy` (`:774-780`) = `CountOwnedNow(pItem)` +
  `DeploysInto` + `UndeploysInto`.
- `BuildLimitGroupUpgradeCheck` (`:802-1013`) and `ReachedBuildLimit`
  (`:1036-1188`) duplicate the cap logic; `QueuedNum` (`:1015-1034`) is
  `CountTotal(pType)` minus the factory's held object when it is that exact
  type; `RemoveProduction` (`:782-800`) drops queued copies on a match.

1. Short-circuit: empty `Types` (`:806`) and, more strictly, empty `Types` or
   `Nums` (`:1040-1041`) mean no cap. VERA uses the union, keeping the
   all-empty constructor default inert (stock behaviour unchanged).
2. ExtraLimit quota (`:809-843` / `:1049-1077`): `limits` is a local copy of
   `Nums`. Only when `ExtraLimit.Types` and `.Nums` are both non-empty, each
   index `i` pairs `Types[i]/Nums[i]/MaxCount[i]`; `count` is the type's owned
   units (`GetUpgradesAmount` for PowersUp upgrades, else `CountOwnedNow`),
   clamped by `MaxCount[i] > 0`; every `limit` gains `count * Nums[i]` (only
   when `Nums[i] > 0`) and is then capped by `MaxNum > 0`. Out-of-range and
   `<= 0` tokens are silently ignored. `MaxCount` caps the counted units;
   `MaxNum` caps each resulting limit.
3. Group contributions (`:845-861`): per group type,
   `contribution = CountOwnedIncludeDeploy * that type's OWN Factor` (not
   pItem's). Queued copies contribute `QueuedNum * that type's Factor`
   (`:1093`, `:1133`).
4. Switches: `factor` is **pItem's own** Factor (`:863`); `notBuildable` /
   `contentIfAny` are `.Get(global)` (`:864-865`, `:1080`, `:1125`).
   `ContentIfAnyMatch=yes` → OR (any member reaching blocks), `no` → AND.
   `NotBuildableIfQueueMatch=yes` → a queue-only reach is blocked too.
5. `Nums.len() == 1` (`:867-890`): sum all contributions;
   `sum >= limits[0] + 1 - factor` (equiv. `num >= 1 - factor`) → unbuildable.
6. Otherwise (`:892-947`): pair by index with `size = min(limits, types)`;
   pItem's own slot uses threshold `1 - factor`, other slots `0`;
   `ContentIfAnyMatch` makes it OR, else AND.
7. Final gate (`:1112`, `:1180`): `reached && (realReached || notBuildable)` —
   queue-only reaches are blocked only when `NotBuildableIfQueueMatch` is set.
   Queue matches are exact TechnoType pointers, not group/category.

## Native evidence (gamemd / Phobos hooks)

| Address | Function | Source |
| --- | --- | --- |
| `0x4F8361` | `HouseClass_CanBuild_UpgradesInteraction` | `Ext/BuildingType/Hooks.Upgrade.cpp:93-111` (calls UpgradeCheck at `:107`) |
| `0x4F7877` | `HouseClass_CanBuild_UpgradesInteraction_WithoutAres` | `Hooks.Upgrade.cpp:113` (self-disabling patch) |
| `0x50B669` | `HouseClass_ShouldDisableCameo_GreyCameo` | `Ext/House/Hooks.cpp:493-503` (sole `ReachedBuildLimit` caller) |
| `0x452678` / `0x4408EB` | `BuildingClass_CanUpgrade` / `Unlimbo` UpgradeBuildings | `Hooks.Upgrade.cpp:31,48` |
| `0x004F7870` | `HouseClass::CanBuild` | VERA `can_build` owner |
| `0x0050B370` | `HouseClass::CheckBuildLimit` | VERA `check_build_limit` owner |
| `0x004CA670` | `FactoryClass::CountTotal` | `FactoryRegistry::count_total` (`factory.rs:1011`) |
| `0x005F7900` | `ObjectTypeClass::FindFactory` | `can_build::find_factory` |

`Building/Body.cpp` and `Techno/Body.cpp` contain no `BuildLimitGroup` match.
VERA has no Ares injection point, so it does **not** reproduce Phobos's
"only PowersUp buildings go through `0x4F8361`" split; one shared gate is
added at the top of `build_limit()` (before both the `ares` and non-`ares`
return blocks) and of `check_build_limit()`, reusing `HouseTracking::owned_count`,
`produced_count`, `held_in_factory()` and `sim.production.factories()`.

## Two Phobos inconsistencies — ruling

The two duplicated algorithms disagree. VERA follows
`BuildLimitGroupUpgradeCheck` and records the deviations:

1. `BuildLimitGroupUpgradeCheck:901` uses `num = contribution - limit`;
   `ReachedBuildLimit:1142` uses `num *= Factor - limits[i]`
   (i.e. `owned * (factor - limit)`), which is mathematically different and
   looks like a Phobos defect. VERA uses `contribution - limit`.
2. `ReachedBuildLimit:1102/1140` counts only `CountOwnedNow` (no deploy
   conversions), while `BuildLimitGroupUpgradeCheck:858` uses
   `CountOwnedIncludeDeploy`. VERA uses the latter.

`BuildLimitGroupUpgradeCheck` also has a second pass (`:949-1012`) with a
`contentIfAny` loop over all items at a `1 - factor` threshold and an
`includeQueued` / `FindByOwnerAndProduct` release. VERA keeps the first pass's
paired-threshold semantics (`1 - factor` for pItem, `0` for others) and the
`ReachedBuildLimit` queue/notBuildable gate; the second pass is not ported.

## Reader: index alignment (`read_ares_int_list` trap)

`IniSection::read_ares_int_list` (`ini_value.rs:627`) uses `filter_map`, so an
unparseable `Nums` token is dropped and the `Types`/`Nums`/`MaxCount` pairing
shifts. VERA reads integer lists with `read_list` (`ini_value.rs:1043`) and
`parse_ares_int` (`ini_value.rs:1957`, Ares/Phobos `Parser<int>`), keeping a
`0` placeholder for a rejected token — no `str::parse`, one slot per token.
Type-name lists use `read_valueable_type_vector` and keep unresolved names as
`Vec<String>` for the consumer to resolve (the `phobos_grinding` convention).

## Residuals (trigger · impact · frequency · risk)

- **No retail / visual / GPU validation**: this checkout has no retail `ini/`,
  `config.toml` or `RA2_DIR`, so retail-INI tests skip. Trigger: any run
  against real RA2 data. Impact: cap behavior only verified on synthetic
  fixtures. Frequency: always in this workspace. Risk: medium — real INI
  values may exercise untested `Types`/`Nums` shapes.
- **PowersUp upgrades `GetUpgradesAmount`** (`UpgradeCheck:824-825`, `:855-856`,
  `ReachedBuildLimit:1058-1060`, `:1137-1138`): VERA has no
  `PowersUp_Buildings` equivalent and uses `owned_count` for every group member.
  Trigger: a group member is an upgrade building. Impact: its contribution may
  differ. Frequency: rare. Risk: low.
- **`RemoveProduction` queue deletion** (`:782-800`): VERA's gate is read-only;
  it does not remove queued copies on a match (the caller owns UI/queue state).
  Trigger: a group cap is reached with a matching queue. Impact: cameo greys
  but the queue is not purged. Frequency: rare. Risk: low.
- **`UpgradeCheck` second pass** (`:949-1012`, `FindByOwnerAndProduct` release,
  contentIfAny all-item `1 - factor` loop): not ported; VERA uses the first
  pass's paired thresholds plus the `ReachedBuildLimit` queue gate. Trigger:
  `ContentIfAnyMatch=yes` with a queue-only reach. Impact: a borderline OR
  case may differ. Frequency: rare. Risk: low.
- **Held-object deduction scope**: `QueuedNum` uses the primary factory of the
  type; VERA's `held_in_factory()` matches any factory of the owner, so a held
  object in a different strip is deduced from a queue count it was never part
  of (the count is clamped at 0). Trigger: a type held in a cross-strip
  factory. Impact: that type's queued contribution may be 0. Frequency: never
  on synthetic fixtures. Risk: low.
- **Paired (`Nums.len() > 1`) branch**: implemented from `:892-947` but not
  covered by a dedicated production-code regression test (the three required
  tests exercise the single-`Nums` sum branch). Trigger: authored INIs with
  multiple `Nums`. Impact: unverified OR/AND pairing. Frequency: rare. Risk:
  low.
