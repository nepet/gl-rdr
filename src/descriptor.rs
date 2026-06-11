use anyhow::{anyhow, bail, Context, Result};
use prost::Message as _;
use prost_reflect::prost_types::FileDescriptorSet;
use prost_reflect::{DescriptorPool, MethodDescriptor};
use std::path::Path;
use std::sync::OnceLock;

const DESCRIPTOR_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));

/// Services we resolve bare method names against, in priority order.
pub(crate) const DEFAULT_SERVICES: [&str; 2] = ["cln.Node", "greenlight.Node"];

/// The process-wide protobuf descriptor pool, decoded once.
pub fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(DESCRIPTOR_BYTES).expect("bundled descriptor set must decode")
    })
}

/// Build a pool from an external descriptor set layered over the bundled schema.
///
/// The external files load first, then the bundled schema is added underneath.
/// prost-reflect skips a file whose name is already present, so on a name clash
/// the external file wins and the bundled one is dropped, while bundled-only
/// files (e.g. `greenlight.proto`) are kept. This lets a newer `node.proto`
/// replace the bundled copy (see ADR 0003). The external set must be
/// self-contained (`protoc --include_imports`), since it resolves before the
/// bundle.
pub fn pool_with_set(bytes: &[u8]) -> Result<DescriptorPool> {
    note_overrides(bytes);
    let mut pool = DescriptorPool::new();
    pool.decode_file_descriptor_set(bytes)
        .context("not a valid protobuf descriptor set")?;
    pool.decode_file_descriptor_set(DESCRIPTOR_BYTES)
        .context("descriptor set conflicts with the bundled schema")?;
    Ok(pool)
}

/// Build the descriptor pool for this run. With no path, returns a clone of the
/// bundled pool. With a path, that set overrides the bundle (same-name files win).
pub fn effective_pool(extra: Option<&Path>) -> Result<DescriptorPool> {
    match extra {
        None => Ok(pool().clone()), // cheap Arc clone; see pool_with_set doc
        Some(path) => {
            let bytes = std::fs::read(path).with_context(|| {
                format!("failed to read descriptor set from {}", path.display())
            })?;
            pool_with_set(&bytes).with_context(|| {
                format!("failed to load descriptor set from {}", path.display())
            })
        }
    }
}

/// Note (on stderr) each file in `bytes` that overrides a bundled file of the
/// same name, so it's visible which parts of the schema the descriptor replaced.
fn note_overrides(bytes: &[u8]) {
    let Ok(set) = FileDescriptorSet::decode(bytes) else {
        return; // malformed input — the real error surfaces at decode time
    };
    for file in &set.file {
        let name = file.name();
        if pool().get_file_by_name(name).is_some() {
            eprintln!("note: descriptor set overrides bundled `{name}`");
        }
    }
}

/// The fully-qualified gRPC path for a method, e.g. `/cln.Node/Getinfo`.
pub fn grpc_path(method: &MethodDescriptor) -> String {
    format!("/{}/{}", method.parent_service().full_name(), method.name())
}

/// Resolve a user-supplied method reference to a `MethodDescriptor`.
///
/// Accepts a bare name (`getinfo`, case-insensitive), an explicit
/// `service/Method` or `/service/Method`, and honours an optional
/// `--service` override. Bare names resolve against `cln.Node` first,
/// then `greenlight.Node`.
pub fn resolve(
    pool: &DescriptorPool,
    input: &str,
    service_override: Option<&str>,
) -> Result<MethodDescriptor> {
    // Explicit path form: "svc/Method" or "/svc/Method".
    let trimmed = input.trim_start_matches('/');
    if let Some((svc, method)) = trimmed.split_once('/') {
        let service = pool
            .get_service_by_name(svc)
            .ok_or_else(|| anyhow!("unknown service `{svc}`"))?;
        return service
            .methods()
            .find(|m| m.name().eq_ignore_ascii_case(method))
            .ok_or_else(|| anyhow!("unknown method `{method}` on service `{svc}`"));
    }

    // Bare name, optionally constrained to one service.
    let services: Vec<&str> = match service_override {
        Some(s) => vec![s],
        None => DEFAULT_SERVICES.to_vec(),
    };
    for svc in &services {
        let Some(service) = pool.get_service_by_name(svc) else {
            bail!("unknown service `{svc}`");
        };
        if let Some(m) = service
            .methods()
            .find(|m| m.name().eq_ignore_ascii_case(input))
        {
            return Ok(m);
        }
    }
    Err(anyhow!(
        "unknown method `{input}`; try `glrdr help` to list available methods"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bare_name_case_insensitively() {
        let m = resolve(pool(), "getinfo", None).unwrap();
        assert_eq!(grpc_path(&m), "/cln.Node/Getinfo");
        let m2 = resolve(pool(), "GetInfo", None).unwrap();
        assert_eq!(grpc_path(&m2), "/cln.Node/Getinfo");
    }

    #[test]
    fn resolves_explicit_path() {
        let m = resolve(pool(), "cln.Node/ListFunds", None).unwrap();
        assert_eq!(grpc_path(&m), "/cln.Node/ListFunds");
        let m2 = resolve(pool(), "/cln.Node/ListFunds", None).unwrap();
        assert_eq!(grpc_path(&m2), "/cln.Node/ListFunds");
    }

    #[test]
    fn service_override_selects_greenlight() {
        let m = resolve(pool(), "Configure", Some("greenlight.Node")).unwrap();
        assert_eq!(grpc_path(&m), "/greenlight.Node/Configure");
    }

    #[test]
    fn unknown_method_errors() {
        assert!(resolve(pool(), "definitely_not_a_method", None).is_err());
    }
}
