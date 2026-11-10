# Optional Phobos compatibility

VERA20K can build in partial, Rust-native support for some Phobos extensions. It does not load the
Phobos DLL and is not general mod support.

Default builds use the retail readers and dispatch; an INI or client setting cannot enable omitted
extensions. Build the supported extensions explicitly:

```sh
cargo run --release --bin vera20k --features phobos
```

The capability is independent of Ares and NPatch: a Phobos build needs neither feature, and all
three combine with `--features ares,phobos,npatch`. Every implemented Phobos chain must keep working
when the other capabilities are compiled in — the compatibility rule and the pattern for two
extensions touching one native site are in
[NPatch placeholder](npatch.md#cross-extension-compatibility).

Phobos builds identify themselves in the window title, the startup log, and the build description
drawn at the tactical view's top right during a match. That description is Phobos's own text: the
product name, the version and the build-type warning derived exactly as `Phobos.version.h` derives
them, so a Phobos build prints

```
Phobos v0.5.0.0-alpha1. Please test the build before shipping.
```

in red on black, ten pixels in from the view's right edge — the same string, site and colors as
Phobos's `GScreenClass::DrawText` hook. Saves retain the build capability and cannot load in the
other mode, even with identical retail INIs.

Supported extensions:

- [Custom radiation types](phobos-custom-radiation.md)
- [Shields](phobos-shields.md) (damage absorption, shield bar, shield tint, break, respawn and self-healing)
- [Custom ore spawners](phobos-ore-spawners.md)
- [Custom gather animations](phobos-harvester-gather-animations.md)
- [Ore-stage animation hiding](phobos-hide-if-no-ore-threshold.md)
- [Superweapon techno gates](phobos-superweapon-technos.md)
- [Superweapon chaining and link groups](phobos-sw-chaining.md) (`SW.Next*` chained launches with weights/rolls and the two ignore flags, and `SW.Link*` grant/ready/reset groups)
- [Digital display](phobos-digital-display.md) (numeric health/ammo/cargo/timer values near the health bar)
- [Health-bar switches](health-bars.md#phobos-type-switches) (`HealthBar.Hide=`, `.HidePips=`, `.Permanent=` and `.Permanent.PipScale=`)
- [Shared transport ammo](phobos-shared-ammo.md) (`Ammo.Shared=` and `Ammo.Shared.Group=` feed a reloading open-topped transport's rounds to its passengers)
- [Credits indicator step limit](phobos-credits-indicator.md) (`CreditsIndicator.MaxStep=` and `.Smooth=` shape the sidebar balance roll)
- [Passenger deletion](phobos-passenger-deletion.md) (`PassengerDeletion.*` counts down and deletes a transport's passenger, optionally paying Soylent)
- [Warhead trigger conditions](phobos-warhead-triggers.md) (`AffectsAbovePercent`, `AffectsBelowPercent`, `AffectsVeterancy` and `AffectsInvokerOnly.*` on a detonation's targets)
- [Overload tables](phobos-overload.md) (`Overload.Count`, `.Damage`, `.Frames`, `.DeathSound`, `.ParticleSys` and `.ParticleSysCount` for mind-control managers)
- [Grinding gates](phobos-grinding.md) (`Grinding.AllowAllies`, `.AllowOwner`, `.AllowTypes`, `.DisallowTypes`, `.Sound`, `.PlayDieSound`, `.Weapon` and `.Weapon.RequiredCredits`)
- [Harvester area guard](phobos-harvester-guard.md) (`Harvester.CanGuardArea` and `.RequireTarget`: a human harvester holds Area Guard, optionally only with a threat in range)
- [Kill weapons](phobos-kill-weapon.md) (`KillWeapon`, `.OnFirer`, their `AffectsHouse`/`AffectsTarget` gates, and `SuppressKillWeapons`)
- [Ranking overrides](phobos-ranking.md) (campaign `Ranking.ParTime*` and title/message keys in the scenario's MISSIONMD section)
- [Gattling rate-down](phobos-rate-down.md) (`RateDown.Delay`, `.Reset`, `.Cover.Value` and `.Cover.AmmoBelow`)
- [Repair-service overrides](phobos-units-repair.md) (`Units.RepairRate`, `.RepairStep`, `.RepairPercent` and `.UseRepairCost` on hospitals, depots and `UnitReload` buildings)
- [Placement adjacency](phobos-adjacent.md) (`Adjacent.Allowed`, `.Disallowed`, `.Disallowed.Prohibit` and `.ProhibitDistance`)
- [Drain money display](phobos-drain-money-display.md) (`DrainMoneyDisplay.*` flying credit labels for a DrainWeapon link)
- [TransactMoney warhead keys](phobos-transact-money.md) (`TransactMoney`, its `Display.*` label and the `0x6FC339` insufficient-funds fire gate)
- [Jumpjet tilt](phobos-jumpjet-tilt.md) (`JumpjetTilt` and the four `JumpjetTilt.*` factors pitch and roll an airborne jumpjet body)
- [Forced weapon selection](phobos-force-weapon.md) (`ForceWeapon.*` / `ForceAAWeapon.*` force the SelectWeapon slot by target state, range and class)
- [Deploy ammo](phobos-ammo-deploy.md) (`Ammo.AddOnDeploy`, `.DeployUnlock*` and `.AutoDeploy*` shape a vehicle deployer's ammo and deploy admission)
- [Penetrates transport](phobos-penetrates-transport.md) (`PenetratesTransport.*` lets a warhead damage a transport's passengers)
- [Warp-in weapon](phobos-warp-in-weapon.md) (`WarpInWeapon`, `WarpInMinRangeWeapon` and `.UseDistanceAsDamage` detonate at a teleport's arrival)
- [Missile cruise and takeoff](phobos-missile.md) (`Missile.Cruise`, `.TakeOffSeparation` and `.TakeOffAnim` on a rocket-flying AircraftType)
- [Drop pods](phobos-drop-pod.md) (`DropPod.*` twelve per-type keys: descent angle/speed/height, trailer cadence and attachment, air/landing weapon, puff and ground anims)
- [Powers-up host list](phobos-powers-up.md) (`PowersUp.Buildings` lets one upgrade serve several hosts and `PowersUp.Owner` filters the installer)
- [Placement preview](phobos-placement-preview.md) (building artwork and grid translucency while placing)
- [Sidebar positions](phobos-sidebar-gdi.md) (per-side GDI/NOD sidebar coordinates)
- [Infantry auto deploy](phobos-infantry-auto-deploy.md) (`InfantryAutoDeploy` for AI GI-like infantry)
- [Fly wobble switch](phobos-fly-no-wobbles.md) (`FlyNoWobbles` for `Locomotor=Fly`, per type and globally)
- [Centred pause-menu background](phobos-center-pause-menu.md) (`uimd.ini [Sidebar] CenterPauseMenuBackground`)
- [Building production queue](phobos-building-queue.md) (queue buildings and start the next after placement)
- [Skirmish AI dehardcoding](phobos-ai-dehardcode.md) (last-legs sale/hunt switches and MCV deploy regroup)
- [Parallel AI queue limits](phobos-parallel-ai-queues.md) (per-category and per-type AI clone forbids)
- [Unlimited skirmish colours](phobos-skirmish-unlimited-colors.md) (spawner colour indices address any `[Colors]` entry)
- [Superweapon sidebar](phobos-superweapon-sidebar.md) (left-edge superweapon cameos, toggle and positional hotkeys)
- [Prone movement speed](phobos-prone-speed.md) (`ProneSpeed` for crawling and non-crawling prone infantry)
- [Type select for buildings](phobos-building-type-selectable.md) (`BuildingTypeSelectable` for every building type)
- [Power-delta indicator](phobos-power-delta-indicator.md) (`PowerDelta.*` net-power text beside the credits counter)
- [Burst with no delay](phobos-burst-no-delay.md) (`Burst.NoDelay=` empties a burst in one frame)
- [Harvester counter](phobos-harvester-counter.md) (`HarvesterCounter.*` working/total harvesters, `Harvester.Counted=`)
- [Weeds counter](phobos-weeds-counter.md) (`WeedsCounter.*` the House's stored weed total beside the credits counter)
- [AutoDeath](phobos-auto-death.md) (`AutoDeath.Behavior=kill|vanish|sell`, the condition arms and timers, `AutoDeath.VanishAnimation`)
- [Shrapnel admission](phobos-shrapnel.md) (`Shrapnel.AffectsGround=` / `Shrapnel.AffectsBuildings=` gate whether a bullet spawns its shrapnel)
- [Cluster scatter distance](phobos-cluster-scatter.md) (`ClusterScatter.Min=` / `ClusterScatter.Max=` set each cluster successor's distance from the impact)
- [Timer blink colour](phobos-timer-blink-color.md) (`[AudioVisual] TimerBlinkColorScheme=` names a `[Colors]` entry)
- [Missing cameo](phobos-missing-cameo.md) (`[AudioVisual] MissingCameo=` for types with no cameo art)
- [Power plant enhancers](phobos-power-plant-enhancer.md) (`PowerPlantEnhancer.*` output boosts, `PowerPlant.DamageFactor` and placement range rings)
- [Warhead self-damage](phobos-allow-damage-on-self.md) (`AllowDamageOnSelf` per warhead and globally, and the airborne `DamageSelf` fix)
- [Projectile gravity](phobos-projectile-gravity.md) (`[Projectile] Gravity=` per projectile type)
- [Warhead decloaking](phobos-decloak-damaged-targets.md) (`DecloakDamagedTargets` per warhead and globally)
- [Airstrike flare](phobos-airstrike-flare.md) (`LaserTargetColor` and `AirstrikeLineColor` per TechnoType, and the line depth fix)
- [AttachEffect](phobos-attach-effect.md) (`[AttachEffectTypes]` catalog, warhead and TechnoType attach lists, stat multipliers, tint, discard conditions, expire/revenge/reflect weapons and the weapon-side gates)
- [Critical hits](phobos-critical-hit.md) (per-warhead `Crit.*` chance, extra damage, second warhead and animations, `ImmuneToCrit`, rank abilities)
- [Selection flash](phobos-selection-flash.md) (`[AudioVisual] SelectionFlashDuration` behind `[Phobos] ShowFlashOnSelecting`)
- [Semantic locomotor aliases](phobos-locomotor-aliases.md) (`Locomotor=Drive` and friends beside the braced CLSIDs)
- [Airstrike target eligibility](phobos-airstrike-targets.md) (`AllowAirstrike` per TechnoType, `AirstrikeTargets` per warhead, and object targets for non-buildings)
- [Deployable-building alternate cameo](phobos-deployable-cameo.md) (Ares' `UndeploysInto` cameo arm, fixed)
- [Animation bracket draw offsets](phobos-anim-offset.md) (`XDrawOffset.*` and `YDrawOffset.*` selection-bracket keys for owned animations)
- [Building turret animations](phobos-turret-anim.md) (`TurretAnim.IdleFrames`, `.LowPowerIdleFrames`, `.FiringFrames`, `.LowPowerFiringFrames`, `.IdleRate` and `.FiringRate` frame selection for SHP slot-9 turrets)
- [Build limit groups](phobos-build-limit-group.md) (`BuildLimitGroup.*` nine keys, `[General]` fallbacks, ExtraLimit quota and the queue-match gate)
- [Delayed fire](phobos-delayed-fire.md) (`DelayedFire.*` wind-up delay, timer, pause, wind-up animation and `Burst.NoDelay` compatibility; 11 keys)
- [Aircraft spawner customizations](phobos-spawner.md) (`Spawner.*` launch spacing, immediate attack, turret facing, pursue range and the recycle point; 9 keys + 4 `[General]` fallbacks)
- [Detonate on all map objects](phobos-detonate-all-map.md) (`DetonateOnAllMapObjects.*` whole-map detonation, target/house/type filters and the Full/damage-area arms)
- [Keep target on move](phobos-keep-target-on-move.md) (`KeepTargetOnMove.*` move-order target retention, the Unit AI range re-check and the temporary ExtraDistance)
- [OpenTopped per-type properties](phobos-open-topped.md) (eleven `OpenTopped.*` TechnoType keys, eight `[General]` globals, two fallback chains and the firing/range/cloak/temporal hooks)
- [Promotion animations and promoted spawns](phobos-promote.md) (`Promote.*` rank-crossing animation lists with the `[AudioVisual]` fallbacks, and the spawner child rank lift; 3 keys + 3 globals)
- [Attack move](phobos-attack-move.md) (`AttackMove.IgnoreWeaponCheck`, `.Aggressive` and `.UpdateTarget`: the weapon check on the order, unarmed buildings as targets, and the target swap)
- [Attack-move followers](phobos-attack-move-follow.md) (`AttackMove.Follow*`: escort the nearest friendly attack-mover, air inclusion, the mind-control-full condition and the per-leader follower count)
- [Trajectory base and Straight](phobos-trajectory-straight.md) (`Trajectory` selection, `Trajectory.Speed` and the 30 `Trajectory.Straight.*` keys: speed flight, snap/detonation distances, pass/proximity hits, obstacles and attenuation; 32 keys)
- [Spy-effect superweapon launches](phobos-spy-effect.md) (`SpyEffect.*` victim/infiltrator launches on a spy infiltration, with the recharge kept; 3 keys)
- [Rearm and reload holds](phobos-no-rearm.md) (`NoRearm.*` / `NoReload.*` freeze the rearm and ammo-reload timers while EMP'd or Temporal-held; 4 keys + 4 `[General]` globals)
- [Passenger owner sync](phobos-passengers-sync-owner.md) (`Passengers.SyncOwner` re-houses cargo on the transport's house change, `Passengers.SyncOwner.RevertOnExit` restores board-time houses on exit and on the carrier's death, and the threat-eval owner substitution skip; 2 keys + 2 `[General]` globals)
- [Driver-killed passengers](phobos-driver-killed.md) (`DriverKilled.KeptPassengers` and `.KillPassengers` choose keep-aboard, kill or the stock ejection for a driverless vehicle's passengers; 2 keys + 2 globals)
- [No manual move](phobos-no-manual-move.md) (`NoManualMove`: the shared player-control answer refuses the type while its AI keeps moving it)
- [Health-band conversion](phobos-convert-health.md) (`Convert.Health` and its `AbovePercent`/`BelowPercent` band: a Foot converts when its health ratio enters the range; 3 keys)
- [Deploy/Undeploy conversion](phobos-convert-undeploy.md) (`Convert.Deploy` / `Convert.Undeploy`: an infantryman converts once per completed Deploy or Undeploy sequence, with the DoingAI latch reset; 2 keys)
- [Owner-change conversion](phobos-convert-owner-change.md) (`Convert.HumanToComputer` / `Convert.ComputerToHuman`: a Foot converts on a human/computer owner change and the computer-to-human arm repairs its mission; 2 keys)
- [Mind-control reset on conversion](phobos-convert-reset-mind-control.md) (`Convert.ResetMindControl`: the capture-manager limits are rebuilt from the new type's weapons, trimming or deleting the manager; 1 key + 1 `[General]` global)
- [Convert pairs](phobos-type-convert-group.md) (`Convert{i}.From`/`.To`/`.AffectsHouse` on a warhead or superweapon: the per-target and whole-array Foot conversions)
- [Explodes keys](phobos-explodes.md) (`Explodes.KillPassengers` / `Explodes.DuringBuildup`: the death arm's passenger kill and a building's death weapon under Construction/Selling; 2 keys + 2 `[General]` globals)
- [Extra warheads](phobos-extra-warheads.md) (`ExtraWarheads` and its damage, chance, roll, weight and full-detonation lists: warheads detonated after the weapon's own at the same impact; 6 keys)
- [Cloned infantry strength](phobos-initial-cloning.md) (`InitialStrength.Cloning` scales a cloning vat's exiting clone to a percentage or `min,max` range of its own Strength; 1 key)
- [Warhead shield modifiers](phobos-shield-warhead.md) (`Shield.*` on a warhead: attach/remove/replace, break, respawn/self-healing modifiers, the absorb/pass split and penetration; 39 keys)

- [Return warhead](phobos-return-warhead.md) (`ReturnWarhead` and its damage, chance, full-detonation, target and house filters: the per-target strike back at the firer; 5 keys)

- [Tiberium eater](phobos-tiberium-eater.md) (`TiberiumEater.*`: FLH-cell tiberium eating with cash, delay, per-tiberium anims and EMP/limbo holds; 11 keys)
- [Ammo auto-convert](phobos-convert-ammo.md) (`Ammo.AutoConvertType`/`.MinimumAmount`/`.MaximumAmount`: a Foot converts when its ammo enters the band; 3 keys)
- [LaunchSW](phobos-launch-sw.md) (`LaunchSW` and its real-launch, inhibitor/designator and display-money options: a warhead launches superweapons at its impact; 7 keys)
- [NotHuman death sequences](phobos-not-human.md) (`NotHuman.DeathSequence` / `NotHuman.RandomDeathSequence`: a NotHuman infantryman's Die1..Die5 selection and the paradrop fix; 2 keys + 1 `[General]` global)
- [KeepRange](phobos-keep-range.md) (`KeepRange` and its AllowAI/AllowPlayer/EarlyStopFrame gates: an Infantry/Unit holds a distance while rearming; 4 keys + 3 `[General]` globals)
- [Parasite keys](phobos-parasite.md) (`Parasite.ParticleSystem`/`.DisableParticleSystem`/`.CullingTarget`/`.GrappleAnim`/`.AllowWaterExit`, `RemoveParasite`/`.Allow`/`.Disallow`: the bite, grapple and removal customizations; 8 keys + 3 globals)
- [Superweapon Detonate](phobos-sw-detonate.md) (`Detonate.Warhead`/`.Weapon`/`.Damage`/`.Warhead.Full`/`.AtFirer`: a superweapon detonates a weapon or warhead at its target; 5 keys)
- [Cloak keys](phobos-cloak.md) (`Cloak.KickOutParasite`: a cloaking host ejects its parasite; 1 key + 1 `[General]` global)

## Original project and authors

VERA20K implements the behavior in Rust. It does not load `Phobos.dll`, import
Phobos C++ source or redistribute Phobos binaries; the native identities cited
by these pages are the Phobos sources named in each page.

Phobos is developed by the Phobos Developers. The features ported here
originate with:

- Custom radiation types - **Otamaa**, who ported and fixed the **Ares
  developers**' unfinished RadTypes code.
- Shields - **Uranusian**, **secsome**, **Belonit**; shield warhead
  modifications **Starkku**; absorption/passthrough **Morton**; respawn
  animation and weapon **Ollerus**.
- Custom ore spawners - **Kerbiter**.
- Custom gather animations - **secsome**, **Uranusian**.
- Ore-stage animation hiding (`HideIfNoOre` threshold) - **Otamaa**.
- Superweapon techno gates (`SW.AuxTechnos`, `SW.NegTechnos`,
  `SW.TechLevel`) - **NetsuNegi**; their building-upgrade counts -
  **NetsuNegi**, **Ollerus**.
- Digital display - **ststl**, **FlyStar**, **NaotoYuuki**, **Saigyouji**,
  **JunJacobYoung**, **CrimRecya**.
- Placement preview - **Otamaa**, **Belonit**.
- Sidebar GDI/NOD positions - **Belonit**.
- Infantry auto deploy - **TaranDahl**.
- Fly wobble switch (`FlyNoWobbles`) - **Noble_Fish**.
- Centred pause-menu background - **Starkku**.
- Unhardcoded timer blinking colour scheme - **Starkku**.
- Semantic locomotor aliases (`Locomotor=Drive` and the like) - **Belonit**.
- Missing cameo - **Uranusian**.
- Power plant enhancers - **secsome**, **NetsuNegi** (range), **Ollerus**
  (`MaxCount`), **Trsdy** (EMP).
- Power plant damage factor - **Otamaa**, **Ollerus**.
- Building production queue - **CrimRecya**.
- INI `$Include` / `$Inherits` - **Morton**.
- Skirmish AI dehardcoding (last-legs sale/hunt, MCV deploy regroup) -
  **TaranDahl**.
- Parallel AI queue forbids - **Trsdy**, **NetsuNegi** (by category),
  **Starkku** (by TechnoType).
- Unlimited skirmish colours - **Morton**.
- Superweapon sidebar - **NetsuNegi**, **CrimRecya**.
- Prone speed customization - **TaranDahl**.
- Type select for buildings - **TaranDahl** (code), **Ollerus** (doc).
- Flashing Technos on selecting - **Fryone**.
- Warhead self-damaging toggle (`AllowDamageOnSelf`) - **Starkku**; its and
  `DamageSelf`'s airborne fix - **NetsuNegi**.
- Custom gravity for projectiles - **secsome**.
- Warhead decloaking toggle (`DecloakDamagedTargets`) - **Starkku**.
- Airstrike flare visual customizations and fixes - **Starkku**.
- AttachEffect - **Starkku**, with target filtering **Flactine** and
  attach-event 606 **FS-21**.
- Critical hits - **Starkku** (logic), **AutoGavy** (warhead crit), and
  **Trsdy** (shield `ImmuneToCrit`).

## VERA20K porting credits

The Rust ports are individual VERA20K contributions; GitHub usernames name
the merged pull requests that carried each chain:

- **1372704087** — every Phobos chain listed above: shields, custom
  radiation types, the three ore chains, superweapon techno gates, digital
  display, placement preview, sidebar GDI/NOD positions, infantry auto
  deploy, `FlyNoWobbles`, the centred pause-menu background, building
  production queue, skirmish AI dehardcoding, parallel AI queue forbids,
  unlimited skirmish colours, the superweapon sidebar, prone speed, type
  select for buildings, timer blink colour, missing cameo, power plant
  enhancers, semantic locomotor aliases, selection flash,
  `AllowDamageOnSelf`, `DecloakDamagedTargets`, projectile gravity, the
  airstrike flare colours, AttachEffect with the critical-hit chain, and
  the `$Include`/`$Inherits` native hooks.
- **Krisztiaan** — the shield damage/recovery hook-ordering fix.
- **CCDawn** — the Phobos build description drawn at the tactical view's
  top right.

The project's full contributor list is in
[Phobos CREDITS.md](https://github.com/Phobos-developers/Phobos/blob/develop/CREDITS.md).
VERA's include and inheritance traversal also adapts ra2-mono's `IniFile`;
see [INI directives](ini-directives.md#source-mapping).

简体中文：[扩展支持与原作者](zh-CN/credits.md)。
