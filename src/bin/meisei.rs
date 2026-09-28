//! CLI: demonstrate cross-layer ids with deterministic fixture placeholders.
//! This is not a maturity run or evidence of maturity.

use daruma_domain::Actor;
use enma::Link;
use meisei_pipeline::PipelineError;
use torii::raw_item::{NewRawItem, RawItemKind};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (text, kind) = match args.as_slice() {
        [cmd, text] if cmd == "fixture" && !text.is_empty() && text != "--text" => {
            (text.clone(), RawItemKind::Event)
        }
        [cmd, flag, text] if cmd == "fixture" && flag == "--text" && !text.is_empty() => {
            (text.clone(), RawItemKind::Text)
        }
        _ => {
            eprintln!("usage: meisei fixture [--text] \"<raw text>\"");
            std::process::exit(2);
        }
    };

    println!("meisei — deterministic lineage fixture (not a maturity run: brief/packet fields are fixture placeholders)\n");
    let raw = NewRawItem::new("cli://meisei", kind, text).build();
    let run = match meisei_pipeline::run_lineage_fixture(raw, Actor::user()) {
        Ok(run) => run,
        Err(PipelineError::NotDecisionWorthy) => {
            eprintln!("fixture stopped: ADR-0011 — input is not decision-worthy. No Decision, PlanBrief, ActionPacket or NewPlan was created.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("fixture stopped: {e:?}");
            std::process::exit(1);
        }
    };

    let sensing_ref = run
        .decision
        .links
        .iter()
        .find_map(|l| match l {
            Link::Sensemaking { reference } => Some(reference.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let decision_refs: Vec<&str> = run
        .packet
        .linked_decisions
        .iter()
        .map(|l| l.id.as_str())
        .collect();

    println!(
        "  1 intake       {}  \"{}\"",
        run.raw.id,
        clip(&run.raw.body)
    );
    println!(
        "  2 sensemaking  {}  {:?} <- raw {}",
        run.sensing.id, run.sensing.kind, run.raw.id
    );
    println!(
        "  3 decisions    {}  <- sensing {}",
        run.decision.id, sensing_ref
    );
    println!(
        "  4 planning     brief \"{}\" <- decision {}",
        clip(&run.brief.goal),
        run.decision.id
    );
    println!(
        "  5 actions      packet \"{}\" <- decision {}",
        clip(&run.packet.goal),
        decision_refs.join(", ")
    );
    println!(
        "  6 execution    plan \"{}\" -> daruma NewPlan",
        clip(&run.plan.title)
    );
    println!(
        "\nlineage: plan.source_brief carries {}",
        decision_refs.join(", ")
    );
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= 60 {
        s.to_string()
    } else {
        s.chars().take(57).collect::<String>() + "..."
    }
}
