# Load external proto schemas at runtime with `--descriptor`

`glrdr` bundles the Core Lightning and Greenlight protos it was built with, and
resolves and decodes every call against that bundled schema. When a node runs
ahead of the bundle — a newer `cln.Node` method like `xpay`, or a changed
message — `--descriptor` (or `GL_DESCRIPTOR`) points at a precompiled
`FileDescriptorSet` that **overrides** the bundled schema for that run. It is
load-only: the resulting schema feeds the normal resolve-and-decode path and
`glrdr help`; raw mode never consults the schema and is untouched.

## Considered Options

- **External files override bundled same-name files (chosen).** The external set
  is loaded first, then the bundled schema is added underneath; prost-reflect
  skips a file whose name is already present, so on a clash the external file
  wins and the bundled one is dropped, while bundled-only files (e.g.
  `greenlight.proto`) are kept. You supply your node's full `node.proto` — the
  vendored one with your methods, or simply the upstream file from the CLN
  version your node runs — and the new method then routes under `cln.Node` *and*
  decodes to CLN-flavored JSON, with no rebuild.
- **Additive merge, bundle wins (original, rejected).** A descriptor could only
  *add* new, separately-named services; a same-name file was skipped. This can't
  extend an existing service, because protobuf forbids defining `cln.Node` across
  two files — any second definition conflicts. On a stock node, where every
  method lives under the already-bundled `cln.Node`, that made the feature
  unusable. Verification caught this: the new-service tests passed, but no real
  node serves a brand-new service path.
- **Graft individual methods onto a bundled service.** Lets you supply only a
  delta proto, but requires merging service method-lists and message-lists inside
  `glrdr` by hand — fragile descriptor-proto surgery against `glrdr`'s thin-client
  ethos ([ADR 0001](0001-rdr-is-a-pure-passthrough-client.md)). Rejected; the
  override route reaches the same result by replacing one file.
- **Rebuild `glrdr` with the new protos.** Works today but defeats the point: a
  node upgrade would need a fresh release before its methods could be called.

## Consequences

- Override is keyed on **file name**: to replace `cln.Node`, the descriptor must
  contain a file named `node.proto` (the bundled name). The natural source — your
  node's own `node.proto` — already has that name.
- You supply the **complete** file, since it replaces wholesale; a partial
  `node.proto` would drop the methods it omits.
- Build the set with `protoc --include_imports --descriptor_set_out=…` so it is
  self-contained — it resolves before the bundle, so its imports must be inside
  it.
- Brand-new, separately-named services are still simply added (override is a
  superset of the old additive behavior).
- Raw mode is deliberately exempt: it passes a **Path** through literally and
  prints response bytes as hex, so `--descriptor` has no effect there.
