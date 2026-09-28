# Meisei 明晰 — deterministic lineage fixture

> This wrapper demonstrates how ids connect Meisei layer objects. It does not
> demonstrate how decisions are made and is not a maturity run or proof of maturity.

[![License: Apache-2.0 WITH Commons-Clause](https://img.shields.io/badge/license-Apache--2.0%20WITH%20Commons--Clause-blue.svg)](LICENSE)

<sub>
<a href="https://github.com/tupical/torii">torii</a> ·
<a href="https://github.com/tupical/satori">satori</a> ·
<a href="https://github.com/tupical/enma">enma</a> ·
<a href="https://github.com/tupical/yatagarasu">yatagarasu</a> ·
<a href="https://github.com/tupical/fujin">fujin</a> ·
<a href="https://github.com/tupical/daruma">daruma</a>
&nbsp;—&nbsp; intake · sensemaking · decisions · planning · actions · execution (terminal)
</sub>

## What this repo is

`meisei-pipeline` is a **deterministic lineage fixture/demo** connecting the
Meisei layers in-process, down to a Daruma `NewPlan`. It proves cross-layer id
connectivity, not production semantics. The layers and `layer-kit` are pinned
as git submodules so the fixture builds from this checkout.

**Hypothesis/text does not become a Decision** ([ADR-0011](https://github.com/tupical/meisei.ru/blob/main/adr/0011-hypothesis-not-decision-worthy.md)).
A real transition to a decision is a semantically justified enma operation,
not an automatic step. The fixture accepts only `Insight` for its lineage demo.

Filled `PlanBrief`/`ActionPacket` fields are marked `[fixture]` placeholders,
not planner decisions. A real host obtains them from the layers' AI operations;
without that output it stops, rather than synthesizing missing fields.

## The fixture's object chain

```text
  RawItem (torii / intake)
    → SensingItem (satori / sensemaking)
      → Decision (enma / decisions)
        → PlanBrief (yatagarasu / planning)
          → ActionPacket (fujin / actions, §13 maturity gate)
            → NewPlan (daruma / execution)
```

Every produced object carries the *id* of its source, so the chain can be
walked backwards — from the final `NewPlan` to the originating `RawItem`:

```text
  RawItem ──id──▶ SensingLink.target               (intake → sensemaking)
  SensingItem ──id──▶ Decision.links[Sensemaking]  (sensemaking → decisions)
  Decision ──id──▶ PlanBrief.decisions_made        (decisions → planning)
               ──id──▶ ActionPacket.linked_decisions (planning → actions)
                     ──▶ NewPlan.source_brief        (actions → daruma)
```

`tests/lineage.rs` checks this id roundtrip, fixture labels, and refusal of text
hypotheses. It does not verify semantic maturity.

## The layers

| Layer | Repo | Role | Artifact |
| --- | --- | --- | --- |
| **torii** 鳥居 | [tupical/torii](https://github.com/tupical/torii) | intake — the single entry point for raw material | `RawItem` |
| **satori** 悟り | [tupical/satori](https://github.com/tupical/satori) | sensemaking — raw material → understanding | `SensingItem` |
| **enma** 閻魔 | [tupical/enma](https://github.com/tupical/enma) | decisions — understanding → direction | `Decision` |
| **yatagarasu** 八咫烏 | [tupical/yatagarasu](https://github.com/tupical/yatagarasu) | planning — decisions → plans | `PlanBrief` |
| **fujin** 風神 | [tupical/fujin](https://github.com/tupical/fujin) | actions — the maturity boundary before execution | `ActionPacket` |
| **daruma** 達磨 | [tupical/daruma](https://github.com/tupical/daruma) | execution (terminal) — plans driven to *done* | `NewPlan` |

Shared cross-layer infra (HTTP/MCP server scaffold, auth, storage):
[tupical/layer-kit](https://github.com/tupical/layer-kit). Each layer also
ships an independently deployable server (`<layer>-server`) exposing its MCP
surface — that is how production hosts run the pipeline over HTTP hops.

## Quickstart

```sh
git clone --recurse-submodules https://github.com/tupical/meisei
cd meisei
cargo run --quiet -- fixture "Read latency spiked"
```

```text
meisei — deterministic lineage fixture (not a maturity run: brief/packet fields are fixture placeholders)

  1 intake       ri_…  "Read latency spiked"
  2 sensemaking  si_…  Insight <- raw ri_…
  3 decisions    dec_…  <- sensing si_…
  4 planning     brief "[fixture] Trace cross-layer ids" <- decision dec_…
  5 actions      packet "[fixture] Trace cross-layer ids" <- decision dec_…
  6 execution    plan "[fixture] Trace cross-layer ids" -> daruma NewPlan

lineage: plan.source_brief carries dec_…
```

```sh
# Default input kind is Event → Insight. Text → Hypothesis is refused:
cargo run --quiet -- fixture --text "maybe cache is slow"
# exits 1
```

```text
meisei — deterministic lineage fixture (not a maturity run: brief/packet fields are fixture placeholders)

fixture stopped: ADR-0011 — input is not decision-worthy. No Decision, PlanBrief, ActionPacket or NewPlan was created.
```

```sh
cargo test          # lineage roundtrip, fixture labels, and refusal regression
```

## How the wrapper works

- `src/adapters.rs` translates sibling types for the fixture. Only `Insight`
  is eligible; `Hypothesis` and all other kinds yield `NotDecisionWorthy`.
- `src/lib.rs` — `run_lineage_fixture(raw, actor)` threads ids through the
  adapters and fujin's `to_handoff` check into an in-memory Daruma `NewPlan`.
- `fixture_brief`/`fixture_packet` supply marked placeholders solely to
  exercise that wiring offline. Passing checks with these placeholders is
  not evidence of maturity, and they are never a fallback for missing AI output
  in a real host. Nothing is persisted to Daruma.

## Repository layout

- `torii/` `satori/` `enma/` `yatagarasu/` `fujin/` `daruma/` `layer-kit/` —
  the layers as git submodules, pinned to exact commits.
- `src/` — the `meisei-pipeline` library and the `meisei` CLI.
- `tests/` — lineage fixture tests.

Already cloned without `--recurse-submodules`? Run
`git submodule update --init`.

## Docs

Pipeline canon and layer contracts: https://meisei.ru/docs

## License

Apache-2.0 WITH Commons-Clause — see [LICENSE](LICENSE) and
[LICENSE.commons-clause.md](LICENSE.commons-clause.md).
