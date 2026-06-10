# Match lightning-cli for inputs and outputs

`glrdr` deliberately mimics `lightning-cli`'s ergonomics so it reads as a
near-drop-in client against a Greenlight node, even though the wire underneath
is typed protobuf rather than CLN's native JSON-RPC.

**Inputs:** snake_case proto field names (the descriptor is configured to use
proto names, not protobuf-JSON's lowerCamelCase), supplied as `key=value` pairs
(the `lightning-cli -k` experience) or a full `--params-json` object, with
auto/`--text`/`--strict-json` value parsing.

**Outputs:** instead of canonical protobuf-JSON, we render CLN-flavored JSON —
64-bit integers as JSON numbers and `bytes` as hex — via a descriptor-walking
serializer (prost-reflect's `stringify_64_bit_integers(false)` for the integers,
plus a hex post-pass over byte fields, since protobuf-JSON mandates base64).

**Discovery:** the same fidelity extends to help. `glrdr --help` is the standard
clap usage/options/examples, but `glrdr help` lists all methods grouped by
service and `glrdr help <method>` prints that method's request fields from the
descriptor — the analog of `lightning-cli help` / `help <cmd>`. The bare word
`help` is intercepted before method resolution (there is no `Help` RPC in
`cln.Node` to shadow). Method-name shell completion is out of scope for now.

## Considered Options

- **CLN-flavored JSON (chosen).** Familiar to `lightning-cli` users; lets
  existing habits and scripts transfer. Costs a small custom serializer to
  maintain.
- **Canonical protobuf-JSON** (prost-reflect default: u64 as strings, bytes as
  base64). Free and matches `grpcurl`, but reads foreign to CLN users and would
  undercut the whole "lightning-cli for Greenlight" stance.

## Consequences

- Large 64-bit values (e.g. msat) are emitted as JSON numbers. `glrdr`'s own
  output text is exact (serde_json preserves u64), but downstream float-based
  parsers can lose precision above 2^53 — identical to `lightning-cli`'s own
  behavior, so the trade-off is inherited rather than introduced.
- The custom serializer and the input field-name mapping must track the bundled
  descriptor; refreshing protos may shift field names/types and should be
  eyeballed against CLN's documented schema.
- `--raw` mode is exempt: it is bytes-in/bytes-out (hex) with no CLN-flavored
  rendering, by definition.
