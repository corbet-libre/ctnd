# ctnd — Attend

Implementation in progress. No production success is available without the
required verified lower-layer capabilities.

## Scope

Device public-room participation through the sole Link port, verified Groups decisions and Threads MLS capabilities. Own consent/join progress, ordered-commit coordination, encrypted key handover, bounded history and offline-admitter retry. No second forum connection, group administrator, raw-roster authority, new MLS, accounting ledger or cryptographic primitive.

## Validation

Stable native and actual wasm vectors; exact 100% line and branch coverage on
nightly LLVM. Current green status must come from the exact CI run, never from
this contract. Expensive room load tests use owned CI and real Tor when the
lower layers are ready. See [Contract](docs/CONTRACT.md).

## Reuse

Use existing key, guard, MLS, Link and volatile storage owners. Serde and
thiserror provide formats and bounded errors. No own cryptographic primitives.
