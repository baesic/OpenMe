use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1).collect());
    let report = doc_ingest::ingest_documents(&args.config, &args.docs, &args.docx)?;
    println!(
        "doc-ingest: scanned {}, wrote {} files",
        report.scanned, report.written
    );
    for output in report.outputs {
        println!("doc-ingest: {output}");
    }
    Ok(())
}

struct Args {
    config: PathBuf,
    docs: PathBuf,
    docx: PathBuf,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut parsed = Self {
            config: PathBuf::from("config/site.toml"),
            docs: PathBuf::from("imports/docs"),
            docx: PathBuf::from("imports/docx"),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--config" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.config = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--docs" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.docs = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--docx" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.docx = PathBuf::from(value);
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
