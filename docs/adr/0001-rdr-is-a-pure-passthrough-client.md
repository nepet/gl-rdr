# glrdr is a pure pass-through client; key custody and node lifecycle stay out

`gl-rdr` ships a single binary, `glrdr`, that is a generic gRPC pass-through to a
Greenlight-hosted node: it reads a **Device** credentials blob and calls any
RPC, holding nothing secret. Registration, recovery, and the **Signer** — which
all require the seed (`hsm_secret`) and the VLS/backup stack — are deliberately
**out of scope** and remain the job of `glcli`. The model is `lightning-cli`
(thin client) vs `lightningd` + HSM (key custody + lifecycle): `glrdr` is the
former, `glcli` is the latter.

## Considered Options

- **Pure pass-through, defer everything else to `glcli` (chosen).** Keeps `glrdr`
  razor-thin and future-proof, with no seed handling and no `vls`/`backup`/
  `bip39` dependencies. Signature-requiring calls (`pay`, `withdraw`, …)
  complete only when a signer is attached — run `glcli signer run`, ideally on
  always-on hardware.
- **Integrate a signer / registration into `glrdr`.** Rejected: registration,
  recovery, and signing are one heavy seed-custody subsystem. Pulling it in
  would duplicate `glcli` (same author) and spend the one thing that makes
  `glrdr` worth building — being the clean generic client `glcli` lacks. A forked
  signer is also a fund-loss risk and must never be hand-rolled.
- **Optional feature-gated `rdr-signer` companion binary.** Rejected for now:
  `glcli` already runs `gl_client::Signer`; shipping our own daemon adds
  maintenance with no benefit over reusing it.

## Consequences

- Greenlight's capability-scoped rune is leveraged directly: the always-on box
  holds the seed and runs the signer; other machines get a restricted Device
  blob and run `glrdr`. The seed never reaches `glrdr`.
- `glrdr`'s onboarding depends on `glcli` (or equivalent) for first-time
  credential creation: get a Device blob via `glcli scheduler register` /
  `recover`, then point `glrdr` at it.
