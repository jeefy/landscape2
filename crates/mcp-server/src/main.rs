                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            use anyhow::{Context, Result};
use clap::Parser;
use landscape2_core::data::{DataSource, LandscapeData};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the landscape.yml file
    #[arg(long)]
    data_file: Option<PathBuf>,

    /// URL to the landscape.yml file
    #[arg(long, default_value = "https://raw.githubusercontent.com/cncf/landscape/refs/heads/master/landscape.yml")]
    data_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    id: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

struct ServerState {
    landscape_data: LandscapeData,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr so it doesn't interfere with stdout JSON-RPC
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let args = Args::parse();

    let data_source = if let Some(file) = args.data_file {
        info!("Loading landscape data from file: {:?}", file);
        DataSource {
            data_file: Some(file),
            data_url: None,
        }
    } else {
        info!("Loading landscape data from url: {}", args.data_url);
        DataSource {
            data_file: None,
            data_url: Some(args.data_url),
        }
    };

    let landscape_data = LandscapeData::new(&data_source)
        .await
        .context("Failed to load landscape data")?;

    let state = Arc::new(Mutex::new(ServerState { landscape_data }));

    info!("MCP Server started. Waiting for requests on stdin...");

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    while let Some(line) = lines.next() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse JSON-RPC request: {}", e);
                continue;
            }
        };

        let state_clone = state.clone();
        let response = handle_request(request, state_clone).await;

        let response_str = serde_json::to_string(&response)?;
        io::stdout().write_all(response_str.as_bytes())?;
        io::stdout().write_all(b"\n")?;
        io::stdout().flush()?;
    }

    Ok(())
}

async fn handle_request(req: JsonRpcRequest, state: Arc<Mutex<ServerState>>) -> JsonRpcResponse {
    let result = match req.method.as_str() {
        "initialize" => handle_initialize(req.params),
        "notifications/initialized" => Ok(json!({})), // No-op
        "tools/list" => handle_tools_list(),
        "tools/call" => handle_tools_call(req.params, state).await,
        _ => Err(JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }),
    };

    match result {
        Ok(res) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(res),
            error: None,
            id: req.id,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(err),
            id: req.id,
        },
    }
}

fn handle_initialize(_params: Option<Value>) -> Result<Value, JsonRpcError> {
    Ok(json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "landscape2-mcp-server",
            "version": "0.1.0"
        }
    }))
}

fn handle_tools_list() -> Result<Value, JsonRpcError> {
    Ok(json!({
        "tools": [
            {
                "name": "query_landscape",
                "description": "Query the CNCF landscape data for items matching specific criteria.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "Filter by category name" },
                        "subcategory": { "type": "string", "description": "Filter by subcategory name" },
                        "name": { "type": "string", "description": "Filter by item name (case-insensitive substring)" },
                        "maturity": { "type": "string", "description": "Filter by maturity (e.g., graduated, incubating, sandbox)" },
                        "license": { "type": "string", "description": "Filter by license (e.g., Apache-2.0)" },
                        "organization": { "type": "string", "description": "Filter by organization name (exact match)" },
                        "country": { "type": "string", "description": "Filter by country (exact match)" },
                        "tag": { "type": "string", "description": "Filter by tag (exact match)" },
                        "company_type": { "type": "string", "description": "Filter by company type (exact match)" },
                        "industry": { "type": "string", "description": "Filter by industry (exact match in list)" },
                        "specification": { "type": "boolean", "description": "Filter by specification status" },
                        "enduser": { "type": "boolean", "description": "Filter by enduser status" }
                    }
                }
            },
            {
                "name": "get_finance_data",
                "description": "Get finance/funding data for organizations.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "page": { "type": "integer", "default": 1, "description": "Page number to fetch" }
                    }
                }
            }
        ]
    }))
}

use landscape2_core::datasets::full::Full;

async fn handle_tools_call(
    params: Option<Value>,
    state: Arc<Mutex<ServerState>>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or(JsonRpcError {
        code: -32602,
        message: "Invalid params".to_string(),
        data: None,
    })?;

    let name = params.get("name").and_then(|n| n.as_str()).ok_or(JsonRpcError {
        code: -32602,
        message: "Missing tool name".to_string(),
        data: None,
    })?;

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "query_landscape" => {
            let category_filter = args.get("category").and_then(|v| v.as_str());
            let subcategory_filter = args.get("subcategory").and_then(|v| v.as_str());
            let name_filter = args.get("name").and_then(|v| v.as_str());
            let maturity_filter = args.get("maturity").and_then(|v| v.as_str());
            let license_filter = args.get("license").and_then(|v| v.as_str());
            let organization_filter = args.get("organization").and_then(|v| v.as_str());
            let country_filter = args.get("country").and_then(|v| v.as_str());
            let tag_filter = args.get("tag").and_then(|v| v.as_str());
            let company_type_filter = args.get("company_type").and_then(|v| v.as_str());
            let industry_filter = args.get("industry").and_then(|v| v.as_str());
            let specification_filter = args.get("specification").and_then(|v| v.as_bool());
            let enduser_filter = args.get("enduser").and_then(|v| v.as_bool());

            let state = state.lock().await;
            let mut results = Vec::new();

            for item in &state.landscape_data.items {
                if let Some(cat) = category_filter {
                    if item.category != cat {
                        continue;
                    }
                }
                if let Some(sub) = subcategory_filter {
                    if item.subcategory != sub {
                        continue;
                    }
                }
                if let Some(n) = name_filter {
                    if !item.name.to_lowercase().contains(&n.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(mat) = maturity_filter {
                    if item.maturity.as_deref() != Some(mat) {
                        continue;
                    }
                }
                if let Some(lic) = license_filter {
                    let has_license = item.repositories.as_ref().map_or(false, |repos| {
                        repos.iter().any(|repo| {
                            repo.github_data.as_ref().map_or(false, |gh| {
                                gh.license.as_deref() == Some(lic)
                            })
                        })
                    });
                    if !has_license {
                        continue;
                    }
                }
                if let Some(org) = organization_filter {
                    if item.crunchbase_data.as_ref().and_then(|cb| cb.name.as_deref()) != Some(org) {
                        continue;
                    }
                }
                if let Some(country) = country_filter {
                    if item.crunchbase_data.as_ref().and_then(|cb| cb.country.as_deref()) != Some(country) {
                        continue;
                    }
                }
                if let Some(tag) = tag_filter {
                    if item.tag.as_deref() != Some(tag) {
                        continue;
                    }
                }
                if let Some(ctype) = company_type_filter {
                    if item.crunchbase_data.as_ref().and_then(|cb| cb.company_type.as_deref()) != Some(ctype) {
                        continue;
                    }
                }
                if let Some(ind) = industry_filter {
                    let has_industry = item.crunchbase_data.as_ref().map_or(false, |cb| {
                        cb.categories.as_ref().map_or(false, |cats| {
                            cats.iter().any(|c| c == ind)
                        })
                    });
                    if !has_industry {
                        continue;
                    }
                }
                if let Some(spec) = specification_filter {
                    if item.specification.unwrap_or(false) != spec {
                        continue;
                    }
                }
                if let Some(eu) = enduser_filter {
                    if item.enduser.unwrap_or(false) != eu {
                        continue;
                    }
                }

                results.push(item.clone());
            }

            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&results).unwrap_or_default()
                    }
                ]
            }))
        }
        "get_finance_data" => {
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1);
            let page_size = 20;
            let start_index = ((page - 1) * page_size) as usize;

            info!("Fetching finance data from full dataset");

            let full_data_url = "https://landscape.cncf.io/data/full.json";
            let resp = reqwest::get(full_data_url).await.map_err(|e| JsonRpcError {
                code: -32603,
                message: format!("Failed to fetch finance data: {}", e),
                data: None,
            })?;

            let full_data: Full = resp.json().await.map_err(|e| JsonRpcError {
                code: -32603,
                message: format!("Failed to parse finance data: {}", e),
                data: None,
            })?;

            // Filter organizations with funding data
            let mut organizations: Vec<_> = full_data.crunchbase_data.values()
                .filter(|org| org.funding.unwrap_or(0) > 0 || org.funding_rounds.as_ref().map_or(false, |r| !r.is_empty()))
                .collect();
            
            // Sort by funding amount descending
            organizations.sort_by(|a, b| b.funding.unwrap_or(0).cmp(&a.funding.unwrap_or(0)));

            let total_count = organizations.len();
            let total_pages = (total_count + page_size as usize - 1) / page_size as usize;

            let paged_organizations = organizations
                .into_iter()
                .skip(start_index)
                .take(page_size as usize)
                .collect::<Vec<_>>();

            Ok(json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&json!({
                            "page": page,
                            "total_pages": total_pages,
                            "total_items": total_count,
                            "organizations": paged_organizations
                        })).unwrap_or_default()
                    }
                ]
            }))
        }
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Tool not found: {}", name),
            data: None,
        }),
    }
}
