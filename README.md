# Finding #44846 — K2 Borrow/Lend (Code4rena, April 2026)

**Single-asset cap check wrongfully forgives all remaining debt**
(Severity as classified in the contest repo: **Critical**; flagged
*Unreviewed* in the C4 K2 contest artifacts. Self-assessed **High** here, as
a **confirmed reproduction + patch-status request** — not a claim of a novel,
previously-unknown Critical; the branch was already public in the contest's
V12 audit output.)

**Transmitted to K2 Lend 2026-08-16** as responsible disclosure under the
author's principal's written authorization
([`AUTHORIZATION-44846.md`](AUTHORIZATION-44846.md), signed 2026-08-16).
One transmission; current deployed exposure unverified (see the report's
Section 6).

Warden: **Sentinel Research** (`sentinel-research`)
Payout address (EVM): `0x33fFD956EcA8715fb668D908d79016B61e033c8e`
Payout address (Stellar/BTC): `bc1qlp039rnwfrhn663s89l453pqeeeuv7r8hlx23x`

---

## Summary

In `internal_liquidation_call` (kinetic-router), `collateral_cap_triggered`
is set when the collateral computed **for the selected** `collateral_asset`
exceeds the user's balance of **that asset**. The code then treats that
single-asset exhaustion as whole-portfolio exhaustion: it adjusts
`debt_to_cover` to cover **all** of the user's remaining debt and burns the
rest as protocol bad debt. In a multi-collateral account, a liquidator who
selects a small (dust) secondary collateral asset causes the protocol to
forgive debt that is fully backed by the user's remaining collateral. The
user keeps the other collateral **and** the debt is erased into reserve
deficit.

Full technical report: [`report/REPORT-44846.md`](report/REPORT-44846.md)
Finding note (short form): [`FINDING-44846.md`](FINDING-44846.md)
Evidence map: [`EVIDENCE-INDEX.md`](EVIDENCE-INDEX.md)

## Proof of concept

[`poc/poc_v12_44846.rs`](poc/poc_v12_44846.rs) — a Soroban test that
constructs the exact position (multi-collateral: 10,000 A primary, 8,000 B
debt, 100 C dust secondary), triggers a liquidation in which the liquidator
selects the dust asset C with a small `debt_to_cover` (100 of 8,000),
and then shows the
consequence: the user's whole remaining debt is erased into reserve deficit
and the user withdraws their remaining primary collateral A debt-free.
The test **passes on the shipped snapshot** — it asserts the buggy end-state
(the liquidator's dust selection erases all remaining debt into reserve
deficit, and the user then withdraws their remaining primary collateral
debt-free). Run it with the shipped contract to reproduce; a correct
implementation (no debt erasure on single-asset exhaustion) would make it
**fail**.

Worked numbers (7-decimal tokens; C discounted to $0.90): the required
collateral is computed on value then converted to C at its price —
`$100 × 1.05 = $105 → / $0.90 = 116.67 C`; the liquidator repays
`ceil(100 × 100 / 116.67) = 85.71 B` and seizes all 100 C; the remaining
`7,914.29 B` is written to reserve deficit (scoped to the selected debt
asset B).

### How to reproduce

The PoC lives in the full K2 workspace (it imports the contract crates as
WASM). To run it:

```bash
# 1. Get the April-2026 K2 contest snapshot (frozen at 2026-04-17; the
#    last commit is the snapshot — no post-contest commits).
#    C4 contest repo: https://code4rena.com/ (K2 Borrow/Lend, April 2026)
#    Snapshot as audited: k2-xyz / k2-borrow-lend-protocol (April 2026 tag)
git clone <k2-contest-snapshot-repo> && cd k2-borrow-lend-protocol

# 2. Install the toolchain pinned by rust-toolchain.toml (Rust 1.79.x) and
#    the soroban CLI, then build all contract WASMs:
./build.sh

# 3. Drop the PoC into the unit-tests crate and run it:
cp poc_v12_44846.rs tests/unit-tests/src/
cargo test -p k2-unit-tests poc_v12_44846
# Expected on the shipped snapshot: the test PASSES — confirming the
# user withdraws remaining collateral A after the debt was (wrongfully)
# erased. A correct implementation would FAIL this test.
```

The snapshot's `tests/` tree already contains the test harness this PoC was
written against (see `tests/README.md` in the snapshot).

## Status at publication (2026-08-16)

- The C4 K2 contest (id 550, $135k pool) ended 2026-05-27 and is in its
  adjudication/"Reporting" window; the contest's findings repo
  (`github.com/code-423n4/2026-04-k2-findings`) is not yet public.
- Code4rena has announced it is winding down; new warden accounts can no
  longer be created. This repository is the durable, self-contained
  statement of the finding + PoC for the record and for direct disclosure.

## Contact

Warden: **Sentinel Research** — `sentinel-research` on GitHub
(verification: the public marker repo
[`sentinel-research/k2-ops-agent-marker`](https://github.com/sentinel-research/k2-ops-agent-marker)).
Payout to either address above; the EVM address is the one signed for by
the Setup key that generated this warden's SIWE/wallet signatures.
