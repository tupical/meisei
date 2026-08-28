//! The hop adapters: pure translations between sibling-layer types.
//! No layer depends on another; these functions are the only place the
//! types meet.

use enma::{Actor as DecActor, ActorKind as DecActorKind, Alternative, Link, NewDecision};
use satori::types::{LinkKind, SensingItem, SensingItemKind, SensingLink, SensingTarget, Source};
use torii::raw_item::{RawItem, RawItemKind};
use yatagarasu::PlanBrief;
use daruma_domain::{Actor as TaActor, NewPlan as TaNewPlan};

/// Hop 1: intake RawItem → sensemaking SensingItem (+ provenance link).
pub fn sensing_from_raw(raw: &RawItem) -> (SensingItem, SensingLink) {
    let kind = sensing_kind_for(raw.kind);
    let item = SensingItem::new(kind, raw.body.clone())
        .with_source(Source::External { ref_: raw.source.clone() });
    let link = SensingLink::new(
        item.id,
        SensingTarget::RawItem { id: raw.id.to_string() },
        LinkKind::DerivedFrom,
    );
    (item, link)
}

fn sensing_kind_for(kind: RawItemKind) -> SensingItemKind {
    match kind {
        RawItemKind::Event     => SensingItemKind::Insight,
        RawItemKind::Text      => SensingItemKind::Hypothesis,
        RawItemKind::Document  => SensingItemKind::Knowledge,
        RawItemKind::Reference => SensingItemKind::Knowledge,
        RawItemKind::Binary    => SensingItemKind::ResearchGap,
    }
}

/// Hop 2: sensemaking SensingItem → decisions NewDecision (decision-worthy only).
pub fn decision_from_sensing(sensing: &SensingItem, actor: &TaActor) -> Option<NewDecision> {
    if !is_decision_worthy(sensing.kind) {
        return None;
    }
    Some(NewDecision {
        id: None,
        statement: sensing.body.clone(),
        decided_by: dec_actor_from(actor),
        decided_at: None,
        rationale: format!("Promoted from sensing item {}", sensing.id),
        alternatives: Vec::<Alternative>::new(),
        consequences: Vec::new(),
        revisit_when: String::new(),
        links: vec![Link::Sensemaking { reference: sensing.id.to_string() }],
    })
}

fn is_decision_worthy(kind: SensingItemKind) -> bool {
    matches!(kind, SensingItemKind::Insight | SensingItemKind::Hypothesis)
}

fn dec_actor_from(a: &TaActor) -> DecActor {
    match a {
        TaActor::User => DecActor { kind: DecActorKind::User, id: "user".into() },
        TaActor::Agent { name, .. } => DecActor { kind: DecActorKind::Agent, id: name.clone() },
    }
}

/// Hop 3: decisions Decision → planning PlanBrief (provenance only;
/// `enrich_brief` in lib.rs fills the planner-owned fields).
pub fn brief_from_decisions(decisions: &[enma::Decision]) -> PlanBrief {
    PlanBrief {
        decisions_made: decisions.iter().map(|d| d.id.to_string()).collect(),
        ..PlanBrief::default()
    }
}

/// §16 bridge: actions NewPlan → daruma NewPlan. The packet's provenance
/// links ride into `source_brief`, so the daruma-side plan still points at
/// the decision it came from.
pub fn lower_new_plan(src: fujin::NewPlan) -> TaNewPlan {
    let source_brief = serde_json::to_string(&serde_json::json!({
        "linked_decisions": src.linked_decisions,
        "linked_knowledge": src.linked_knowledge,
        "linked_rejected": src.linked_rejected,
    }))
    .ok();
    TaNewPlan {
        project_id: daruma_shared::ProjectId::new(),
        title: src.title,
        owner: ta_actor_owner(&src.owner),
        description: None,
        goal: src.goal,
        success_criteria: src.success_criteria,
        parent_plan_id: None,
        source_brief,
    }
}

fn ta_actor_owner(a: &fujin::Actor) -> TaActor {
    match a.kind {
        fujin::ActorKind::User  => TaActor::User,
        fujin::ActorKind::Agent => TaActor::agent(a.id.clone()),
    }
}
