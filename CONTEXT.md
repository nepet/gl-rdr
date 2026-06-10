# gl-rdr

A small, future-proof command-line client that calls a Greenlight-hosted Core
Lightning node over gRPC. It is the Greenlight counterpart to `cln-rdr`: where
`cln-rdr` is a pass-through over Commando/lnsocket, `gl-rdr` is a pass-through
over Greenlight's gRPC, reusing `gl-client`'s authentication so calls are
verified end-to-end by the node.

## Language

**Node**:
The user's own Core Lightning instance running on Greenlight infrastructure.
The thing `gl-rdr` ultimately talks to.

**Device**:
The identity `gl-rdr` authenticates as — a certificate, its private key, the
CA, and a rune, stored together as one byte blob (`gl-client`'s `Device`
credentials). Distinct from the **Signer**.
_Avoid_: "client cert", "identity file" (these are parts of a Device).

**Rune**:
The capability token that authorizes a specific set of RPC calls on the
**Node**. Carried as gRPC metadata on every call.

**Signer**:
The component that holds the node's secret keys and answers the node's
signature requests. `gl-rdr` is **not** a signer; calls that need a signature
to complete depend on a signer running elsewhere.

**Scheduler**:
The Greenlight service that tells a client the current gRPC address of a
**Node**. Greenlight nodes do not have a stable address, so discovery goes
through the Scheduler.
_Avoid_: "load balancer", "proxy".

**Service**:
A gRPC service exposed by the node: `cln.Node` (the full Core Lightning
surface, the `lightning-cli` analog) or `greenlight.Node` (Greenlight-specific
methods). `cln.Node` is the default.

**Method**:
The friendly, case-insensitive RPC name a user types (`getinfo`). Resolved
against the bundled descriptor to a **Path**.
_Avoid_: "command", "RPC" used interchangeably with Path.

**Path**:
The fully-qualified gRPC route a Method resolves to, e.g. `/cln.Node/Getinfo`.
The user may also supply a Path explicitly to skip name resolution.

**Generic call**:
A call made through `gl-client`'s `GenericClient`: protobuf bytes in, protobuf
bytes out, no per-method code. The single seam `gl-rdr` rides on.

**Authenticated request**:
The end-to-end verification mechanism. `gl-client`'s `AuthService` signs
`(request body ‖ timestamp)` with the Device key and attaches `glauthpubkey`,
`glauthsig`, `glts`, and `glrune` metadata. The Node verifies these directly,
so the Greenlight proxy cannot forge a request.
_Avoid_: "auth header", "JWT".

**Raw mode** (`--raw`):
Bypasses JSON↔protobuf conversion: the user supplies already-encoded protobuf
and receives raw response bytes. The future-proof escape hatch for anything the
bundled descriptor does not cover.

## Out of scope

These are deliberately **not** `gl-rdr`'s job — they belong to the seed
custodian (`glcli`). See [ADR 0001](docs/adr/0001-rdr-is-a-pure-passthrough-client.md).

**Registration**:
First-time enrolment of a **Node** with the **Scheduler**, producing the
**Device** credentials. Requires the seed and a **Signer**. Done with `glcli`;
`glrdr` only consumes the resulting Device blob.

**Recovery**:
Re-deriving **Device** credentials from the seed when they are lost. Same heavy
seed-custody machinery as Registration. Done with `glcli`.

**Running the Signer**:
`glrdr` never runs a Signer. Signature-requiring calls complete only when one is
attached elsewhere (`glcli signer run`, ideally on always-on hardware).

## Example dialogue

> **Dev:** When I type `glrdr getinfo`, what actually goes on the wire?
> **Expert:** `getinfo` is a **Method**. We resolve it against the descriptor to
> the **Path** `/cln.Node/Getinfo`, build the protobuf request, and hand it to
> the **Generic call**. `gl-client` wraps it as an **Authenticated request** —
> signs it with the **Device** key and attaches the **Rune** — so the **Node**
> can verify it end-to-end.
> **Dev:** And if the node added a brand-new method we haven't bundled yet?
> **Expert:** Then there's no descriptor entry to build from, so you drop to
> **Raw mode**: encode the protobuf yourself and pass the **Path** directly.
