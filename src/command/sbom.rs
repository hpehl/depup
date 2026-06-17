//! The `sbom` subcommand: generates a CycloneDX 1.5 SBOM from discovered dependencies.
//!
//! Reuses the check pipeline to discover all dependencies and their versions,
//! then outputs a CycloneDX 1.5 JSON Bill of Materials.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::ArgMatches;

use crate::model::{CheckResult, CommandResult, DependencyKind};
use crate::sbom;

/// Returns exit code 0 on success.
pub async fn sbom(matches: &ArgMatches) -> Result<u8> {
    let setup = super::pipeline::CommandSetup::from_matches(matches);
    let output_path: Option<PathBuf> = matches.get_one::<String>("output").map(PathBuf::from);

    let pipeline = super::pipeline::resolve_versions(&setup.resolve_config()).await?;

    if pipeline.results.is_empty() {
        super::pipeline::print_empty(setup.json, "No supported project found.");
        return Ok(0);
    }

    // Filter to real dependencies (exclude tool versions — they aren't registry packages)
    let filtered: Vec<CheckResult> = pipeline
        .results
        .into_iter()
        .filter(|r| r.kind() != DependencyKind::Tool && setup.filter.matches(r))
        .collect();

    let bom = sbom::build_bom(&filtered);
    let json = serde_json::to_string_pretty(&bom)?;

    if let Some(path) = output_path {
        fs::write(&path, &json)?;
        if !setup.json {
            println!(
                "SBOM written to {} ({} components)",
                path.display(),
                filtered.len()
            );
        }
    } else {
        println!("{json}");
    }

    Ok(0)
}
