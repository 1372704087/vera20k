# Phobos `ShowBriefing*` keys (task 52, Section D subset)

Three keys from `EXTERNAL-TASK-phobos-52.md` Section D. This page records
their native reads, the VERA owners that now hold them, and — importantly —
why the consumer pass is **escalated rather than faked**.

## Native evidence

| Key | Read site | Default | Consumer |
|---|---|---|---|
| `ShowBriefing` | `src/Phobos.INI.cpp:107` `[Phobos] ReadBool` (static `Phobos.h:108`, `:68`); also `Ext/Scenario/Body.cpp:167` `[Basic]` and `Ext/Scenario/Hooks.cpp:21` (scenario section, `Body.h:62` default `false`) | client `yes`, scenario `no` | `DEFINE_HOOK(0x683E41, ScenarioClass_Start_ShowBriefing, 0x6)`, `src/Misc/Hooks.UI.cpp:345-373` |
| `ShowBriefingResumeButtonLabel` | `src/Phobos.INI.cpp:246` `uimd.ini [UISettings] ReadString` | `GUI:Resume` | `DEFINE_HOOK(0x65F764, BriefingDialog_ShowBriefing, 0x5)`, `Hooks.UI.cpp:423-433` |
| `ShowBriefingResumeButtonStatusLabel` | `src/Phobos.INI.cpp:249` `uimd.ini [UISettings] ReadString` | `STT:BriefingButtonReturn` | `DEFINE_HOOK(0x604985, GetDialogUIStatusLabels_ShowBriefing, 0x5)`, `Hooks.UI.cpp:437-446` |

`0x683E41` refuses the dialog unless **all** hold: the client
`Phobos::Config::ShowBriefing`, `ScenarioExt::Global()->ShowBriefing`, the
caller's `showBriefing` stack argument and `SessionClass::IsCampaign()`.
It then sets `BriefingTemp::ShowBriefing`, queues `BriefingTheme`, and returns
to `SkipGameCode = 0x683E6B`. `0x48CE85` and `0x55D14F` call `ShowBriefing()`
to open `Game::SpecialDialog = 9`; `0x683F66` / `0x685D95` skip a redraw while
it is pending; the two label hooks rewrite dialog item `1059` (`0x423`) and
the status line only while that flag is set.

## VERA implementation

- `src/app/persistence/phobos_settings.rs` — `PhobosClientSettings::show_briefing`,
  read with the exact literal `"ShowBriefing"` and the native `true` default,
  alongside `SaveGameOnScenarioStart` (`Phobos.INI.cpp:106`).
- `src/rules/phobos_briefing_ui.rs` — `BriefingDialogUi::from_uimd_bytes` /
  `from_ini` / `from_section` read `uimd.ini [UISettings]` with
  `IniSection::read_string` (the native `INI_EX::ReadString`: default only on
  an absent key, an authored empty value stays empty). The button label uses
  the shared 2048-byte buffer (`Phobos.h:26`), the status label native's
  `char[0x20]`.

Both land in `rules`/`app` with `#[cfg(feature = "phobos")]` module gating;
the stock build is unchanged. All three canonical keys were already in
`G:/20k/scratch/phobos_read_keys.json`, so no registry edit was needed.

## Why the consumer is not wired (escalate, per task 52 rule 6)

VERA **does** have a briefing dialog — `InGameMenuState::MissionBriefing`
renders through `app/frontend/skirmish_shell_render/restate.rs`, using
`ui/shell/restate.rs`'s `RETURN_LABEL = ("GUI:Return", "Return")` and
`RestateControl::Return.help_key() = Some("STT:BriefingButtonReturn")`.
`Phobos`'s item 1059 is decimal `0x423`: **the same button**. But the two are
different *paths* of that dialog:

- VERA's `0x10C` shell is the **F2 restate** path: the button says `Return`
  (`GUI:Return`), and its help key is already `STT:BriefingButtonReturn`.
- Phobos's two labels apply **only while `BriefingTemp::ShowBriefing` is set**,
  i.e. the **scenario-start** path, where the same button becomes `Resume`
  (`GUI:Resume`).

VERA has no `ScenarioClass_Start`/`MainGame`/`AuxLoop` scenario-start briefing
pass (`0x683E41`, `0x48CE85`, `0x55D14F`), no `Game::SpecialDialog = 9`
equivalent and no `BriefingTheme` queue. Wiring the two labels into
`render_restate_shell` would rename the stock F2 "Return" button to "Resume"
— a visible regression, not a port. `ShowBriefing` has the same problem: the
scenario `[Basic]` layer lives in `rules/campaign_loading.rs`, which is owned
by an in-flight branch of task 52 (`wt-bare55-brief`).

So the three keys are read at their exact native layers and left without a
consumer. Building the pass is a separate piece of work:

1. read scenario `[Basic] ShowBriefing` / `BriefingTheme` in the campaign
   loader (coordinate with the in-flight `campaign_loading.rs` owner);
2. add a scenario-start briefing admission gated by the client switch, the
   scenario switch, the caller's `showBriefing` and `IsCampaign`;
3. load `BriefingDialogUi` from `uimd.ini` in
   `app/loading/init_helpers.rs:626-650` (the same `uimd_bytes` chain that
   already builds `PauseMenuUi` / `SidebarIndicatorsUi`), store it in
   `MatchPresentationState`, and use the two labels in the new dialog only;
4. keep `render_restate_shell` on `GUI:Return` / `STT:BriefingButtonReturn`.

## Tests (production readers)

- `phobos_briefing_ui.rs::tests`:
  `an_absent_uisettings_section_keeps_the_phobos_reader_defaults`,
  `authored_uisettings_labels_replace_only_their_own_defaults`,
  `an_authored_empty_status_label_stays_empty_like_read_string`.
- `phobos_settings.rs::tests`:
  `show_briefing_defaults_to_yes_and_reads_an_explicit_no`, plus the extended
  `imported_switches_are_read` and `missing_profile_or_section_keeps_the_source_defaults`.

## Residuals

- No scenario-start briefing dialog pass; all three values are currently
  storage-only. The scanner will see the literal reads, but no behaviour
  changes until the pass above is built.
- `ShowBriefingResumeButtonLabel` is a CSF key resolved by
  `GeneralUtils::LoadStringOrDefault` (`Phobos.INI.cpp:247`); VERA stores the
  key string and would resolve it through `i18n::label` when the dialog exists.
- The scenario `[Basic] ShowBriefing` / `BriefingTheme` layer is not ported
  here to avoid a second owner on the in-flight `campaign_loading.rs`.
