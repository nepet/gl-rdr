use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use gl_client::credentials::{Device, NodeIdProvider};
use gl_client::node::{GClient, Node};
use gl_rdr::{descriptor, help, params, transcode};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "glrdr")]
#[command(about = "GL-RADAR: generic gRPC pass-through for Greenlight nodes", long_about = None)]
#[command(arg_required_else_help = true)]
#[command(after_help = "\
Examples:
  glrdr getinfo
  glrdr -k pay bolt11=lnbc... amount_msat=100000
  glrdr help
  glrdr help pay
  glrdr --raw /cln.Node/Getinfo
")]
pub struct Args {
    /// RPC method name (or `help`, or an explicit service/Method path)
    pub method: String,

    /// Positional params: `key=value` pairs (or, after `help`, a method name)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, conflicts_with = "params_json")]
    pub params: Vec<String>,

    /// Path to the Device credentials blob
    #[arg(long, env = "GL_CREDS", value_name = "PATH")]
    pub creds: Option<PathBuf>,

    /// Connect directly to this gRPC URI instead of using the scheduler
    #[arg(long, value_name = "URI")]
    pub grpc_uri: Option<String>,

    /// Force a specific service for bare method names
    #[arg(long, value_name = "SERVICE")]
    pub service: Option<String>,

    /// Full JSON params object, passed through as-is
    #[arg(long, value_name = "JSON")]
    pub params_json: Option<String>,

    /// Treat trailing params as key=value pairs (accepted for parity; pairs are
    /// detected automatically)
    #[arg(short = 'k', long = "named")]
    pub named: bool,

    /// Treat every param value as plain text
    #[arg(long, conflicts_with = "strict_json")]
    pub text: bool,

    /// Require every param value to be valid JSON
    #[arg(long)]
    pub strict_json: bool,

    /// Raw mode: METHOD is an explicit gRPC path; the single param is a hex
    /// protobuf payload; the response is printed as hex
    #[arg(long)]
    pub raw: bool,
}

impl Args {
    fn param_mode(&self) -> params::ParamMode {
        if self.text {
            params::ParamMode::Text
        } else if self.strict_json {
            params::ParamMode::StrictJson
        } else {
            params::ParamMode::Auto
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();

    // `help` discovery short-circuits before any network or descriptor encode.
    if args.method.eq_ignore_ascii_case("help") {
        match args.params.first() {
            None => print!("{}", help::list_methods()),
            Some(name) => {
                let method = descriptor::resolve(name, args.service.as_deref())
                    .with_context(|| format!("cannot describe `{name}`"))?;
                print!("{}", help::describe_method(&method));
            }
        }
        return Ok(());
    }

    if args.raw {
        return run_raw(&args).await;
    }

    // Resolve method + build the encoded request.
    let method = descriptor::resolve(&args.method, args.service.as_deref())?;
    if method.is_server_streaming() || method.is_client_streaming() {
        bail!(
            "`{}` is a streaming method; glrdr supports unary calls only",
            method.name()
        );
    }
    let path = descriptor::grpc_path(&method);

    let json = params::parse_params(args.params_json.as_deref(), &args.params, args.param_mode())
        .with_context(|| format!("invalid parameters for `{}`", args.method))?;
    let payload = transcode::json_to_bytes(&method.input(), &json)
        .with_context(|| format!("failed to encode request for `{}`", args.method))?;

    let mut client = connect(&args).await?;
    let response = client
        .call(&path, payload)
        .await
        .map_err(|s| anyhow!("RPC `{}` failed: {} ({:?})", args.method, s.message(), s.code()))?;

    let out = transcode::bytes_to_cln_json(&method.output(), &response.into_inner())
        .with_context(|| format!("failed to decode `{}` response", args.method))?;
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

async fn run_raw(args: &Args) -> Result<()> {
    let payload = match args.params.first() {
        Some(hex_str) => hex::decode(hex_str).context("invalid hex payload")?,
        None => Vec::new(),
    };
    let mut client = connect(args).await?;
    let response = client
        .call(&args.method, payload)
        .await
        .map_err(|s| anyhow!("RPC `{}` failed: {} ({:?})", args.method, s.message(), s.code()))?;
    println!("{}", hex::encode(response.into_inner()));
    Ok(())
}

/// Build a connected `GClient` from the Device credentials.
async fn connect(args: &Args) -> Result<GClient> {
    let path = args
        .creds
        .clone()
        .ok_or_else(|| anyhow!("no credentials: pass --creds <path> or set GL_CREDS"))?;
    let data = std::fs::read(&path)
        .with_context(|| format!("failed to read credentials from {}", path.display()))?;
    let device = Device::from_bytes(&data);
    if device.rune.is_empty() {
        bail!(
            "credentials at {} have no rune (not an authenticated Device blob)",
            path.display()
        );
    }
    let node_id = device
        .node_id()
        .map_err(|e| anyhow!("could not derive node id from credentials: {e}"))?;
    let node = Node::new(node_id, device).context("failed to build node client")?;

    match &args.grpc_uri {
        Some(uri) => node
            .connect(uri.clone())
            .await
            .with_context(|| format!("failed to connect to {uri}")),
        None => node
            .schedule()
            .await
            .context("failed to schedule node via the Greenlight scheduler"),
    }
}
