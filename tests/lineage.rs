//! Prove cross-layer id connectivity from the fixture Daruma `NewPlan` to
//! the originating `RawItem`. Placeholder fields are not maturity evidence.

use daruma_domain::Actor;
use enma::Link;
use meisei_pipeline::{run_lineage_fixture, PipelineError, PipelineRun};
use satori::types::{SensingItemKind, SensingTarget};
use torii::raw_item::{NewRawItem, RawItemKind};

#[test]
fn lineage_traces_newplan_back_to_raw_item() {
    // An `Event` raw item maps to a `SensingItem::Insight`, a sensing kind
    // eligible for a fixture Decision — the spine of the lineage we trace.
    let raw = NewRawItem::new(
        "webhook://gh/push",
        RawItemKind::Event,
        "Read latency spiked after the cache eviction change",
    )
    .build();
    let raw_id = raw.id.to_string();

    let PipelineRun {
        raw,
        sensing,
        decision,
        brief,
        packet,
        plan,
    } = run_lineage_fixture(raw, Actor::user())
        .expect("Event/Insight must produce a lineage fixture");

    assert_eq!(sensing.kind, SensingItemKind::Insight);
    for field in [&brief.goal, &brief.daruma_target, &packet.goal, &plan.title]
        .into_iter()
        .chain(&brief.why_now)
        .chain(&brief.in_scope)
        .chain(&brief.completion_criteria)
        .chain(&brief.risks)
        .chain(&brief.constraints)
        .chain(&brief.knowledge_base)
        .chain(&brief.rejected_alternatives)
        .chain(&brief.out_of_scope)
        .chain(&brief.dependencies)
        .chain(&brief.required_artifacts)
        .chain(&packet.expected_artifacts)
        .chain(packet.before_start.iter().map(|gate| &gate.rule))
        .chain(packet.before_complete.iter().map(|gate| &gate.rule))
    {
        assert!(
            field.starts_with("[fixture] "),
            "unmarked placeholder: {field}"
        );
    }
    assert_eq!(raw.id.to_string(), raw_id, "the seed RawItem is preserved");

    // Hop 1 — NewPlan / ActionPacket → Decision.
    assert!(
        packet
            .linked_decisions
            .iter()
            .any(|l| l.id == decision.id.to_string()),
        "ActionPacket.linked_decisions must contain the source Decision id"
    );
    assert!(
        brief
            .decisions_made
            .iter()
            .any(|id| *id == decision.id.to_string()),
        "PlanBrief.decisions_made must carry the Decision id"
    );
    assert!(
        plan.source_brief
            .as_deref()
            .is_some_and(|b| b.contains(&decision.id.to_string())),
        "NewPlan.source_brief must carry the Decision id across the boundary"
    );

    // Hop 2 — Decision → SensingItem.
    let recovered_si_id = decision
        .links
        .iter()
        .find_map(|l| match l {
            Link::Sensemaking { reference } => Some(reference.clone()),
            _ => None,
        })
        .expect("Decision.links must hold a Sensemaking provenance link");
    assert_eq!(recovered_si_id, sensing.id.to_string());

    // Hop 3 — SensingItem → RawItem (the SensingLink produced alongside the
    // sensing item targets the originating RawItem).
    let (_re_sensing, sensing_link) = meisei_pipeline::adapters::sensing_from_raw(&raw);
    let recovered_raw_id = match sensing_link.target {
        SensingTarget::RawItem { id } => id,
        other => panic!("expected RawItem provenance target, got {other:?}"),
    };
    assert_eq!(recovered_raw_id, raw_id);
}

#[test]
fn text_hypothesis_cannot_produce_decision_or_downstream_artifacts() {
    let raw = NewRawItem::new("cli://test", RawItemKind::Text, "maybe cache is slow").build();
    let (sensing, _) = meisei_pipeline::adapters::sensing_from_raw(&raw);
    assert_eq!(sensing.kind, SensingItemKind::Hypothesis);
    assert!(meisei_pipeline::adapters::decision_from_sensing(&sensing, &Actor::user()).is_none());
    // Refusal returns no PipelineRun, hence no Decision/PlanBrief/ActionPacket/NewPlan.
    assert!(matches!(
        run_lineage_fixture(raw, Actor::user()),
        Err(PipelineError::NotDecisionWorthy)
    ));
}

#[test]
fn cli_labels_fixture_and_refuses_text_hypothesis() {
    let success = std::process::Command::new(env!("CARGO_BIN_EXE_meisei"))
        .args(["fixture", "Read latency spiked"])
        .output()
        .unwrap();
    assert!(success.status.success());
    let stdout = String::from_utf8(success.stdout).unwrap();
    assert_eq!(stdout.lines().next(), Some("meisei — deterministic lineage fixture (not a maturity run: brief/packet fields are fixture placeholders)"));
    assert!(stdout.contains("[fixture] "));

    let refusal = std::process::Command::new(env!("CARGO_BIN_EXE_meisei"))
        .args(["fixture", "--text", "maybe cache is slow"])
        .output()
        .unwrap();
    assert_eq!(refusal.status.code(), Some(1));
    let stderr = String::from_utf8(refusal.stderr).unwrap();
    assert!(stderr.contains("ADR-0011"));
    assert!(stderr.contains("input is not decision-worthy"));
    assert!(!String::from_utf8(refusal.stdout)
        .unwrap()
        .contains("  3 decisions"));
}

#[test]
fn non_decision_worthy_input_stops_at_sensemaking() {
    // A `Document` maps to `Knowledge`, which is not decision-worthy: the
    // pipeline stops cleanly instead of forcing a decision out of it.
    let raw = NewRawItem::new(
        "cli://test",
        RawItemKind::Document,
        "API reference for the storage layer",
    )
    .build();

    match run_lineage_fixture(raw, Actor::user()) {
        Err(meisei_pipeline::PipelineError::NotDecisionWorthy) => {}
        Err(e) => panic!("expected NotDecisionWorthy, got {e:?}"),
        Ok(_) => panic!("knowledge must not promote to a decision"),
    }
}
