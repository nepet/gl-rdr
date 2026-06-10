use anyhow::{anyhow, bail, Result};
use prost_reflect::{DescriptorPool, MethodDescriptor};
use std::sync::OnceLock;

const DESCRIPTOR_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/descriptor.bin"));

/// Services we resolve bare method names against, in priority order.
const DEFAULT_SERVICES: [&str; 2] = ["cln.Node", "greenlight.Node"];

/// The process-wide protobuf descriptor pool, decoded once.
pub fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(DESCRIPTOR_BYTES).expect("bundled descriptor set must decode")
    })
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
pub fn resolve(input: &str, service_override: Option<&str>) -> Result<MethodDescriptor> {
    let pool = pool();

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
        let m = resolve("getinfo", None).unwrap();
        assert_eq!(grpc_path(&m), "/cln.Node/Getinfo");
        let m2 = resolve("GetInfo", None).unwrap();
        assert_eq!(grpc_path(&m2), "/cln.Node/Getinfo");
    }

    #[test]
    fn resolves_explicit_path() {
        let m = resolve("cln.Node/ListFunds", None).unwrap();
        assert_eq!(grpc_path(&m), "/cln.Node/ListFunds");
        let m2 = resolve("/cln.Node/ListFunds", None).unwrap();
        assert_eq!(grpc_path(&m2), "/cln.Node/ListFunds");
    }

    #[test]
    fn service_override_selects_greenlight() {
        let m = resolve("Configure", Some("greenlight.Node")).unwrap();
        assert_eq!(grpc_path(&m), "/greenlight.Node/Configure");
    }

    #[test]
    fn unknown_method_errors() {
        assert!(resolve("definitely_not_a_method", None).is_err());
    }
}
