# ctnd agent instructions

Write comments and documentation in English. Owned code forbids unsafe Rust
(`#![forbid(unsafe_code)]`, `src/lib.rs:2`).

## Build and test (lane: `~/agents/scratch/cbuild <lane> ...`)

- `fmt -p ctnd -- --check` (exit 0)
- `test -p ctnd --locked` (exit 0: 10 vectors + 1 refusal unit, doc 0)
- `python3 .ci/lt-rooms.py --validate-contract` (exit 0; plan shape only)
- CI-only (Crow `.ci/verify.sh`): `clippy --locked --all-targets -- -D warnings`
  (red locally: upstream `ckmg` lints under local 1.98, not this crate),
  wasm vectors (`--target wasm32-unknown-unknown --tests`), nightly
  `llvm-cov` 100% line+branch gate (`docs/COVERAGE.md`), `cargo deny`.

## Hard invariants (each with its enforcing test in `tests/`)

- A pending preparation never outlives `valid_until()`, the earliest
  signed-consent or original-policy expiry; a same-epoch policy renewal
  cannot extend it (`docs/CONTRACT.md:9-13`):
  `same_epoch_policy_renewal_does_not_extend_pending_consent`,
  `a_stored_checkpoint_restores_for_withdrawal_and_a_lost_attempt_never_resumes`.
- A join binds Assembly's nonzero volatile ordering generation plus its next
  counter; a mismatch enters Reconciling before any handover is released
  (`docs/CONTRACT.md:17-24`):
  `volatile_restart_and_swapped_order_never_publish_a_join`.
- A lost Joining checkpoint reconciles, never resumes; checkpoints carry the
  original bound and no raw identifiers (`docs/CONTRACT.md:12-15`):
  `real_encrypted_checkpoint_precedes_retry_and_handover`,
  `cancelled_checkpoint_stays_in_reconciliation`.
- Refusals never create join authority; empty or unbound evidence is refused:
  `phase_and_proof_refusals_never_create_join_authority`,
  `join_evidence_constructors_refuse_empty_or_unbound_input`.
- Policy changes and a real writer conflict block publication; history keeps
  its circle window, order and cap:
  `policy_changes_and_real_writer_conflict_block_publication`,
  `history_respects_circle_window_order_and_message_cap`.
- Consent requires a signed group preparation under the current policy:
  `signed_group_preparation_fences_consent_and_current_policy`.
- Vectors never establish unlinkability, MLS, Tor, restart proofs or
  end-to-end acceptance (`docs/CONTRACT.md:5-7`).

Pointers: `docs/CONTRACT.md`, `docs/COVERAGE.md`, `docs/integration/LT-ROOMS.md`.
