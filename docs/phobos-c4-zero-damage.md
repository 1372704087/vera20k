# Zero-damage C4 building minimum

`CanC4.AllowZeroDamage=` on BuildingType removes the minimum one point of
damage a `CanC4=no` building takes when a non-zero raw hit is scaled below 1.

```ini
[SOMEBUILDING]                      ; BuildingType
CanC4.AllowZeroDamage=false         ; Valueable<bool>, constructor false
```

Reader: `Ext/BuildingType/Body.cpp:194`; it exists only on the BuildingType
extension, so VERA reads it for buildings only.

## Native body

`ObjectClass::ReceiveDamage @ 0x005F5390` rejects a zero raw packet at
`0x005F53B4..0x005F53B7` before the Verses kernel. The kernel writes its scaled
result through the same packet pointer at `0x005F5414`, and the building
minimum at `0x5F5416` reads it again: `cmp eax,1; jge keep; mov eax,1`, so a
non-zero raw hit that `Verses` lowers to 0 becomes 1 on a `CanC4=no` building.

Phobos `ObjectClass_ReceiveDamage_CanC4DamageRounding`
(`Ext/BuildingType/Hooks.cpp:182-195`) replaces that six-instruction body and
skips to `0x5F5456`. Its replacement writes 1 only for an exactly-zero packet,
only on a building, only while `CanC4=no`, and only while the key is off.
Negative packets, which the original `< 1` floor also raised to 1, now bypass
the floor entirely regardless of the key.

## VERA

`sim/combat/receiver_health.rs` passes the type's `phobos_can_c4_allow_zero_damage`
and `sim/combat/object_health.rs::commit` applies the hook's replacement:
`packet == 0 && !allow_zero_damage` floors to 1 under the `phobos` feature,
while the default build keeps the original `packet < 1` floor.

## Residuals

- The zero-packet arm is exercised through the production receiver by a
  weapon whose `Verses` scale a non-zero raw hit to 0; callers that hand the
  receiver a zero raw packet still return at the native entry gate.
- Retail rules author no `CanC4.AllowZeroDamage`; the default reproduces the
  original minimum.
