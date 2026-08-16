# Evidence index — K2 #44846 disclosure package

> **INTERNAL WORKING FILE — NOT part of the send bundle** (see README.md).
> Reference for the principal/agent; not to be transmitted to K2.

Maps every substantive claim in REPORT-44846.md to a durable, committed,
reproducible source. Nothing in the report is an unbacked assertion.

All paths relative to the K2 corpus repo (~/mission/earn/2026-04-k2/).
git HEAD at assembly (tick 40): dbe2040 (package commit; snapshot base 2485e33).
t47 (2026-08-16 05:33 UTC): repro command `cargo test -p k2-unit-tests
poc_44846` re-run fresh from current HEAD — 1 passed / 0 failed; broader
`poc_448` filter 3 passed / 0 failed (all evidence links still valid).

## The finding
| Claim | Source |
|---|---|
| Root-cause branch exists in `internal_liquidation_call` (single-asset cap ⇒ portfolio-wide deficit burn) | `contracts/kinetic-router/src/liquidation.rs` (N-08 / H-02 / H-05 markers), read in full tick 24 |
| `collateral_amount_to_transfer` / debt scaling math | `contracts/kinetic-router/src/calculation.rs` `calculate_liquidation_amounts_with_reserves` |
| Exploit is real & reproducible (position, price shock, dust leg, debt→deficit, debt-free withdraw) | `tests/unit-tests/src/poc_v12_44846.rs` (354 lines, commit f2dd56d) — PASSING; re-run green from HEAD tick 40 (`cargo test -p k2-unit-tests poc_448` → 3 passed / 0 failed) |
| Liquidation whitelist OFF by default (Sybil liquidator possible) | `is_address_whitelisted_for_liquidation` in the router (returns true when `LIQ_WHITELIST_FLAG` unset) |
| Full technical writeup (root cause + exploit + impact) | `FINDING-44846.md` (repo root, commit f2dd56d) |

## Novelty / prior art
| Claim | Source |
|---|---|
| Not in WatchPug rev3 (4 rounds) or Halborn reports | Keyword search across `k2-watchpug-audit-report-rev3.pdf` + the Halborn report committed in-repo ⇒ 0 matching hits (tick 24); both PDFs present in repo root |
| Internal AI-audit note flagged it but left Unreviewed, no PoC | `K2-V12-Critical-output.md` (note #44846), in repo root |
| This is the first passing end-to-end reproduction | `poc_v12_44846.rs` (f2dd56d) |

## Live deployment
| Claim | Source |
|---|---|
| K2 live on Stellar mainnet; full deployment manifest (contract IDs, reserve params, artifact hashes, admin) | `live/k2-mainnet-manifest.json` (commit 6b1f819), fetched from `app.k2lend.com/k2-mainnet-manifest.json` |
| Router contract exists on mainnet | soroban-rpc `getContractDetails` on `CCTUJZLY…` → exists=true (tick 34) |
| Liquidation system deployed (router + liquidation-engine both in deployed set) | `live/ONCHAIN-VERIFICATION.md` (commit c41ee4e), item 4 |
| Deployed bytes NOT confirmed identical to snapshot (honest residual) | `live/ONCHAIN-VERIFICATION.md` items 5–7 (3 endpoint families, 3 ticks, all read-only, all fail); hash comparison 0/15 = non-informative |

## Non-findings (so reviewers don't re-derive)
| Claim | Source |
|---|---|
| aToken unauth `initialize`: code-fact on fresh deploy; live re-init defeated by sdk 23.5.3 archived-entry restoration | `tests/unit-tests/src/poc_v12_44869.rs` (commit 948faf1) |
| Reserve-counter TTL reuse invalidated (same restoration property) | `K2-V12-Critical-output.md` #44792/#44793 invalidation + poc_v12_44869.rs empirical confirmation |
| flash-loan / debt-token / a-token / swap-DEX cleared, no new fund-loss | state.md module-pass entries (ticks 26–30) |

## Build note (for anyone reproducing)
The repo's committed `Cargo.lock` pins `ed25519-dalek 3.0.0` which is
incompatible with `soroban-env-host 23.0.1`'s test RNG. Local build uses
`cargo update -p ed25519-dalek@3.0.0 --precise 2.2.0` (original lockfile
backed up at `/tmp/k2-Cargo.lock.orig`). This is a test-harness dependency
quirk, not a product issue.
