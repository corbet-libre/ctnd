# Room integration contract

The adjacent plan contains all six LT-ROOMS loads and thresholds. The manual
`.crow/ccid.yaml` invokes the shared verified command library with the
`lt-rooms` selector. Hosted CI must not execute these scale, churn or load cases.
The cheap `--validate-contract` mode validates only configuration.

The owning application must supply an exact SHA-256-bound driver artifact. The
driver accepts `--plan FILE --output DIRECTORY`, uses the generated cmsg action
registry and actual owner adapters, and writes `acceptance.json` only after all
six cases pass. The receipt contains `suite: LT-ROOMS`, `status: passed`,
`cases: [1,2,3,4,5,6]`, and an `artifacts` mapping from every required plan
artifact to its absolute job-owned path and SHA-256. The launcher checks
artifact identity and completeness; it does not certify the driver's semantics.
Driver source, exact dependency locks, real measurement traces and the receipt
require review together. No driver or load success currently exists here.

The driver must establish signed consent through Groups; perform the actual
OpenMLS external commit and encrypted profile/history handover through Threads;
order and relay solely through Link and Assembly; retry an offline admitter
without a new role; and converge after lost replies and volatile restart. Tor
must be real, with bootstrap/circuit and peer-delivery traces. An in-process or
direct socket path cannot satisfy the transport case. The phone case needs an
actual low-end phone runtime measurement; desktop wasm timing is insufficient.

Production cap remains 100. The declared 150/250/500 capacity probes require an
explicitly scoped test driver and owner-reviewed protocol fixture; this package
does not raise the product cap or mint substitute membership/permit authority.

Current blockers: anonymous permit/restart/handover construction; cthr sealed
room preparation and verified order capability; clnk live room transport;
current signed policy/G3/Contacts and shared cwst adapters; true Valkey parity;
all six application-level driver cases and phone evidence. Crow repository
registration and driver-archive binding must be verified before dispatch.
Absent artifacts produce a typed blocked result, never a skipped green suite.
