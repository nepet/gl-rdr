use crate::descriptor::{self, grpc_path};
use prost_reflect::{Kind, MethodDescriptor};

/// A short human label for a field's type.
fn kind_label(kind: &Kind) -> String {
    match kind {
        Kind::Double | Kind::Float => "number".into(),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => "i32".into(),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => "i64".into(),
        Kind::Uint32 | Kind::Fixed32 => "u32".into(),
        Kind::Uint64 | Kind::Fixed64 => "u64".into(),
        Kind::Bool => "bool".into(),
        Kind::String => "string".into(),
        Kind::Bytes => "hex".into(),
        Kind::Enum(e) => format!("enum {}", e.name()),
        Kind::Message(m) => format!("message {}", m.name()),
    }
}

fn emit_service(svc: &prost_reflect::ServiceDescriptor, out: &mut String) {
    out.push_str(svc.full_name());
    out.push_str(":\n");
    let mut names: Vec<String> = svc
        .methods()
        .filter(|m| !(m.is_server_streaming() || m.is_client_streaming()))
        .map(|m| m.name().to_lowercase())
        .collect();
    names.sort();
    for name in names {
        out.push_str("  ");
        out.push_str(&name);
        out.push('\n');
    }
    out.push('\n');
}

/// List all methods, grouped by service. Default services come first in
/// resolution-priority order; any additional services in the pool (e.g. those
/// loaded via `--descriptor`) follow.
pub fn list_methods(pool: &prost_reflect::DescriptorPool) -> String {
    let mut out = String::new();

    // Default services first, in priority order.
    for svc_name in descriptor::DEFAULT_SERVICES {
        if let Some(svc) = pool.get_service_by_name(svc_name) {
            emit_service(&svc, &mut out);
        }
    }
    // Then any other services present in the pool.
    for svc in pool.services() {
        if !descriptor::DEFAULT_SERVICES.contains(&svc.full_name()) {
            emit_service(&svc, &mut out);
        }
    }

    out.push_str("Use `glrdr help <method>` for a method's parameters.\n");
    out
}

/// Describe one method's request fields.
pub fn describe_method(method: &MethodDescriptor) -> String {
    let mut out = format!("{}  ({})\n", method.name().to_lowercase(), grpc_path(method));
    if method.is_server_streaming() || method.is_client_streaming() {
        out.push_str("  (streaming method — not supported by glrdr)\n");
        return out;
    }
    let input = method.input();
    let fields: Vec<_> = input.fields().collect();
    if fields.is_empty() {
        out.push_str("  (no parameters)\n");
        return out;
    }
    for f in fields {
        let card = if f.is_list() {
            "repeated"
        } else if f.supports_presence() {
            "optional"
        } else {
            "required"
        };
        out.push_str(&format!(
            "  {:<24} {:<14} ({card})\n",
            f.name(),
            kind_label(&f.kind())
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::resolve;
    use prost::Message as _;
    use prost_reflect::DescriptorPool;

    #[test]
    fn list_groups_by_service_and_includes_getinfo() {
        let listing = list_methods(descriptor::pool());
        assert!(listing.contains("cln.Node:"));
        assert!(listing.contains("greenlight.Node:"));
        assert!(listing.contains("getinfo"));
        assert!(listing.contains("listfunds"));
    }

    #[test]
    fn list_includes_non_default_services() {
        let fds = protox::compile(["tests/fixtures/addon.proto"], ["tests/fixtures"]).unwrap();
        let pool = DescriptorPool::decode(fds.encode_to_vec().as_slice()).unwrap();
        let listing = list_methods(&pool);
        assert!(listing.contains("addon.Addon:"), "listing was: {listing}");
        assert!(listing.contains("ping"), "listing was: {listing}");
    }

    #[test]
    fn describe_getinfo_renders_path() {
        let m = resolve(descriptor::pool(), "getinfo", None).unwrap();
        let text = describe_method(&m);
        assert!(text.contains("/cln.Node/Getinfo"));
    }

    #[test]
    fn describe_pay_lists_bolt11_field() {
        let m = resolve(descriptor::pool(), "pay", None).unwrap();
        let text = describe_method(&m);
        assert!(text.contains("bolt11"));
    }
}
