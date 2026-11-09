# Convert.Undeploy

An infantryman that finishes its Undeploy sequence can convert to another
type once per sequence.

```ini
[A]                                ; InfantryType
Convert.Undeploy=                  ; Valueable<TechnoTypeClass*>, absent/none = off
```

Reader `Ext/TechnoType/Body.cpp:1014`, declaration
`Ext/TechnoType/Body.h:249`, constructor null at `:681`.

## Behavior

`InfantryClass_DoingAI_DeployConvert_Undeploy @ 0x520B99`
(`Ext/Infantry/Hooks.cpp:36-50`), in the DoingAI Undeploy completion case:
when the type sets `Convert.Undeploy` and `HasUndeployConverted` is false,
set the latch, clear `HasDeployConverted`, and call
`TechnoExt::ConvertToType`. The latch is reset by
`InfantryClass_DoingAI_DeployConvert_ResetFlags @ 0x520E75`
(`:52-66`) whenever the current `SequenceAnim` is neither Deploy nor
Undeploy, so the next Undeploy converts again.

## Where VERA20K owns it

`rules::phobos_convert` reads the key. `Simulation::infantry_deploy_completion`
(`sim/deploy.rs`), the production owner the infantry sequencer calls with
`completed_doing == 31` for the Undeploy completion, runs the conversion
through the single `convert_techno_type` owner before the completion's
Ready `Do_Action`, as the native hook does. The `HasUndeployConverted`
latch is a new phobos-gated `GameEntity` field (serde default, hashed only
when set). `infantry_sequencer` (`sim/movement/infantry_action.rs`) runs the
`0x520E75` reset after its action dispatch whenever the current sequence is
not Deploy (27) or Undeploy (31).

RESIDUAL: the deploy-side twin `InfantryClass_DoingAI_DeployConvert_Deploy
@ 0x520B3E` reads `Convert.Deploy`, the Ares key VERA already consumes for
Unit simple deployers. Porting the infantry arm would need that key under
`phobos` builds without Ares; it is a separate chain, so VERA stores only
the undeploy latch (the deploy latch would gate nothing).

RESIDUAL: native sets `HasUndeployConverted = false` inside the deploy arm;
without that arm the latch is only reset by the `0x520E75` rule, which
clears it on the first non-Deploy/Undeploy visit anyway.
