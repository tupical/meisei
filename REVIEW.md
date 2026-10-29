# REVIEW.md — what matters when reviewing meisei

Only what is specific to this repository. The review procedure (branches,
merge order, the `REVIEW_RESULT` marker, round cap, PRE-EXISTING, acceptance
criteria, the two operating modes invariant, the real-scenario requirement,
checkout ownership) lives in the reviewer prompt
`~/.hermes/scripts/mcpbox_review.sh` and is not repeated here.

This repo is the umbrella: `meisei-pipeline`, a deterministic lineage fixture
(`src/lib.rs`, `src/adapters.rs`, the `meisei` CLI in `src/bin/meisei.rs`,
`tests/lineage.rs`) plus the layers pinned as git submodules. It owns no
business logic and makes no AI or network calls.

## What "broken" means here

Collected from real fixes and review stops of this project, not invented. A
blocker is a defect this diff introduces from one of these classes, or an
acceptance criterion with no line in the diff.

1. **The fixture claims maturity.** The whole point of `a42ba01` (ADR-0011):
   the wrapper promoted a `Text`→`Hypothesis` to a `Decision`, synthesized
   `PlanBrief`/`ActionPacket` fields and the README called it a maturity run.
   Every placeholder written by `fixture_brief`/`fixture_packet` must start
   with `[fixture] ` (asserted in `tests/lineage.rs`); README, CLI banner and
   doc comments must keep saying "lineage fixture, not a maturity run".
   A new field filled without the prefix, a placeholder used as a fallback for
   missing AI output, or wording that sells the fixture as evidence — blocker.
2. **A layer rule is copied into the wrapper.** The wrapper only calls what the
   layers export. `34af421` removed the wrapper's private Insight-only copy of
   the decision-worthy rule in favour of `enma::is_decision_worthy`; two
   predicates had drifted. A new `match` on layer kinds, a re-implemented
   validation, or a gate the layer already has — blocker; the fix belongs in
   the layer repo, followed by a submodule bump here.
3. **Lineage is cut at a hop.** Each object carries its source id:
   `SensingLink.target` → `Decision.links[Sensemaking]` →
   `PlanBrief.decisions_made` → `ActionPacket.linked_decisions` →
   `NewPlan.source_brief`. `project_from_packet` and `lower_new_plan` must
   carry all three `linked_*` lists. Dropping or renaming one, or a test that
   checks a hop by something other than the id, is a blocker.
4. **The pins do not build from a fresh clone.** Cargo resolves the layers
   only through the submodules (`path = "torii"` …), and enma, fujin, satori
   and torii find `layer-kit` as `../layer-kit` — inside this checkout that is
   the `layer-kit/` submodule, not the sibling clone in
   `~/projects/meisei/layer-kit`. A bump is correct only if the new gitlink
   commit is on the layer's default branch on origin (torii, satori,
   yatagarasu, daruma: `main`; enma, fujin, layer-kit: `master`) and a layer
   bump that needs newer layer-kit bumps `layer-kit/` in the same diff. Check:
   `git ls-tree HEAD <layer>` and `git -C ~/projects/meisei/<layer>
   merge-base --is-ancestor <sha> origin/<default>`. Green in the sibling
   clone does not prove the umbrella builds.
5. **Something leaks into a public repo.** This repo and all layers are
   public. A tracked `.omc/`, `.serena/` or other agent state, home paths,
   tokens or internal ids (the class behind the torii/enma/fujin
   `hud-stdin-cache.json` finding) — blocker. A new link from public text
   to the private `tupical/meisei.ru` — blocker; the two existing ADR-0011
   links in `README.md` and `src/adapters.rs` are a known open finding, not
   this diff's.
6. **The README lies.** The Quickstart output block must match what
   `cargo run --quiet -- fixture …` prints (the banner line is asserted
   literally in `cli_labels_fixture_and_refuses_text_hypothesis`); the exit
   code of `--text` is 1; the layer table names the right repo and artifact.

## Not a blocker

- Style, naming, "could be cleaner" — at most **3 remarks** per pass, in one
  comment after the blockers. Drop the rest.
- A defect inside a layer's own code reached through a submodule bump: review
  it in that layer's task and repo; here check only the pin, the build and
  the lineage tests.
- `cargo fmt` drift in files the diff does not touch.

## Paths to skip

- `torii/` `satori/` `enma/` `yatagarasu/` `fujin/` `daruma/` `layer-kit/` —
  submodule contents. Review only the gitlink change (class 4).
- `target/` — build output. `Cargo.lock` — only check that it matches a
  `Cargo.toml` change or a submodule bump in the same diff.
- `.omc/`, `.serena/` — local agent state, never part of a diff here. Their
  presence in `git status` is a checkout-ownership question for the reviewer
  prompt; tracked, they are class 5.

## Checks

`cargo clippy --all-targets -- -D warnings` and `cargo test --no-fail-fast`
(four tests in `tests/lineage.rs`), from a checkout with
`git submodule update --init --recursive`. CI (`.github/workflows/rust.yml`)
runs the same on push to `main` and on PRs touching `.rs`, `Cargo.*`, the
workflow itself or a gitlink.

## Evidence

Every blocker carries `path:line` on the branch under review (for a pin:
the gitlink line from `git ls-tree` and the ancestry command that fails), a
reproduction and the criterion it breaks. A guess from a function or file
name is not a blocker: open the code and find the line. Without `path:line`
the remark goes to comments, not blockers.

## Summary

The second line of the verdict (right after `REVIEW ACCEPT`/`REVIEW REJECT`)
starts with a count: `Blockers: N`. With none, say so: `Blockers: 0 — nothing
to fix.` Then the blockers in order, then at most three remarks.
