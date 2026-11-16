use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use ghostline_renderer::GameSource;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    export,
    model::{self, Library, Project},
    preview::{self, Session},
};

struct App {
    library: Mutex<Library>,
    catalogue: Option<crate::catalogue::Catalogue>,
    session: Mutex<Session>,
    source: Arc<GameSource>,
    work: PathBuf,
    game: PathBuf,
    actor_names: Mutex<Option<Vec<(String, String)>>>,
}
type Shared = Arc<App>;

impl App {
    fn compile(&self, project: &Project) -> Result<model::Compiled> {
        let mut refreshed = project.clone();
        let source_issue = crate::retrofit::refresh_sources(&mut refreshed, &self.source)
            .err()
            .map(|error| format!("Saved shell retained for editing and preview. World-sector edits need a new survey before export: {error}"));
        let project = &refreshed;
        let mut library = self
            .library
            .lock()
            .map_err(|_| anyhow::anyhow!("Library unavailable"))?;
        if let Some(catalogue) = &self.catalogue {
            crate::shell::hydrate(project, &mut library, &self.source)?;
            catalogue.hydrate(project, &mut library, &self.source)?;
            if source_issue.is_none() {
                crate::retrofit::validate(
                    &self.source,
                    &project.retrofit,
                    &crate::terrain::masks(project),
                )?;
            }
        } else {
            anyhow::bail!("Published items catalogue is required; set --catalogue");
        }
        let mut compiled = model::compile(project, &library)?;
        crate::shell::compile_effects(&mut compiled, &self.source)?;
        if let Some(issue) = source_issue {
            compiled.diagnostics.push(issue);
        }
        Ok(compiled)
    }
}

struct ApiError(anyhow::Error);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error":format!("{:#}",self.0)})),
        )
            .into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}
type ApiResult<T> = std::result::Result<T, ApiError>;

async fn local(request: axum::extract::Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let local_host = host
        .strip_prefix("127.0.0.1:")
        .or_else(|| host.strip_prefix("localhost:"))
        .is_some_and(|p| p.parse::<u16>().is_ok());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    if !local_host || origin.is_some_and(|o| o != format!("http://{host}")) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response
}

fn ui_asset(name: &str, bundled: &str) -> String {
    if let Some(directory) = std::env::var_os("GHOSTLINE_BUILDING_UI")
        && let Ok(text) = fs::read_to_string(Path::new(&directory).join(name))
    {
        return text;
    }
    if cfg!(debug_assertions)
        && let Ok(text) = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("ui/dist")
                .join(name),
        )
    {
        return text;
    }
    bundled.to_owned()
}

pub async fn serve(
    game: &Path,
    schema: Option<&Path>,
    port: u16,
    work: &Path,
    catalogue: &Path,
) -> Result<()> {
    let source = GameSource::open(game, schema)?;
    let work = source.validate_output(work)?;
    fs::create_dir_all(work.join("projects"))?;
    fs::create_dir_all(work.join("exports"))?;
    let session = Session::new(source)?;
    let state = Arc::new(App {
        library: Mutex::new(model::library()?),
        catalogue: crate::catalogue::Catalogue::open(catalogue)?,
        source: Arc::clone(&session.source),
        session: Mutex::new(session),
        work,
        game: game.to_path_buf(),
        actor_names: Mutex::new(None),
    });
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                Html(ui_asset(
                    "index.html",
                    include_str!("../ui/dist/index.html"),
                ))
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    ui_asset("app.js", include_str!("../ui/dist/app.js")),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    ui_asset("style.css", include_str!("../ui/dist/style.css")),
                )
            }),
        )
        .route("/api/bootstrap", get(bootstrap))
        .route("/api/library", get(library))
        .route("/api/survey", post(survey))
        .route("/api/world-sectors", post(world_sectors))
        .route("/api/actor-options", get(actor_options))
        .route("/api/import-shell", post(import_shell))
        .route("/api/shell/survey", post(shell_survey))
        .route(
            "/api/shell/sites",
            get(|| async {
                Json(
                    serde_json::from_str::<Value>(include_str!("../data/retrofit-sites.json"))
                        .expect("bundled retrofit sites"),
                )
            }),
        )
        .route("/api/shell/import", post(shell_import))
        .route("/api/shell/operation", post(shell_operation))
        .route("/api/shell/preview", post(shell_preview))
        .route("/api/catalogue-media", get(catalogue_media))
        .route("/api/compile", post(compile))
        .route("/api/route", post(route))
        .route("/api/preview", post(render))
        .route("/api/save", post(save))
        .route("/api/open", get(open))
        .route("/api/example", get(example))
        .route("/api/export", post(native_export))
        .route("/api/download", get(download))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn(local))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Building builder: http://127.0.0.1:{port}/");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn bootstrap(State(app): State<Shared>) -> ApiResult<Json<Value>> {
    let result = (|| -> Result<Value> {
        let mut projects = Vec::new();
        for entry in fs::read_dir(app.work.join("projects"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                projects.push(
                    path.file_stem()
                        .context("Missing file name")?
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
        projects.sort();
        Ok(
            json!({"starter":serde_json::from_str::<Value>(include_str!("../examples/workshop.building.json"))?,"kit":model::kit("common-3m")?,"kits":model::kits()?,"projects":projects,"catalogue_count":app.catalogue.as_ref().map_or(0,|c|c.count()),"library_source":"Published items catalogue"}),
        )
    })()?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    q: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    offset: usize,
}
async fn library(
    State(app): State<Shared>,
    Query(search): Query<Search>,
) -> ApiResult<Json<Value>> {
    let words: Vec<_> = search
        .q
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let entries = app
        .catalogue
        .as_ref()
        .map_or_else(Vec::new, |c| c.search(&words, &search.role))
        .into_iter()
        .filter(|entry| {
            search.kind.is_empty() || entry["kind"].as_str() == Some(search.kind.as_str())
        })
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"total":entries.len(),"entries":entries.into_iter().skip(search.offset).take(60).collect::<Vec<_>>()}),
    ))
}

#[derive(Deserialize)]
struct Survey {
    sector: String,
    position: [f32; 3],
    radius: f32,
}

#[derive(Deserialize)]
struct NearbySectors {
    position: [f32; 3],
    radius: f32,
}

/// Uses the game's TweakDB dictionary so custom text is never the only way to pick an actor.
async fn actor_options(
    State(app): State<Shared>,
    Query(search): Query<Search>,
) -> ApiResult<Json<Value>> {
    let result = tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut cached = app
            .actor_names
            .lock()
            .map_err(|_| anyhow::anyhow!("Actor options unavailable"))?;
        if cached.is_none() {
            use ghostline_red::tweakdb::{RecordTypes, TweakDb, TweakNames, read_names};
            let database = ["r6/cache/tweakdb_ep1.bin", "r6/cache/tweakdb.bin"]
                .into_iter()
                .map(|p| app.game.join(p))
                .find(|p| p.is_file())
                .context("Game TweakDB cache missing")?;
            let db = TweakDb::from_bytes(&fs::read(database)?)?;
            let names = TweakNames::resolve(
                &db,
                read_names(
                    &fs::read(
                        app.game
                            .join("bin/x64/plugins/cyber_engine_tweaks/tweakdb/tweakdbstr.kark"),
                    )
                    .context("TweakDB name dictionary missing")?,
                )?,
            );
            let types = RecordTypes::new(app.source.schema());
            let mut values = db
                .records
                .iter()
                .filter(|(_, kind)| types.is_a(**kind, "gamedataCharacter_Record"))
                .filter_map(|(id, _)| {
                    names.records.get(id).map(|name| {
                        (
                            name.clone(),
                            db.property(*id, "appearanceName")
                                .and_then(|flat| flat.value.as_str())
                                .unwrap_or("")
                                .to_owned(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            values.sort();
            *cached = Some(values);
        }
        let query = search.q.to_lowercase();
        let values = cached
            .as_ref()
            .context("Actor cache unavailable")?
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&query))
            .take(100)
            .collect::<Vec<_>>();
        let records = values.iter().map(|(name, _)| name).collect::<Vec<_>>();
        let appearances = values
            .iter()
            .map(|(name, appearance)| (name.as_str(), appearance.as_str()))
            .collect::<std::collections::BTreeMap<_, _>>();
        Ok(json!({"records":records,"appearances":appearances}))
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

async fn world_sectors(
    State(app): State<Shared>,
    Json(request): Json<NearbySectors>,
) -> ApiResult<Json<Value>> {
    let sectors = tokio::task::spawn_blocking(move || -> Result<Value> {
        crate::retrofit::nearby_sectors(&app.source, request.position, request.radius)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(sectors))
}
async fn survey(State(app): State<Shared>, Json(request): Json<Survey>) -> ApiResult<Json<Value>> {
    let result = tokio::task::spawn_blocking(move || -> Result<Value> {
        crate::retrofit::survey(
            &app.source,
            &request.sector,
            request.position,
            request.radius,
        )
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}
#[derive(Deserialize)]
struct ShellImport {
    project: Project,
    sector: crate::retrofit::SectorPatch,
    selection: Vec<crate::retrofit::ImportSelection>,
}
async fn import_shell(
    State(app): State<Shared>,
    Json(mut request): Json<ShellImport>,
) -> ApiResult<Json<Value>> {
    let result = tokio::task::spawn_blocking(move || -> Result<Value> {
        {
            crate::retrofit::import(
                &mut request.project,
                &app.source,
                app.catalogue.as_ref().context("Items catalogue required")?,
                &request.sector,
                &request.selection,
            )?;
        }
        if !request
            .project
            .retrofit
            .iter()
            .any(|p| p.path == request.sector.path)
        {
            request.project.retrofit.push(request.sector);
        }
        request.project.revision += 1;
        Ok(serde_json::to_value(app.compile(&request.project)?)?)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

async fn shell_survey(
    State(app): State<Shared>,
    Json(request): Json<crate::shell::SurveyRequest>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || crate::shell::survey(&app.source, &request))
            .await
            .map_err(anyhow::Error::from)??,
    ))
}

async fn shell_import(
    State(app): State<Shared>,
    Json(request): Json<crate::shell::ImportRequest>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || -> Result<Value> {
            let project = crate::shell::import(&app.source, request)?;
            Ok(serde_json::to_value(app.compile(&project)?)?)
        })
        .await
        .map_err(anyhow::Error::from)??,
    ))
}

async fn shell_operation(
    State(app): State<Shared>,
    Json(request): Json<crate::shell::OperationRequest>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        tokio::task::spawn_blocking(move || -> Result<Value> {
            let project = crate::shell::operate(&app.source, request)?;
            Ok(serde_json::to_value(app.compile(&project)?)?)
        })
        .await
        .map_err(anyhow::Error::from)??,
    ))
}

async fn shell_preview(
    State(app): State<Shared>,
    Json(request): Json<crate::shell::PreviewRequest>,
) -> ApiResult<Response> {
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let project = crate::shell::import(&app.source, request.import)?;
        let compiled = app.compile(&project)?;
        app.session
            .lock()
            .map_err(|_| anyhow::anyhow!("Renderer unavailable"))?
            .render(&compiled, request.options)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
struct CatalogueMedia {
    path: String,
}
async fn catalogue_media(
    State(app): State<Shared>,
    Query(query): Query<CatalogueMedia>,
) -> ApiResult<Response> {
    let path = app
        .catalogue
        .as_ref()
        .context("Items catalogue is not connected")?
        .media_path(&query.path)?;
    let bytes = tokio::fs::read(path).await.map_err(anyhow::Error::from)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "private, max-age=3600"),
        ],
        bytes,
    )
        .into_response())
}

async fn compile(
    State(app): State<Shared>,
    Json(project): Json<Project>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        serde_json::to_value(app.compile(&project)?).map_err(anyhow::Error::from)?,
    ))
}

#[derive(Deserialize)]
struct Route {
    project: Project,
    start: [f32; 3],
    end: [f32; 3],
}
async fn route(State(app): State<Shared>, Json(request): Json<Route>) -> ApiResult<Json<Value>> {
    let value = tokio::task::spawn_blocking(move || -> Result<Value> {
        anyhow::ensure!(
            request
                .start
                .iter()
                .chain(&request.end)
                .all(|v| v.is_finite()),
            "Route coordinates must be finite"
        );
        let compiled = app.compile(&request.project)?;
        let nav = crate::navigation::compile(&compiled)?;
        crate::navigation::route(&nav, request.start, request.end)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(value))
}

#[derive(Deserialize)]
struct Preview {
    project: Project,
    #[serde(default)]
    options: preview::Options,
}
async fn render(State(app): State<Shared>, Json(request): Json<Preview>) -> ApiResult<Response> {
    let mime = if request.options.browser_scene {
        "model/gltf-binary"
    } else {
        "image/png"
    };
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let compiled = app.compile(&request.project)?;
        app.session
            .lock()
            .map_err(|_| anyhow::anyhow!("Renderer session unavailable"))?
            .render(&compiled, request.options)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok((
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    )
        .into_response())
}

async fn save(State(app): State<Shared>, Json(project): Json<Project>) -> ApiResult<Json<Value>> {
    let compiled = app.compile(&project)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(app.work.join("projects")).map_err(anyhow::Error::from)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), &compiled.project)
        .map_err(anyhow::Error::from)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(anyhow::Error::from)?;
    temporary
        .persist(
            app.work
                .join("projects")
                .join(format!("{}.json", project.id)),
        )
        .map_err(anyhow::Error::from)?;
    Ok(Json(
        json!({"saved":project.id,"revision":project.revision}),
    ))
}

#[derive(Deserialize)]
struct Identity {
    id: String,
}
async fn example(Query(query): Query<Identity>) -> ApiResult<Json<Value>> {
    let document = match query.id.as_str() {
        "workshop" => include_str!("../examples/workshop.building.json"),
        "stairs-and-devices" => include_str!("../examples/stairs-and-devices.building.json"),
        "motel-modules" => include_str!("../examples/motel-modules.building.json"),
        _ => return Err(anyhow::anyhow!("Unknown bundled example").into()),
    };
    Ok(Json(
        serde_json::from_str(document).map_err(anyhow::Error::from)?,
    ))
}
async fn open(State(app): State<Shared>, Query(query): Query<Identity>) -> ApiResult<Json<Value>> {
    model::identifier(&query.id)?;
    let bytes = fs::read(app.work.join("projects").join(format!("{}.json", query.id)))
        .map_err(anyhow::Error::from)?;
    let project = serde_json::from_slice(&bytes).map_err(anyhow::Error::from)?;
    let compiled = app.compile(&project)?;
    Ok(Json(
        serde_json::to_value(compiled.project).map_err(anyhow::Error::from)?,
    ))
}

async fn native_export(
    State(app): State<Shared>,
    Json(project): Json<Project>,
) -> ApiResult<Json<Value>> {
    let result = tokio::task::spawn_blocking(move || -> Result<Value> {
        let compiled = app.compile(&project)?;
        let identity = format!(
            "{}_{}",
            project.id,
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let output = app.work.join("exports").join(&identity);
        let mut report = export::export(&compiled, &app.source, &output)?;
        report["export_id"] = json!(identity);
        let mut files = fs::read_dir(&output)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        files.sort();
        report["files"] = json!(files);
        Ok(report)
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct Download {
    id: String,
    file: String,
}
async fn download(State(app): State<Shared>, Query(query): Query<Download>) -> ApiResult<Response> {
    model::identifier(&query.id)?;
    if query.file.is_empty()
        || query.file.len() >= 128
        || !query
            .file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
        || query.file.contains("..")
    {
        return Err(anyhow::anyhow!("Invalid export file").into());
    }
    let bytes = fs::read(app.work.join("exports").join(&query.id).join(&query.file))
        .map_err(anyhow::Error::from)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        header::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", query.file))
            .map_err(anyhow::Error::from)?,
    );
    Ok((headers, Body::from(bytes)).into_response())
}
