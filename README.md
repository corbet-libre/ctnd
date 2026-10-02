# ctnd — Attend

Implementation in progress. No production success is available without the
required verified lower-layer capabilities.

## Scope

### Purpose

The member side of public rooms: joining with signed consent to mutual profile visibility through an ordered join protocol, with profile keys handed over inside the group and only bounded history for newcomers.

### Owns

Signed visibility consent at joining; external join handled by a rotating online admitting member selected from epoch and room without creating a privileged role; profile-key handover as encrypted group messages with paced joins (at most 6 joins per hour in rooms above 59 members) and newcomer history limited to the band window and count; room-pass requests through Link with retry when the admitter is offline and state persisted before ordering.

### Never

No second forum connection; no profile key or roster sent to the server; no access from unsigned consent or unordered commits; no history beyond policy; no direct first contact without a Wave; no administrator role; no new MLS implementation or cryptographic primitive.

### States

Outside to ConsentPending to Joining to AwaitingOrderedCommit to Joined with an epoch; gaps or restart enter Reconciling; leave, revocation, or shrink enters Withdrawn.

### Test obligations

Real external joins with consent through Groups, Threads, Link, and Assembly; deterministic admission survives an offline admitter without extra authority; replay, conflict, and gap recovery; join pacing holds; key handover travels only as encrypted group messages; Circle newcomers receive no history and other bands receive only their window; blocked, departed, and non-consenting forks gain no visibility. Common obligations apply: native and Wasm builds with identical vectors, explicit state machine with injected clock, randomness, storage, and network ports, full line and branch coverage with real round trips and injected faults, atomic publication with reconciliation of unknown outcomes, per-community isolation, bounded work with secrets kept out of errors, reuse of maintained third-party code with no own cryptography, and no personal identifiers in outbound requests.

## Validation

Stable native and actual wasm vectors; exact 100% line and branch coverage on
nightly LLVM. Current green status must come from the exact CI run, never from
this contract. Expensive room load tests use owned CI and real Tor when the
lower layers are ready. See [Contract](docs/CONTRACT.md).

## Reuse

Use existing key, guard, MLS, Link and volatile storage owners. Serde and
thiserror provide formats and bounded errors. No own cryptographic primitives.
