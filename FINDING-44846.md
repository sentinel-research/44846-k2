# FINDING — K2 Lend (Stellar/Soroban): single-asset collateral-cap
# forgiveness erases all remaining debt in a multi-collateral position

- Severity (self-assessed): **High** (direct protocol fund loss: the
  remaining balance of the selected debt asset is forgiven into reserve
  deficit while the borrower keeps other collateral). The contest's V12
  note classified it Critical (Unreviewed); this report self-assesses High
  as a confirmed reproduction + patch-status request.
- Source: V12 AI-audit note **#44846** ("Single-asset cap check wrongfully
  forgives all remaining debt"), marked **Validity: Unreviewed** in the
  Code4rena `2026-04-k2` contest repo. Independently re-verified this tick
  (2026-08-15) with a passing end-to-end PoC.
- Target code: `contracts/kinetic-router/src/liquidation.rs`,
  `internal_liquidation_call` (the `collateral_cap_triggered` / H-02 / H-05 /
  N-08 branch), `contracts/kinetic-router/src/calculation.rs`
  `calculate_liquidation_amounts_with_reserves`.
- PoC: `tests/unit-tests/src/poc_v12_44846.rs`
  (`poc_44846_single_asset_cap_forgives_all_debt`) — **PASSES** against the
  April-2026 C4 snapshot (build verified: `cargo check` clean; 13 wasm32
  contract builds OK).

## Root cause

`collateral_cap_triggered` is set when, for the **single selected**
`collateral_asset`, the computed `collateral_amount_to_transfer` exceeds the
user's balance **of that one asset**. The code then treats that single-asset
exhaustion as **whole-position** exhaustion:

1. N-08: `debt_to_cover` is scaled to `ceil(dtc * ucb / cat)` (the amount that
   corresponds to the full selected-asset balance).
2. H-02/H-05: because `collateral_cap_triggered`, the **entire remaining
   balance of the selected debt asset** is burned via `burn_scaled` and moved
   to `add_reserve_deficit` (protocol bad debt), on the assumption that "all
   collateral is seized — remaining debt is unrecoverable." The burn is
   scoped to this leg's `debt_asset`, not to every debt asset in a
   multi-debt portfolio.

Assumption 2 is false in a multi-collateral account: the borrower can still
hold large balances in **other** collateral assets. Only the one selected
asset was exhausted.

## Exploit (proven by the passing PoC)

Setup (7-decimal tokens; prices 14-dec; LT=8500 bps, LTV=8000, bonus=500 bps;
partial-liq threshold 0.5 WAD):
- Borrower holds: 10,000 A (primary collateral) + 100 C (dust secondary
  collateral) and owes 8,000 B.
- At $1.00: HF = (10100 × 0.85) / 8000 = 1.073 (healthy).
- Price shock: A and C → $0.90. HF = (10100 × 0.9 × 0.85) / 8000 = 0.966
  (liquidatable; still > 0.5 ⇒ close factor 5000).
- Liquidator (the borrower's own Sybil — liquidation whitelist is OFF by
  default, `is_address_whitelisted_for_liquidation` returns true when
  `LIQ_WHITELIST_FLAG` is unset) selects the **dust collateral C** and
  requests `debt_to_cover = 100` B.
  - The collateral required is computed on value and converted to C tokens at
    C's discounted price: `100 B → $100 → × 1.05 = $105 → / $0.90 = 116.67 C`.
    `116.67 > 100` (C balance) ⇒ cap triggers.
  - N-08 scales debt to `ceil(100×100/116.67) = 85.71`.
  - H-02/H-05 burns the **remaining 7,914.29 B** as bad debt / reserve
    deficit.

Net effect (all asserted in the PoC):
- Protocol loses 7,914.29 B of debt (moved to reserve deficit).
- Borrower's `total_debt_base` → ~0.
- Borrower **keeps the 10,000 A** (worth ~$9,000) and **withdraws it
  debt-free** (`try_withdraw` succeeds).

The beneficiary is the **borrower** (whose debt is erased). The liquidation
leg is run from a Sybil liquidator the borrower funds with 85.71 B and receives
the 100 C back (worth ~$90 at the discounted price). In a deeply-underwater position (HF < 0.5 ⇒ close factor
100%) the full remaining debt can be erased in one leg.

## Why it matters (impact)
A borrower can shed their debt in the selected debt asset (in a single-
debt position: their entire borrowed position) by liquidating a dust
secondary collateral, converting recoverable debt into protocol deficit
while retaining all other collateral. Direct loss of protocol value.

## Novelty / prior-art check (primary source, this tick)
- NOT present in the two professional audit reports committed to the repo:
  - WatchPug rev3 (4 rounds, Oct 2025 – Mar 2026): no matching finding
    (keyword search: single-asset / cap check / forgive / portfolio
    exhaustion / dust collateral / multi-collateral ⇒ 0 hits).
  - Halborn (Sep 2025): no matching finding (1 unrelated hit re "dust
    collateral reserves").
- V12 (autonomous AI auditor) flagged it as Critical but **Unreviewed** — it
  did NOT produce a passing PoC in the artifact, and it was never
  adjudicated in the materials available. This PoC is the first passing
  end-to-end reproduction I can find.

## Open items (NOT yet resolved — must verify before any disclosure)
1. **Contest adjudication**: was V12 #44846 accepted (⇒ patched) or
   rejected/invalid in the final Code4rena `2026-04-k2` report (contest
   ended 2026-05-27)? The final report is not exposed in the pages I could
   reach (`/reports/2026-04-k2` → 404). This determines whether the finding
   is already known+patched (no value) or known+rejected (independent PoC
   has value) — and I need the report.
2. **Live-deployed status**: the README states the audit "includes
   live/deployed code" (deployed to mainnet.stellar.org). I have NOT verified
   whether this exact branch is still present in the *currently deployed*
   contract (the C4 snapshot is April-2026; the protocol may have patched it
   during/after the contest). I need the live contract source/WASM or a
   statement from the team.
3. **Disclosure venue**: K2 has no Immunefi program I could confirm (both
   slug guesses 307-redirected with no program). Disclosure would go to K2's
   security contact (k2lend.com / x.com/K2_Lend). Requires the principal's
   authorization + a channel (see outstanding inbox request).

## Reproduce
    source $HOME/.cargo/env
    cd ~/mission/earn/2026-04-k2
    # (wasm32 builds already done: target/wasm32v1-none/release/*.optimized.wasm)
    cargo test -p k2-unit-tests poc_44846
    # => test poc_v12_44846::poc_44846_single_asset_cap_forgives_all_debt ... ok

Note on the environment: the repo's committed `Cargo.lock` pins
`ed25519-dalek 3.0.0` (needs `rand_core 0.10`), which is incompatible with
`soroban-env-host 23.0.1`'s test `ChaCha20Rng` (written for `rand_core 0.9`).
To build the test suite I locally ran
`cargo update -p ed25519-dalek@3.0.0 --precise 2.2.0` (original lockfile
backed up at `/tmp/k2-Cargo.lock.orig`). This is a **test-harness**
dependency quirk, not a product issue.
