//! Run the whole Meisei maturity pipeline for real and prove the object
//! lineage can be walked backwards from the Daruma `NewPlan` all the way to
//! the originating `RawItem`. Nothing here is mocked — every hop goes
//! through the layer's own adapter.

use daruma_domain::Actor;
use enma::Link;
use meisei_pipeline::{run, PipelineRun};
use satori::types::SensingTarget;
use torii::raw_item::{NewRawItem, RawItemKind};

#[test]
fn lineage_traces_newplan_back_to_raw_item() {
    // An `Event` raw item maps to a `SensingItem::Insight`, a sensing kind
    // that promotes to a *Decision* — the spine of the lineage we trace.
    let raw = NewRawItem::new(
        "webhook://gh/push",
        RawItemKind::Event,
        "Read latency spiked after the cache eviction change",
    )
    .build();
    let raw_id = raw.id.to_string();

    let PipelineRun { raw, sensing, decision, brief, packet, plan } =
        run(raw, Actor::user()).expect("the Event/Insight item must mature into a plan");

    assert!(!plan.title.is_empty(), "pipeline must lower a real NewPlan");
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
fn non_decision_worthy_input_stops_at_sensemaking() {
    // A `Document` maps to `Knowledge`, which is not decision-worthy: the
    // pipeline stops cleanly instead of forcing a decision out of it.
    let raw = NewRawItem::new(
        "cli://test",
        RawItemKind::Document,
        "API reference for the storage layer",
    )
    .build();

    match run(raw, Actor::user()) {
        Err(meisei_pipeline::PipelineError::NotDecisionWorthy) => {}
        Err(e) => panic!("expected NotDecisionWorthy, got {e:?}"),
        Ok(_) => panic!("knowledge must not promote to a decision"),
    }
}
