//! CLI: run one raw idea through the whole maturity pipeline and print the
//! lineage chain.

use daruma_domain::Actor;
use enma::Link;
use torii::raw_item::{NewRawItem, RawItemKind};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = match args.as_slice() {
        [cmd, text] if cmd == "run" && !text.is_empty() => text.clone(),
        _ => {
            eprintln!("usage: meisei run \"<raw text>\"");
            std::process::exit(2);
        }
    };

    let raw = NewRawItem::new("cli://meisei", RawItemKind::Event, text).build();
    let run = match meisei_pipeline::run(raw, Actor::user()) {
        Ok(run) => run,
        Err(e) => {
            eprintln!("pipeline stopped: {e:?}");
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

    println!("meisei — one run through the maturity pipeline\n");
    println!("  1 intake       {}  \"{}\"", run.raw.id, clip(&run.raw.body));
    println!("  2 sensemaking  {}  {:?} <- raw {}", run.sensing.id, run.sensing.kind, run.raw.id);
    println!("  3 decisions    {}  <- sensing {}", run.decision.id, sensing_ref);
    println!("  4 planning     brief \"{}\" <- decision {}", clip(&run.brief.goal), run.decision.id);
    println!("  5 actions      packet \"{}\" mature <- decision {}", clip(&run.packet.goal), decision_refs.join(", "));
    println!("  6 execution    plan \"{}\" -> daruma NewPlan", clip(&run.plan.title));
    println!("\nlineage: plan.source_brief carries {}", decision_refs.join(", "));
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= 60 {
        s.to_string()
    } else {
        s.chars().take(57).collect::<String>() + "..."
    }
}
