# Contract

Device public-room participation through the sole Link port, verified Groups decisions and Threads MLS capabilities. Own consent/join progress, ordered-commit coordination, encrypted key handover, bounded history and offline-admitter retry. No second forum connection, group administrator, raw-roster authority, new MLS, accounting ledger or cryptographic primitive.

Production integration remains blocked until verified owner capabilities exist.
Independent transition tests never establish permit unlinkability, MLS, Tor,
restart proof correctness or end-to-end acceptance.

A pending preparation retains `PreparedEffect::valid_until()`, the earliest
signed-consent or original-policy expiry. Every prepare/order/accept transition
checks that bound as well as the current authenticated policy epoch. A
same-epoch policy renewal cannot extend the consent. Checkpoints store the
original bound; production loaders must restore it without modification.
Persist-before-order and reconciliation-before-await also cover cancellation.
Already committed encrypted outbox delivery is distinct from a pending join.

Pending joins bind Assembly's nonzero volatile ordering generation in addition
to its next counter. A restart can repeat a numeric counter; that does not make
the new generation interchangeable with the member's retained request. Both
the Link reply and sealed Threads evidence must match the retained generation
and next counter. A mismatch enters Reconciling before any handover is released.
The generation is not an MLS epoch, membership proof or permission to resume
without checking other members. Older checkpoint encodings lack this mandatory
field and must refuse rather than defaulting to a current room generation.
