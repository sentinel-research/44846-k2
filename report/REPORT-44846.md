# Responsible disclosure — K2 Lend: single-asset collateral-cap check
# forgives remaining debt (multi-collateral fund loss)

**Severity (self-assessed): High** — direct protocol fund loss: recoverable
debt is written off into reserve deficit while the borrower keeps and
withdraws their other collateral.

**Author:** Sentinel Research

**Framing:** this is a **confirmed reproduction + patch-status request**, not
a claim of a novel, previously-unknown Critical. The same branch was already
flagged as a Critical (Validity: **Unreviewed**) in the contest's own V12
autonomous-audit output (#44846, Code4rena `2026-04-k2`). This report adds the
first passing, end-to-end, source-traced reproduction and asks K2 to confirm
its status in the deployed build. Current deployed exposure is **unverified**
(Section 6).

**Status:** transmitted to K2 Lend as responsible disclosure, 2026-08-16,
under the principal's written authorization (`authorization-44846.md`, signed
2026-08-16). Public mirror: `github.com/sentinel-research/44846-k2`.

---

## 1. Executive summary

In K2 Lend's kinetic router, a borrower holding **multiple** collateral
assets can erase an arbitrarily large portion of their debt by having a
complicit liquidator liquidate a **dust-sized secondary collateral**.

The single-asset cap branch (`collateral_cap_triggered` in
`kinetic-router/src/liquidation.rs`) treats the exhaustion of *one* selected
collateral asset as exhaustion of the *entire* position: it burns the
user's **entire remaining balance of that debt asset** as protocol bad
de (`add_reserve_deficit`) even though the borrower still holds substantial
balances in other collateral assets — which they then withdraw debt-free.

A fully reproducible, passing PoC is provided (Section 8). In the PoC,
~$7,914 of debt is forgiven while the borrower retains ~$9,000 of primary
collateral, using ~$86 of liquidator capital. (The liquidator must post the
5% liquidation bonus on a dust asset that has dropped in price, so the bonus
costs more of the dust asset than its face value — see the worked numbers in
Section 4.) In a deeply underwater position where the selected asset can
cover the full requested leg (close factor 100%, HF < 0.5), the **entire
remaining debt of that debt asset** can be erased in one leg.

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
   burns the **entire remaining balance of that debt asset**
   (`burn_scaled` + `add_reserve_deficit`) — i.e. every unit of the user's
   debt in the selected `debt_asset` that the liquidator did not pay in this
   leg.

Step 2's premise is wrong when the user holds **other collateral assets**.
Only the single selected collateral asset was exhausted; the user's other
collateral remains in the position, recoverable, and withdrawable — yet the
debt is written off as if no collateral were left.

**Scope (precision):** the burn applies to the **selected `debt_asset`** of
the liquidation leg, not to every debt asset in a multi-debt portfolio. A
separate debt asset would require its own liquidation leg. In the PoC the
user has a single debt asset (B), so the bug erases the user's entire
outstanding debt. In a multi-debt portfolio the same branch erases the
remaining balance of the one selected debt asset per leg.

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
  - The collateral required is computed on **value** and converted to C
    tokens at C's (discounted) price:
    `100 B → $100 → × 1.05 bonus = $105 → / $0.90 = 116.67 C`.
    `collateral_amount_to_transfer = 116.67 C > 100 C` (C balance) ⇒
    cap triggers.
  - N-08 scales the leg to `ceil(100 × 100 / 116.67) = 85.71 B` — the debt
    amount that corresponds to the user's full C balance.
  - The liquidator therefore repays only **85.71 B** and receives all **100 C**.
  - H-02/H-05 then burns the **remaining 7,914.29 B** into reserve deficit.

Result (all asserted in the PoC; exact integer values at 7-decimal precision):

| Party | Position before | Position after |
|---|---|---|
| Protocol (debt asset B) | 8,000 B outstanding debt | 85.71 B repaid; 7,914.29 B → reserve deficit |
| Borrower debt (B) | 8,000 B | ~0 B (8,000 − 85.71 = 7,914.29 burned) |
| Borrower collateral | 10,000 A + 100 C | 10,000 A (all 100 C paid to liquidator) |
| Borrower action | — | `try_withdraw` 10,000 A **succeeds** |
| Liquidator cost | — | 85.71 B funded, gets 100 C back (~$90) |

The borrower spent ~$86 of debt to shed a ~$7,914 debt while keeping ~$9,000
of primary collateral. In a deeply underwater position (HF < 0.5 ⇒ close
factor 100%) where the selected asset can cover the full requested leg, the
full remaining debt of that asset is erased in the same single leg.

## 5. Why existing guards don't stop it

- **Health factor gate**: the position is only liquidatable after a real
  price move; the exploit uses the *intended* liquidation path, not a
  bypass.
- **Per-asset cap (N-08)**: correctly caps the *selected* asset at its
  balance — but the downstream deficit burn (H-02/H-05) writes off the
  whole balance of the selected debt asset.
- **`LIQ_WHITELIST_FLAG`**: when unset, **all** addresses may liquidate —
  so a Sybil liquidator is trivial. Even with the whitelist ON, the protocol
  operator whitelists addresses it controls; the economic beneficiary is
  still the borrower.
- **No minimum-dust check** on the selected collateral: 100 units of a
  $100,000-position is enough.
- Prior audit rounds (WatchPug rev3, Halborn — both committed to the repo)
  contain **no** finding on this branch (keyword searches: single-asset /
  cap check / forgive / portfolio exhaustion / dust collateral ⇒ 0 hits).
  The contest's own V12 autonomous-audit output flagged the same branch as
  Critical (note #44846) but left it **Unreviewed**, with no passing PoC.
  This report provides the first passing, end-to-end, source-traced
  reproduction.

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
`collateral_cap_triggered` ⇒ `burn_scaled` of the entire remaining
balance of the selected debt asset into `add_reserve_deficit`, without
verifying that no other collateral assets remain in the position.

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
cd <k2 contest snapshot, base commit 2485e33>
# 1. Build all contract WASMs (the tests import them via contractimport!):
stellar contract build        # or: ./build.sh
# 2. The PoC is already wired into the unit-tests crate:
#    tests/unit-tests/src/lib.rs declares `mod poc_v12_44846;`
#    (see commit f2dd56d, which added both the PoC file and the module line).
cargo test -p k2-unit-tests poc_44846
# => test poc_v12_44846::poc_44846_single_asset_cap_forgives_all_debt ... ok
```

The test (`tests/unit-tests/src/poc_v12_44846.rs`, 354 lines) constructs
the full multi-collateral position above on the in-process Soroban
environment, executes the liquidation leg, and asserts the buggy end-state:
(i) the remainder of the debt (7,914.29 B) moved to `reserve_deficit`,
(ii) the borrower's total debt is ~0, (iii) the borrower's 10,000 A balance
is intact, (iv) `try_withdraw(10,000 A)` **succeeds**. On a **fixed** build
(where single-asset exhaustion does not write off the whole position's debt)
this test **fails**.

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
Transmitted to K2 Lend 2026-08-16 under the principal's written
authorization (signed 2026-08-16). Public mirror:
`github.com/sentinel-research/44846-k2`.*
