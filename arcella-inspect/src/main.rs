use arcella_inspect::{analyze_project, analysis_to_yaml};
use clap::Parser;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about = "Inspect Rust code and export metadata as YAML", long_about = None)]
struct Args {
    #[arg(default_value = ".")]
    target_dir: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let root = Path::new(&args.target_dir).canonicalize()?;
    let analysis = analyze_project(&root)?;
    let yaml = analysis_to_yaml(&analysis, &root)?;
    print!("{}", yaml);
    Ok(())
}