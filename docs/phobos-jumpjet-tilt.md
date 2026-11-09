# Jumpjet tilt

Phobos can pitch and roll an airborne jumpjet body from its own motion,
without changing how it flies (`Ext/UnitType/Body.cpp:63-67`, consumer
`Ext/TechnoType/Hooks.MatrixOp.cpp:391-523`, vtable `0x7ECD8C`).

```ini
[AudioVisual]                       ; global defaults
JumpjetTilt=false                   ; bool, default false
JumpjetTilt.ForwardAccelFactor=1.0  ; double, default 1.0
JumpjetTilt.ForwardSpeedFactor=1.0  ; double, default 1.0
JumpjetTilt.SidewaysRotationFactor=1.0 ; double, default 1.0
JumpjetTilt.SidewaysSpeedFactor=1.0 ; double, default 1.0

[TILTUNIT]                          ; UnitType, all Nullable
JumpjetTilt=                        ; unset inherits [AudioVisual]
JumpjetTilt.ForwardAccelFactor=
JumpjetTilt.ForwardSpeedFactor=
JumpjetTilt.SidewaysRotationFactor=
JumpjetTilt.SidewaysSpeedFactor=
```

## Behavior

When the stock `TiltCrashJumpjet=` arm has no rocking angle, a live Unit
whose type enables the tilt and whose jumpjet locomotor is airborne, moving
(`CurrentSpeed > 0`) and not held by a Magnetron draws with:

- a forwards pitch from `(accel * ForwardAccelFactor + speed *
  ForwardSpeedFactor) * (pi/2/4)/32`, clamped to `+/- pi/2`, on the
  `RotateY` axis;
- a sideways roll from `speed * SidewaysSpeedFactor * turn_difference *
  SidewaysRotationFactor * (pi/2/4)/(32768*32)`, clamped the same way, on
  `RotateX`, only while the locomotor facing is turning.

Each angle is quantized through `DirStruct` to 128 facings and a zero axis
is skipped, so the draw is yaw then the surviving rotations. This is a
presentation-only pose: no simulation field changes, and the crashed
`TiltCrashJumpjet=` arm keeps precedence.

## Where VERA20K owns it

`rules::phobos_jumpjet_tilt` reads the five `[AudioVisual]` globals and the
five per-type `Nullable` overrides (the type field falls back to the global
in `resolve`). `ObjectType` carries the resolved inputs;
`app::presentation::instances::units` applies the pose in
`body_pose_with_angles`, and `render::vxl_raster::BodyTilt::JumpjetTilt`
renders yaw/X/Y without the crash arm's translation. Stock builds keep the
plain matrix.

## Residuals

- The matching shadow arm (`UnitClass::DrawVoxelShadow` hook `0x73C47A`,
  `Ext/TechnoType/Hooks.MatrixOp.cpp:729-773`) is not ported. Trigger: a
  drawn Unit shadow while the tilt is active. Effect: the shadow stays
  untitled while the body pitches and rolls. Frequency: every frame such a
  unit casts a shadow in view. Risk: visual mismatch only; simulation and
  the body draw are unaffected.
