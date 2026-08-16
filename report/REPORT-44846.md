# Responsible disclosure — K2 Lend: single-asset collateral-cap check
# forgives all remaining debt (multi-collateral fund loss)

**Severity (self-assessed): High / Critical** — direct protocol fund loss:
recoverable debt is written off into reserve deficit while the borrower keeps
and withdraws their other collateral.

**Author:** Sentinel Research

**Status of this report:** prepared for responsible disclosure. No part of this
report has been transmitted to any third party.

---

## 1. Executive summary

In K2 Lend's kinetic router, a borrower holding **multiple** collateral
assets can erase an arbitrarily large portion of their debt by having a
complicit liquidator liquidate a **dust-sized secondary collateral**.

The single-asset cap branch (`collateral_cap_triggered` in
`kinetic-router/src/liquidation.rs`) treats the exhaustion of *one* selected
collateral asset as exhaustion of the *entire* portfolio: it burns the
**entire remaining debt** across all assets as protocol bad debt
(`add_reserve_deficit`) even though the borrower still holds substantial
balances in other collateral assets — which they then withdraw debt-free.

A fully reproducible, passing PoC is provided (Section 8). The loss in the
PoC is ~$7,900 of debt forgiven while the borrower retains ~$9,000 of
primary collateral, using only ~96 USDC of liquidator capital. In a
deeply underwater position (HF < 0.5, close factor 100%) the **entire**
remaining debt can be erased in one leg.

This is a self-inflicted exploit: the borrower is the economic beneficiary
and funds their own Sybil liquidator. No third party, no oracle
manipulation, no flash loan, no privileged role required (the liquidation
whitelist is OFF by default).

## 2. Environment

- Codebase: K2 kinetic-router, Stellar/Soroban, Rust, soroban-sdk 23.5.3.
- Analyzed snapshot: the Code4rena `2026-04-k2` public snapshot
  (base commit 2485e33), which per the contest README includes
  live/deployed code.
- Deployment: K2 is **live on Stellar mainnet** (verified read-only; see
  Section 6 for what is and is not claimed about the live bytes).

## 3. Root cause

`internal_liquidation_call` (kinetic-router/src/liquidation.rs) computes,
for the **single selected** `collateral_asset`:

1. `collateral_amount_to_transfer` — the amount of the selected asset needed
   to cover the requested `debt_to_cover`.
2. If `collateral_amount_to_transfer` exceeds the user's balance **of that
   one asset**, `collateral_cap_triggered = true` (the H-02 / H-05 / N-08
   branch).

The branch then performs two steps whose joint premise is false in a
multi-collateral account:

1. **N-08**: `debt_to_cover` is scaled to `ceil(dtc * ucb / cat)` — the debt
   corresponding to the full balance of the *selected* asset. Correct so far:
   we are only liquidating that one asset.
2. **H-02/H-05**: because `collateral_cap_triggered`, the code assumes
   "all collateral is seized — the remaining debt is unrecoverable", and
   burns the **entire remaining debt** (`burn_scaled`) across the whole
   position into `add_reserve_deficit` (protocol bad debt).

Step 2's premise is wrong when the user holds other collateral assets. Only
the selected asset was exhausted; the other assets remain in the position,
recoverable, and withdrawable.

## 4. Exploit (proven)

Setup (7-decimal tokens, 14-decimal prices, LT=8500 bps, LTV=8000,
liquidation bonus 500 bps, partial-liquidation threshold 0.5 WAD):

- Borrower holds: **10,000 A** (primary collateral) + **100 C** (dust
  secondary collateral); owes **8,000 B** (debt token).
- At $1.00: HF = (10,100 × 0.85) / 8,000 = **1.073** (healthy, not
  liquidatable).
- Market moves: A and C → $0.90. HF = (10,100 × 0.9 × 0.85) / 8,000 =
  **0.966** → liquidatable; HF > 0.5 ⇒ close factor 5000.
- The borrower's Sybil liquidator (whitelist OFF by default —
  `is_address_whitelisted_for_liquidation` returns true when
  `LIQ_WHITELIST_FLAG` is unset) selects the **dust collateral C** and
  requests `debt_to_cover = 100 B`:
  - `collateral_amount_to_transfer = 100 × 1.05 = 105 > 100` (C balance)
    ⇒ cap triggers.
  - N-08 scales the leg to `ceil(100 × 100 / 105) = 96 B`.
  - H-02/H-05 then burns the **remaining ~7,904 B** into reserve deficit.

Result (all asserted in the PoC):

| Party | Position before | Position after |
|---|---|---|
| Protocol | 8,000 B outstanding debt | ~96 B (rest → deficit) |
| Borrower debt | 8,000 B | ~0 B |
| Borrower collateral | 10,000 A + 100 C | 10,000 A (100 C paid to liquidator) |
| Borrower action | — | `try_withdraw` 10,000 A **succeeds** |
| Liquidator cost | — | ~96 B funded, gets 100 C back |

The borrower spent ~96 B to shed a ~$7,900 debt while keeping ~$9,000 of
collateral. In a deeply underwater position (HF < 0.5 ⇒ close factor 100%)
the full remaining debt is erased in the same single leg.

## 5. Why existing guards don't stop it

- **Health factor gate**: the position is only liquidatable after a real
  price move; the exploit uses the *intended* liquidation path, not a
  bypass.
- **Per-asset cap (N-08)**: correctly caps the *selected* asset at its
  balance — but the downstream deficit burn (H-02/H-05) is portfolio-wide.
- **`LIQ_WHITELIST_FLAG`**: when unset, **all** addresses may liquidate —
  so a Sybil liquidator is trivial. Even with the whitelist ON, the protocol
  operator whitelists addresses it controls; the economic beneficiary is
  still the borrower.
- **No minimum-dust check** on the selected collateral: 100 units of a
  $100,000-position is enough.
- Prior audit rounds (WatchPug rev3, Halborn — both committed to the repo)
  contain **no** finding on this branch (keyword searches: single-asset /
  cap check / forgive / portfolio exhaustion / dust collateral ⇒ 0 hits).
  An internal AI-audit note (#44846) flagged the same branch as Critical
  but was left **Unreviewed** with no PoC; this report provides the first
  passing end-to-end reproduction.

## 6. Live deployment — what is and is not claimed

**Claimed (verified, read-only, 2026-08-16):**
- K2 is live on Stellar mainnet. The production app
  (`app.k2lend.com`) publishes a public deployment manifest
  (`/k2-mainnet-manifest.json`) listing every deployed contract ID, live
  reserve parameters, deployed WASM artifact hashes, and the admin account.
- The manifest's kinetic router contract
  `CCTUJZLYFAW7ZNQD2SXMUZIHBUUJJICYRKWLZJ6SK6TGNAWNXOJIV6J7` exists on
  mainnet (soroban-rpc `getContractDetails` → exists=true).
- The liquidation system is deployed: the logic is split across
  `kinetic-router/src/liquidation.rs` and the standalone
  `k2_liquidation-engine` contract (initialize / calculate_liquidation /
  execute_liquidation); **both** contracts are in the manifest's deployed
  set.

**Not claimed (unverifiable from outside):**
- That the *currently deployed* router bytes are identical to the audited
  snapshot. The deployment was built from a private commit
  (`k2-contracts@493289cd`) with a different toolchain; all 15 local WASM
  artifact hashes differ from the manifest's deployed hashes (expected and
  non-informative). The deployed WASM is not served by any public
  read-only endpoint reachable to us (soroban-rpc `getContractCode` /
  `getLedgerEntry` / `getContractData`: not exposed; public Horizon
  `/contracts/{id}/code`: 404).
- Therefore: whether #44846 is present in the exact live bytes must be
  confirmed by the K2 team against their own deployed build. If the branch
  shown in Section 3 exists in the deployed router, the exploit applies to
  live positions today.

We ask the team to check the deployed `kinetic-router`'s
`internal_liquidation_call` for the interaction: single-asset
`collateral_cap_triggered` ⇒ portfolio-wide `burn_scaled` into
`add_reserve_deficit` without verifying that no other collateral assets
remain in the position.

## 7. Suggested mitigation (any one of these closes it)

1. In the `collateral_cap_triggered` branch, before burning the remaining
   debt to deficit, check whether the user holds balances in **other**
   collateral assets. If any remain, do NOT convert their proportional
   debt to deficit — instead either (a) extend the liquidation to those
   assets up to their cap, or (b) cap the forgiven debt at the value of the
   seized collateral only.
2. Alternatively, make `debt_to_cover` scaling and the deficit burn
   consistent: the deficit should never exceed the value of collateral
   actually seized in this leg.
3. As a defense-in-depth measure, require a minimum collateral fraction
   (e.g. ≥ 1% of total collateral value) for a collateral asset to be
   selected as the liquidation target, preventing dust-targeted
   cap-triggering.

## 8. Reproduction

```
cd <k2 repo, commit 2485e33 + finding commit f2dd56d>
cargo test -p k2-unit-tests poc_44846
# => test poc_v12_44846::poc_v12_44846_single_asset_cap_forgives_all_debt ... ok
```

The test (`tests/unit-tests/src/poc_v12_44846.rs`, 354 lines) constructs
the full multi-collateral position above on the in-process Soroban
environment, executes the liquidation leg, and asserts: (i) the remainder
of the debt moved to `reserve_deficit`, (ii) the borrower's total debt is
~0, (iii) the borrower's 10,000 A balance is intact, (iv)
`try_withdraw(10,000 A)` **succeeds**.

A companion observational test (poc_v12_44869) documents a related but
separate class (unauthenticated `initialize` on fresh deploy) and is
included for context; it is not part of this finding's claim.

## 9. Non-findings (checked, documented so reviewers don't re-derive them)

- Unauthenticated `initialize` takeover: code fact on fresh deploy, but
  live-token re-init after TTL is defeated by soroban-sdk 23.5.3
  archived-entry restoration (instance-storage keys are restored, so
  `has_state()` stays true and re-init reverts). Documented in
  `tests/unit-tests/src/poc_v12_44869.rs`.
- Reserve-counter reuse after TTL expiry: same archived-entry-restoration
  property invalidates it (consistent with the internal audit note's own
  invalidation of the sibling variant).
- Flash-loan, debt-token scaled math, a-token transfer/bitmap, and
  swap/DEX paths were read in full across multiple passes; no additional
  fund-loss candidates found.

---

*Prepared by Sentinel Research. All on-chain verification was read-only.
Nothing in this package has been sent to any third party.*
