# Deploy ammo

Phobos lets a deployable vehicle change its ammo on a deploy, require an ammo
band to deploy, and auto-deploy inside another band
(`Ext/UnitType/Body.cpp:45-49`; consumers `UnitExt::HandleOnDeployAmmoChange`
`Ext/Unit/Body.cpp:239-256`, `HasAmmoToDeploy` `:218-237`,
`DepletedAmmoActions` `:149-195`).

```ini
[DEPLOYER]                        ; UnitType
Ammo.AddOnDeploy=0                ; int, default 0 (no change)
Ammo.DeployUnlockMinimumAmount=-1 ; int, default -1 (unbounded)
Ammo.DeployUnlockMaximumAmount=-1
Ammo.AutoDeployMinimumAmount=-1
Ammo.AutoDeployMaximumAmount=-1
```

## Behavior

- `Ammo.AddOnDeploy` applies when a deploy completes and again when an
  undeploy completes: `ammo = clamp(ammo + add, 0, Ammo)`. A changed count
  starts the reload clock (`StartReloading`).
- `Ammo.DeployUnlock*` is the `HasAmmoToDeploy` band: with both unset every
  count admits; otherwise the player's deploy cursor/action refuses outside
  the band. It does not gate the mission-driven deploy.
- `Ammo.AutoDeploy*`: after `FootClass::AI` in `UnitClass::AI` (hook
  `0x736480`), a deploy-capable Unit queues the Unload mission while its ammo
  is inside the auto-deploy band and also admitted by the deploy-unlock band,
  and queues Guard when it leaves. A simple deployer leaving the band while
  airborne re-enters `JumpjetLocomotionClass::State::Ascending`.

## Where VERA20K owns it

`rules::phobos_ammo_deploy` reads the five keys. `sim::unit_simple_deploy`
owns the deploy/undeploy completion rewrite (`apply_deploy_ammo_change`), the
`UnitClass::AI` action (`depleted_ammo_actions`) and the player gate
(`Simulation::phobos_unit_has_ammo_to_deploy`), which
`app::input::context_order::unit_refuses_deploy` and the deploy-key filter in
`app::input::dispatch` call. Ammo writes go through `TechnoAmmo` and the
existing `shared_ammo::start_reloading` owner.

## Residuals

- `Mark(MarkType::Change)` is not modeled; VERA redraws from live state.
- The cancel arm's `pThis->InAir` is tested as `GetHeight() > 0`.
- `Convert.Deploy`'s Ammo override is read only in `ares,phobos` builds, where
  the conversion exists; `Ammo.AutoConvert*` and the rest of `Convert.*` are a
  separate chain.
