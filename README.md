# glrdr

A small command-line client for calling RPCs on a
[Greenlight](https://blockstream.com/lightning/greenlight/)-hosted
[Core Lightning](https://github.com/ElementsProject/lightning) node over gRPC.

Think of it as `lightning-cli` for a Greenlight node. It's the gRPC sibling of
[`cln-rdr`](https://github.com/nepet/cln-rdr): a thin pass-through that takes a
method name and some `key=value` params, makes the call, and prints the JSON
back. Authentication rides on `gl-client`, so every request is signed and
verified end-to-end by your node — the Greenlight proxy in the middle can't
forge it.

`glrdr` deliberately does one thing: make calls. It never holds your seed, never
runs a signer, and doesn't register or recover nodes — that all lives in
`glcli`. (See [ADR 0001](docs/adr/0001-rdr-is-a-pure-passthrough-client.md).)

## Install

```bash
cargo install --path .
```

You'll need `protoc` on your PATH (`brew install protobuf`) — the `gl-client`
dependency compiles `.proto` files during the build. `cargo install` puts
`glrdr` on your PATH; a plain `cargo build --release` leaves it at
`target/release/glrdr`.

> The crate currently pins `gl-client` by local path. That switches to
> `gl-client = "0.6"` from crates.io once published.

## A first call

You need two things: a credentials file, and to know which network your node is
on.

```bash
export GL_CREDS=~/.gl/creds
glrdr getinfo
```

That's the whole flow for read-only calls. `glrdr` reads the node id from your
credentials, asks the Greenlight **scheduler** where the node is living right
now, connects, and makes the call.

**Credentials** are a single Device blob — cert, key, CA, and a rune — that
`glcli` writes (`glcli scheduler register` the first time, or `glcli scheduler
recover` if you've lost them). `glrdr` only ever reads this file; if it's
missing or has no rune, it says so instead of failing cryptically.

**Network.** The scheduler defaults to mainnet
(`scheduler.gl.blckstrm.com`). If your node lives somewhere else, set
`GL_SCHEDULER_GRPC_URI` — otherwise the scheduler simply won't find it. To skip
discovery and hit a known address directly:

```bash
glrdr --grpc-uri https://node.example.com:7171 getinfo
```

## Parameters

Params are `key=value` pairs using the proto field names — snake_case, exactly
what `lightning-cli` takes:

```bash
glrdr pay bolt11=lnbc... amount_msat=100000
```

Values are parsed smartly: `100000` becomes a number, `true` a bool, bare words
stay strings. Force it with `--text` (everything is a string) or `--strict-json`
(everything must be valid JSON). Or hand over the whole payload yourself:

```bash
glrdr pay --params-json '{"bolt11":"lnbc...","amount_msat":100000}'
```

(`-k` is accepted for muscle memory but isn't needed — pairs are detected
automatically.)

## Finding your way around

`glrdr` bundles the full schema, so it can tell you what's available:

```bash
glrdr help          # every method, grouped by service
glrdr help pay      # the fields `pay` takes
```

Bare names resolve against `cln.Node` (the Core Lightning surface) first, then
`greenlight.Node`. Pin one with `--service`, or name it outright:
`glrdr cln.Node/ListFunds`.

## Output

Responses come back as pretty JSON in CLN's style — 64-bit values as numbers,
byte fields as hex, enums by name — so it pipes straight into `jq` and existing
`lightning-cli` scripts. Like `lightning-cli`, large msat amounts are emitted as
JSON numbers, so float-based parsers can lose precision above 2^53. More in
[ADR 0002](docs/adr/0002-lightning-cli-fidelity.md).

## Signing: what works without a signer

Greenlight keeps your keys in a separate signer, not in `glrdr`. So:

- **Read calls** (`getinfo`, `listfunds`, `listpeers`, …) work on their own.
- **Calls that move money** (`pay`, `keysend`, `withdraw`, `fundchannel`, …) are
  sent fine but won't *complete* until a signer answers. Run one on your
  always-on box: `glcli signer run`.

## Raw mode

For anything the bundled schema doesn't cover, `--raw` is a dumb pipe: give it
the gRPC path and a hex-encoded protobuf request, get hex back.

```bash
glrdr --raw /cln.Node/Getinfo
glrdr --raw /cln.Node/ListFunds 0a00
```

## Security

The Device blob is a credential — it holds the device key and a rune, and anyone
with it can make any call the rune allows. Treat it like a private key, prefer
`GL_CREDS` over `--creds` to keep it out of shell history, and if you copy it to
a less-trusted machine, scope its rune down first. The seed itself never touches
`glrdr`.

## License

MIT. See [LICENSE](./LICENSE).
