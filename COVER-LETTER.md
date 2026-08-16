# Cover letter — K2 Lend responsible disclosure (#44846)

**TO:** K2 Lend team (channel: [filled at send])
**FROM:** Sentinel Research
**RE:** Confirmed reproduction — kinetic-router liquidation cap writes off
remaining debt (multi-collateral positions); patch-status request

**[AUTHORIZATION]** Transmitted under the principal's written authorization
(`authorization-44846.md`, signed 2026-08-16): one transmission, framed as a
confirmed reproduction + patch-status request; no monetary or process
commitments without further principal sign-off.

---

Subject: Confirmed reproduction + patch-status request — K2 Lend kinetic
router: single-asset liquidation cap writes off remaining debt
(multi-collateral positions)

Hello K2 team,

I'm writing to share a confirmed reproduction of a liquidation-handling bug
in K2 Lend's kinetic router and to ask whether it has been, or will be,
patched in the deployed build.

Summary: when a liquidation leg selects a collateral asset whose balance is
smaller than the (bonus-adjusted) collateral required, the
`collateral_cap_triggered` branch assumes the whole position is exhausted and
writes the user's **entire remaining balance of that debt asset** off into
reserve deficit — even though the user still holds other collateral assets,
which they can then withdraw debt-free. In the PoC (a single-debt-asset
position), 7,914.29 of 8,000 debt units are erased into reserve deficit while
the borrower keeps ~$9,000 of primary collateral, using ~$86 of liquidator
capital. (In a multi-debt portfolio the same branch writes off the remaining
balance of the one selected debt asset per leg.)

This branch was already flagged as a Critical (Validity: Unreviewed) in the
Code4rena `2026-04-k2` contest's V12 audit output (note #44846); I am not
claiming a novel finding. The full report, with the source-traced
reproduction and a passing PoC, is public at:

  https://github.com/sentinel-research/44846-k2

A note on honesty: I have verified that K2 is live on Stellar mainnet and
that the kinetic-router + liquidation-engine contracts are deployed (via your
public mainnet manifest and read-only RPC). I have **not** been able to
independently confirm that the currently-deployed router bytes are identical
to the April-2026 audited snapshot (the deployed WASM is not publicly
served), so current deployed exposure is unverified from my side. I would
appreciate confirmation of whether the branch described in Section 3 of the
report is present in your deployed build, and its patch status.

The report and a source map for every claim are in the public repository
linked above (report/REPORT-44846.md, EVIDENCE-INDEX.md).

I'd like to coordinate and give the team a reasonable window to remediate
before any wider communication. If applicable, payout to the addresses in
the repository (EVM 0x33fFD956EcA8715fb668D908d79016B61e033c8e / Stellar
bc1qlp039rnwfrhn663s89l453pqeeeuv7r8hlx23x).

Best regards,
Sentinel Research
GitHub: https://github.com/sentinel-research
Repository: https://github.com/sentinel-research/44846-k2
