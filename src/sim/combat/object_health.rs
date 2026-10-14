//! The object's health (`ObjectClass+0x6C`) and Object5F5390's signed HP
//! commit. Damage preparation never predicts this result.
//! Original-byte comparisons: tools/spatial_oracle/object_health (718 rows).
use super::{GameEntity, damage::DamageState};
use crate::util::native_x87::{MaskedX87Chop53 as X87, MaskedX87Ordering, NativeF64Bits};

/// Signed actual ObjectClass health (`ObjectClass+0x6C`, Strength), owned by
/// this module: ReceiveDamage's commit below writes it, and every other
/// native writer goes through [`Health::set`] or [`Health::heal`]. The live
/// ObjectType's `strength` is the cap and ratio denominator; EstimatedHealth
/// (`+0x70`) is independent state these writes never update.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Health {
    current: i32,
}

impl Health {
    /// The health a constructor or fixture gives a new object.
    pub const fn new(current: i32) -> Self {
        Self { current }
    }

    /// The signed actual health.
    pub const fn current(self) -> i32 {
        self.current
    }

    /// Store `value`, as the native writers outside ReceiveDamage do: a kill
    /// before UnInit stores 0 (a crash, a crush, a garrison's or a dying
    /// transport's passengers), PostMortem, ship sinking and the infantry
    /// health reset store 1, a completed repair (an engineer's, the repair
    /// step's or the depot's), a spawn reload and a slave heal store the
    /// type's Strength, and map admission, conversion and crew survival store
    /// their computed value. Each caller cites its native writer.
    pub(crate) fn set(&mut self, value: i32) {
        self.current = value;
    }

    /// `Strength += amount` with wrapping, uncapped: the repair step, the
    /// depot's radio heal and the self-heal pulses, whose callers cap at
    /// Strength themselves.
    pub(crate) fn heal(&mut self, amount: i32) {
        self.current = self.current.wrapping_add(amount);
    }

    /// ObjectClass::GetHealthRatio (gamemd.exe 0x005F5C60..0x005F5C7F).
    /// Signed loads and PC53/chop division; zero Strength keeps the native
    /// masked infinity/NaN result for the caller's actual comparison/conversion.
    pub fn ratio(self, strength: i32) -> crate::util::native_x87::MaskedX87Value {
        X87::div(X87::load_i32(self.current), X87::load_i32(strength))
    }

    /// [`Self::ratio`] compared `>=` `Rules+0x16F8` with an ordered FCOMP
    /// (the C0-clear test): 0/0 (NaN) is not full and x/0 for x > 0 (+inf)
    /// is. AudioVisual `0x0066B323..0x0066B32D` writes that threshold as 1.0
    /// unconditionally; there is no ConditionGreen key. Object radio
    /// (`0x005F5339`), Techno repair (`0x006F4DE5`), the healer fire errors and
    /// the hospital and depot tests ask it. The quotient of two dwords cannot
    /// round across 1.0, so the integer form is exact.
    pub(crate) fn is_full(self, strength: i32) -> bool {
        match strength.cmp(&0) {
            std::cmp::Ordering::Greater => self.current >= strength,
            std::cmp::Ordering::Less => self.current <= strength,
            std::cmp::Ordering::Equal => self.current > 0,
        }
    }

    /// Compare the native ratio without converting masked values to a host float.
    pub fn compare_ratio(self, strength: i32, threshold: f64) -> MaskedX87Ordering {
        X87::compare(
            self.ratio(strength),
            X87::load_f64(NativeF64Bits::from_bits(threshold.to_bits())),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HealthCallback {
    Changed,
    Kill,
    Destroy,
}

/// The callback boundary is synchronous. The world adapter stages existing kill/
/// destruction transactions after releasing its entity borrow; that adapter does
/// not claim complete native trigger/callback-body parity.
pub(super) fn commit(
    target: &mut GameEntity,
    packet: &mut i32,
    strength: i32,
    admitted: bool,
    building_no_c4: bool,
    condition_red: f64,
    mut callback: impl FnMut(&mut GameEntity, HealthCallback),
) -> DamageState {
    let previous = target.health.current;
    if previous <= 0 || !admitted {
        return DamageState::Unaffected;
    }
    if building_no_c4 {
        *packet = (*packet).max(1);
    }
    if *packet == 0 {
        return DamageState::Unaffected;
    }
    if *packet < 0 {
        // 5F546A..5495: the intermediate write precedes the signed cap;
        // changed callback receives the capped value, and cannot change return0.
        target.health.current = previous.wrapping_sub(*packet);
        if target.health.current > strength {
            target.health.current = strength;
        }
        if target.health.current != previous {
            callback(target, HealthCallback::Changed);
        }
        return DamageState::Unaffected;
    }
    *packet = (*packet).min(previous);
    let post = previous.wrapping_sub(*packet);
    let mut result = DamageState::Damaged;
    let yellow = strength >> 1;
    if yellow <= previous && post < yellow {
        result = DamageState::Yellow;
    }
    let red = X87::mul(
        X87::load_i32(strength),
        X87::load_f64(NativeF64Bits::from_bits(condition_red.to_bits())),
    );
    if X87::compare(red, X87::load_i32(previous)) == MaskedX87Ordering::Less
        && X87::compare(X87::load_i32(post), red) == MaskedX87Ordering::Less
    {
        result = DamageState::Red;
    }
    target.health.current = post;
    // Alive0 is result5, independent of the newly written health (including0).
    if !target.lifecycle.object_alive {
        return DamageState::AlreadyDead;
    }
    if target.health.current == 0 {
        callback(target, HealthCallback::Kill);
        callback(target, HealthCallback::Destroy);
        result = DamageState::Dead;
    }
    result
}

#[cfg(test)]
#[path = "object_health_tests.rs"]
mod tests;
