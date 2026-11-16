//! Standalone native building authoring application.
mod bundle;
mod catalogue;
mod encounters;
mod export;
mod interiors;
mod model;
mod navigation;
mod preview;
mod retrofit;
mod server;
mod shell;
mod stairs;
mod technical;
mod terrain;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ghostline_renderer::GameSource;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Published items package. Also configurable with GHOSTLINE_ITEMS_CATALOGUE.
    #[arg(long, global = true)]
    catalogue: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args)]
struct OptionalNative {
    #[arg(long)]
    game: Option<PathBuf>,
    #[arg(long, requires = "game")]
    schema: Option<PathBuf>,
}
impl OptionalNative {
    fn source(&self) -> Result<Option<GameSource>> {
        self.game
            .as_ref()
            .map(|game| GameSource::open(game, self.schema.as_deref()).map_err(Into::into))
            .transpose()
    }
}

#[derive(Subcommand)]
enum Command {
    /// Write native navigation tiles and metadata for independent inspection.
    Navigation {
        project: PathBuf,
        #[command(flatten)]
        native: OptionalNative,
        #[arg(long)]
        out: PathBuf,
    },
    /// Query generated Human navigation between local XYZ endpoints.
    Route {
        project: PathBuf,
        #[command(flatten)]
        native: OptionalNative,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        start: Vec<f32>,
        #[arg(long, num_args = 3, allow_hyphen_values = true)]
        end: Vec<f32>,
    },
    /// Inspect exact native material bindings, UV ranges and prepared maps.
    Inspect {
        project: PathBuf,
        #[arg(long)]
        game: PathBuf,
        #[arg(long)]
        schema: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the local browser editor.
    Serve {
        #[arg(long)]
        game: PathBuf,
        #[arg(long)]
        schema: Option<PathBuf>,
        #[arg(long, default_value_t = 8792)]
        port: u16,
        #[arg(long, default_value = "work")]
        work: PathBuf,
    },
    /// Create a portable starter building document.
    New {
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate and compile a project without accessing the game.
    Validate {
        project: PathBuf,
        #[command(flatten)]
        native: OptionalNative,
    },
    /// Render a project on the GPU using native game materials.
    Preview {
        project: PathBuf,
        #[arg(long)]
        game: PathBuf,
        #[arg(long)]
        schema: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1024)]
        size: u32,
        #[arg(long)]
        closed: bool,
        #[arg(long)]
        camera: Option<PathBuf>,
    },
    /// Export an archive, ArchiveXL registration and independent readback receipt.
    Export {
        project: PathBuf,
        #[arg(long)]
        game: PathBuf,
        #[arg(long)]
        schema: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
}

fn read(path: &Path, catalogue: &Path, source: Option<&GameSource>) -> Result<model::Compiled> {
    let mut project: model::Project = serde_json::from_slice(&fs::read(path)?)?;
    let mut library = model::library()?;
    {
        let source = source
            .ok_or_else(|| anyhow::anyhow!("Compilation requires --game for catalogue geometry"))?;
        retrofit::refresh_sources(&mut project, source)?;
        shell::hydrate(&project, &mut library, source)?;
        catalogue::Catalogue::open(catalogue)?
            .ok_or_else(|| anyhow::anyhow!("Items catalogue not found"))?
            .hydrate(&project, &mut library, source)?;
        retrofit::validate(source, &project.retrofit, &terrain::masks(&project))?;
    }
    let mut compiled = model::compile(&project, &library)?;
    shell::compile_effects(&mut compiled, source.context("Native source required")?)?;
    Ok(compiled)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let catalogue = args
        .catalogue
        .unwrap_or_else(catalogue::Catalogue::default_root);
    match args.command {
        Command::Navigation {
            project,
            out,
            native,
        } => {
            use base64::Engine;
            let source = native.source()?;
            let compiled = read(&project, &catalogue, source.as_ref())?;
            let nav = crate::navigation::compile(&compiled)?;
            let resource = crate::navigation::resource(&compiled, &nav)?;
            fs::create_dir_all(&out)?;
            for (index, buffer) in resource["tileBuffers"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Tile buffers absent"))?
                .iter()
                .enumerate()
            {
                fs::write(
                    out.join(format!("tile-{index}.vand")),
                    base64::engine::general_purpose::STANDARD.decode(
                        buffer["Bytes"]
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("Tile bytes absent"))?,
                    )?,
                )?;
            }
            fs::write(
                out.join("navigation.json"),
                serde_json::to_vec_pretty(&resource)?,
            )?;
        }
        Command::Route {
            project,
            start,
            end,
            native,
        } => {
            let source = native.source()?;
            let compiled = read(&project, &catalogue, source.as_ref())?;
            let start: [f32; 3] = start
                .try_into()
                .map_err(|_| anyhow::anyhow!("Start requires XYZ"))?;
            let end: [f32; 3] = end
                .try_into()
                .map_err(|_| anyhow::anyhow!("End requires XYZ"))?;
            println!(
                "{}",
                crate::navigation::route(&crate::navigation::compile(&compiled)?, start, end)?
            );
        }
        Command::Inspect {
            project,
            game,
            schema,
            out,
        } => {
            let source = GameSource::open(&game, schema.as_deref())?;
            preview::inspect(&read(&project, &catalogue, Some(&source))?, &source, &out)?;
        }
        Command::New { out } => {
            anyhow::ensure!(!out.exists(), "Destination already exists");
            fs::write(out, include_bytes!("../examples/workshop.building.json"))?;
        }
        Command::Validate { project, native } => {
            let source = native.source()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&read(&project, &catalogue, source.as_ref())?)?
            )
        }
        Command::Preview {
            project,
            game,
            schema,
            out,
            size,
            closed,
            camera,
        } => {
            let mut session = preview::Session::new(GameSource::open(&game, schema.as_deref())?)?;
            let compiled = read(&project, &catalogue, Some(&session.source))?;
            session.source.validate_output(&out)?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                out,
                session.render(
                    &compiled,
                    preview::Options {
                        size,
                        cutaway: !closed,
                        camera: camera
                            .map(|path| -> Result<_> {
                                Ok(serde_json::from_slice(&fs::read(path)?)?)
                            })
                            .transpose()?,
                        ..Default::default()
                    },
                )?,
            )?;
        }
        Command::Export {
            project,
            game,
            schema,
            out,
        } => {
            let source = GameSource::open(&game, schema.as_deref())?;
            println!(
                "{}",
                export::export(&read(&project, &catalogue, Some(&source))?, &source, &out)?
            );
        }
        Command::Serve {
            game,
            schema,
            port,
            work,
        } => server::serve(&game, schema.as_deref(), port, &work, &catalogue).await?,
    }
    Ok(())
}
