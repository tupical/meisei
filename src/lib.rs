//! Meisei 明晰 — a deterministic lineage fixture, not a maturity run or
//! evidence of maturity. It proves only cross-layer id connectivity.
//!
//! Every layer is its own crate with no dependency on its siblings; the
//! adapters live here, in the wrapper. [`run_lineage_fixture`] threads one object's output
//! into the next, and each produced object carries the *id* of its source,
//! so the whole chain can be walked backwards — from the final Daruma
//! [`NewPlan`] to the originating [`RawItem`]:
//!
//! ```text
//!   RawItem ──id──▶ SensingLink.target               (intake → sensemaking)
//!   SensingItem ──id──▶ Decision.links[Sensemaking]  (sensemaking → decisions)
//!   Decision ──id──▶ PlanBrief.decisions_made        (decisions → planning)
//!                ──id──▶ ActionPacket.linked_decisions (planning → actions)
//!                      ──▶ NewPlan.source_brief        (actions → daruma)
//! ```
//!
//! Filled brief/packet fields are fixture placeholders, not planner decisions.
//! A real host obtains these fields from the layers' AI operations and stops
//! when they are unavailable; it must not fabricate missing fields.

pub mod adapters;

use daruma_domain::{Actor, NewPlan};
use daruma_shared::time;
use enma::Decision;
use fujin::{
    assess_with, ActionPacket, FujinStrictness, Gate, HandoffProject, Maturity, TaskCandidate,
};
use satori::types::SensingItem;
use torii::raw_item::RawItem;
use yatagarasu::PlanBrief;

/// Everything the lineage fixture produced, kept together so the lineage can be
/// walked from the final [`NewPlan`] all the way back to the originating
/// [`RawItem`]. The intermediate objects *are* the lineage: the back-links
/// they carry are opaque ids, so recovering the source object requires
/// holding the object it points at.
pub struct PipelineRun {
    /// The raw intake scrap that seeded the run.
    pub raw: RawItem,
    /// Sensing item derived from `raw`.
    pub sensing: SensingItem,
    /// The fixture decision from `sensing` (carries `sensing.id` in `links`).
    pub decision: Decision,
    /// The plan brief built from `decision` (carries `decision.id` in
    /// `decisions_made`).
    pub brief: PlanBrief,
    /// The fixture action packet (carries `decision.id` in `linked_decisions`).
    pub packet: ActionPacket,
    /// The §16 handoff target lowered for Daruma (`source_brief` carries the
    /// packet's provenance links).
    pub plan: NewPlan,
}

/// A refusal or validation failure in the lineage fixture.
#[derive(Debug)]
pub enum PipelineError {
    /// The sensing item is not decision-worthy (`Hypothesis`, `Question`, …) and
    /// so yields no `Decision`.
    NotDecisionWorthy,
    /// An artifact failed a maturity check on the way.
    Immature(String),
}

/// Build a deterministic lineage fixture from intake to a Daruma NewPlan.
///
/// Threads each adapter's output into the next, preserving provenance at
/// every hop. Returns the whole [`PipelineRun`] so the caller can verify
/// lineage end to end. This is not a maturity run or evidence of maturity:
/// filled brief/packet fields are fixture placeholders, not planner decisions.
/// Real hosts obtain them from layer AI operations and stop if unavailable,
/// rather than fabricating them. `actor` is recorded on the fixture decision.
pub fn run_lineage_fixture(raw: RawItem, actor: Actor) -> Result<PipelineRun, PipelineError> {
    // 1. intake → sensemaking
    let (sensing, _sensing_link) = adapters::sensing_from_raw(&raw);

    // 2. sensemaking → decisions (decision-worthy sensing only)
    let new_decision = adapters::decision_from_sensing(&sensing, &actor)
        .ok_or(PipelineError::NotDecisionWorthy)?;
    let decision = new_decision
        .into_decision(time::now())
        .map_err(|e| PipelineError::Immature(format!("decision: {e:?}")))?;

    // 3. decisions → planning
    let brief = fixture_brief(adapters::brief_from_decisions(std::slice::from_ref(
        &decision,
    )));

    // 4. planning → actions
    let packet = fixture_packet(fujin::packet_from_brief(
        &serde_json::to_value(&brief).expect("PlanBrief serializes"),
    ));

    // 5. Exercise the §13 gate (soft provenance, as before) and lower onto
    // Daruma's intake contract. Passing with fixture placeholders proves
    // connectivity, not maturity.
    if let Maturity::NotReady { missing } = assess_with(&packet, FujinStrictness::Soft) {
        return Err(PipelineError::Immature(format!(
            "action packet is not mature; missing §13 fields: {}",
            missing.join(", ")
        )));
    }
    let project = project_from_packet(&packet);
    let plan = adapters::lower_new_plan(&project, Actor::User);

    Ok(PipelineRun {
        raw,
        sensing,
        decision,
        brief,
        packet,
        plan,
    })
}

/// Fill §15 fields with marked fixture placeholders, preserving `decisions_made`.
/// These are not planner decisions or evidence of maturity. A real host obtains
/// them from the planning layer's AI operation and stops if unavailable.
fn fixture_brief(mut brief: PlanBrief) -> PlanBrief {
    if brief.goal.is_empty() {
        brief.goal = "[fixture] Trace cross-layer ids".into();
    }
    brief.in_scope = vec!["[fixture] Wire the five OSS layers end to end".into()];
    brief.completion_criteria = vec!["[fixture] cargo test green; lineage roundtrips".into()];
    brief.daruma_target = "[fixture] meisei lineage plan".into();
    brief.why_now = Some("[fixture] Prove Meisei object lineage end to end".into());
    brief.risks = vec!["[fixture] Provenance lost at a layer boundary".into()];
    brief.constraints = vec!["[fixture] No new business logic in the wrapper".into()];
    brief.knowledge_base = vec!["[fixture] Meisei maturity pipeline canon".into()];
    brief.rejected_alternatives = vec!["[fixture] Mocking the chain instead of running it".into()];
    brief.out_of_scope = vec!["[fixture] Persisting to a real Daruma store".into()];
    brief.dependencies = vec!["[fixture] intake/sensemaking/decisions/planning/actions OSS".into()];
    brief.required_artifacts = vec!["[fixture] lineage test".into()];
    brief
}

/// Marked fixture placeholders for the execution-only §13 fields
/// (`expected_artifacts`, `target_files.owned`, `before_start`,
/// `before_complete`), which have no §15 source. Like [`fixture_brief`], these are not planner decisions or
/// evidence of maturity; a real host stops without the layer's AI output.
fn fixture_packet(mut packet: ActionPacket) -> ActionPacket {
    packet.expected_artifacts = vec!["[fixture] NewPlan lowered for Daruma".into()];
    packet.target_files.owned = vec!["[fixture] src/lib.rs".into()];
    packet.before_start = vec![Gate {
        rule: "[fixture] brief is ready".into(),
    }];
    packet.before_complete = vec![Gate {
        rule: "[fixture] cargo test green".into(),
    }];
    packet
}

/// Shape one fixture Daruma project from a packet (title/goal/criteria/tasks).
fn project_from_packet(packet: &ActionPacket) -> HandoffProject {
    HandoffProject {
        project_title: "[fixture] Meisei lineage".into(),
        plan_title: packet.goal.clone(),
        goal: packet.goal.clone(),
        success_criteria: packet.completion_criteria.clone(),
        tasks: vec![TaskCandidate {
            title: "[fixture] Run the lineage test".into(),
            description: packet.why.clone(),
        }],
        // §13 provenance rides along: dropping it here would sever lineage at
        // the actions→execution boundary this crate exists to prove.
        linked_decisions: packet.linked_decisions.clone(),
        linked_knowledge: packet.linked_knowledge.clone(),
        linked_rejected: packet.linked_rejected.clone(),
    }
}
