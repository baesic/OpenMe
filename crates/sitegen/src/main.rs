use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1).collect());
    let report = sitegen::build_site(&args.config)?;
    println!(
        "sitegen: rendered {} posts into {}",
        report.posts_rendered,
        report.output_dir.display()
    );
    Ok(())
}

struct Args {
    config: PathBuf,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut config = PathBuf::from("config/site.toml");
        let mut index = 0;
        while index < args.len() {
            if args[index] == "--config" {
                if let Some(value) = args.get(index + 1) {
                    config = PathBuf::from(value);
                    index += 1;
                }
            }
            index += 1;
        }
        Self { config }
    }
}
