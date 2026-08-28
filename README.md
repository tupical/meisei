# Meisei 明晰 — all layers, one run

> **Meisei** 明晰 (“clarity”) is an open pipeline that carries raw intent through
> understanding → decision → plan → action to a finished result.

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

The six Meisei layers each live in their own repository and none depends on a
sibling — so nothing *shows* they belong together. This repo is the missing
umbrella: a **minimal wrapper** (`meisei-pipeline`) that ties all five maturity
layers into a single run ending at Daruma, the execution layer.

It owns **no business logic**. It only calls the adapters each layer already
exposes and threads one object's output into the next — the same route a
production host runs over HTTP hops against the deployed layer servers, done
in-process. All six layers plus `layer-kit` are vendored as git submodules in
the root, so the file tree above shows exactly what the pipeline is made of,
and the whole run builds from this checkout alone.

## The maturity route

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

`tests/lineage.rs` runs the whole pipeline through the real adapters and
asserts this walk succeeds.

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
cargo run --quiet -- run "Read latency spiked after the cache eviction change"
```

```text
meisei — one run through the maturity pipeline

  1 intake       ri_…  "Read latency spiked after the cache eviction change"
  2 sensemaking  si_…  Insight <- raw ri_…
  3 decisions    de_…  <- sensing si_…
  4 planning     brief "Ship the lineage-traceable pipeline" <- decision de_…
  5 actions      packet "…" mature <- decision de_…
  6 execution    plan "…" -> daruma NewPlan

lineage: plan.source_brief carries de_…
```

```sh
cargo test          # lineage proof: NewPlan → … → RawItem, all real adapters
```

## How the wrapper works

- `src/adapters.rs` — the only place sibling types meet: pure translations
  between layer types, one per hop.
- `src/lib.rs` — `run(raw, actor)` threads the hops; fujin's §13 maturity
  gate (`to_handoff`) guards the crossing into Daruma: an immature packet is
  refused, never forced.
- Inputs that are not decision-worthy (a reference document, a question) stop
  cleanly at sensemaking — a legitimate pipeline outcome, not an error.
- Without an AI provider key the layers' AI operations answer
  `503 ai_not_configured`; the deterministic path here fills the
  planner-owned brief/packet fields itself (`enrich_brief`/`mature`) so the
  run is reproducible offline.

## Repository layout

- `torii/` `satori/` `enma/` `yatagarasu/` `fujin/` `daruma/` `layer-kit/` —
  the layers as git submodules, pinned to exact commits.
- `src/` — the `meisei-pipeline` library and the `meisei` CLI.
- `tests/` — the end-to-end lineage proof.

Already cloned without `--recurse-submodules`? Run
`git submodule update --init`.

## Docs

Pipeline canon and layer contracts: https://meisei.ru/docs

## License

Apache-2.0 WITH Commons-Clause — see [LICENSE](LICENSE) and
[LICENSE.commons-clause.md](LICENSE.commons-clause.md).
