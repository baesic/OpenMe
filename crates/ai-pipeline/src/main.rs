use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1).collect());
    let reports = ai_pipeline::run_pipeline(
        &args.concepts,
        &args.output,
        &args.providers,
        &args.rubric,
        &args.site_config,
    )?;
    println!("ai-pipeline: generated {} post(s)", reports.len());
    for report in reports {
        println!("ai-pipeline: {} -> {}", report.concept, report.output);
    }
    Ok(())
}

struct Args {
    concepts: PathBuf,
    output: PathBuf,
    providers: PathBuf,
    rubric: PathBuf,
    site_config: PathBuf,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut parsed = Self {
            concepts: PathBuf::from("concepts"),
            output: PathBuf::from("content/posts"),
            providers: PathBuf::from("config/ai-providers.toml"),
            rubric: PathBuf::from("config/evaluation-rubric.toml"),
            site_config: PathBuf::from("config/site.toml"),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--concepts" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.concepts = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--output" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.output = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--providers" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.providers = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--rubric" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.rubric = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--site-config" | "--config" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.site_config = PathBuf::from(value);
                        index += 1;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        parsed
    }
}
