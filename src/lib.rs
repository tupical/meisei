//! Meisei 明晰 — the minimal wrapper that ties all five maturity layers
//! into a single run.
//!
//! Every layer is its own crate with no dependency on its siblings; the
//! adapters live here, in the host. [`run`] threads one object's output
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
//! This crate writes **no new business logic**: it only orchestrates the
//! adapters the layers already expose. Production hosts (e.g. MCPBox) run
//! the same route over HTTP hops against the deployed layer servers; this
//! is the in-process minimal path.

pub mod adapters;

use enma::Decision;
use fujin::{into_new_plan, to_handoff, ActionPacket, Gate, HandoffProject, TaskCandidate};
use satori::types::SensingItem;
use daruma_domain::{Actor, NewPlan};
use daruma_shared::time;
use torii::raw_item::RawItem;
use yatagarasu::PlanBrief;

/// Everything the pipeline produced, kept together so the lineage can be
/// walked from the final [`NewPlan`] all the way back to the originating
/// [`RawItem`]. The intermediate objects *are* the lineage: the back-links
/// they carry are opaque ids, so recovering the source object requires
/// holding the object it points at.
pub struct PipelineRun {
    /// The raw intake scrap that seeded the run.
    pub raw: RawItem,
    /// Sensing item derived from `raw`.
    pub sensing: SensingItem,
    /// The decision promoted from `sensing` (carries `sensing.id` in `links`).
    pub decision: Decision,
    /// The plan brief built from `decision` (carries `decision.id` in
    /// `decisions_made`).
    pub brief: PlanBrief,
    /// The mature action packet (carries `decision.id` in `linked_decisions`).
    pub packet: ActionPacket,
    /// The §16 handoff target lowered for Daruma (`source_brief` carries the
    /// packet's provenance links).
    pub plan: NewPlan,
}

/// A legitimate outcome of the maturity pipeline, not an orchestration bug.
#[derive(Debug)]
pub enum PipelineError {
    /// The sensing item is not decision-worthy (`Question`, `Risk`, …) and
    /// so yields no `Decision`.
    NotDecisionWorthy,
    /// An artifact failed a maturity check on the way.
    Immature(String),
}

/// Run the full intake → … → Daruma pipeline on a single raw item.
///
/// Threads each adapter's output into the next, preserving provenance at
/// every hop. Returns the whole [`PipelineRun`] so the caller can verify
/// lineage end to end. `actor` is recorded as the decider on the promoted
/// decision.
pub fn run(raw: RawItem, actor: Actor) -> Result<PipelineRun, PipelineError> {
    // 1. intake → sensemaking
    let (sensing, _sensing_link) = adapters::sensing_from_raw(&raw);

    // 2. sensemaking → decisions (decision-worthy sensing only)
    let new_decision = adapters::decision_from_sensing(&sensing, &actor)
        .ok_or(PipelineError::NotDecisionWorthy)?;
    let decision = new_decision
        .into_decision(time::now())
        .map_err(|e| PipelineError::Immature(format!("decision: {e:?}")))?;

    // 3. decisions → planning
    let brief = enrich_brief(adapters::brief_from_decisions(std::slice::from_ref(&decision)));

    // 4. planning → actions
    let packet = mature(fujin::packet_from_brief(
        &serde_json::to_value(&brief).expect("PlanBrief serializes"),
    ));

    // 5. actions → execution: the §13 maturity gate guards the crossing —
    // `to_handoff` refuses an immature packet — then the packet's project
    // lowers onto Daruma's intake contract.
    let project = project_from_packet(&packet);
    to_handoff(&packet, vec![project.clone()])
        .map_err(|e| PipelineError::Immature(e.to_string()))?;
    let lowered = into_new_plan(&project, fujin::ProjectId::new(), fujin::Actor::user());
    let plan = adapters::lower_new_plan(lowered.plan);

    Ok(PipelineRun { raw, sensing, decision, brief, packet, plan })
}

/// Deterministic content for the §15 brief fields that `brief_from_decisions`
/// leaves to the planner, preserving `decisions_made`. A real host asks the
/// planning layer's AI operation (`yatagarasu.plan_ai`); without an AI
/// provider the layer answers `503 ai_not_configured`, so the minimal
/// in-process path fills them deterministically instead.
fn enrich_brief(mut brief: PlanBrief) -> PlanBrief {
    if brief.goal.is_empty() {
        brief.goal = "Ship the lineage-traceable pipeline".into();
    }
    brief.in_scope = vec!["Wire the five OSS layers end to end".into()];
    brief.completion_criteria = vec!["cargo test green; lineage roundtrips".into()];
    brief.daruma_target = "meisei pipeline plan".into();
    brief.why_now = Some("Prove Meisei object lineage end to end".into());
    brief.risks = vec!["Provenance lost at a layer boundary".into()];
    brief.constraints = vec!["No new business logic in the orchestrator".into()];
    brief.knowledge_base = vec!["Meisei maturity pipeline canon".into()];
    brief.rejected_alternatives = vec!["Mocking the chain instead of running it".into()];
    brief.out_of_scope = vec!["Persisting to a real Daruma store".into()];
    brief.dependencies = vec!["intake/sensemaking/decisions/planning/actions OSS".into()];
    brief.required_artifacts = vec!["lineage test".into()];
    brief
}

/// Deterministic content for the three execution-only §13 fields
/// (`expected_artifacts`, `before_start`, `before_complete`), which have no
/// §15 source — same reasoning as [`enrich_brief`].
fn mature(mut packet: ActionPacket) -> ActionPacket {
    packet.expected_artifacts = vec!["NewPlan lowered for Daruma".into()];
    packet.before_start = vec![Gate { rule: "brief is ready".into() }];
    packet.before_complete = vec![Gate { rule: "cargo test green".into() }];
    packet
}

/// Shape one Daruma project from a packet (title/goal/criteria/tasks).
fn project_from_packet(packet: &ActionPacket) -> HandoffProject {
    HandoffProject {
        project_title: "Meisei pipeline".into(),
        plan_title: packet.goal.clone(),
        goal: packet.goal.clone(),
        success_criteria: packet.completion_criteria.clone(),
        tasks: vec![TaskCandidate {
            title: "Run the lineage test".into(),
            description: packet.why.clone(),
        }],
        // §13 provenance rides along: dropping it here would sever lineage at
        // the actions→execution boundary this crate exists to prove.
        linked_decisions: packet.linked_decisions.clone(),
        linked_knowledge: packet.linked_knowledge.clone(),
        linked_rejected: packet.linked_rejected.clone(),
    }
}
