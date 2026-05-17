use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1).collect());
    let report = image_brief::process_posts(&args.posts, &args.images)?;
    println!(
        "image-brief: scanned {}, updated {} post(s)",
        report.scanned, report.updated
    );
    for output in report.outputs {
        println!("image-brief: {output}");
    }
    Ok(())
}

struct Args {
    posts: PathBuf,
    images: PathBuf,
}

impl Args {
    fn parse(args: Vec<String>) -> Self {
        let mut parsed = Self {
            posts: PathBuf::from("content/posts"),
            images: PathBuf::from("static/images/posts"),
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--posts" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.posts = PathBuf::from(value);
                        index += 1;
                    }
                }
                "--images" => {
                    if let Some(value) = args.get(index + 1) {
                        parsed.images = PathBuf::from(value);
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
