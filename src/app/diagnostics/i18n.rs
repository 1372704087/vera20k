//! Bilingual (zh-CN / en) string table for the optional dev-ui diagnostics.
//!
//! Process-scoped tooling state: the active language lives in
//! [`crate::app::diagnostics::state::DiagnosticsState`] and is toggled with
//! the dev chord (`Ctrl+Shift+I`) or the segmented button in the Developer
//! Overlay. It is deliberately *not* persisted to the options profile — the
//! shipped player UI never renders these strings.
//!
//! ## Usage
//! - Static text: `i18n::t(lang, "panel.sim")`.
//! - Formatted text: `i18n::t_f(lang, "ev.blocked", &[&tick, &x, &y])` — `{}`
//!   placeholders are replaced positionally, in order.
//! - Unknown keys fall through to the key itself so a missing translation is
//!   obvious in the UI instead of silently blank.
//!
//! ## Dependency rules
//! - Part of the app layer — may depend on everything. This module itself
//!   depends on nothing so it stays usable from any dev-ui surface.

/// Active UI language for the diagnostic panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Lang {
    /// English (default — matches the codebase's primary language).
    #[default]
    En,
    /// Simplified Chinese.
    Zh,
}

impl Lang {
    /// The other language — used by the toggle hotkey and overlay button.
    pub(crate) fn toggled(self) -> Self {
        match self {
            Lang::En => Lang::Zh,
            Lang::Zh => Lang::En,
        }
    }
}

/// Every translatable string in the diagnostics UI: (key, zh-CN, en).
///
/// Keys are dot-namespaced by surface: `panel.*`, `sim.*`, `ov.*`,
/// `terrain.*`, `ent.*`, `ev.*`, `hk.*`, `top.*`, `status.*`, `legend.*`.
static STRINGS: &[(&str, &str, &str)] = &[
    // ---- chrome / top bar ----
    ("top.lang", "界面语言", "UI language"),
    // ---- panel titles ----
    ("panel.sim", "开发者总控", "Developer"),
    ("panel.terrain", "地形调试", "Terrain"),
    ("panel.hotkeys", "快捷键参考", "Hotkeys"),
    ("panel.audio", "音频", "Audio"),
    // ---- sim panel ----
    ("sim.speed", "模拟速度", "Sim speed"),
    ("sim.reset", "复位", "Reset"),
    ("sim.pause", "暂停", "Pause"),
    ("sim.resume", "继续", "Resume"),
    ("sim.step", "步进 1 tick", "Step 1 tick"),
    ("sim.toMenu", "返回主菜单", "Return to menu"),
    ("sim.tickBudget", "Tick 预算", "Tick budget"),
    ("sim.entities", "实体数", "Entities"),
    ("sim.tick", "当前 Tick", "Tick"),
    ("sim.fps", "帧率", "FPS"),
    ("sim.frame", "帧耗时", "Frame"),
    ("sim.saveLoad", "保存 / 读取", "Save / Load"),
    ("sim.saveName", "存档名", "Name"),
    ("sim.saveAs", "另存", "Save as"),
    ("sim.recent", "最近存档", "Recent"),
    ("sim.noSaves", "（暂无存档）", "(no saves)"),
    ("sim.reload", "重新读取上次存档", "Reload last load"),
    ("sim.load", "读取", "Load"),
    ("sim.lastSave", "上次保存", "Last save"),
    ("sim.lastSaveNone", "本次会话未保存", "(none this session)"),
    // ---- overlays panel ----
    ("ov.title", "调试叠加层", "Debug overlays"),
    ("ov.pathgrid", "地形代价", "PathGrid"),
    ("ov.heightmap", "高度图", "Heightmap"),
    ("ov.cellgrid", "格子对齐", "Cell grid"),
    ("ov.reveal", "解锁迷雾", "Reveal map"),
    ("ov.inspector", "事件检查器", "Event inspector"),
    ("ov.footPath", "寻路步骤", "Foot path"),
    ("ov.auto", "自动", "Auto"),
    ("ov.speedTypeHint", "留空 = 跟随首个选中单位", "empty = follow first selected unit"),
    ("ov.keyHint", "热键", "hotkey"),
    // ---- SpeedType display names (identifiers stay English in both UIs) ----
    ("st.foot", "步兵 Foot", "Foot"),
    ("st.track", "履带 Track", "Track"),
    ("st.wheel", "轮式 Wheel", "Wheel"),
    ("st.hover", "悬停 Hover", "Hover"),
    ("st.winged", "飞行 Winged", "Winged"),
    ("st.float", "浮渡 Float", "Float"),
    ("st.amphibious", "两栖 Amphibious", "Amphibious"),
    ("st.floatBeach", "滩涂浮渡 FloatBeach", "FloatBeach"),
    ("st.invalid", "无效 Invalid", "Invalid"),
    // ---- entity category short labels ----
    ("cat.unit", "单位", "Unit"),
    ("cat.infantry", "步兵", "Inf"),
    ("cat.structure", "建筑", "Bld"),
    ("cat.aircraft", "飞行器", "Air"),
    // ---- terrain panel ----
    ("terrain.grid", "网格", "Grid"),
    ("terrain.cursor", "光标格子", "Cursor cell"),
    ("terrain.elevation", "海拔", "Elevation"),
    ("terrain.walkable", "可通行", "WALKABLE"),
    ("terrain.blocked", "阻挡", "BLOCKED"),
    ("terrain.pathgrid", "寻路网格", "PathGrid"),
    ("terrain.costs", "地形代价（按移动方式）", "Terrain cost (per SpeedType)"),
    ("terrain.entities", "格上实体", "Entities at cell"),
    ("terrain.none", "（无）", "(none)"),
    ("terrain.footprint", "位于 footprint：", "In footprint: "),
    ("terrain.vxl", "VXL 精灵", "VXL sprites"),
    ("terrain.selected", "选中单位", "Selected unit"),
    ("terrain.selNone", "（未选中）", "(none selected)"),
    ("terrain.pathIdle", "静止", "idle"),
    ("terrain.stepsLeft", "剩余步数", "steps left"),
    ("terrain.goal", "目标", "Goal"),
    ("terrain.blockedBy", "受阻", "blocked"),
    ("terrain.miner", "矿车状态", "Miner"),
    ("terrain.cargo", "载矿", "Cargo"),
    ("terrain.stage", "阶段", "Stage"),
    ("terrain.refinery", "冶炼厂", "Refinery"),
    ("terrain.archivedOre", "预记矿点", "Archived ore"),
    ("terrain.movement", "移动", "Movement"),
    ("terrain.active", "进行中", "active"),
    ("terrain.idle", "空闲", "idle"),
    // ---- entity inspector ----
    ("ent.title", "单位事件历史", "Event History"),
    ("ent.hint", "选中一个单位以查看事件", "Select a unit to inspect"),
    ("ent.noEvents", "（无事件记录）", "(no events recorded)"),
    ("ent.notActive", "生成时未开启检查器", "inspector not active when spawned"),
    ("ent.recent", "最近 30 条事件", "Recent 30 events"),
    // ---- event log (formatted; {} placeholders) ----
    ("ev.phase", "[{}] {} → {} ({})", "[{}] {} → {} ({})"),
    ("ev.repath", "[{}] 重新寻路：{}（{} 步）", "[{}] Repath: {} ({} steps)"),
    ("ev.blocked", "[{}] 阻塞于 ({},{}){}", "[{}] Blocked at ({},{}){}"),
    ("ev.blockedBy", " 被 #{} 阻挡", " by #{}"),
    ("ev.stuck", "[{}] 卡死中止（{} tick 后）", "[{}] STUCK ABORT after {} ticks"),
    ("ev.segment", "[{}] 路径段完成{}", "[{}] Path segment complete{}"),
    ("ev.segmentRepath", "，重寻路至 ({},{})", ", repathing to ({},{})"),
    ("ev.miner", "[{}] 矿车：{} → {}", "[{}] Miner: {} → {}"),
    ("ev.specialStart", "[{}] {} 开始", "[{}] {} started"),
    ("ev.specialPhase", "[{}] → {}", "[{}] → {}"),
    ("ev.specialEnd", "[{}] 特殊移动结束", "[{}] Special movement ended"),
    ("ev.overrideOn", "[{}] 覆盖开启：{}", "[{}] Override ON: {}"),
    ("ev.overrideOff", "[{}] 覆盖关闭：{}", "[{}] Override OFF: {}"),
    // ---- hotkeys panel ----
    ("hk.game", "游戏控制", "Game controls"),
    ("hk.debug", "调试叠加层", "Debug overlays"),
    ("hk.mouse", "鼠标", "Mouse"),
    ("hk.devChord", "^+ = Ctrl+Shift（与原版热键零冲突）", "^+ = Ctrl+Shift (no retail conflicts)"),
    ("hk.lmb", "选择 / 放置", "Select / Place"),
    ("hk.rmb", "下令 / 取消选择", "Order / Deselect"),
    ("hk.wheel", "滚动侧边栏", "Scroll sidebar"),
    ("hk.f1", "本帮助面板", "This help panel"),
    ("hk.quicksave", "快速保存", "Quicksave"),
    ("hk.quickload", "读取最近存档", "Load most recent save"),
    ("hk.saveLoad", "保存 / 读取面板", "Save/load panel"),
    ("hk.attackMove", "攻击移动（临时）", "Attack move mode (interim)"),
    ("hk.move", "移动模式（临时）", "Move mode (interim)"),
    ("hk.pathgrid", "地形代价 + 调试面板", "Terrain costs + debug panel"),
    ("hk.cycleSt", "循环移动方式", "Cycle SpeedType"),
    ("hk.heightmap", "高度图（蓝=桥面）", "Height map (blue=bridge)"),
    ("hk.cellgrid", "格子对齐（青+黄）", "Cell grid (cyan+yellow)"),
    ("hk.fog", "切换战争迷雾", "Toggle fog of war"),
    ("hk.inspector", "事件检查器", "Unit inspector (event log)"),
    ("hk.footPath", "选中步兵的寻路步骤", "Foot-path steps of selection"),
    ("hk.debugPause", "调试暂停", "Debug pause"),
    ("hk.frameStep", "暂停时单帧步进", "Frame step (while paused)"),
    ("hk.lang", "切换界面语言", "Toggle UI language"),
    ("hk.stop", "停止选中单位", "Stop selected units"),
    ("hk.deploy", "部署 / 收起", "Deploy / Undeploy"),
    ("hk.guard", "守卫模式", "Guard mode"),
    ("hk.selectType", "选择同类型单位", "Select same type"),
    ("hk.ctrlGroup", "选择控制组", "Select control group"),
    ("hk.assignGroup", "指派控制组", "Assign control group"),
    ("hk.base", "跳转基地（MCV）", "Jump to base (MCV)"),
    ("hk.radar", "跳转雷达事件", "Jump to radar event"),
    ("hk.esc", "暂停 / 取消", "Pause / Cancel"),
    // ---- status bar ----
    ("status.live", "运行中", "LIVE"),
    ("status.paused", "已暂停", "PAUSED"),
    // ---- legend ----
];

/// Look up a static string. Falls back to the key itself when missing.
///
/// The return lifetime is tied to `key` so the fallback can hand back the
/// caller's own key; hits return `&'static str` table entries, which coerce.
pub(crate) fn t<'a>(lang: Lang, key: &'a str) -> &'a str {
    for (k, zh, en) in STRINGS {
        if *k == key {
            return match lang {
                Lang::Zh => zh,
                Lang::En => en,
            };
        }
    }
    key
}

/// Look up a formatted string, replacing `{}` placeholders positionally.
///
/// Unused placeholders are left as-is; extra args are ignored.
pub(crate) fn t_f(lang: Lang, key: &str, args: &[&dyn std::fmt::Display]) -> String {
    let template = t(lang, key);
    let mut out = String::with_capacity(template.len() + 16);
    let mut arg_idx = 0usize;
    let mut rest = template;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        if let Some(arg) = args.get(arg_idx) {
            use std::fmt::Write as _;
            let _ = write!(out, "{arg}");
            arg_idx += 1;
        } else {
            out.push_str("{}");
        }
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_resolve() {
        assert_eq!(t(Lang::En, "panel.sim"), "Developer");
        assert_eq!(t(Lang::Zh, "panel.sim"), "开发者总控");
    }

    #[test]
    fn unknown_key_falls_through_to_key() {
        assert_eq!(t(Lang::En, "nope.missing"), "nope.missing");
        assert_eq!(t(Lang::Zh, "nope.missing"), "nope.missing");
    }

    #[test]
    fn toggle_switches_language() {
        assert_eq!(Lang::En.toggled(), Lang::Zh);
        assert_eq!(Lang::Zh.toggled(), Lang::En);
        assert_eq!(Lang::default(), Lang::En);
    }

    #[test]
    fn positional_formatting() {
        let args: [&dyn std::fmt::Display; 4] = [&1204, &"SearchOre", &"Harvest", &"ore found"];
        assert_eq!(
            t_f(Lang::En, "ev.phase", &args),
            "[1204] SearchOre → Harvest (ore found)"
        );
        let zh_args: [&dyn std::fmt::Display; 4] = [&1204, &"SearchOre", &"Harvest", &"ore found"];
        assert_eq!(
            t_f(Lang::Zh, "ev.phase", &zh_args),
            "[1204] SearchOre → Harvest (ore found)"
        );
    }

    #[test]
    fn missing_placeholder_is_preserved() {
        // Two placeholders, one arg: the second stays literal by design.
        assert_eq!(
            t_f(Lang::En, "ev.stuck", &[&45]),
            "[45] STUCK ABORT after {} ticks"
        );
        assert_eq!(
            t_f(Lang::En, "ev.stuck", &[]),
            "[{}] STUCK ABORT after {} ticks"
        );
    }

    /// `t()` falls back to the key itself on a miss, so a key referenced by
    /// the panels but dropped from (or never added to) `STRINGS` compiles
    /// clean and only shows up as raw `word.word` text on screen. This is the
    /// guard: it scans the two panel modules as text and requires every
    /// key-shaped literal to resolve. Scanning keeps the covered set
    /// self-maintaining — new `t(lang, "…")` calls are checked automatically.
    #[test]
    fn every_panel_key_resolves() {
        for source in [include_str!("debug_panel.rs"), include_str!("dev_overlay.rs")] {
            let mut rest = source;
            while let Some(open) = rest.find('"') {
                rest = &rest[open + 1..];
                let Some(close) = rest.find('"') else { break };
                let literal = &rest[..close];
                rest = &rest[close + 1..];
                if is_key_shape(literal) {
                    assert!(
                        STRINGS.iter().any(|(k, _, _)| *k == literal),
                        "key {literal:?} is referenced by a panel but missing from STRINGS"
                    );
                }
            }
        }
    }

    /// Key shape: a known table family prefix plus a dot-separated tail of
    /// letters/digits/dots/underscores. Restricting to known families keeps
    /// unrelated dotted literals (file names like `"ra2.mix"`, `"shell.pcx"`)
    /// out of the scan.
    fn is_key_shape(s: &str) -> bool {
        const FAMILIES: &[&str] = &[
            "app", "cat", "ent", "ev", "hk", "legend", "ov", "panel", "sim", "st", "status",
            "terrain", "top",
        ];
        let Some((family, tail)) = s.split_once('.') else {
            return false;
        };
        FAMILIES.contains(&family)
            && !tail.is_empty()
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    }
}
