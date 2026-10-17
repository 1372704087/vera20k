//! End-of-match frame-time report.
//!
//! Written once, when the local player's outcome is ready, from evidence the
//! engine already owns: the simulation's own phase stamps
//! (`sim::frame_profile`) plus the app's frame timer and tick-cost ring. The
//! output is a self-contained HTML page — inline SVG pies, no scripts, no
//! network fetches — so it stays readable years later and can be mailed or
//! attached to a bug report.
//!
//! ## Where it lands
//! Inside the dashboard's web root (`<web root>/reports/`) when the telemetry
//! server is running, so the dashboard can serve and link the file it just
//! wrote. With no server the report still lands in `./reports/`; nothing here
//! is telemetry-only, and losing the report is never fatal.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::sim::frame_profile::FrameProfileReport;

/// One slice of a pie: a label, a millisecond value and a palette colour.
struct Slice {
    label: &'static str,
    ms: f64,
    color: &'static str,
}

/// Palette shared by both pies; the last entry is the "unaccounted" grey.
const PALETTE: [&str; 6] = [
    "#3f6fa8", "#3f9f5f", "#e08b2a", "#d94f5c", "#4fb8a8", "#9a9a9a",
];

/// Write the report for the match that just ended. Returns the file it wrote.
pub fn write_match_report(state: &crate::app::AppState) -> Option<PathBuf> {
    let runtime = state.match_state.sim_runtime.as_ref()?;
    let simulation = &runtime.simulation;
    let profile = simulation.frame_profile().report()?;

    // The app's own view of the same loop: mean wall time per rendered frame
    // and the tick-cost ring. Both are process facts, not simulation ones.
    let frame_ms = f64::from(state.diag.frame_timer.frame_ms_mean());
    let tick_ms = f64::from(
        state
            .match_state
            .telemetry_timing
            .summarize(crate::app::types::SIM_TICK_MS as f32)
            .p50_ms,
    );

    let tick = simulation.session.tick;
    let houses: Vec<(String, bool, u32)> = simulation
        .houses
        .iter()
        .map(|(owner, house)| {
            (
                simulation.interner.resolve(*owner).to_string(),
                house.is_human,
                house.tracking.buildings().max(0) as u32,
            )
        })
        .collect();
    let outcome = state
        .match_state
        .scenario_outcome
        .as_ref()
        .map(|wait| format!("{:?}", wait.kind()))
        // On-demand reports run mid-match, when no outcome exists yet.
        .unwrap_or_else(|| "进行中".to_string());

    let map = format!(
        "{}x{}",
        simulation.session.map_width, simulation.session.map_height
    );
    let html = render_report(ReportFacts {
        profile,
        frame_ms,
        tick_ms,
        tick,
        houses,
        outcome,
        map,
        render: state.diag.render_timing.report(),
    });

    let path = report_path(state, tick)?;
    let Some(parent) = path.parent() else {
        return None;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        log::warn!("frame report: cannot create {}: {error}", parent.display());
        return None;
    }

    if let Err(error) = std::fs::write(&path, &html) {
        log::warn!("frame report write failed ({}): {error}", path.display());
        return None;
    }
    // A stable name the dashboard can link without knowing timestamps: same
    // bytes, one extra write at a moment the match has already ended.
    if let Err(error) = std::fs::write(parent.join("latest.html"), &html) {
        log::warn!("frame report alias write failed: {error}");
    }
    log::info!("frame report written: {}", path.display());
    Some(path)
}

fn report_path(state: &crate::app::AppState, tick: u64) -> Option<PathBuf> {
    let root = state
        .diag
        .telemetry_server
        .as_ref()
        .map(|server| server.web_root().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default();
    Some(
        root.join("reports")
            .join(format!("frame-report-tick{tick}-{stamp}.html")),
    )
}

struct ReportFacts {
    profile: FrameProfileReport,
    frame_ms: f64,
    tick_ms: f64,
    tick: u64,
    houses: Vec<(String, bool, u32)>,
    outcome: String,
    map: String,
    /// Per-phase render means, when the app rendered at least one frame.
    render: Option<crate::sim::telemetry::RenderTimingReport>,
}

/// Chinese labels for the frame ledger, in `PHASE_KEYS` order. The array length
/// is the key count, so adding a phase without a label fails to compile.
///
/// The archived report is engine-side and has no dashboard language to follow;
/// the dashboard's own report is built in the browser from the live snapshot.
const PHASE_LABELS: [&str; crate::sim::frame_profile::PHASE_COUNT] = [
    "命令摄入",
    "命令入列",
    "触发器",
    "迷雾消耗",
    "光照过渡",
    "矿石生长",
    "炸弹/自爆",
    "超级武器",
    "辐射衰减",
    "小队 AI",
    "对象遍历",
    "碾压墙体",
    "尾迹动画",
    "视野/迷雾",
    "战斗尾段",
    "乘客系统",
    "离子炮",
    "木箱再生",
    "工厂步进",
    "停靠/维修",
    "新生成补雾",
    "房屋 AI",
    "覆盖物收尾",
    "调试断言",
    "动画推进",
    "终局计分",
    "状态哈希",
];

/// A phase's milliseconds, at the precision its size deserves.
fn format_ms(value: f64) -> String {
    if value <= 0.0 {
        "—".to_string()
    } else if value < 0.01 {
        "<0.01".to_string()
    } else if value < 0.1 {
        format!("{value:.3}")
    } else {
        format!("{value:.2}")
    }
}

/// The full ledger behind the two pies: every simulation phase, then every
/// render phase, then the frame they add up to. A reader can check any wedge.
fn phase_table(
    profile: &FrameProfileReport,
    render: Option<&crate::sim::telemetry::RenderTimingReport>,
    frame_ms: f64,
) -> String {
    let sim_total = profile.mean_frame_ms.max(0.0001);
    let frame_total = frame_ms.max(profile.mean_frame_ms);
    let share = |ms: f64, total: f64| if total > 0.0 { ms / total * 100.0 } else { 0.0 };

    let mut sim_rows: Vec<(&str, f64)> = PHASE_LABELS
        .iter()
        .copied()
        .zip(profile.phases_ms)
        .collect();
    sim_rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    let sim_body: String = sim_rows
        .iter()
        .map(|(label, ms)| {
            format!(
                "<tr><td>{label}</td><td class='n'>{value}</td>\
                 <td class='n'>{of_sim:.1}%</td><td class='n'>{of_frame:.1}%</td></tr>",
                value = format_ms(*ms),
                of_sim = share(*ms, sim_total),
                of_frame = share(*ms, frame_total),
            )
        })
        .collect();

    let render_body = match render {
        None => String::new(),
        Some(spans) => {
            let mut rows = vec![
                ("世界实例（地形/覆盖物/实体）", f64::from(spans.world_ms)),
                ("调试叠加层", f64::from(spans.debug_ms)),
                ("迷雾缓冲", f64::from(spans.shroud_ms)),
                ("小地图 / UI", f64::from(spans.ui_ms)),
                ("侧边栏", f64::from(spans.sidebar_ms)),
                ("GPU 上传", f64::from(spans.upload_ms)),
                ("其它（战斗光照/地形准备）", f64::from(spans.other_ms)),
            ];
            let render_sum: f64 = rows.iter().map(|(_, ms)| *ms).sum();
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            let body: String = rows
                .iter()
                .map(|(label, ms)| {
                    format!(
                        "<tr><td>{label}</td><td class='n'>{value}</td>\
                         <td class='n'>—</td><td class='n'>{of_frame:.1}%</td></tr>",
                        value = format_ms(*ms),
                        of_frame = share(*ms, frame_total),
                    )
                })
                .collect();
            // `render_game` is not the whole render wedge: surface acquisition,
            // encoder creation, submit and present sit outside it. Listing the
            // remainder keeps this group summing to the footer.
            let outside = (frame_total - profile.mean_frame_ms - render_sum).max(0.0);
            let outside_row = if outside > 0.01 {
                format!(
                    "<tr><td>提交 / 等待 / 循环</td><td class='n'>{}</td>\
                     <td class='n'>—</td><td class='n'>{:.1}%</td></tr>",
                    format_ms(outside),
                    share(outside, frame_total),
                )
            } else {
                String::new()
            };
            format!("<tr class='group'><td colspan='4'>渲染 / 提交</td></tr>{body}{outside_row}")
        }
    };

    let render_total = frame_total - profile.mean_frame_ms;
    format!(
        "<table class='ph'><thead><tr><th>阶段</th><th class='n'>ms</th>\
         <th class='n'>占 sim</th><th class='n'>占整帧</th></tr></thead>\
         <tbody>{sim_body}</tbody>\
         <tbody>{render_body}</tbody>\
         <tfoot><tr><td>模拟合计</td><td class='n'>{sim}</td><td class='n'>100%</td>\
         <td class='n'>{sim_share:.1}%</td></tr>\
         <tr><td>渲染 / 提交</td><td class='n'>{render}</td><td class='n'>—</td>\
         <td class='n'>{render_share:.1}%</td></tr>\
         <tr><td>整帧</td><td class='n'>{frame}</td><td class='n'>—</td>\
         <td class='n'>100%</td></tr></tfoot></table>",
        sim = format_ms(profile.mean_frame_ms),
        sim_share = share(profile.mean_frame_ms, frame_total),
        render = format_ms(render_total.max(0.0)),
        render_share = share(render_total.max(0.0), frame_total),
        frame = format_ms(frame_total),
    )
}

/// The whole frame: the four simulation phases plus everything outside them
/// (instance building, present, and the host's own scheduling).
fn frame_slices(facts: &ReportFacts) -> Vec<Slice> {
    let profile = facts.profile;
    let render_ms = (facts.frame_ms - profile.mean_frame_ms).max(0.0);
    let mut slices = vec![
        Slice {
            label: "模拟 · 对象遍历",
            ms: profile.objects_ms,
            color: PALETTE[0],
        },
        Slice {
            label: "模拟 · 前置逻辑",
            ms: profile.logic_ms,
            color: PALETTE[1],
        },
        Slice {
            label: "渲染 · 远近/提交",
            ms: render_ms,
            color: PALETTE[2],
        },
        Slice {
            label: "模拟 · 收尾",
            ms: profile.tail_ms,
            color: PALETTE[3],
        },
        Slice {
            label: "模拟 · 视野/迷雾",
            ms: profile.vision_ms,
            color: PALETTE[4],
        },
    ];
    if profile.unmarked_ms > 0.01 {
        slices.push(Slice {
            label: "模拟未标记",
            ms: profile.unmarked_ms,
            color: PALETTE[5],
        });
    }
    slices
}

/// The simulation alone: the same four phases, normalised to their own total.
fn sim_slices(profile: &FrameProfileReport) -> Vec<Slice> {
    let mut slices = vec![
        Slice {
            label: "对象遍历 (AI+移动+开火)",
            ms: profile.objects_ms,
            color: PALETTE[0],
        },
        Slice {
            label: "前置逻辑 (命令/触发/脚本)",
            ms: profile.logic_ms,
            color: PALETTE[1],
        },
        Slice {
            label: "收尾 (战斗尾/生产/胜负)",
            ms: profile.tail_ms,
            color: PALETTE[3],
        },
        Slice {
            label: "视野/迷雾",
            ms: profile.vision_ms,
            color: PALETTE[4],
        },
    ];
    if profile.unmarked_ms > 0.01 {
        slices.push(Slice {
            label: "模拟未标记",
            ms: profile.unmarked_ms,
            color: PALETTE[5],
        });
    }
    slices
}

/// Render one pie as an inline SVG: slices, percentage labels inside, and a
/// legend beside it with the millisecond values.
fn render_pie(title: &str, slices: &[Slice], total: f64) -> String {
    let size = 260.0f64;
    let center = size / 2.0;
    let radius = size / 2.0 - 6.0;
    let mut svg = format!(
        "<svg viewBox='0 0 {size} {size}' width='{size}' height='{size}' role='img' aria-label='{title}'>"
    );

    let mut angle = -90.0f64; // start at 12 o'clock
    for slice in slices {
        if slice.ms <= 0.0 || total <= 0.0 {
            continue;
        }
        let fraction = (slice.ms / total).min(1.0);
        let sweep = fraction * 360.0;
        let (x0, y0) = polar(center, center, radius, angle);
        let (x1, y1) = polar(center, center, radius, angle + sweep);
        // A full circle cannot be one arc; split it in half.
        let large_arc = if sweep > 180.0 { 1 } else { 0 };
        let path = if (sweep - 360.0).abs() < f64::EPSILON {
            format!(
                "M {cx} {cy} m 0 {r} a {r} {r} 0 1 1 0 -{two_r} a {r} {r} 0 1 1 0 {two_r}",
                cx = center,
                cy = center,
                r = radius,
                two_r = radius * 2.0
            )
        } else {
            format!(
                "M {center} {center} L {x0} {y0} A {radius} {radius} 0 {large_arc} 1 {x1} {y1} Z"
            )
        };
        let _ = write!(
            svg,
            "<path d='{path}' fill='{}' stroke='#ffffff' stroke-width='1.5'><title>{} {:.2} ms</title></path>",
            slice.color, slice.label, slice.ms
        );
        if fraction >= 0.05 {
            let (lx, ly) = polar(center, center, radius * 0.62, angle + sweep / 2.0);
            let _ = write!(
                svg,
                "<text x='{lx:.1}' y='{ly:.1}' fill='#ffffff' font-size='13' font-weight='600' \
                 text-anchor='middle' dominant-baseline='central'>{:.0}%</text>",
                fraction * 100.0
            );
        }
        angle += sweep;
    }
    svg.push_str("</svg>");

    let mut legend = String::new();
    for slice in slices {
        let percent = if total > 0.0 {
            slice.ms / total * 100.0
        } else {
            0.0
        };
        let _ = write!(
            legend,
            "<li><span class='sw' style='background:{}'></span>\
             <span class='lb'>{}</span><span class='ms'>{:.2} ms · {:.1}%</span></li>",
            slice.color, slice.label, slice.ms, percent
        );
    }

    format!(
        "<figure class='pie'><figcaption>{title} <b>{total:.1} ms</b></figcaption>\
         <div class='body'>{svg}<ul>{legend}</ul></div></figure>"
    )
}

fn polar(cx: f64, cy: f64, radius: f64, degrees: f64) -> (f64, f64) {
    let radians = degrees.to_radians();
    (cx + radius * radians.cos(), cy + radius * radians.sin())
}

fn render_report(facts: ReportFacts) -> String {
    let profile = facts.profile;
    let frame_total = facts.frame_ms.max(profile.mean_frame_ms);
    let sim_total = profile.mean_frame_ms.max(0.0001);
    let sim_share = if frame_total > 0.0 {
        profile.mean_frame_ms / frame_total * 100.0
    } else {
        0.0
    };
    let sim_seconds = profile.mean_frame_ms / 1000.0 * profile.frames as f64;
    let sim_ticks_per_second = if sim_seconds > 0.0 {
        profile.frames as f64 / sim_seconds
    } else {
        0.0
    };

    let frame_slices = frame_slices(&facts);
    let sim_slices = sim_slices(&profile);
    // The ledger behind both pies: every simulation phase, then the render's
    // own phases, then the frame total.
    let ledger = phase_table(&profile, facts.render.as_ref(), frame_total);

    let house_rows: String = facts
        .houses
        .iter()
        .map(|(name, is_human, buildings)| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(name),
                if *is_human { "human" } else { "ai" },
                buildings
            )
        })
        .collect();

    format!(
        "<!doctype html>\n<html lang='zh-CN'>\n<head>\n<meta charset='utf-8'>\n\
         <title>VERA20k 帧占用报告 · tick {tick}</title>\n<style>\n\
         * {{ box-sizing: border-box; margin: 0; }}\n\
         body {{ font: 14px/1.5 -apple-system, 'Segoe UI', 'PingFang SC', 'Microsoft YaHei', sans-serif;\
         color: #1d2733; background: #f7f9fc; padding: 28px 32px; }}\n\
         h1 {{ font-size: 24px; font-weight: 700; letter-spacing: .02em; }}\n\
         h2 {{ font-size: 14px; font-weight: 600; color: #4a5b6e; margin: 26px 0 10px; }}\n\
         .sub {{ color: #5b6b7d; font-size: 13px; margin-top: 6px; }}\n\
         .pies {{ display: flex; gap: 40px; flex-wrap: wrap; }}\n\
         figure.pie {{ background: #fff; border: 1px solid #e2e8f0; border-radius: 8px;\
         padding: 18px 20px; box-shadow: 0 1px 3px rgba(16,24,40,.06); }}\n\
         figcaption {{ font-size: 13px; color: #4a5b6e; margin-bottom: 10px; }}\n\
         figcaption b {{ color: #1d2733; }}\n\
         .body {{ display: flex; align-items: center; gap: 22px; }}\n\
         ul {{ list-style: none; }}\n\
         li {{ display: flex; align-items: center; gap: 8px; margin: 5px 0; font-size: 12.5px; }}\n\
         .sw {{ width: 10px; height: 10px; border-radius: 2px; flex: none; }}\n\
         .lb {{ min-width: 170px; }}\n\
         .ms {{ color: #5b6b7d; font-variant-numeric: tabular-nums; }}\n\
         table {{ border-collapse: collapse; font-variant-numeric: tabular-nums; }}\n\
         th, td {{ text-align: left; padding: 6px 18px 6px 0; border-bottom: 1px solid #e8edf3; }}\n\
         th {{ color: #6b7b8d; font-weight: 600; font-size: 12px; }}\n\
         .note {{ margin-top: 22px; color: #7b8b9d; font-size: 12px; }}\n\
         .grid {{ display: flex; gap: 40px; flex-wrap: wrap; }}\n\
         table.ph {{ width: 100%; margin-top: 4px; }}\n\
         table.ph th.n, table.ph td.n {{ text-align: right; padding-right: 0; }}\n\
         table.ph td {{ padding: 4px 22px 4px 0; font-size: 13px; }}\n\
         table.ph tr.group td {{ color: #4a5b6e; font-weight: 600; padding-top: 14px; }}\n\
         table.ph tfoot td {{ border-bottom: none; font-weight: 600; color: #1d2733; }}\n\
         table.ph tfoot tr:first-child td {{ border-top: 2px solid #cbd5e1; }}\n\
         table.ph tfoot tr:last-child td {{ border-top: 1px solid #cbd5e1; }}\n\
         </style></head>\n<body>\n\
<h1>VERA20k 帧占用报告</h1>\n\
<div class='sub'>tick {tick} · 模拟 {sim_seconds:.0} s · {sim_ticks_per_second:.1} 模拟帧/s · \
地图 {map} · 结果 {outcome}<br>\
帧均 {frame_total:.1} ms · 模拟均 {sim:.1} ms（{share:.0}%） · 最慢模拟帧 {peak:.1} ms · \
tick 成本中位数 {tick_ms:.2} ms</div>\n\
<h1 style='font-size:0'></h1>\n<div class='pies'>{frame_pie}{sim_pie}</div>\n\
<h2>耗时明细</h2>\n{ledger}\n\
<h2>参与方</h2>\n<table><thead><tr><th>house</th><th>control</th><th>buildings</th></tr></thead>\
<tbody>{house_rows}</tbody></table>\n\
<p class='note'>渲染 = 帧均 − 模拟均 = {render_ms:.1} ms（含 CPU 帧准备、present 等待与主循环杂项）。\
模拟内部四段之和 {bucketed:.1} ms{unmarked_note}。分段计时取自帧内的五个时钟读数，累计 {frames} 帧。</p>\n\
</body></html>\n",
        tick = facts.tick,
        sim_seconds = sim_seconds,
        sim_ticks_per_second = sim_ticks_per_second,
        map = escape(&facts.map),
        outcome = escape(&facts.outcome),
        frame_total = frame_total,
        sim = sim_total,
        share = sim_share,
        peak = profile.peak_frame_ms,
        tick_ms = facts.tick_ms,
        frame_pie = render_pie("整帧占用", &frame_slices, frame_total),
        sim_pie = render_pie("模拟内部", &sim_slices, sim_total),
        house_rows = house_rows,
        render_ms = (frame_total - profile.mean_frame_ms).max(0.0),
        bucketed = profile.logic_ms + profile.objects_ms + profile.vision_ms + profile.tail_ms,
        unmarked_note = if profile.unmarked_ms > 0.01 {
            format!("，未标记 {:.2} ms", profile.unmarked_ms)
        } else {
            String::new()
        },
        frames = profile.frames,
        ledger = ledger,
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::frame_profile::{phase, FramePhases, FrameProfile};
    use std::time::Duration;

    fn sample_profile() -> FrameProfileReport {
        let mut profile = FrameProfile::default();
        for _ in 0..10 {
            // 100 µs logic + 500 µs objects + 200 µs vision + 200 µs tail.
            let mut phases = FramePhases::default();
            phases.set(phase::TEAM_AI, Duration::from_micros(100));
            phases.set(phase::OBJECTS, Duration::from_micros(500));
            phases.set(phase::VISION, Duration::from_micros(200));
            phases.set(phase::COMBAT, Duration::from_micros(200));
            profile.record(phases, Duration::from_micros(1000));
        }
        profile.report().expect("ten frames recorded")
    }

    #[test]
    fn report_contains_both_pies_and_the_headline_numbers() {
        let facts = ReportFacts {
            profile: sample_profile(),
            frame_ms: 40.0,
            tick_ms: 0.5,
            tick: 1234,
            houses: vec![("Americans".to_string(), true, 3)],
            outcome: "Defeat".to_string(),
            map: "120x120".to_string(),
            render: None,
        };
        let html = render_report(facts);
        assert!(html.contains("VERA20k 帧占用报告"));
        assert!(html.contains("整帧占用"));
        assert!(html.contains("模拟内部"));
        assert!(html.contains("tick 1234"));
        // 1.0 ms of simulation in a 40 ms frame.
        assert!(html.contains("40.0 ms"), "frame total is on the page");
        assert!(html.contains("1.0 ms"), "simulation total is on the page");
        assert!(html.contains("Americans"));
    }

    #[test]
    fn render_share_is_frame_minus_simulation() {
        let facts = ReportFacts {
            profile: sample_profile(),
            frame_ms: 40.0,
            tick_ms: 0.5,
            tick: 7,
            houses: Vec::new(),
            outcome: "match ended".to_string(),
            map: "0x0".to_string(),
            render: None,
        };
        let slices = frame_slices(&facts);
        let render = slices
            .iter()
            .find(|slice| slice.label.contains("渲染"))
            .expect("the render slice exists");
        assert!(
            (render.ms - 39.0).abs() < 0.001,
            "40 ms frame minus 1 ms simulation leaves 39 ms of render"
        );
        let sum: f64 = slices.iter().map(|slice| slice.ms).sum();
        assert!(
            (sum - 40.0).abs() < 0.001,
            "the frame slices cover the frame"
        );
    }

    #[test]
    fn a_frame_below_the_simulation_cost_does_not_produce_negative_render() {
        // Window unfocused: the app's frame timer can lag the simulation's own
        // mean. The report must never claim negative render time.
        let facts = ReportFacts {
            profile: sample_profile(),
            frame_ms: 0.2,
            tick_ms: 0.5,
            tick: 7,
            houses: Vec::new(),
            outcome: "match ended".to_string(),
            map: "0x0".to_string(),
            render: None,
        };
        let slices = frame_slices(&facts);
        let render = slices
            .iter()
            .find(|slice| slice.label.contains("渲染"))
            .expect("the render slice exists");
        assert_eq!(render.ms, 0.0);
    }

    #[test]
    fn ledger_lists_every_simulation_phase_and_the_render_split() {
        let facts = ReportFacts {
            profile: sample_profile(),
            frame_ms: 40.0,
            tick_ms: 0.5,
            tick: 42,
            houses: Vec::new(),
            outcome: "match ended".to_string(),
            map: "0x0".to_string(),
            render: Some(crate::sim::telemetry::RenderTimingReport {
                frames: 10,
                world_ms: 5.0,
                debug_ms: 0.0,
                shroud_ms: 0.5,
                ui_ms: 2.0,
                sidebar_ms: 0.25,
                upload_ms: 0.1,
                other_ms: 0.05,
            }),
        };
        let html = render_report(facts);
        assert!(html.contains("耗时明细"), "the ledger has a heading");
        assert!(html.contains("渲染 / 提交"), "render group is present");
        assert!(html.contains("世界实例"), "render phases are listed");
        // The remainder outside `render_game` keeps the render group summing to
        // the footer; 40 ms frame − 1 ms sim − 17.9 ms phases leaves 21.1 ms.
        assert!(
            html.contains("提交 / 等待 / 循环"),
            "the render group must account for the time outside render_game"
        );
        // Every simulation phase label appears; a missing one would mean the
        // table and `PHASE_KEYS` drifted apart.
        for label in PHASE_LABELS {
            assert!(html.contains(label), "phase `{label}` is missing");
        }
    }

    #[test]
    fn html_escapes_house_names() {
        assert_eq!(escape("<script>&"), "&lt;script&gt;&amp;");
    }

}
